//! Lista de espera do launcher (acesso antecipado).
//!
//! Com `CONEXAO_WAITLIST` ligado, só usa o launcher quem a equipe liberou:
//! - no site, a pessoa entra com o Discord e deixa o nick (`/v1/waitlist/*`, servido no
//!   domínio do site, com um cookie só daquele domínio);
//! - a equipe aprova no painel (Equipe Conexão → Lista de espera);
//! - a API recusa o login do launcher (Microsoft e Discord) e o manifesto dos modpacks
//!   para quem não foi liberado, e o `/baixar` manda essas pessoas de volta para a lista.
//!
//! Passam sempre: a equipe Conexão, quem já tinha conta antes da lista (migração 0010)
//! e quem faz parte da equipe de algum servidor parceiro.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Redirect, Response};
use chrono::{DateTime, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{ACCOUNT_OFFLINE, Staff, User, audit, constant_eq, lookup_minecraft_name};
use crate::error::{ApiError, ApiResult};
use crate::routes::discord::{discord_app, discord_user_with};
use crate::state::AppState;

/// Sessão do site (quem entrou com o Discord), assinada com uma chave só dela.
const SESSION_COOKIE: &str = "cx_lista";
/// `state` do OAuth, conferido na volta do Discord.
const STATE_COOKIE: &str = "cx_lista_estado";
const SESSION_DAYS: i64 = 30;
const DISCORD_AUTHORIZE: &str = "https://discord.com/oauth2/authorize";

pub const STATUS_PENDING: &str = "pending";
pub const STATUS_APPROVED: &str = "approved";
pub const STATUS_REJECTED: &str = "rejected";

// ---------------------------------------------------------------- bloqueio do launcher

/// Mensagem para quem ainda não foi liberado (aparece no launcher).
pub fn blocked_message(state: &AppState) -> String {
    let site = state.config.site_url.trim_start_matches("https://").trim_start_matches("http://");
    let discord = state.config.discord_invite.trim_start_matches("https://").trim_start_matches("http://");
    format!(
        "O Conexão está em acesso antecipado e a sua conta ainda não foi liberada. \
         Entre na lista de espera em {site} e acompanhe as liberações no Discord: {discord}"
    )
}

/// Pode usar o launcher? `user` = conta já existente; `minecraft_uuid` = conta original que
/// está entrando; `discord_id` = Discord da conta não original.
pub async fn launcher_allowed(
    state: &AppState,
    user: Option<&User>,
    minecraft_uuid: Option<Uuid>,
    discord_id: Option<&str>,
) -> ApiResult<bool> {
    if !state.config.waitlist {
        return Ok(true);
    }
    if user.is_some_and(|u| u.is_staff || u.launcher_access) {
        return Ok(true);
    }
    if minecraft_uuid.is_some_and(|id| state.config.staff_uuids.contains(&id)) {
        return Ok(true);
    }
    if discord_id.is_some_and(|id| state.config.staff_discord_ids.iter().any(|s| s == id)) {
        return Ok(true);
    }
    if let Some(user) = user {
        let member: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM server_members WHERE user_id = $1)")
            .bind(user.id)
            .fetch_one(&state.pool)
            .await?;
        if member {
            return Ok(true);
        }
    }
    let approved: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM waitlist WHERE status = 'approved'
            AND ((minecraft_uuid IS NOT NULL AND minecraft_uuid = $1)
              OR (discord_id IS NOT NULL AND discord_id = $2)))",
    )
    .bind(minecraft_uuid)
    .bind(discord_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(approved)
}

/// Como [`launcher_allowed`], mas já devolve o erro com o convite para a lista.
pub async fn require_launcher_access(
    state: &AppState,
    user: Option<&User>,
    minecraft_uuid: Option<Uuid>,
    discord_id: Option<&str>,
) -> ApiResult<()> {
    if launcher_allowed(state, user, minecraft_uuid, discord_id).await? {
        Ok(())
    } else {
        Err(ApiError::Waitlist(blocked_message(state)))
    }
}

