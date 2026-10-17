//! Contas não originais: login pelo Discord.
//!
//! Fluxo: o launcher abre `authorize` do Discord numa janela própria, pega o `code`
//! quando a janela chega no redirect e o envia para `POST /v1/auth/discord`. A API troca
//! o código (com o segredo do app), descobre quem é no Discord e:
//! - se já existe conta, devolve a sessão;
//! - senão, devolve um `signup_token` para o launcher pedir o nick
//!   (`POST /v1/auth/offline/register`).
//!
//! A sessão do launcher é um segredo longo (`offline_sessions`) trocado por JWT em
//! `POST /v1/auth/offline/session`, porque não há token Microsoft para renovar.

use axum::Json;
use axum::extract::State;
use axum::response::Html;
use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agents::sha256_hex;
use crate::auth::{ACCOUNT_OFFLINE, USER_COLUMNS, User, audit, issue_token, minecraft_name_exists};
use crate::error::{ApiError, ApiResult};
use crate::routes::waitlist;
use crate::state::AppState;

/// Validade do cadastro em andamento (entre o login no Discord e a escolha do nick).
const SIGNUP_MINUTES: i64 = 15;

pub(crate) fn discord_app(state: &AppState) -> ApiResult<(&str, &str)> {
    match (&state.config.discord_client_id, &state.config.discord_client_secret) {
        (Some(id), Some(secret)) => Ok((id, secret)),
        _ => Err(ApiError::bad("O login pelo Discord ainda não está disponível.")),
    }
}

#[derive(Serialize)]
pub struct DiscordConfig {
    pub enabled: bool,
    pub client_id: Option<String>,
    pub redirect_uri: String,
    pub scope: &'static str,
}

/// O launcher monta a URL de autorização com estes dados (o client ID não fica embutido no app).
pub async fn config(State(state): State<AppState>) -> Json<DiscordConfig> {
    Json(DiscordConfig {
        enabled: discord_app(&state).is_ok(),
        client_id: state.config.discord_client_id.clone(),
        redirect_uri: state.config.discord_redirect_uri.clone(),
        scope: "identify",
    })
}

/// Página do redirect. O launcher lê o `code` da URL e fecha a janela; isto só aparece
/// se alguém abrir o link num navegador comum.
pub async fn callback() -> Html<&'static str> {
    Html(
        "<!doctype html><html lang=\"pt-BR\"><meta charset=\"utf-8\"><title>Conexão</title>\
         <body style=\"font-family:sans-serif;background:#0e1518;color:#e8f0ea;display:grid;place-items:center;height:100vh;margin:0\">\
         <p>Login concluído. Pode voltar para o launcher do Conexão.</p></body></html>",
    )
}

#[derive(Deserialize)]
pub struct DiscordLogin {
    pub code: String,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DiscordLoginResponse {
    /// Conta existente: sessão pronta.
    Ok(OfflineLogin),
    /// Primeiro acesso: falta escolher o nick.
    NeedsNick { signup_token: String, discord_name: String, suggested_nick: Option<String> },
}

#[derive(Serialize)]
pub struct OfflineLogin {
    pub token: String,
    /// Segredo de longa duração; o launcher guarda e troca por um JWT novo quando precisar.
    pub session: String,
    pub user: User,
}

#[derive(Deserialize)]
struct DiscordToken {
    access_token: String,
}

#[derive(Deserialize)]
pub(crate) struct DiscordUser {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub global_name: Option<String>,
    /// Hash do avatar (`cdn.discordapp.com/avatars/<id>/<hash>.png`).
    #[serde(default)]
    pub avatar: Option<String>,
}

async fn discord_user(state: &AppState, code: &str) -> ApiResult<DiscordUser> {
    discord_user_with(state, code, &state.config.discord_redirect_uri).await
}

/// Troca o `code` pelo usuário do Discord. `redirect_uri` precisa ser o mesmo usado na autorização
/// (o do launcher ou o do site, na lista de espera).
pub(crate) async fn discord_user_with(state: &AppState, code: &str, redirect_uri: &str) -> ApiResult<DiscordUser> {
    let (client_id, client_secret) = discord_app(state)?;
    let response = state
        .http
        .post(format!("{}/oauth2/token", state.config.discord_api_url))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ])
        .send()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    if response.status().is_client_error() {
        return Err(ApiError::bad("O login no Discord expirou. Tente de novo."));
    }
    let token: DiscordToken = response
        .error_for_status()
        .map_err(|e| ApiError::Internal(e.into()))?
        .json()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    state
        .http
        .get(format!("{}/users/@me", state.config.discord_api_url))
        .bearer_auth(&token.access_token)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| ApiError::Internal(e.into()))?
        .json()
        .await
        .map_err(|e| ApiError::Internal(e.into()))
}

