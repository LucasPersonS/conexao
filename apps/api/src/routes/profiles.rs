//! Perfis personalizados: nome de exibição, pronomes, bio, links e o tema visual
//! (gradiente/fundo, moldura animada, fonte, efeito no nome).
//!
//! O tema é só dado estruturado (cores #hex, ângulo e IDs do catálogo); quem
//! desenha é o card compartilhado (`packages/profile-ui`). Nada de HTML/CSS livre.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthUser, MaybeUser, Staff, User, audit};
use crate::error::{ApiError, ApiResult};
use crate::moderation;
use crate::state::AppState;

const CATALOG_JSON: &str = include_str!("../../../../packages/profile-ui/src/catalog.json");
const PROFILE_CSS: &str = include_str!("../../../../packages/profile-ui/src/profile.css");
pub const MAX_BACKGROUND: usize = 3 * 1024 * 1024;

// ---------------------------------------------------------------- catálogo

#[derive(Deserialize, Serialize, Clone)]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
    pub tier: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animated: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Catalog {
    pub fonts: Vec<CatalogItem>,
    pub frames: Vec<CatalogItem>,
    pub name_effects: Vec<CatalogItem>,
    pub gradients: serde_json::Value,
}

pub fn catalog() -> &'static Catalog {
    static CATALOG: std::sync::OnceLock<Catalog> = std::sync::OnceLock::new();
    CATALOG.get_or_init(|| serde_json::from_str(CATALOG_JSON).expect("catalog.json inválido"))
}

pub async fn get_catalog(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({ "premium_enforced": state.config.premium_enforced, "catalog": catalog() }))
}

// -------------------------------------------------------------------- tema

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Background {
    pub kind: String,
    pub colors: Vec<String>,
    pub angle: i32,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Theme {
    pub background: Background,
    pub panel: String,
    pub accent: String,
    pub frame: String,
    pub font: String,
    pub name_effect: String,
    pub name_colors: Vec<String>,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            background: Background {
                kind: "gradient".into(),
                colors: vec!["#0a0f11".into(), "#2a4a10".into(), "#b7fe00".into()],
                angle: 160,
            },
            panel: "escuro".into(),
            accent: "#b7fe00".into(),
            frame: "simples".into(),
            font: "padrao".into(),
            name_effect: "nenhum".into(),
            name_colors: vec!["#b7fe00".into(), "#f2f6ef".into()],
        }
    }
}

fn is_hex(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn tier_of<'a>(items: &'a [CatalogItem], id: &str) -> Option<&'a str> {
    items.iter().find(|i| i.id == id).map(|i| i.tier.as_str())
}

/// Confere o tema e devolve os itens premium usados.
pub fn validate_theme(theme: &Theme) -> Result<Vec<String>, String> {
    let catalog = catalog();
    if !["gradient", "image"].contains(&theme.background.kind.as_str()) {
        return Err("Tipo de fundo inválido.".into());
    }
    if !(1..=4).contains(&theme.background.colors.len()) || !theme.background.colors.iter().all(|c| is_hex(c)) {
        return Err("O gradiente aceita de 1 a 4 cores no formato #rrggbb.".into());
    }
    if !(0..=360).contains(&theme.background.angle) {
        return Err("Ângulo do gradiente deve ficar entre 0 e 360.".into());
    }
    if !["claro", "escuro"].contains(&theme.panel.as_str()) {
        return Err("Painel do card deve ser claro ou escuro.".into());
    }
    if !is_hex(&theme.accent) || theme.name_colors.len() > 2 || !theme.name_colors.iter().all(|c| is_hex(c)) {
        return Err("Cores devem estar no formato #rrggbb.".into());
    }
    let mut premium = Vec::new();
    for (items, id, label) in [
        (&catalog.frames, &theme.frame, "Moldura"),
        (&catalog.fonts, &theme.font, "Fonte"),
        (&catalog.name_effects, &theme.name_effect, "Efeito do nome"),
    ] {
        match tier_of(items, id) {
            None => return Err(format!("{label} desconhecida: {id}.")),
            Some("premium") => premium.push(id.clone()),
            Some(_) => {}
        }
    }
    Ok(premium)
}

// ------------------------------------------------------------------- links

const LINK_DOMAINS: &[(&str, &[&str])] = &[
    ("youtube", &["youtube.com", "www.youtube.com", "youtu.be", "m.youtube.com"]),
    ("twitch", &["twitch.tv", "www.twitch.tv"]),
    ("tiktok", &["tiktok.com", "www.tiktok.com"]),
    ("instagram", &["instagram.com", "www.instagram.com"]),
    ("x", &["x.com", "twitter.com", "www.x.com", "www.twitter.com"]),
    ("bluesky", &["bsky.app"]),
];

