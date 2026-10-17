//! Autenticação: conta Conexão = conta Minecraft verificada.
//!
//! O launcher envia o access token da Minecraft Services; a API confirma o
//! perfil chamando a própria Mojang e devolve um JWT curto do Conexão.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const TOKEN_HOURS: i64 = 12;
/// Token do `conexao-publish`: curto e só para publicar modpack.
const PUBLISH_TOKEN_HOURS: i64 = 2;
pub const SCOPE_PUBLISH: &str = "publish";

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: Uuid,
    name: String,
    exp: i64,
    iat: i64,
    /// Sem escopo = sessão completa (launcher e painel). `publish` = só as rotas de publicação.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
}

/// Rotas que um token `publish` pode chamar (as que o `conexao-publish` usa).
pub fn publish_scope_allows(method: &axum::http::Method, path: &str) -> bool {
    use axum::http::Method;
    let path = path.strip_prefix("/v1").unwrap_or(path);
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    matches!(
        (method, segments.as_slice()),
        (&Method::GET, ["servers", _] | ["servers", _, "manifest"])
            | (
                &Method::POST,
                ["servers", _, "versions"] | ["servers", _, "versions", "plan"] | ["files", "missing" | "resolve"]
            )
            | (&Method::PUT, ["files", _])
    )
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub minecraft_uuid: Uuid,
    pub minecraft_name: String,
    pub is_staff: bool,
    /// `microsoft` (conta original) ou `offline` (conta não original, login pelo Discord).
    pub account_type: String,
    /// Liberado na lista de espera (ou conta de antes dela).
    pub launcher_access: bool,
}

pub const ACCOUNT_OFFLINE: &str = "offline";

/// Colunas da tabela `users` lidas por `query_as::<_, User>`.
pub const USER_COLUMNS: &str = "id, minecraft_uuid, minecraft_name, is_staff, account_type, launcher_access";

impl User {
    pub fn is_offline(&self) -> bool {
        self.account_type == ACCOUNT_OFFLINE
    }
}

pub fn issue_token(state: &AppState, user: &User) -> ApiResult<String> {
    sign(state, user, TOKEN_HOURS, None)
}

/// Token só de publicação, para colar no terminal do `conexao-publish`.
pub fn issue_publish_token(state: &AppState, user: &User) -> ApiResult<String> {
    sign(state, user, PUBLISH_TOKEN_HOURS, Some(SCOPE_PUBLISH))
}

fn sign(state: &AppState, user: &User, hours: i64, scope: Option<&str>) -> ApiResult<String> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user.id,
        name: user.minecraft_name.clone(),
        iat: now,
        exp: now + hours * 3600,
        scope: scope.map(str::to_string),
    };
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .map_err(|e| ApiError::Internal(e.into()))
}

fn bearer(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
}

pub fn staff_token_ok(state: &AppState, parts: &Parts) -> bool {
    let Some(expected) = state.config.staff_token.as_deref() else {
        return false;
    };
    parts
        .headers
        .get("x-conexao-staff-token")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|given| constant_eq(given.as_bytes(), expected.as_bytes()))
}

pub fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Usuário autenticado (JWT válido).
pub struct AuthUser(pub User);

/// Usuário opcional, para rotas públicas que mudam com login.
pub struct MaybeUser(pub Option<User>);

/// Equipe Conexão: usuário com `is_staff` ou cabeçalho `X-Conexao-Staff-Token`.
pub struct Staff(pub Option<User>);

async fn user_from_parts(parts: &Parts, state: &AppState) -> ApiResult<Option<User>> {
    let Some(token) = bearer(parts) else {
        return Ok(None);
    };
    let data = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| ApiError::Unauthorized)?;
    match data.claims.scope.as_deref() {
        None => {}
        Some(SCOPE_PUBLISH) if publish_scope_allows(&parts.method, parts.uri.path()) => {}
        Some(_) => return Err(ApiError::Forbidden),
    }
    let user = sqlx::query_as::<_, User>(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1"))
    .bind(data.claims.sub)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::Unauthorized)?;
    Ok(Some(user))
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        user_from_parts(parts, state).await?.map(AuthUser).ok_or(ApiError::Unauthorized)
    }
}

/// Quem está enviando arquivos: um admin pelo painel ou o agente/Companion de um servidor.
pub enum Uploader {
    User(User),
    Agent(Uuid),
}

impl FromRequestParts<AppState> for Uploader {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        if let Some(token) = bearer(parts).filter(|t| t.starts_with("cagt_")) {
            let server_id: Uuid = sqlx::query_scalar("SELECT server_id FROM agents WHERE token_sha256 = $1")
                .bind(crate::agents::sha256_hex(token.trim()))
                .fetch_optional(&state.pool)
                .await?
                .ok_or(ApiError::Unauthorized)?;
            return Ok(Uploader::Agent(server_id));
        }
        user_from_parts(parts, state).await?.map(Uploader::User).ok_or(ApiError::Unauthorized)
    }
}

impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        Ok(MaybeUser(user_from_parts(parts, state).await?))
    }
}

impl FromRequestParts<AppState> for Staff {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        if staff_token_ok(state, parts) {
            return Ok(Staff(None));
        }
        match user_from_parts(parts, state).await? {
            Some(user) if user.is_staff => Ok(Staff(Some(user))),
            Some(_) => Err(ApiError::Forbidden),
            None => Err(ApiError::Unauthorized),
        }
    }
}

/// Papel de um membro na equipe de um servidor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Moderator,
    Admin,
    Owner,
}

impl Role {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "moderator" => Role::Moderator,
            "admin" => Role::Admin,
            "owner" => Role::Owner,
            _ => return None,
        })
    }
}