/// Marca a conta como liberada (depois de um login aceito), para as próximas checagens
/// (manifesto) não precisarem refazer a busca.
pub async fn grant(state: &AppState, mut user: User) -> ApiResult<User> {
    if !user.launcher_access {
        sqlx::query("UPDATE users SET launcher_access = TRUE WHERE id = $1").bind(user.id).execute(&state.pool).await?;
        user.launcher_access = true;
    }
    Ok(user)
}

/// Checagem barata das rotas do jogo (manifesto): só olha a conta já logada.
pub fn require_session_access(state: &AppState, user: Option<&User>) -> ApiResult<()> {
    if !state.config.waitlist || user.is_some_and(|u| u.is_staff || u.launcher_access) {
        Ok(())
    } else {
        Err(ApiError::Waitlist(blocked_message(state)))
    }
}

// ---------------------------------------------------------------- sessão do site

#[derive(Debug, Serialize, Deserialize)]
struct SiteClaims {
    discord_id: String,
    name: String,
    #[serde(default)]
    avatar: Option<String>,
    exp: i64,
}

fn site_secret(state: &AppState) -> Vec<u8> {
    format!("{}:waitlist-site", state.config.jwt_secret).into_bytes()
}

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_string())
}

fn set_cookie(state: &AppState, name: &str, value: &str, max_age_secs: i64) -> ApiResult<HeaderValue> {
    let secure = if state.config.site_url.starts_with("https://") { "; Secure" } else { "" };
    HeaderValue::from_str(&format!("{name}={value}; Path=/; Max-Age={max_age_secs}; HttpOnly; SameSite=Lax{secure}"))
        .map_err(|e| ApiError::Internal(eyre::eyre!(e)))
}

fn site_session(state: &AppState, headers: &HeaderMap) -> Option<SiteClaims> {
    let token = cookie(headers, SESSION_COOKIE)?;
    jsonwebtoken::decode::<SiteClaims>(&token, &DecodingKey::from_secret(&site_secret(state)), &Validation::default())
        .ok()
        .map(|data| data.claims)
}

fn random_hex(bytes: usize) -> String {
    let mut raw = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut raw);
    hex::encode(raw)
}

/// Volta para a seção da lista no site, com um código de erro opcional para o aviso.
fn back_to_site(error: Option<&str>) -> String {
    match error {
        Some(code) => format!("/?erro={code}#lista"),
        None => "/#lista".into(),
    }
}

/// `GET /v1/waitlist/discord`: manda para a autorização do Discord (só `identify`).
pub async fn discord_start(State(state): State<AppState>) -> ApiResult<Response> {
    let Ok((client_id, _)) = discord_app(&state) else {
        return Ok(Redirect::to(&back_to_site(Some("indisponivel"))).into_response());
    };
    let nonce = random_hex(16);
    let url = reqwest::Url::parse_with_params(
        DISCORD_AUTHORIZE,
        &[
            ("response_type", "code"),
            ("client_id", client_id),
            ("scope", "identify"),
            ("prompt", "none"),
            ("redirect_uri", state.config.waitlist_redirect_uri.as_str()),
            ("state", nonce.as_str()),
        ],
    )
    .map_err(|e| ApiError::Internal(e.into()))?;
    let mut response = Redirect::to(url.as_str()).into_response();
    response.headers_mut().append(header::SET_COOKIE, set_cookie(&state, STATE_COOKIE, &nonce, 600)?);
    Ok(response)
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
}

