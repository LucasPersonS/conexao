use axum::Json;
use axum::extract::{Path, State};
use conexao_types::{Loader, LoaderSpec, MinecraftVersion, ServerStatus, ServerSummary};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::{AuthUser, MaybeUser, Role, Staff, audit, lookup_minecraft_name, require_role, upsert_user};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(sqlx::FromRow)]
pub struct ServerRow {
    id: Uuid,
    slug: String,
    name: String,
    description: String,
    address: String,
    minecraft_version: String,
    loader: String,
    loader_version: String,
    tags: Vec<String>,
    icon_url: Option<String>,
    banner_url: Option<String>,
    discord_url: Option<String>,
    whitelist_required: bool,
    approval: String,
    featured_until: Option<DateTime<Utc>>,
    status_online: bool,
    status_players: i32,
    status_max_players: i32,
    status_latency_ms: Option<i32>,
    status_checked_at: Option<DateTime<Utc>>,
    current_version: Option<String>,
    pack_size: Option<i64>,
    allow_offline: bool,
    stars: i64,
    pro_until: Option<DateTime<Utc>>,
    supportable: bool,
    whitelist_form_url: Option<String>,
    has_page: bool,
}

const SELECT: &str = "SELECT s.id, s.slug, s.name, s.description, s.address, s.minecraft_version, s.loader,
        s.loader_version, s.tags, s.icon_url, s.banner_url, s.discord_url, s.whitelist_required, s.approval,
        s.featured_until, s.status_online, s.status_players, s.status_max_players, s.status_latency_ms,
        s.status_checked_at, s.allow_offline, pv.version AS current_version,
        (SELECT SUM((f->>'size')::BIGINT)::BIGINT FROM jsonb_array_elements(pv.manifest->'files') f) AS pack_size,
        (SELECT COUNT(*) FROM server_stars st WHERE st.server_id = s.id) AS stars,
        s.pro_until,
        EXISTS (SELECT 1 FROM server_payout_accounts pa WHERE pa.server_id = s.id AND pa.support_enabled) AS supportable,
        s.whitelist_form_url,
        (s.page <> '{}'::JSONB) AS has_page
   FROM servers s
   LEFT JOIN server_channels sc ON sc.server_id = s.id AND sc.channel = 'stable'
   LEFT JOIN pack_versions pv ON pv.id = sc.version_id";

pub fn parse_loader(value: &str) -> Loader {
    match value {
        "fabric" => Loader::Fabric,
        "forge" => Loader::Forge,
        _ => Loader::NeoForge,
    }
}

pub fn loader_str(loader: Loader) -> &'static str {
    match loader {
        Loader::Fabric => "fabric",
        Loader::Forge => "forge",
        Loader::NeoForge => "neoforge",
    }
}

#[derive(Serialize)]
pub struct ServerDetail {
    #[serde(flatten)]
    pub summary: ServerSummary,
    pub approval: String,
    pub featured_until: Option<DateTime<Utc>>,
    pub pro_until: Option<DateTime<Utc>>,
}

impl ServerRow {
    fn into_detail(self) -> ServerDetail {
        let featured = self.featured_until.is_some_and(|until| until > Utc::now());
        let pro = self.pro_until.is_some_and(|until| until > Utc::now());
        let supportable = self.supportable && self.approval == "approved";
        ServerDetail {
            approval: self.approval,
            featured_until: self.featured_until.filter(|until| *until > Utc::now()),
            pro_until: self.pro_until.filter(|until| *until > Utc::now()),
            summary: ServerSummary {
                id: self.id,
                slug: self.slug,
                name: self.name,
                description: self.description,
                address: self.address,
                minecraft: self
                    .minecraft_version
                    .parse()
                    .unwrap_or(conexao_types::MIN_SUPPORTED_MINECRAFT),
                loader: LoaderSpec { kind: parse_loader(&self.loader), version: self.loader_version },
                tags: self.tags,
                featured,
                icon_url: self.icon_url,
                banner_url: self.banner_url,
                discord_url: self.discord_url,
                whitelist_required: self.whitelist_required,
                status: ServerStatus {
                    online: self.status_online,
                    players_online: u32::try_from(self.status_players).unwrap_or(0),
                    players_max: u32::try_from(self.status_max_players).unwrap_or(0),
                    latency_ms: self.status_latency_ms.and_then(|v| u32::try_from(v).ok()),
                    checked_at: self.status_checked_at,
                },
                current_version: self.current_version,
                pack_size: self.pack_size.and_then(|v| u64::try_from(v).ok()),
                allow_offline: self.allow_offline,
                stars: u32::try_from(self.stars).unwrap_or(0),
                pro,
                supportable,
                whitelist_form_url: self.whitelist_form_url,
                // Página personalizada só aparece com o PRO ativo (fica guardada quando vence).
                has_page: pro && self.has_page,
            },
        }
    }
}