/// Discord: nome de usuário (`lucas.rp`) ou convite (`https://discord.gg/abc`).
fn valid_discord(value: &str) -> bool {
    let username = (2..=32).contains(&value.len())
        && value.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'_');
    let invite = reqwest::Url::parse(value).is_ok_and(|u| {
        u.scheme() == "https"
            && matches!(u.host_str(), Some("discord.gg" | "discord.com" | "www.discord.com"))
            && u.username().is_empty()
    });
    username || invite
}

pub fn validate_links(links: &BTreeMap<String, String>) -> Result<BTreeMap<String, String>, String> {
    let mut clean = BTreeMap::new();
    for (key, value) in links {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if value.len() > 200 {
            return Err(format!("Link de {key} longo demais."));
        }
        if key == "discord" {
            let value = value.trim_start_matches('@').to_lowercase();
            if !valid_discord(&value) && !valid_discord(value.trim()) {
                return Err("Discord: use seu nome de usuário (ex.: lucas.rp) ou um convite discord.gg.".into());
            }
            clean.insert(key.clone(), value);
            continue;
        }
        let Some((_, hosts)) = LINK_DOMAINS.iter().find(|(k, _)| k == key) else {
            return Err(format!("Rede não suportada: {key}."));
        };
        let ok = reqwest::Url::parse(value).is_ok_and(|u| {
            u.scheme() == "https" && u.username().is_empty() && u.host_str().is_some_and(|h| hosts.contains(&h))
        });
        if !ok {
            return Err(format!("O link de {key} deve ser https://{}/…", hosts[0]));
        }
        clean.insert(key.clone(), value.to_string());
    }
    Ok(clean)
}

// ------------------------------------------------------------------ perfil

#[derive(sqlx::FromRow)]
struct ProfileRow {
    id: Uuid,
    minecraft_uuid: Uuid,
    minecraft_name: String,
    is_staff: bool,
    created_at: DateTime<Utc>,
    display_name: Option<String>,
    pronouns: Option<String>,
    bio: Option<String>,
    links: Option<serde_json::Value>,
    theme: Option<serde_json::Value>,
    discord_visibility: Option<String>,
    background_url: Option<String>,
    background_status: Option<String>,
    profile_banned_at: Option<DateTime<Utc>>,
    profile_ban_reason: Option<String>,
}

const PROFILE_SELECT: &str = "SELECT u.id, u.minecraft_uuid, u.minecraft_name, u.is_staff, u.created_at,
        p.display_name, p.pronouns, p.bio, p.links, p.theme, p.discord_visibility, p.background_url, p.background_status,
        u.profile_banned_at, u.profile_ban_reason
   FROM users u LEFT JOIN user_profiles p ON p.user_id = u.id";

#[derive(Serialize)]
pub struct Badge {
    pub id: &'static str,
    pub label: String,
}

#[derive(Serialize)]
pub struct PublicProfile {
    pub minecraft_uuid: Uuid,
    pub minecraft_name: String,
    pub display_name: Option<String>,
    pub pronouns: Option<String>,
    pub bio: Option<String>,
    pub links: BTreeMap<String, String>,
    pub theme: Theme,
    pub background_url: Option<String>,
    pub badges: Vec<Badge>,
}

#[derive(Serialize)]
pub struct OwnProfile {
    #[serde(flatten)]
    pub public: PublicProfile,
    pub discord_visibility: String,
    /// Fundo enviado, inclusive quando removido pela moderação (só o dono vê).
    pub pending_background_url: Option<String>,
    pub background_status: Option<String>,
    /// Perfil bloqueado pela equipe: não pode mais ser personalizado.
    pub banned: bool,
    pub ban_reason: Option<String>,
    pub premium: bool,
    pub premium_enforced: bool,
    pub public_url: String,
}

async fn badges(state: &AppState, row: &ProfileRow) -> ApiResult<Vec<Badge>> {
    let mut badges = Vec::new();
    if row.is_staff {
        badges.push(Badge { id: "equipe", label: "Equipe Conexão".into() });
    }
    let roles: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT m.role FROM server_members m JOIN servers s ON s.id = m.server_id
          WHERE m.user_id = $1 AND s.approval = 'approved'",
    )
    .bind(row.id)
    .fetch_all(&state.pool)
    .await?;
    if roles.iter().any(|r| r == "owner") {
        badges.push(Badge { id: "parceiro", label: "Dono de servidor".into() });
    } else if !roles.is_empty() {
        badges.push(Badge { id: "staff", label: "Staff de servidor".into() });
    }
    if let Some(until) = state.config.pioneer_until
        && row.created_at < until
    {
        badges.push(Badge { id: "pioneiro", label: "Pioneiro".into() });
    }
    Ok(badges)
}