/// `GET /v1/waitlist/discord/callback`: confere o `state`, descobre quem é e grava a sessão do site.
pub async fn discord_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> ApiResult<Response> {
    let expected = cookie(&headers, STATE_COOKIE);
    let code = match (query.code.as_deref(), query.state.as_deref(), expected.as_deref()) {
        (Some(code), Some(got), Some(want))
            if !code.is_empty() && code.len() <= 200 && constant_eq(got.as_bytes(), want.as_bytes()) =>
        {
            Some(code)
        }
        _ => None,
    };
    let (location, session) = match code {
        None => (back_to_site(Some("cancelado")), None),
        Some(code) => match discord_user_with(&state, code, &state.config.waitlist_redirect_uri).await {
            Ok(user) => {
                let claims = SiteClaims {
                    name: user.global_name.clone().unwrap_or_else(|| user.username.clone()),
                    avatar: user.avatar.filter(|a| a.len() <= 64 && a.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')),
                    discord_id: user.id,
                    exp: Utc::now().timestamp() + SESSION_DAYS * 86_400,
                };
                let token = jsonwebtoken::encode(&Header::default(), &claims, &EncodingKey::from_secret(&site_secret(&state)))
                    .map_err(|e| ApiError::Internal(e.into()))?;
                (back_to_site(None), Some(token))
            }
            Err(error) => {
                tracing::warn!("lista de espera: login no Discord falhou: {error}");
                (back_to_site(Some("discord")), None)
            }
        },
    };
    let mut response = Redirect::to(&location).into_response();
    let cookies = response.headers_mut();
    cookies.append(header::SET_COOKIE, set_cookie(&state, STATE_COOKIE, "", 0)?);
    if let Some(token) = session {
        cookies.append(header::SET_COOKIE, set_cookie(&state, SESSION_COOKIE, &token, SESSION_DAYS * 86_400)?);
    }
    Ok(response)
}

/// `POST /v1/waitlist/sair`: apaga a sessão do site.
pub async fn logout(State(state): State<AppState>) -> ApiResult<Response> {
    let mut response = Json(serde_json::json!({ "ok": true })).into_response();
    response.headers_mut().append(header::SET_COOKIE, set_cookie(&state, SESSION_COOKIE, "", 0)?);
    Ok(response)
}

// ---------------------------------------------------------------- status no site

#[derive(sqlx::FromRow)]
struct EntryRow {
    id: Uuid,
    minecraft_name: String,
    account_type: String,
    status: String,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct SiteDiscord {
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Serialize)]
pub struct SiteEntry {
    pub status: String,
    pub minecraft_name: String,
    pub original: bool,
    /// Posição na fila (1 = próximo), só enquanto está aguardando.
    pub position: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct SiteMe {
    /// Lista de espera ligada. Desligada, o site mostra o download para todo mundo.
    pub enabled: bool,
    /// Login pelo Discord configurado.
    pub discord_login: bool,
    pub discord: Option<SiteDiscord>,
    pub entry: Option<SiteEntry>,
    /// Pode baixar o instalador (liberado, equipe ou conta de antes da lista).
    pub can_download: bool,
    pub staff: bool,
    pub waiting: i64,
    pub approved: i64,
    pub discord_invite: String,
}

/// O Discord já tem acesso por outro caminho: equipe ou conta (não original) já liberada.
async fn discord_has_access(state: &AppState, discord_id: &str) -> ApiResult<(bool, bool)> {
    let staff_listed = state.config.staff_discord_ids.iter().any(|s| s == discord_id);
    let (staff, access): (bool, bool) = sqlx::query_as(
        "SELECT COALESCE(bool_or(is_staff), FALSE), COALESCE(bool_or(launcher_access OR is_staff), FALSE)
           FROM users WHERE discord_id = $1",
    )
    .bind(discord_id)
    .fetch_one(&state.pool)
    .await?;
    Ok((staff_listed || staff, staff_listed || access))
}

async fn site_me(state: &AppState, claims: Option<&SiteClaims>) -> ApiResult<SiteMe> {
    let (waiting, approved): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status = 'pending'), count(*) FILTER (WHERE status = 'approved') FROM waitlist",
    )
    .fetch_one(&state.pool)
    .await?;
    let mut me = SiteMe {
        enabled: state.config.waitlist,
        discord_login: discord_app(state).is_ok(),
        discord: None,
        entry: None,
        can_download: !state.config.waitlist,
        staff: false,
        waiting,
        approved,
        discord_invite: state.config.discord_invite.clone(),
    };
    let Some(claims) = claims else {
        return Ok(me);
    };
    me.discord = Some(SiteDiscord {
        name: claims.name.clone(),
        avatar_url: claims
            .avatar
            .as_ref()
            .map(|hash| format!("https://cdn.discordapp.com/avatars/{}/{hash}.png?size=64", claims.discord_id)),
    });
    let entry = sqlx::query_as::<_, EntryRow>(
        "SELECT id, minecraft_name, account_type, status, created_at FROM waitlist WHERE discord_id = $1",
    )
    .bind(&claims.discord_id)
    .fetch_optional(&state.pool)
    .await?;
    let (staff, access) = discord_has_access(state, &claims.discord_id).await?;
    me.staff = staff;
    me.can_download |= access || entry.as_ref().is_some_and(|e| e.status == STATUS_APPROVED);
    if let Some(entry) = entry {
        let position = if entry.status == STATUS_PENDING {
            let ahead: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM waitlist WHERE status = 'pending' AND (created_at, id) < ($1, $2)",
            )
            .bind(entry.created_at)
            .bind(entry.id)
            .fetch_one(&state.pool)
            .await?;
            Some(ahead + 1)
        } else {
            None
        };
        me.entry = Some(SiteEntry {
            status: entry.status,
            minecraft_name: entry.minecraft_name,
            original: entry.account_type != ACCOUNT_OFFLINE,
            position,
            created_at: entry.created_at,
        });
    }
    Ok(me)
}