pub async fn server_id_by_slug(pool: &PgPool, slug: &str) -> ApiResult<Uuid> {
    sqlx::query_scalar("SELECT id FROM servers WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)
}

/// Diretório público: só servidores aprovados; destaque pago primeiro.
pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<ServerSummary>>> {
    let rows = sqlx::query_as::<_, ServerRow>(&format!(
        "{SELECT} WHERE s.approval = 'approved'
         ORDER BY (s.featured_until IS NOT NULL AND s.featured_until > now()) DESC,
                  s.status_online DESC, s.status_players DESC, s.name"
    ))
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows.into_iter().map(|r| r.into_detail().summary).collect()))
}

pub async fn get_one(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<ServerDetail>> {
    let row = sqlx::query_as::<_, ServerRow>(&format!("{SELECT} WHERE s.slug = $1"))
        .bind(&slug)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    if row.approval != "approved" {
        let user = user.ok_or(ApiError::NotFound)?;
        require_role(&state.pool, row.id, &user, Role::Moderator)
            .await
            .map_err(|_| ApiError::NotFound)?;
    }
    Ok(Json(row.into_detail()))
}

#[derive(Deserialize)]
pub struct ServerInput {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub address: String,
    pub minecraft: String,
    pub loader: LoaderSpec,
    #[serde(default)]
    pub tags: Vec<String>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub discord_url: Option<String>,
    #[serde(default = "yes")]
    pub whitelist_required: bool,
}

fn yes() -> bool {
    true
}

fn valid_slug(slug: &str) -> bool {
    (2..=48).contains(&slug.len())
        && slug.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !slug.starts_with('-')
}

fn validate_address(address: &str) -> ApiResult<()> {
    let address = address.trim();
    if address.is_empty() || address.contains(char::is_whitespace) || address.contains(['/', '<', '>']) {
        return Err(ApiError::bad("Informe o endereço do servidor, por exemplo jogar.seuservidor.com:25565."));
    }
    let host = address.split(':').next().unwrap_or_default().to_lowercase();
    if matches!(host.as_str(), "localhost" | "127.0.0.1" | "0.0.0.0") {
        return Err(ApiError::bad("Troque localhost pelo endereço que os jogadores usarão para entrar."));
    }
    Ok(())
}

/// HTTPS, ou imagens enviadas ao próprio Conexão (em testes locais a API é http).
fn validate_url(state: &AppState, url: &Option<String>) -> ApiResult<()> {
    match url {
        Some(url) if !url.starts_with("https://") && !url.starts_with(&format!("{}/images/", state.config.public_url)) => {
            Err(ApiError::bad("Use URLs HTTPS para imagens e links."))
        }
        _ => Ok(()),
    }
}

fn validate_tags(tags: &[String]) -> ApiResult<Vec<String>> {
    if tags.len() > 8 {
        return Err(ApiError::bad("Use no máximo 8 tags."));
    }
    // Mantém a grafia do dono ("Medieval", "RP médio"), sem repetir ignorando maiúsculas.
    let mut seen = std::collections::HashSet::new();
    Ok(tags
        .iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty() && t.chars().count() <= 24 && seen.insert(t.to_lowercase()))
        .collect())
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(input): Json<ServerInput>,
) -> ApiResult<Json<ServerDetail>> {
    if !valid_slug(&input.slug) {
        return Err(ApiError::bad("O identificador deve usar letras minúsculas, números e hífen."));
    }
    if input.name.trim().is_empty() || input.name.len() > 64 {
        return Err(ApiError::bad("Informe um nome de até 64 caracteres."));
    }
    validate_address(&input.address)?;
    let minecraft = MinecraftVersion::parse_supported(&input.minecraft).map_err(|e| ApiError::bad(e.to_string()))?;
    for url in [&input.icon_url, &input.banner_url, &input.discord_url] {
        validate_url(&state, url)?;
    }
    let tags = validate_tags(&input.tags)?;
    if input.description.chars().count() > 2000 {
        return Err(ApiError::bad("A descrição pode ter no máximo 2000 caracteres."));
    }
    if !user.is_staff {
        // Cada servidor dá direito a enviar arquivos: limite de servidores em análise por conta.
        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM servers s JOIN server_members m ON m.server_id = s.id
              WHERE m.user_id = $1 AND m.role = 'owner' AND s.approval IN ('pending', 'rejected')",
        )
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;
        if pending >= state.config.max_pending_servers {
            return Err(ApiError::bad(format!(
                "Você já tem {pending} servidor(es) aguardando análise ou recusados. Espere a equipe Conexão analisar antes de cadastrar outro."
            )));
        }
    }

    let id = Uuid::new_v4();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO servers (id, slug, name, description, address, minecraft_version, loader, loader_version,
                              tags, icon_url, banner_url, discord_url, whitelist_required)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(id)
    .bind(&input.slug)
    .bind(input.name.trim())
    .bind(&input.description)
    .bind(input.address.trim())
    .bind(minecraft.to_string())
    .bind(loader_str(input.loader.kind))
    .bind(&input.loader.version)
    .bind(&tags)
    .bind(&input.icon_url)
    .bind(&input.banner_url)
    .bind(&input.discord_url)
    .bind(input.whitelist_required)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO server_members (server_id, user_id, role) VALUES ($1, $2, 'owner')")
        .bind(id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    audit(&state.pool, Some(id), Some(user.id), "server.created", json!({ "slug": input.slug })).await?;
    get_one(State(state), MaybeUser(Some(user)), Path(input.slug)).await
}