#[derive(Debug, Serialize, Deserialize)]
struct SignupClaims {
    discord_id: String,
    discord_name: String,
    exp: i64,
}

/// Chave própria: um `signup_token` nunca vale como sessão (e vice-versa).
fn signup_secret(state: &AppState) -> Vec<u8> {
    format!("{}:discord-signup", state.config.jwt_secret).into_bytes()
}

/// Nick sugerido a partir do nome no Discord (só se já for um nick válido).
fn suggest_nick(user: &DiscordUser) -> Option<String> {
    [user.global_name.as_deref(), Some(user.username.as_str())]
        .into_iter()
        .flatten()
        .map(|name| name.replace([' ', '.', '-'], "_"))
        .find(|name| conexao_types::offline::valid_nick(name))
}

async fn new_session(state: &AppState, user: &User) -> ApiResult<String> {
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    let session = format!("cofs_{}", hex::encode(raw));
    sqlx::query("INSERT INTO offline_sessions (token_sha256, user_id) VALUES ($1, $2)")
        .bind(sha256_hex(&session))
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1").bind(user.id).execute(&state.pool).await?;
    Ok(session)
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<DiscordLogin>,
) -> ApiResult<Json<DiscordLoginResponse>> {
    if body.code.is_empty() || body.code.len() > 200 {
        return Err(ApiError::bad("Código do Discord inválido."));
    }
    let discord = discord_user(&state, &body.code).await?;
    let existing = sqlx::query_as::<_, User>(&format!("SELECT {USER_COLUMNS} FROM users WHERE discord_id = $1"))
        .bind(&discord.id)
        .fetch_optional(&state.pool)
        .await?;
    if let Some(user) = existing {
        if !user.is_offline() {
            return Err(ApiError::bad(
                "Este Discord está ligado a uma conta original. Entre com a conta Microsoft.",
            ));
        }
        waitlist::require_launcher_access(&state, Some(&user), None, Some(&discord.id)).await?;
        let session = new_session(&state, &user).await?;
        let token = issue_token(&state, &user)?;
        return Ok(Json(DiscordLoginResponse::Ok(OfflineLogin { token, session, user })));
    }
    // Sem conta ainda: só segue para a escolha do nick quem foi liberado na lista.
    waitlist::require_launcher_access(&state, None, None, Some(&discord.id)).await?;
    let claims = SignupClaims {
        discord_id: discord.id.clone(),
        discord_name: discord.global_name.clone().unwrap_or_else(|| discord.username.clone()),
        exp: Utc::now().timestamp() + SIGNUP_MINUTES * 60,
    };
    let signup_token = jsonwebtoken::encode(&Header::default(), &claims, &EncodingKey::from_secret(&signup_secret(&state)))
        .map_err(|e| ApiError::Internal(e.into()))?;
    Ok(Json(DiscordLoginResponse::NeedsNick {
        signup_token,
        suggested_nick: suggest_nick(&discord),
        discord_name: claims.discord_name,
    }))
}

#[derive(Deserialize)]
pub struct RegisterInput {
    pub signup_token: String,
    pub nick: String,
}