/// `GET /v1/waitlist/me`: tamanho da fila e, com a sessão do site, a situação da pessoa.
pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Json<SiteMe>> {
    let claims = site_session(&state, &headers);
    Ok(Json(site_me(&state, claims.as_ref()).await?))
}

#[derive(Deserialize)]
pub struct JoinInput {
    pub nick: String,
    /// Tem Minecraft original (o nick é conferido na Mojang).
    pub original: bool,
}

/// `POST /v1/waitlist/me`: entra na fila (ou troca o nick enquanto aguarda). A posição é mantida.
pub async fn join(State(state): State<AppState>, headers: HeaderMap, Json(input): Json<JoinInput>) -> ApiResult<Json<SiteMe>> {
    let claims = site_session(&state, &headers).ok_or(ApiError::Unauthorized)?;
    let nick = input.nick.trim();
    let (name, uuid, kind) = if input.original {
        let (uuid, name) = lookup_minecraft_name(&state, nick).await?;
        (name, Some(uuid), "microsoft")
    } else {
        if !conexao_types::offline::valid_nick(nick) {
            return Err(ApiError::bad("O nick precisa ter de 3 a 16 caracteres: letras, números e _."));
        }
        (nick.to_string(), None, ACCOUNT_OFFLINE)
    };
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM waitlist WHERE discord_id = $1")
        .bind(&claims.discord_id)
        .fetch_optional(&state.pool)
        .await?;
    match status.as_deref() {
        None => {
            sqlx::query(
                "INSERT INTO waitlist (id, discord_id, discord_name, discord_avatar, minecraft_name, minecraft_uuid, account_type)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(Uuid::new_v4())
            .bind(&claims.discord_id)
            .bind(&claims.name)
            .bind(&claims.avatar)
            .bind(&name)
            .bind(uuid)
            .bind(kind)
            .execute(&state.pool)
            .await?;
        }
        Some(STATUS_PENDING) => {
            sqlx::query(
                "UPDATE waitlist SET discord_name = $2, discord_avatar = $3, minecraft_name = $4, minecraft_uuid = $5,
                        account_type = $6, updated_at = now()
                  WHERE discord_id = $1",
            )
            .bind(&claims.discord_id)
            .bind(&claims.name)
            .bind(&claims.avatar)
            .bind(&name)
            .bind(uuid)
            .bind(kind)
            .execute(&state.pool)
            .await?;
        }
        Some(_) => return Err(ApiError::bad("O seu pedido já foi analisado pela equipe.")),
    }
    Ok(Json(site_me(&state, Some(&claims)).await?))
}

/// O `/baixar` entrega o instalador? Com a lista ligada, só para quem tem a sessão do site liberada.
pub async fn download_allowed(state: &AppState, headers: &HeaderMap) -> ApiResult<bool> {
    if !state.config.waitlist {
        return Ok(true);
    }
    let Some(claims) = site_session(state, headers) else {
        return Ok(false);
    };
    Ok(site_me(state, Some(&claims)).await?.can_download)
}

/// Para onde o `/baixar` manda quem ainda não pode baixar.
pub fn waitlist_url(state: &AppState) -> String {
    format!("{}/#lista", state.config.site_url)
}

// ---------------------------------------------------------------- comunidade no Discord

#[derive(Clone, Serialize)]
pub struct Community {
    pub invite: String,
    pub members: Option<u64>,
    pub online: Option<u64>,
}

#[derive(Deserialize)]
struct DiscordInvite {
    #[serde(default)]
    approximate_member_count: Option<u64>,
    #[serde(default)]
    approximate_presence_count: Option<u64>,
}

/// Código do convite (`https://discord.gg/<código>` ou `discord.com/invite/<código>`).
fn invite_code(invite: &str) -> Option<&str> {
    let code = invite.trim_end_matches('/').rsplit('/').next()?;
    (!code.is_empty() && code.len() <= 64 && code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        .then_some(code)
}

static COMMUNITY: std::sync::Mutex<Option<(std::time::Instant, Community)>> = std::sync::Mutex::new(None);
const COMMUNITY_CACHE: std::time::Duration = std::time::Duration::from_secs(600);

/// `GET /v1/waitlist/comunidade`: membros e online do Discord (pelo convite público, cache de 10 min).
pub async fn community(State(state): State<AppState>) -> Json<Community> {
    if let Ok(cache) = COMMUNITY.lock()
        && let Some((at, value)) = cache.as_ref()
        && at.elapsed() < COMMUNITY_CACHE
    {
        return Json(value.clone());
    }
    let mut value = Community { invite: state.config.discord_invite.clone(), members: None, online: None };
    if let Some(code) = invite_code(&state.config.discord_invite) {
        let response = state
            .http
            .get(format!("{}/invites/{code}?with_counts=true", state.config.discord_api_url))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status);
        match response {
            Ok(response) => {
                if let Ok(invite) = response.json::<DiscordInvite>().await {
                    value.members = invite.approximate_member_count;
                    value.online = invite.approximate_presence_count;
                }
            }
            Err(error) => tracing::warn!("convite do Discord: {error}"),
        }
    }
    if let Ok(mut cache) = COMMUNITY.lock() {
        *cache = Some((std::time::Instant::now(), value.clone()));
    }
    Json(value)
}

// ---------------------------------------------------------------- equipe Conexão

#[derive(Serialize, sqlx::FromRow)]
pub struct StaffEntry {
    pub id: Uuid,
    pub discord_id: Option<String>,
    pub discord_name: String,
    pub discord_avatar: Option<String>,
    pub minecraft_name: String,
    pub minecraft_uuid: Option<Uuid>,
    pub account_type: String,
    pub status: String,
    pub note: String,
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decided_by_name: Option<String>,
}

#[derive(Serialize)]
pub struct StaffList {
    pub enabled: bool,
    pub entries: Vec<StaffEntry>,
    pub pending: i64,
    pub approved: i64,
    pub rejected: i64,
}

#[derive(Deserialize)]
pub struct StaffListQuery {
    status: Option<String>,
}

const STAFF_SELECT: &str = "SELECT w.id, w.discord_id, w.discord_name, w.discord_avatar, w.minecraft_name, w.minecraft_uuid,
        w.account_type, w.status, w.note, w.created_at, w.decided_at, u.minecraft_name AS decided_by_name
   FROM waitlist w LEFT JOIN users u ON u.id = w.decided_by";

fn valid_status(status: &str) -> bool {
    matches!(status, STATUS_PENDING | STATUS_APPROVED | STATUS_REJECTED)
}

/// `GET /v1/staff/waitlist?status=pending`: fila (mais antigos primeiro) ou decididos (mais recentes primeiro).
pub async fn staff_list(
    State(state): State<AppState>,
    _staff: Staff,
    Query(query): Query<StaffListQuery>,
) -> ApiResult<Json<StaffList>> {
    let status = query.status.unwrap_or_else(|| STATUS_PENDING.into());
    if !valid_status(&status) {
        return Err(ApiError::bad("Situação inválida."));
    }
    let order = if status == STATUS_PENDING { "w.created_at, w.id" } else { "w.decided_at DESC NULLS LAST, w.created_at DESC" };
    let entries = sqlx::query_as::<_, StaffEntry>(&format!("{STAFF_SELECT} WHERE w.status = $1 ORDER BY {order} LIMIT 1000"))
        .bind(&status)
        .fetch_all(&state.pool)
        .await?;
    let (pending, approved, rejected): (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status = 'pending'), count(*) FILTER (WHERE status = 'approved'),
                count(*) FILTER (WHERE status = 'rejected') FROM waitlist",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(StaffList { enabled: state.config.waitlist, entries, pending, approved, rejected }))
}

/// Aplica a decisão às contas que já existem: liberar marca `launcher_access`, recusar tira.
async fn sync_users(state: &AppState, minecraft_uuid: Option<Uuid>, discord_id: Option<&str>, access: bool) -> ApiResult<()> {
    sqlx::query(
        "UPDATE users SET launcher_access = $3
          WHERE NOT is_staff
            AND ((account_type = 'microsoft' AND $1::uuid IS NOT NULL AND minecraft_uuid = $1)
              OR ($2::text IS NOT NULL AND discord_id = $2))",
    )
    .bind(minecraft_uuid)
    .bind(discord_id)
    .bind(access)
    .execute(&state.pool)
    .await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct DecideInput {
    pub status: String,
    pub note: Option<String>,
}

/// `POST /v1/staff/waitlist/{id}`: aprova, recusa ou devolve para a fila.
pub async fn staff_decide(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Path(id): Path<Uuid>,
    Json(input): Json<DecideInput>,
) -> ApiResult<Json<StaffEntry>> {
    if !valid_status(&input.status) {
        return Err(ApiError::bad("Situação inválida: use approved, rejected ou pending."));
    }
    let note = input.note.map(|n| n.trim().chars().take(300).collect::<String>());
    let actor = staff.as_ref().map(|u| u.id);
    let (minecraft_uuid, discord_id, name): (Option<Uuid>, Option<String>, String) = sqlx::query_as(
        "UPDATE waitlist
            SET status = $2, note = COALESCE($3, note), updated_at = now(),
                decided_at = CASE WHEN $2 = 'pending' THEN NULL ELSE now() END,
                decided_by = CASE WHEN $2 = 'pending' THEN NULL ELSE $4 END
          WHERE id = $1
      RETURNING minecraft_uuid, discord_id, minecraft_name",
    )
    .bind(id)
    .bind(&input.status)
    .bind(note)
    .bind(actor)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    sync_users(&state, minecraft_uuid, discord_id.as_deref(), input.status == STATUS_APPROVED).await?;
    audit(&state.pool, None, actor, &format!("waitlist.{}", input.status), serde_json::json!({ "nick": name, "discord_id": discord_id }))
        .await?;
    let entry = sqlx::query_as::<_, StaffEntry>(&format!("{STAFF_SELECT} WHERE w.id = $1"))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(entry))
}

#[derive(Deserialize)]
pub struct NextInput {
    pub count: i64,
}

/// `POST /v1/staff/waitlist/next`: libera os N primeiros da fila de uma vez.
pub async fn staff_approve_next(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Json(input): Json<NextInput>,
) -> ApiResult<Json<serde_json::Value>> {
    if !(1..=500).contains(&input.count) {
        return Err(ApiError::bad("Escolha de 1 a 500 pessoas."));
    }
    let actor = staff.as_ref().map(|u| u.id);
    let approved: Vec<(Option<Uuid>, Option<String>)> = sqlx::query_as(
        "UPDATE waitlist SET status = 'approved', decided_at = now(), decided_by = $2, updated_at = now()
          WHERE id IN (SELECT id FROM waitlist WHERE status = 'pending' ORDER BY created_at, id LIMIT $1 FOR UPDATE SKIP LOCKED)
      RETURNING minecraft_uuid, discord_id",
    )
    .bind(input.count)
    .bind(actor)
    .fetch_all(&state.pool)
    .await?;
    for (minecraft_uuid, discord_id) in &approved {
        sync_users(&state, *minecraft_uuid, discord_id.as_deref(), true).await?;
    }
    audit(&state.pool, None, actor, "waitlist.approved_batch", serde_json::json!({ "count": approved.len() })).await?;
    Ok(Json(serde_json::json!({ "approved": approved.len() })))
}

#[derive(Deserialize)]
pub struct ManualInput {
    pub nick: String,
}

/// `POST /v1/staff/waitlist/manual`: libera alguém pelo nick, sem passar pelo site
/// (conta que já existe no Conexão, ou conta original conferida na Mojang).
pub async fn staff_manual(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Json(input): Json<ManualInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let nick = input.nick.trim();
    let actor = staff.as_ref().map(|u| u.id);
    let existing: Option<(Uuid, String)> = sqlx::query_as(
        "UPDATE users SET launcher_access = TRUE WHERE lower(minecraft_name) = lower($1)
      RETURNING id, minecraft_name",
    )
    .bind(nick)
    .fetch_optional(&state.pool)
    .await?;
    if let Some((_, name)) = existing {
        audit(&state.pool, None, actor, "waitlist.manual", serde_json::json!({ "nick": name, "existing": true })).await?;
        return Ok(Json(serde_json::json!({ "nick": name, "existing": true })));
    }
    let (uuid, name) = lookup_minecraft_name(&state, nick).await?;
    // Já pediu pelo site com esse nick: aprova o pedido em vez de criar outro.
    let updated = sqlx::query(
        "UPDATE waitlist SET status = 'approved', decided_at = now(), decided_by = $2, updated_at = now()
          WHERE minecraft_uuid = $1 AND status <> 'approved'",
    )
    .bind(uuid)
    .bind(actor)
    .execute(&state.pool)
    .await?
    .rows_affected();
    let already: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM waitlist WHERE minecraft_uuid = $1 AND status = 'approved')")
        .bind(uuid)
        .fetch_one(&state.pool)
        .await?;
    if updated == 0 && !already {
        sqlx::query(
            "INSERT INTO waitlist (id, minecraft_name, minecraft_uuid, account_type, status, decided_at, decided_by, note)
             VALUES ($1, $2, $3, 'microsoft', 'approved', now(), $4, 'Liberado pelo painel')",
        )
        .bind(Uuid::new_v4())
        .bind(&name)
        .bind(uuid)
        .bind(actor)
        .execute(&state.pool)
        .await?;
    }
    audit(&state.pool, None, actor, "waitlist.manual", serde_json::json!({ "nick": name, "existing": false })).await?;
    Ok(Json(serde_json::json!({ "nick": name, "existing": false })))
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header};

    use super::{back_to_site, cookie, invite_code};

    #[test]
    fn reads_invite_codes() {
        assert_eq!(invite_code("https://discord.gg/conexaosmps"), Some("conexaosmps"));
        assert_eq!(invite_code("https://discord.com/invite/abc-123/"), Some("abc-123"));
        assert_eq!(invite_code("https://discord.gg/"), None);
        assert_eq!(invite_code("https://discord.gg/a?b"), None);
    }

    #[test]
    fn reads_cookies() {
        let mut headers = HeaderMap::new();
        headers.append(header::COOKIE, HeaderValue::from_static("a=1; cx_lista=abc.def; b=2"));
        headers.append(header::COOKIE, HeaderValue::from_static("cx_lista_estado=xyz"));
        assert_eq!(cookie(&headers, "cx_lista").as_deref(), Some("abc.def"));
        assert_eq!(cookie(&headers, "cx_lista_estado").as_deref(), Some("xyz"));
        assert_eq!(cookie(&headers, "outro"), None);
    }

    #[test]
    fn returns_to_the_list() {
        assert_eq!(back_to_site(None), "/#lista");
        assert_eq!(back_to_site(Some("discord")), "/?erro=discord#lista");
    }
}