#[derive(Deserialize)]
pub struct ServerPatch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub tags: Option<Vec<String>>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub discord_url: Option<String>,
    pub whitelist_required: Option<bool>,
    /// Aceitar contas não originais (exige Companion 0.2+ e `online-mode=false`).
    pub allow_offline: Option<bool>,
    /// Google Forms para pedir whitelist; `""` remove.
    pub whitelist_form_url: Option<String>,
}

/// Só Google Forms (`docs.google.com/forms/...` ou `forms.gle/...`): o launcher abre o link
/// no navegador do jogador, então nada de link qualquer na página do servidor.
pub fn validate_form_url(url: &str) -> ApiResult<()> {
    let bad = || ApiError::bad("Use um link do Google Forms (https://docs.google.com/forms/... ou https://forms.gle/...).");
    if url.len() > 500 || url.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"' | '\\')) {
        return Err(bad());
    }
    let ok = url.strip_prefix("https://docs.google.com/forms/").is_some_and(|rest| !rest.is_empty())
        || url.strip_prefix("https://forms.gle/").is_some_and(|rest| !rest.is_empty());
    if ok { Ok(()) } else { Err(bad()) }
}

/// Versão mínima do Companion que aplica o passe de entrada.
const COMPANION_JOIN_TICKETS: (u64, u64) = (0, 2);

fn companion_supports_tickets(agent_version: &str) -> bool {
    let Some(version) = agent_version.strip_prefix("companion-").and_then(|rest| rest.rsplit('-').next()) else {
        return false;
    };
    let mut parts = version.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    let current = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    current >= COMPANION_JOIN_TICKETS
}

/// Só deixa ligar contas não originais quando o servidor está pronto para barrar impostores.
async fn check_offline_ready(state: &AppState, server_id: Uuid) -> ApiResult<()> {
    let agent_version: Option<String> = sqlx::query_scalar("SELECT agent_version FROM agents WHERE server_id = $1")
        .bind(server_id)
        .fetch_optional(&state.pool)
        .await?
        .flatten();
    if !agent_version.as_deref().is_some_and(companion_supports_tickets) {
        return Err(ApiError::bad(
            "Para aceitar contas não originais, instale o Conexão Companion 0.2 ou mais novo no servidor.",
        ));
    }
    if !state.agents.is_connected(server_id) {
        return Err(ApiError::bad("Ligue o servidor com o Companion conectado e tente de novo."));
    }
    let info: Option<serde_json::Value> = sqlx::query_scalar("SELECT info FROM server_snapshots WHERE server_id = $1")
        .bind(server_id)
        .fetch_optional(&state.pool)
        .await?
        .flatten();
    let info: Option<conexao_types::agent::ServerInfo> = info.and_then(|v| serde_json::from_value(v).ok());
    let Some(info) = info else {
        return Err(ApiError::bad("O Companion ainda não enviou os dados do servidor. Tente de novo em instantes."));
    };
    if info.online_mode {
        return Err(ApiError::bad(
            "Defina online-mode=false no server.properties e reinicie o servidor antes de aceitar contas não originais.",
        ));
    }
    if info.enforce_secure_profile == Some(true) {
        return Err(ApiError::bad(
            "Defina enforce-secure-profile=false no server.properties: sem isso, contas não originais são expulsas.",
        ));
    }
    Ok(())
}