/// O visitante é equipe de algum servidor em que o dono do perfil pediu whitelist?
async fn viewer_is_staff_for(state: &AppState, viewer: Option<&User>, target: Uuid) -> ApiResult<bool> {
    let Some(viewer) = viewer else { return Ok(false) };
    if viewer.is_staff || viewer.id == target {
        return Ok(true);
    }
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM whitelist_entries w JOIN server_members m ON m.server_id = w.server_id
          WHERE w.user_id = $1 AND m.user_id = $2)",
    )
    .bind(target)
    .bind(viewer.id)
    .fetch_one(&state.pool)
    .await?)
}

async fn build_public(state: &AppState, row: &ProfileRow, viewer: Option<&User>) -> ApiResult<PublicProfile> {
    let mut links: BTreeMap<String, String> =
        row.links.clone().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let visibility = row.discord_visibility.as_deref().unwrap_or("everyone");
    let show_discord = match visibility {
        "everyone" => true,
        "staff" => viewer_is_staff_for(state, viewer, row.id).await?,
        _ => viewer.is_some_and(|v| v.id == row.id),
    };
    if !show_discord {
        links.remove("discord");
    }
    let theme: Theme = row.theme.clone().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    // Moderação depois de publicar: o fundo aparece na hora, a menos que a equipe remova.
    let background_url = row.background_url.clone().filter(|_| row.background_status.as_deref() != Some("rejected"));
    Ok(PublicProfile {
        minecraft_uuid: row.minecraft_uuid,
        minecraft_name: row.minecraft_name.clone(),
        display_name: row.display_name.clone(),
        pronouns: row.pronouns.clone(),
        bio: row.bio.clone(),
        links,
        theme,
        background_url,
        badges: badges(state, row).await?,
    })
}

async fn load_row_by_id(state: &AppState, id: Uuid) -> ApiResult<ProfileRow> {
    sqlx::query_as::<_, ProfileRow>(&format!("{PROFILE_SELECT} WHERE u.id = $1"))
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)
}

async fn load_row_by_name(state: &AppState, name: &str) -> ApiResult<ProfileRow> {
    sqlx::query_as::<_, ProfileRow>(&format!(
        "{PROFILE_SELECT} WHERE lower(u.minecraft_name) = lower($1) ORDER BY u.last_login_at DESC LIMIT 1"
    ))
    .bind(name)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)
}