/// Confere se o nick pode virar uma conta não original. Mensagens para o jogador.
async fn check_nick(state: &AppState, nick: &str) -> ApiResult<()> {
    if !conexao_types::offline::valid_nick(nick) {
        return Err(ApiError::bad("O nick precisa ter de 3 a 16 caracteres: letras, números e _."));
    }
    let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE lower(minecraft_name) = lower($1))")
        .bind(nick)
        .fetch_one(&state.pool)
        .await?;
    if taken {
        return Err(ApiError::bad(format!("O nick {nick} já está em uso no Conexão.")));
    }
    // Nick de conta original fica reservado para o dono, mesmo que ele nunca tenha usado o Conexão.
    let original = minecraft_name_exists(state, nick).await.map_err(|_| {
        ApiError::bad("Não foi possível conferir o nick agora. Tente de novo em instantes.")
    })?;
    if original {
        return Err(ApiError::bad(format!(
            "{nick} é o nick de uma conta original do Minecraft. Escolha outro."
        )));
    }
    Ok(())
}

pub async fn register(
    State(state): State<AppState>,
    Json(input): Json<RegisterInput>,
) -> ApiResult<Json<OfflineLogin>> {
    let claims = jsonwebtoken::decode::<SignupClaims>(
        &input.signup_token,
        &DecodingKey::from_secret(&signup_secret(&state)),
        &Validation::default(),
    )
    .map_err(|_| ApiError::bad("O cadastro expirou. Entre com o Discord de novo."))?
    .claims;
    let nick = input.nick.trim();
    waitlist::require_launcher_access(&state, None, None, Some(&claims.discord_id)).await?;
    check_nick(&state, nick).await?;
    let user = sqlx::query_as::<_, User>(&format!(
        "INSERT INTO users (id, minecraft_uuid, minecraft_name, discord_id, account_type)
         VALUES ($1, $2, $3, $4, '{ACCOUNT_OFFLINE}')
         RETURNING {USER_COLUMNS}"
    ))
    .bind(Uuid::new_v4())
    .bind(conexao_types::offline::offline_uuid(nick))
    .bind(nick)
    .bind(&claims.discord_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|error| match ApiError::from(error) {
        ApiError::Conflict { .. } => ApiError::bad("Este nick ou este Discord já tem conta no Conexão."),
        other => other,
    })?;
    audit(&state.pool, None, Some(user.id), "account.offline_created", serde_json::json!({ "nick": nick })).await?;
    let user = waitlist::grant(&state, user).await?;
    let session = new_session(&state, &user).await?;
    let token = issue_token(&state, &user)?;
    Ok(Json(OfflineLogin { token, session, user }))
}

#[derive(Deserialize)]
pub struct SessionInput {
    pub session: String,
}

#[derive(Serialize)]
pub struct SessionLogin {
    pub token: String,
    pub user: User,
}

/// Troca a sessão longa do launcher por um JWT novo.
pub async fn session(
    State(state): State<AppState>,
    Json(input): Json<SessionInput>,
) -> ApiResult<Json<SessionLogin>> {
    let user_id: Uuid = sqlx::query_scalar(
        "UPDATE offline_sessions SET last_used_at = now() WHERE token_sha256 = $1 RETURNING user_id",
    )
    .bind(sha256_hex(input.session.trim()))
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::Unauthorized)?;
    let user = sqlx::query_as::<_, User>(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1"))
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?;
    let discord_id: Option<String> = sqlx::query_scalar("SELECT discord_id FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?;
    waitlist::require_launcher_access(&state, Some(&user), None, discord_id.as_deref()).await?;
    let token = issue_token(&state, &user)?;
    Ok(Json(SessionLogin { token, user }))
}

/// Sair: apaga a sessão deste launcher.
pub async fn logout(State(state): State<AppState>, Json(input): Json<SessionInput>) -> ApiResult<Json<serde_json::Value>> {
    sqlx::query("DELETE FROM offline_sessions WHERE token_sha256 = $1")
        .bind(sha256_hex(input.session.trim()))
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(username: &str, global: Option<&str>) -> DiscordUser {
        DiscordUser { id: "1".into(), username: username.into(), global_name: global.map(str::to_string), avatar: None }
    }

    #[test]
    fn suggests_valid_nicks_only() {
        assert_eq!(suggest_nick(&user("lucas.dev", None)).as_deref(), Some("lucas_dev"));
        assert_eq!(suggest_nick(&user("lucas", Some("Lucas S"))).as_deref(), Some("Lucas_S"));
        assert_eq!(suggest_nick(&user("ab", Some("Ç"))), None);
    }
}