pub async fn update(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Json(patch): Json<ServerPatch>,
) -> ApiResult<Json<ServerDetail>> {
    let id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, id, &user, Role::Admin).await?;
    if let Some(address) = &patch.address {
        validate_address(address)?;
    }
    for url in [&patch.icon_url, &patch.banner_url, &patch.discord_url] {
        validate_url(&state, url)?;
    }
    if patch.name.as_deref().is_some_and(|n| n.trim().is_empty() || n.len() > 64) {
        return Err(ApiError::bad("Informe um nome de até 64 caracteres."));
    }
    if patch.description.as_deref().is_some_and(|d| d.chars().count() > 2000) {
        return Err(ApiError::bad("A descrição pode ter no máximo 2000 caracteres."));
    }
    let tags = patch.tags.as_deref().map(validate_tags).transpose()?;
    let form_url = patch.whitelist_form_url.as_deref().map(str::trim);
    if let Some(url) = form_url.filter(|u| !u.is_empty()) {
        validate_form_url(url)?;
    }
    if patch.allow_offline == Some(true) {
        check_offline_ready(&state, id).await?;
    }
    sqlx::query(
        "UPDATE servers SET
            name = COALESCE($2, name), description = COALESCE($3, description),
            address = COALESCE($4, address), tags = COALESCE($5, tags),
            icon_url = COALESCE($6, icon_url), banner_url = COALESCE($7, banner_url),
            discord_url = COALESCE($8, discord_url), whitelist_required = COALESCE($9, whitelist_required),
            allow_offline = COALESCE($10, allow_offline),
            whitelist_form_url = CASE WHEN $11::TEXT IS NULL THEN whitelist_form_url ELSE NULLIF($11, '') END
          WHERE id = $1",
    )
    .bind(id)
    .bind(patch.name.as_deref().map(str::trim))
    .bind(&patch.description)
    .bind(patch.address.as_deref().map(str::trim))
    .bind(&tags)
    .bind(&patch.icon_url)
    .bind(&patch.banner_url)
    .bind(&patch.discord_url)
    .bind(patch.whitelist_required)
    .bind(patch.allow_offline)
    .bind(form_url)
    .execute(&state.pool)
    .await?;
    audit(&state.pool, Some(id), Some(user.id), "server.updated", json!({ "allow_offline": patch.allow_offline })).await?;
    // Ligar/desligar contas não originais troca os UUIDs da whitelist (online ↔ offline).
    if patch.whitelist_required.is_some() || patch.allow_offline.is_some() {
        crate::agents::push_whitelist(&state, id).await?;
    }
    if patch.allow_offline.is_some() {
        crate::agents::push_join_tickets(&state, id).await?;
    }
    get_one(State(state), MaybeUser(Some(user)), Path(slug)).await
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Member {
    pub user_id: Uuid,
    pub minecraft_uuid: Uuid,
    pub minecraft_name: String,
    pub role: String,
}

pub async fn members(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<Vec<Member>>> {
    let id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, id, &user, Role::Moderator).await?;
    let rows = sqlx::query_as::<_, Member>(
        "SELECT u.id AS user_id, u.minecraft_uuid, u.minecraft_name, m.role
           FROM server_members m JOIN users u ON u.id = m.user_id
          WHERE m.server_id = $1 ORDER BY m.role DESC, u.minecraft_name",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct MemberInput {
    pub minecraft_name: String,
    /// `admin`, `moderator` ou `none` para remover.
    pub role: String,
}

pub async fn set_member(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Json(input): Json<MemberInput>,
) -> ApiResult<Json<Vec<Member>>> {
    let id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, id, &user, Role::Owner).await?;
    let (uuid, name) = lookup_minecraft_name(&state, input.minecraft_name.trim()).await?;
    let target = upsert_user(&state.pool, uuid, &name, false).await?;
    if target.id == user.id {
        return Err(ApiError::bad("O dono não pode alterar o próprio papel."));
    }
    match input.role.as_str() {
        "none" => {
            sqlx::query("DELETE FROM server_members WHERE server_id = $1 AND user_id = $2 AND role <> 'owner'")
                .bind(id)
                .bind(target.id)
                .execute(&state.pool)
                .await?;
        }
        "admin" | "moderator" => {
            sqlx::query(
                "INSERT INTO server_members (server_id, user_id, role) VALUES ($1, $2, $3)
                 ON CONFLICT (server_id, user_id) DO UPDATE SET role = EXCLUDED.role
                 WHERE server_members.role <> 'owner'",
            )
            .bind(id)
            .bind(target.id)
            .bind(&input.role)
            .execute(&state.pool)
            .await?;
        }
        _ => return Err(ApiError::bad("Papel inválido: use admin, moderator ou none.")),
    }
    audit(&state.pool, Some(id), Some(user.id), "member.set", json!({ "member": name, "role": input.role })).await?;
    members(State(state), AuthUser(user), Path(slug)).await
}