async fn is_premium(state: &AppState, user_id: Uuid) -> ApiResult<bool> {
    Ok(sqlx::query_scalar("SELECT premium_until IS NOT NULL AND premium_until > now() FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?)
}

async fn own_profile(state: &AppState, user: &User) -> ApiResult<OwnProfile> {
    let row = load_row_by_id(state, user.id).await?;
    let public = build_public(state, &row, Some(user)).await?;
    Ok(OwnProfile {
        discord_visibility: row.discord_visibility.clone().unwrap_or_else(|| "everyone".into()),
        pending_background_url: row.background_url.clone(),
        background_status: row.background_status.clone(),
        banned: row.profile_banned_at.is_some(),
        ban_reason: row.profile_ban_reason.clone(),
        premium: is_premium(state, user.id).await?,
        premium_enforced: state.config.premium_enforced,
        public_url: format!("{}/u/{}", state.config.public_url, row.minecraft_name),
        public,
    })
}

pub async fn get_me(State(state): State<AppState>, AuthUser(user): AuthUser) -> ApiResult<Json<OwnProfile>> {
    Ok(Json(own_profile(&state, &user).await?))
}

#[derive(Deserialize)]
pub struct ProfileInput {
    pub display_name: Option<String>,
    pub pronouns: Option<String>,
    pub bio: Option<String>,
    #[serde(default)]
    pub links: BTreeMap<String, String>,
    pub theme: Theme,
    #[serde(default = "everyone")]
    pub discord_visibility: String,
}

fn everyone() -> String {
    "everyone".into()
}

fn clean_text(value: Option<String>, max: usize, label: &str, multiline: bool) -> ApiResult<Option<String>> {
    let Some(value) = value else { return Ok(None) };
    let value = if multiline {
        value.lines().map(str::trim_end).collect::<Vec<_>>().join("\n").trim().to_string()
    } else {
        value.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > max {
        return Err(ApiError::bad(format!("{label}: no máximo {max} caracteres.")));
    }
    if multiline && value.lines().count() > 6 {
        return Err(ApiError::bad(format!("{label}: no máximo 6 linhas.")));
    }
    if value.chars().any(|c| c.is_control() && c != '\n') {
        return Err(ApiError::bad(format!("{label}: caracteres inválidos.")));
    }
    if moderation::has_link(&value) {
        return Err(ApiError::bad(format!("{label}: links ficam nos campos de redes sociais.")));
    }
    if moderation::blocked_term(&value).is_some() {
        return Err(ApiError::bad(format!("{label}: contém termos não permitidos no Conexão.")));
    }
    Ok(Some(value))
}

pub async fn put_me(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(input): Json<ProfileInput>,
) -> ApiResult<Json<OwnProfile>> {
    ensure_not_banned(&state, &user).await?;
    let display_name = clean_text(input.display_name, 32, "Nome de exibição", false)?;
    let pronouns = clean_text(input.pronouns, 24, "Pronomes", false)?;
    let bio = clean_text(input.bio, 300, "Biografia", true)?;
    let links = validate_links(&input.links).map_err(ApiError::bad)?;
    if !["everyone", "staff", "nobody"].contains(&input.discord_visibility.as_str()) {
        return Err(ApiError::bad("Visibilidade do Discord inválida."));
    }
    let premium_used = validate_theme(&input.theme).map_err(ApiError::bad)?;
    if state.config.premium_enforced && !premium_used.is_empty() && !is_premium(&state, user.id).await? {
        return Err(ApiError::bad(format!("Itens exclusivos do Conexão+: {}.", premium_used.join(", "))));
    }
    let links_json = serde_json::to_value(&links).map_err(|e| ApiError::Internal(e.into()))?;
    let theme_json = serde_json::to_value(&input.theme).map_err(|e| ApiError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO user_profiles (user_id, display_name, pronouns, bio, links, theme, discord_visibility, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, now())
         ON CONFLICT (user_id) DO UPDATE SET display_name = EXCLUDED.display_name, pronouns = EXCLUDED.pronouns,
            bio = EXCLUDED.bio, links = EXCLUDED.links, theme = EXCLUDED.theme,
            discord_visibility = EXCLUDED.discord_visibility, updated_at = now()",
    )
    .bind(user.id)
    .bind(display_name)
    .bind(pronouns)
    .bind(bio)
    .bind(links_json)
    .bind(theme_json)
    .bind(&input.discord_visibility)
    .execute(&state.pool)
    .await?;
    Ok(Json(own_profile(&state, &user).await?))
}

async fn ensure_not_banned(state: &AppState, user: &User) -> ApiResult<()> {
    let banned: Option<String> = sqlx::query_scalar(
        "SELECT COALESCE(profile_ban_reason, '') FROM users WHERE id = $1 AND profile_banned_at IS NOT NULL",
    )
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?;
    match banned {
        None => Ok(()),
        Some(reason) if reason.is_empty() => Err(ApiError::bad("Seu perfil foi bloqueado pela equipe Conexão.")),
        Some(reason) => Err(ApiError::bad(format!("Seu perfil foi bloqueado pela equipe Conexão: {reason}"))),
    }
}

/// Envia uma imagem de fundo. Aparece na hora; a equipe Conexão vê os fundos novos
/// na fila de moderação e remove o que for impróprio.
pub async fn upload_background(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    body: Bytes,
) -> ApiResult<Json<OwnProfile>> {
    ensure_not_banned(&state, &user).await?;
    if body.len() > MAX_BACKGROUND {
        return Err(ApiError::bad("Imagem grande demais (máximo 3 MB)."));
    }
    let url = crate::routes::images::store_image(&state, &body).await?;
    let blocked: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM blocked_images WHERE url = $1)")
        .bind(&url)
        .fetch_one(&state.pool)
        .await?;
    if blocked {
        return Err(ApiError::bad("Esta imagem foi removida pela moderação do Conexão. Escolha outra."));
    }
    // Imagem já vista pela equipe (mesmo arquivo) não volta para a fila.
    let approved_before: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM user_profiles WHERE background_url = $1 AND background_status = 'approved')",
    )
    .bind(&url)
    .fetch_one(&state.pool)
    .await?;
    let status = if approved_before || user.is_staff { "approved" } else { "pending" };
    sqlx::query(
        "INSERT INTO user_profiles (user_id, background_url, background_status) VALUES ($1, $2, $3)
         ON CONFLICT (user_id) DO UPDATE SET background_url = EXCLUDED.background_url,
            background_status = EXCLUDED.background_status, updated_at = now()",
    )
    .bind(user.id)
    .bind(&url)
    .bind(status)
    .execute(&state.pool)
    .await?;
    Ok(Json(own_profile(&state, &user).await?))
}