/// Exige que o usuário tenha pelo menos `min` no servidor. Staff passa sempre.
pub async fn require_role(pool: &PgPool, server_id: Uuid, user: &User, min: Role) -> ApiResult<Role> {
    if user.is_staff {
        return Ok(Role::Owner);
    }
    let role: Option<String> =
        sqlx::query_scalar("SELECT role FROM server_members WHERE server_id = $1 AND user_id = $2")
            .bind(server_id)
            .bind(user.id)
            .fetch_optional(pool)
            .await?;
    match role.as_deref().and_then(Role::parse) {
        Some(role) if role >= min => Ok(role),
        _ => Err(ApiError::Forbidden),
    }
}

#[derive(Deserialize)]
struct MinecraftProfile {
    id: String,
    name: String,
}

/// Confirma o access token na Minecraft Services e devolve (uuid, nome).
pub async fn verify_minecraft_token(state: &AppState, access_token: &str) -> ApiResult<(Uuid, String)> {
    let response = state
        .http
        .get(&state.config.minecraft_profile_url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(ApiError::Unauthorized);
    }
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(ApiError::bad("Esta conta Microsoft não possui Minecraft Java."));
    }
    let profile: MinecraftProfile = response
        .error_for_status()
        .map_err(|e| ApiError::Internal(e.into()))?
        .json()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    let uuid = Uuid::parse_str(&profile.id).map_err(|_| ApiError::bad("UUID de perfil inválido"))?;
    Ok((uuid, profile.name))
}

/// Busca o UUID de um nick na Mojang (para adicionar alguém à whitelist pelo nome).
pub async fn lookup_minecraft_name(state: &AppState, name: &str) -> ApiResult<(Uuid, String)> {
    if !(3..=16).contains(&name.len()) || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return Err(ApiError::bad("Nick de Minecraft inválido."));
    }
    let response = state
        .http
        .get(format!("{}/{}", state.config.mojang_lookup_url, name))
        .send()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    if matches!(response.status(), reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::NO_CONTENT) {
        return Err(ApiError::bad(format!("O nick {name} não existe no Minecraft.")));
    }
    let profile: MinecraftProfile = response
        .error_for_status()
        .map_err(|e| ApiError::Internal(e.into()))?
        .json()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    let uuid = Uuid::parse_str(&profile.id).map_err(|_| ApiError::bad("UUID de perfil inválido"))?;
    Ok((uuid, profile.name))
}

/// O nick pertence a uma conta original? `Ok(false)` só quando a Mojang confirma que não existe.
pub async fn minecraft_name_exists(state: &AppState, name: &str) -> ApiResult<bool> {
    let response = state
        .http
        .get(format!("{}/{}", state.config.mojang_lookup_url, name))
        .send()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    match response.status() {
        reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::NO_CONTENT => Ok(false),
        status if status.is_success() => Ok(true),
        status => Err(ApiError::Internal(eyre::eyre!("Mojang respondeu {status} ao consultar o nick"))),
    }
}

/// Cria ou atualiza o usuário (conta original) pelo UUID Minecraft.
pub async fn upsert_user(pool: &PgPool, minecraft_uuid: Uuid, name: &str, login: bool) -> ApiResult<User> {
    let user = sqlx::query_as::<_, User>(&format!(
        "INSERT INTO users (id, minecraft_uuid, minecraft_name)
         VALUES ($1, $2, $3)
         ON CONFLICT (minecraft_uuid) DO UPDATE
            SET minecraft_name = EXCLUDED.minecraft_name,
                last_login_at = CASE WHEN $4 THEN now() ELSE users.last_login_at END
         RETURNING {USER_COLUMNS}"
    ))
    .bind(Uuid::new_v4())
    .bind(minecraft_uuid)
    .bind(name)
    .bind(login)
    .fetch_one(pool)
    .await?;
    Ok(user)
}

pub async fn audit(
    pool: &PgPool,
    server_id: Option<Uuid>,
    actor: Option<Uuid>,
    action: &str,
    details: serde_json::Value,
) -> ApiResult<()> {
    sqlx::query("INSERT INTO audit_log (server_id, actor_id, action, details) VALUES ($1, $2, $3, $4)")
        .bind(server_id)
        .bind(actor)
        .bind(action)
        .bind(details)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::http::Method;

    use super::publish_scope_allows;

    #[test]
    fn publish_token_only_publishes() {
        assert!(publish_scope_allows(&Method::GET, "/v1/servers/paravexia"));
        assert!(publish_scope_allows(&Method::GET, "/servers/paravexia/manifest"));
        assert!(publish_scope_allows(&Method::POST, "/v1/servers/paravexia/versions"));
        assert!(publish_scope_allows(&Method::PUT, "/v1/files/da39a3ee5e6b4b0d3255bfef95601890afd80709"));
        assert!(publish_scope_allows(&Method::POST, "/v1/files/missing"));
        assert!(!publish_scope_allows(&Method::GET, "/v1/me"));
        assert!(!publish_scope_allows(&Method::PUT, "/v1/me/profile"));
        assert!(!publish_scope_allows(&Method::POST, "/v1/auth/pair/approve"));
        assert!(!publish_scope_allows(&Method::POST, "/v1/auth/publish-token"));
        assert!(!publish_scope_allows(&Method::PATCH, "/v1/servers/paravexia"));
        assert!(!publish_scope_allows(&Method::PUT, "/v1/servers/paravexia/members"));
        assert!(!publish_scope_allows(&Method::POST, "/v1/servers/paravexia/featured/checkout"));
    }
}