pub async fn staff_list(State(state): State<AppState>, _staff: Staff) -> ApiResult<Json<Vec<ServerDetail>>> {
    let rows = sqlx::query_as::<_, ServerRow>(&format!("{SELECT} ORDER BY s.created_at DESC"))
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(rows.into_iter().map(ServerRow::into_detail).collect()))
}

#[derive(Deserialize)]
pub struct ApprovalInput {
    pub approval: String,
}

pub async fn set_approval(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Path(slug): Path<String>,
    Json(input): Json<ApprovalInput>,
) -> ApiResult<Json<serde_json::Value>> {
    if !matches!(input.approval.as_str(), "pending" | "approved" | "rejected" | "suspended") {
        return Err(ApiError::bad("Use pending, approved, rejected ou suspended."));
    }
    let id = server_id_by_slug(&state.pool, &slug).await?;
    sqlx::query(
        "UPDATE servers SET approval = $2,
                approval_updated_at = CASE WHEN approval = $2 THEN approval_updated_at ELSE now() END
          WHERE id = $1",
    )
        .bind(id)
        .bind(&input.approval)
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(id), staff.map(|s| s.id), "server.approval", json!({ "approval": input.approval })).await?;
    Ok(Json(json!({ "slug": slug, "approval": input.approval })))
}

#[derive(Deserialize)]
pub struct FeaturedInput {
    /// Fim do destaque pago; `null` remove o destaque.
    pub until: Option<DateTime<Utc>>,
}

pub async fn set_featured(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Path(slug): Path<String>,
    Json(input): Json<FeaturedInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let id = server_id_by_slug(&state.pool, &slug).await?;
    sqlx::query("UPDATE servers SET featured_until = $2 WHERE id = $1")
        .bind(id)
        .bind(input.until)
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(id), staff.map(|s| s.id), "server.featured", json!({ "until": input.until })).await?;
    Ok(Json(json!({ "slug": slug, "featured_until": input.until })))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AuditEntry {
    pub action: String,
    pub details: serde_json::Value,
    pub actor_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Histórico de deploys, whitelist e equipe (últimos 200 eventos).
pub async fn audit_log(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<Vec<AuditEntry>>> {
    let id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, id, &user, Role::Admin).await?;
    let rows = sqlx::query_as::<_, AuditEntry>(
        "SELECT a.action, a.details, u.minecraft_name AS actor_name, a.created_at
           FROM audit_log a LEFT JOIN users u ON u.id = a.actor_id
          WHERE a.server_id = $1 ORDER BY a.id DESC LIMIT 200",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Serialize)]
pub struct AgentStatus {
    pub configured: bool,
    pub connected: bool,
    pub agent_version: Option<String>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

pub async fn agent_status(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<AgentStatus>> {
    let id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, id, &user, Role::Admin).await?;
    let row: Option<(Option<String>, Option<DateTime<Utc>>)> =
        sqlx::query_as("SELECT agent_version, last_seen_at FROM agents WHERE server_id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    Ok(Json(AgentStatus {
        configured: row.is_some(),
        connected: state.agents.is_connected(id),
        agent_version: row.as_ref().and_then(|r| r.0.clone()),
        last_seen_at: row.and_then(|r| r.1),
    }))
}

#[cfg(test)]
mod tests {
    use super::{companion_supports_tickets, validate_form_url};

    #[test]
    fn form_links() {
        assert!(validate_form_url("https://docs.google.com/forms/d/e/1FAIpQ/viewform?usp=pp_url&entry.1={nick}").is_ok());
        assert!(validate_form_url("https://forms.gle/AbC123").is_ok());
        assert!(validate_form_url("https://forms.gle/").is_err());
        assert!(validate_form_url("http://forms.gle/AbC123").is_err());
        assert!(validate_form_url("https://docs.google.com.evil.com/forms/x").is_err());
        assert!(validate_form_url("https://docs.google.com/document/d/x").is_err());
        assert!(validate_form_url("https://forms.gle/a b").is_err());
        assert!(validate_form_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn companion_version_gate() {
        assert!(companion_supports_tickets("companion-neoforge-0.2.0"));
        assert!(companion_supports_tickets("companion-fabric-1.0.3"));
        assert!(!companion_supports_tickets("companion-neoforge-0.1.0"));
        assert!(!companion_supports_tickets("0.3.0"));
        assert!(!companion_supports_tickets("companion-neoforge-dev"));
    }
}