pub async fn get_public(
    State(state): State<AppState>,
    MaybeUser(viewer): MaybeUser,
    Path(name): Path<String>,
) -> ApiResult<Json<PublicProfile>> {
    let row = load_row_by_name(&state, &name).await?;
    Ok(Json(build_public(&state, &row, viewer.as_ref()).await?))
}

/// Perfil resumido para listas (pedidos de whitelist), já com o filtro de visibilidade.
pub async fn public_by_id(state: &AppState, id: Uuid, viewer: &User) -> ApiResult<PublicProfile> {
    let row = load_row_by_id(state, id).await?;
    build_public(state, &row, Some(viewer)).await
}

// --------------------------------------------------------------- denúncias

#[derive(Deserialize)]
pub struct ReportInput {
    pub reason: String,
}

pub async fn report(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(name): Path<String>,
    Json(input): Json<ReportInput>,
) -> ApiResult<StatusCode> {
    let reason = input.reason.trim();
    if reason.is_empty() || reason.chars().count() > 300 {
        return Err(ApiError::bad("Conte em até 300 caracteres o que há de errado."));
    }
    let row = load_row_by_name(&state, &name).await?;
    if row.id == user.id {
        return Err(ApiError::bad("Você não pode denunciar o próprio perfil."));
    }
    let open: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM profile_reports WHERE reporter_id = $1 AND created_at > now() - interval '1 day'")
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;
    if open >= 20 {
        return Err(ApiError::bad("Muitas denúncias hoje. Tente amanhã."));
    }
    sqlx::query(
        "INSERT INTO profile_reports (id, target_id, reporter_id, reason) VALUES ($1, $2, $3, $4)
         ON CONFLICT (target_id, reporter_id) WHERE status = 'open' DO UPDATE SET reason = EXCLUDED.reason",
    )
    .bind(Uuid::new_v4())
    .bind(row.id)
    .bind(user.id)
    .bind(reason)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------ equipe (moderação)

#[derive(Serialize, sqlx::FromRow)]
pub struct ReportItem {
    pub id: Uuid,
    pub target_name: String,
    pub reporter_name: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub open_count: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct BannedItem {
    pub minecraft_name: String,
    pub minecraft_uuid: Uuid,
    pub profile_banned_at: DateTime<Utc>,
    pub profile_ban_reason: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct BackgroundItem {
    pub minecraft_name: String,
    pub minecraft_uuid: Uuid,
    pub background_url: String,
    pub updated_at: DateTime<Utc>,
}

pub async fn staff_queue(State(state): State<AppState>, Staff(_): Staff) -> ApiResult<Json<serde_json::Value>> {
    let reports = sqlx::query_as::<_, ReportItem>(
        "SELECT r.id, t.minecraft_name AS target_name, rp.minecraft_name AS reporter_name, r.reason, r.created_at,
                (SELECT COUNT(*) FROM profile_reports x WHERE x.target_id = r.target_id AND x.status = 'open') AS open_count
           FROM profile_reports r JOIN users t ON t.id = r.target_id JOIN users rp ON rp.id = r.reporter_id
          WHERE r.status = 'open' ORDER BY open_count DESC, r.created_at LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await?;
    let backgrounds = sqlx::query_as::<_, BackgroundItem>(
        "SELECT u.minecraft_name, u.minecraft_uuid, p.background_url, p.updated_at
           FROM user_profiles p JOIN users u ON u.id = p.user_id
          WHERE p.background_status = 'pending' AND p.background_url IS NOT NULL ORDER BY p.updated_at DESC LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await?;
    let banned = sqlx::query_as::<_, BannedItem>(
        "SELECT minecraft_name, minecraft_uuid, profile_banned_at, profile_ban_reason
           FROM users WHERE profile_banned_at IS NOT NULL ORDER BY profile_banned_at DESC LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({ "reports": reports, "backgrounds": backgrounds, "banned": banned })))
}

#[derive(Deserialize)]
pub struct BackgroundDecision {
    pub approve: bool,
}

pub async fn staff_background(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Path(name): Path<String>,
    Json(input): Json<BackgroundDecision>,
) -> ApiResult<StatusCode> {
    let row = load_row_by_name(&state, &name).await?;
    let staff_id = staff.as_ref().map(|s| s.id);
    if input.approve {
        sqlx::query("UPDATE user_profiles SET background_status = 'approved' WHERE user_id = $1")
            .bind(row.id)
            .execute(&state.pool)
            .await?;
    } else {
        remove_background(&state, row.id, staff_id).await?;
    }
    audit(&state.pool, None, staff.map(|s| s.id), "profile.background_reviewed", json!({ "user": name, "approve": input.approve })).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Tira o fundo do perfil e bloqueia o arquivo para não ser enviado de novo.
async fn remove_background(state: &AppState, user_id: Uuid, staff_id: Option<Uuid>) -> ApiResult<()> {
    let url: Option<String> = sqlx::query_scalar(
        "UPDATE user_profiles SET background_status = 'rejected' WHERE user_id = $1 AND background_url IS NOT NULL
         RETURNING background_url",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;
    if let Some(url) = url {
        sqlx::query("INSERT INTO blocked_images (url, blocked_by) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(url)
            .bind(staff_id)
            .execute(&state.pool)
            .await?;
    }
    Ok(())
}

/// Volta o perfil ao padrão (nome, pronomes, bio, links, tema) e remove o fundo.
async fn clear_profile(state: &AppState, user_id: Uuid, staff_id: Option<Uuid>) -> ApiResult<()> {
    remove_background(state, user_id, staff_id).await?;
    sqlx::query(
        "UPDATE user_profiles SET display_name = NULL, pronouns = NULL, bio = NULL, links = '{}', theme = '{}'
          WHERE user_id = $1",
    )
    .bind(user_id)
    .execute(&state.pool)
    .await?;
    Ok(())
}

async fn resolve_reports(state: &AppState, user_id: Uuid, status: &str, staff_id: Option<Uuid>) -> ApiResult<()> {
    sqlx::query(
        "UPDATE profile_reports SET status = $2, resolved_by = $3, resolved_at = now() WHERE target_id = $1 AND status = 'open'",
    )
    .bind(user_id)
    .bind(status)
    .bind(staff_id)
    .execute(&state.pool)
    .await?;
    Ok(())
}

async fn ban_profile(state: &AppState, user_id: Uuid, reason: Option<String>, staff_id: Option<Uuid>) -> ApiResult<()> {
    clear_profile(state, user_id, staff_id).await?;
    sqlx::query("UPDATE users SET profile_banned_at = now(), profile_ban_reason = $2 WHERE id = $1")
        .bind(user_id)
        .bind(reason)
        .execute(&state.pool)
        .await?;
    resolve_reports(state, user_id, "actioned", staff_id).await
}

#[derive(Deserialize)]
pub struct BanInput {
    pub ban: bool,
    pub reason: Option<String>,
}

/// Bloqueia (ou libera) a personalização do perfil. O jogador continua jogando normalmente.
pub async fn staff_ban(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Path(name): Path<String>,
    Json(input): Json<BanInput>,
) -> ApiResult<StatusCode> {
    let row = load_row_by_name(&state, &name).await?;
    let staff_id = staff.as_ref().map(|s| s.id);
    let reason = input.reason.map(|r| r.trim().chars().take(200).collect::<String>()).filter(|r| !r.is_empty());
    if input.ban {
        ban_profile(&state, row.id, reason.clone(), staff_id).await?;
    } else {
        sqlx::query("UPDATE users SET profile_banned_at = NULL, profile_ban_reason = NULL WHERE id = $1")
            .bind(row.id)
            .execute(&state.pool)
            .await?;
    }
    let action = if input.ban { "profile.banned" } else { "profile.unbanned" };
    audit(&state.pool, None, staff_id, action, json!({ "user": name, "reason": reason })).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SeenInput {
    pub names: Vec<String>,
}

/// "Tudo certo" em lote: tira os fundos listados da fila (continuam no perfil).
pub async fn staff_backgrounds_seen(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Json(input): Json<SeenInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let names: Vec<String> = input.names.iter().take(500).map(|n| n.to_lowercase()).collect();
    let done = sqlx::query(
        "UPDATE user_profiles p SET background_status = 'approved' FROM users u
          WHERE u.id = p.user_id AND p.background_status = 'pending' AND lower(u.minecraft_name) = ANY($1)",
    )
    .bind(&names)
    .execute(&state.pool)
    .await?
    .rows_affected();
    audit(&state.pool, None, staff.map(|s| s.id), "profile.backgrounds_seen", json!({ "count": done })).await?;
    Ok(Json(json!({ "seen": done })))
}

#[derive(Deserialize)]
pub struct ReportDecision {
    /// `dismiss` (sem problema), `clear` (perfil volta ao padrão) ou `ban` (volta ao padrão e
    /// o jogador não pode mais personalizar).
    pub action: String,
    pub reason: Option<String>,
}

pub async fn staff_report(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Path(id): Path<Uuid>,
    Json(input): Json<ReportDecision>,
) -> ApiResult<StatusCode> {
    let target: Uuid = sqlx::query_scalar("SELECT target_id FROM profile_reports WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    let staff_id = staff.as_ref().map(|s| s.id);
    let status = match input.action.as_str() {
        "dismiss" => "dismissed",
        "clear" => {
            clear_profile(&state, target, staff_id).await?;
            "actioned"
        }
        "ban" => {
            let reason = input.reason.clone().map(|r| r.trim().chars().take(200).collect::<String>()).filter(|r| !r.is_empty());
            ban_profile(&state, target, reason, staff_id).await?;
            "actioned"
        }
        _ => return Err(ApiError::bad("Ação inválida.")),
    };
    // Resolve todas as denúncias abertas do mesmo perfil de uma vez.
    resolve_reports(&state, target, status, staff_id).await?;
    audit(&state.pool, None, staff.map(|s| s.id), "profile.report_resolved", json!({ "report": id, "action": input.action })).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----------------------------------------------------------- página pública

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn link_label(key: &str) -> &'static str {
    match key {
        "discord" => "Discord",
        "youtube" => "YouTube",
        "twitch" => "Twitch",
        "tiktok" => "TikTok",
        "instagram" => "Instagram",
        "x" => "X",
        "bluesky" => "Bluesky",
        _ => "Link",
    }
}

fn font_family(id: &str) -> String {
    catalog().fonts.iter().find(|f| f.id == id).and_then(|f| f.family.clone()).unwrap_or_else(|| "Inter".into())
}

/// Mesmo HTML do `ProfileCard.vue`, gerado no servidor.
pub fn render_card(profile: &PublicProfile) -> String {
    let theme = &profile.theme;
    let mut colors = theme.background.colors.clone();
    if colors.len() == 1 {
        colors.push(colors[0].clone());
    }
    let bg = match (&theme.background.kind[..], &profile.background_url) {
        ("image", Some(url)) => format!("url(&quot;{}&quot;)", escape(&url.replace(['"', '\'', '(', ')'], ""))),
        _ => format!("linear-gradient({}deg, {})", theme.background.angle, colors.join(", ")),
    };
    let style = format!(
        "--cp-accent:{};--cp-bg:{};--cp-font:&#39;{}&#39;, Inter, sans-serif;--cp-name-a:{};--cp-name-b:{}",
        theme.accent,
        bg,
        escape(&font_family(&theme.font)),
        theme.name_colors.first().unwrap_or(&theme.accent),
        theme.name_colors.get(1).unwrap_or(&theme.accent),
    );
    let name = profile.display_name.clone().unwrap_or_else(|| profile.minecraft_name.clone());
    let name_html = if theme.name_effect == "onda" {
        name.chars().enumerate().map(|(i, c)| format!(r#"<span style="--i:{i}">{}</span>"#, escape(&c.to_string()))).collect()
    } else {
        escape(&name)
    };
    let pronouns = profile.pronouns.as_ref().map(|p| format!("<span>· {}</span>", escape(p))).unwrap_or_default();
    let badges: String = profile.badges.iter().map(|b| format!(r#"<span class="cp-badge">{}</span>"#, escape(&b.label))).collect();
    let bio = profile.bio.as_ref().map(|b| format!(r#"<p class="cp-bio">{}</p>"#, escape(b))).unwrap_or_default();
    let links: String = profile
        .links
        .iter()
        .map(|(k, v)| {
            if v.starts_with("https://") {
                format!(r#"<a class="cp-link cp-link-{k}" href="{}" rel="noopener noreferrer" target="_blank">{}</a>"#, escape(v), link_label(k))
            } else {
                format!(r#"<span class="cp-link cp-link-{k}">{} · {}</span>"#, link_label(k), escape(v))
            }
        })
        .collect();
    format!(
        r#"<article class="cp-card{dark}" style="{style}">
  <div class="cp-bg"></div>
  <div class="cp-top"><div class="cp-avatar cp-frame-{frame}">
    <img class="cp-head" src="https://mc-heads.net/avatar/{uuid}/128" alt="">
    <i></i><i></i><i></i><i></i><i></i><i></i>
  </div></div>
  <div class="cp-body">
    <h2 class="cp-name cp-name-{effect}" data-text="{name_attr}">{name_html}</h2>
    <div class="cp-sub"><span>@{nick}</span>{pronouns}</div>
    {badges_block}
    {bio}
    {links_block}
  </div>
</article>"#,
        dark = if theme.panel == "escuro" { " cp-dark" } else { "" },
        frame = escape(&theme.frame),
        effect = escape(&theme.name_effect),
        uuid = profile.minecraft_uuid,
        name_attr = escape(&name),
        nick = escape(&profile.minecraft_name),
        badges_block = if badges.is_empty() { String::new() } else { format!(r#"<div class="cp-badges">{badges}</div>"#) },
        links_block = if links.is_empty() { String::new() } else { format!(r#"<div class="cp-links">{links}</div>"#) },
    )
}

pub async fn page(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    let profile = match load_row_by_name(&state, &name).await {
        Ok(row) => build_public(&state, &row, None).await.ok(),
        Err(_) => None,
    };
    let Some(profile) = profile else {
        return (StatusCode::NOT_FOUND, Html(public_shell("Perfil não encontrado", "", "<p>Perfil não encontrado.</p>", None))).into_response();
    };
    let shown = profile.display_name.clone().unwrap_or_else(|| profile.minecraft_name.clone());
    let description = format!(
        "{}{}",
        profile.pronouns.as_ref().map(|p| format!("{p} · ")).unwrap_or_default(),
        profile.bio.clone().unwrap_or_else(|| "Perfil no Conexão".into()).lines().next().unwrap_or_default()
    );
    let meta = format!(
        r##"<meta property="og:type" content="profile">
<meta property="og:site_name" content="Conexão">
<meta property="og:title" content="{} (@{}) no Conexão">
<meta property="og:description" content="{}">
<meta property="og:image" content="https://mc-heads.net/avatar/{}/256">
<meta name="theme-color" content="{}">"##,
        escape(&shown),
        escape(&profile.minecraft_name),
        escape(&description),
        profile.minecraft_uuid,
        escape(&profile.theme.accent),
    );
    let family = font_family(&profile.theme.font).replace(' ', "+");
    let card = render_card(&profile);
    let mut response = Html(public_shell(&format!("{} — Conexão", escape(&shown)), &meta, &card, Some(&family))).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("public, max-age=60"));
    response
}

fn public_shell(title: &str, meta: &str, body: &str, font: Option<&str>) -> String {
    let font_link = font
        .filter(|f| *f != "Inter")
        .map(|f| format!(r#"<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family={f}&display=swap">"#))
        .unwrap_or_default();
    format!(
        r#"<!doctype html>
<html lang="pt-BR">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
{meta}
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600;700&display=swap">
{font_link}
<style>
body {{ margin:0; min-height:100vh; display:grid; place-items:center; padding:24px 16px; box-sizing:border-box;
  font-family:Inter,system-ui,sans-serif; color:#f2f6ef; background-color:#0a0f11;
  background-image:radial-gradient(900px 420px at 50% -120px,rgba(183,254,0,.14),transparent 70%),
    linear-gradient(rgba(183,254,0,.035) 1px,transparent 1px),linear-gradient(90deg,rgba(183,254,0,.035) 1px,transparent 1px);
  background-size:auto,44px 44px,44px 44px; }}
footer {{ margin-top:18px; text-align:center; font-size:13px; color:#a3b1ad; }}
footer a {{ color:#b7fe00; font-weight:600; }}
{PROFILE_CSS}
</style>
</head>
<body>
<div>
{body}
<footer>Perfil no <a href="/baixar">Conexão</a>, o launcher da comunidade de Minecraft Roleplay.</footer>
</div>
</body>
</html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_parses_and_validates_default() {
        assert!(!catalog().frames.is_empty());
        assert_eq!(validate_theme(&Theme::default()), Ok(vec![]));
    }

    #[test]
    fn rejects_bad_theme() {
        let theme = Theme { accent: "red; background:url(x)".into(), ..Theme::default() };
        assert!(validate_theme(&theme).is_err());
        let theme = Theme { frame: "inexistente".into(), ..Theme::default() };
        assert!(validate_theme(&theme).is_err());
        let theme = Theme { frame: "arco-iris".into(), ..Theme::default() };
        assert_eq!(validate_theme(&theme), Ok(vec!["arco-iris".to_string()]));
    }

    #[test]
    fn validates_links() {
        let ok = BTreeMap::from([
            ("discord".to_string(), "@Lucas.RP".to_string()),
            ("youtube".to_string(), "https://youtube.com/@lucas".to_string()),
        ]);
        let clean = validate_links(&ok).unwrap();
        assert_eq!(clean["discord"], "lucas.rp");
        let bad = BTreeMap::from([("youtube".to_string(), "https://evil.com/youtube.com".to_string())]);
        assert!(validate_links(&bad).is_err());
        let bad = BTreeMap::from([("discord".to_string(), "javascript:alert(1)".to_string())]);
        assert!(validate_links(&bad).is_err());
    }

    #[test]
    fn card_escapes_user_text() {
        let profile = PublicProfile {
            minecraft_uuid: Uuid::nil(),
            minecraft_name: "Lucas".into(),
            display_name: Some("<script>x</script>".into()),
            pronouns: None,
            bio: Some("\"><img src=x onerror=alert(1)>".into()),
            links: BTreeMap::new(),
            theme: Theme::default(),
            background_url: None,
            badges: vec![],
        };
        let html = render_card(&profile);
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img src=x"));
    }
}
