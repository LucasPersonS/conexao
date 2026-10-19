//! Apoio a servidores (split do Mercado Pago), conta de recebimento dos donos e
//! estatísticas do plano PRO.
//!
//! Como o dinheiro anda: o jogador paga um link criado **com o token do dono** e com
//! `marketplace_fee` = taxa do Conexão. O Mercado Pago credita a parte do dono direto
//! na conta dele e a taxa na conta do Conexão. O Conexão nunca guarda dinheiro de terceiros.
//!
//! Segurança da conexão (OAuth):
//! * só o **dono** do servidor conecta/desconecta a conta que recebe (admin não);
//! * `state` aleatório, guardado só como hash, uso único, vale 10 min, preso ao usuário
//!   e ao servidor; PKCE (S256) junto;
//! * tokens guardados criptografados (`crypto.rs`) e nunca devolvidos ao painel.

use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use base64::Engine;
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::agents::sha256_hex;
use crate::auth::{AuthUser, Role, USER_COLUMNS, User, audit, require_role};
use crate::billing;
use crate::crypto;
use crate::error::{ApiError, ApiResult};
use crate::moderation;
use crate::routes::servers::server_id_by_slug;
use crate::state::AppState;

const STATE_TTL_MINUTES: i64 = 10;
/// Renova o token do dono quando faltar menos que isso para vencer.
const REFRESH_BEFORE_DAYS: i64 = 15;

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn random_token(len: usize) -> String {
    let mut raw = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut raw);
    b64url(&raw)
}

/// PKCE S256: base64url(sha256(verifier)).
pub fn code_challenge(verifier: &str) -> String {
    b64url(&Sha256::digest(verifier.as_bytes()))
}

fn oauth_ready(state: &AppState) -> ApiResult<(&str, &str, &[u8; 32])> {
    match (&state.config.mp_client_id, &state.config.mp_client_secret, &state.config.secrets_key, &state.config.mp_access_token) {
        (Some(id), Some(secret), Some(key), Some(_)) => Ok((id, secret, key)),
        _ => Err(ApiError::bad("O apoio a servidores ainda não está ativo no Conexão. Fale com a equipe.")),
    }
}

/// O dono de verdade do servidor (linha `owner` na equipe). Diferente de `require_role`,
/// a equipe Conexão **não** passa: ninguém além do dono escolhe para onde vai o dinheiro.
async fn require_owner(pool: &sqlx::PgPool, server_id: Uuid, user: &User) -> ApiResult<()> {
    let owner: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM server_members WHERE server_id = $1 AND user_id = $2 AND role = 'owner')")
        .bind(server_id)
        .bind(user.id)
        .fetch_one(pool)
        .await?;
    if owner { Ok(()) } else { Err(ApiError::Forbidden) }
}

// ------------------------------------------------------------------ tokens do dono

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    user_id: Option<serde_json::Value>,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    live_mode: Option<bool>,
}

async fn token_request(state: &AppState, form: serde_json::Value) -> ApiResult<TokenResponse> {
    let response = state
        .http
        .post(format!("{}/oauth/token", state.config.mp_api_url))
        .timeout(Duration::from_secs(20))
        .json(&form)
        .send()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    if !response.status().is_success() {
        let status = response.status();
        // Não registra o corpo inteiro: pode ecoar parâmetros sensíveis.
        tracing::warn!("Mercado Pago recusou o pedido de token OAuth ({status})");
        return Err(ApiError::bad("O Mercado Pago não autorizou a conexão. Tente conectar de novo."));
    }
    let parsed: TokenResponse = response.json().await.map_err(|e| ApiError::Internal(e.into()))?;
    if parsed.access_token.is_empty() || parsed.access_token.len() > 512 {
        return Err(ApiError::Internal(eyre::eyre!("token OAuth inesperado")));
    }
    Ok(parsed)
}

fn mp_user_id(value: Option<&serde_json::Value>) -> Option<String> {
    let id = value?.as_i64().map(|v| v.to_string()).or_else(|| value?.as_str().map(str::to_string))?;
    (!id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_digit())).then_some(id)
}

async fn save_tokens(state: &AppState, server_id: Uuid, user: Option<Uuid>, mp_user: &str, tokens: &TokenResponse) -> ApiResult<()> {
    let (_, _, key) = oauth_ready(state)?;
    let context = server_id.as_bytes();
    let access = crypto::seal(key, context, &tokens.access_token)?;
    let refresh = tokens.refresh_token.as_deref().map(|t| crypto::seal(key, context, t)).transpose()?;
    let expires = Utc::now() + chrono::Duration::seconds(tokens.expires_in.unwrap_or(15_552_000).clamp(3_600, 31_536_000));
    sqlx::query(
        "INSERT INTO server_payout_accounts (server_id, mp_user_id, access_token_enc, refresh_token_enc, token_expires_at, live_mode, connected_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (server_id) DO UPDATE SET mp_user_id = $2, access_token_enc = $3,
             refresh_token_enc = COALESCE($4, server_payout_accounts.refresh_token_enc), token_expires_at = $5, live_mode = $6,
             connected_by = COALESCE($7, server_payout_accounts.connected_by),
             connected_at = CASE WHEN $7 IS NULL THEN server_payout_accounts.connected_at ELSE now() END,
             refresh_attempt_at = NULL",
    )
    .bind(server_id)
    .bind(mp_user)
    .bind(access)
    .bind(refresh)
    .bind(expires)
    .bind(tokens.live_mode.unwrap_or(true))
    .bind(user)
    .execute(&state.pool)
    .await?;
    Ok(())
}

/// Token do dono para criar links e consultar pagamentos de apoio. Renova se estiver vencendo.
pub async fn seller_token(state: &AppState, server_id: Uuid) -> ApiResult<String> {
    let (client_id, client_secret, key) = oauth_ready(state)?;
    let row: Option<(String, Vec<u8>, Option<Vec<u8>>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT mp_user_id, access_token_enc, refresh_token_enc, token_expires_at FROM server_payout_accounts WHERE server_id = $1",
    )
    .bind(server_id)
    .fetch_optional(&state.pool)
    .await?;
    let (mp_user, access, refresh, expires) = row.ok_or_else(|| ApiError::bad("Este servidor ainda não recebe apoios."))?;
    let context = server_id.as_bytes();
    let access = crypto::open(key, context, &access)?;
    if expires - Utc::now() > chrono::Duration::days(REFRESH_BEFORE_DAYS) {
        return Ok(access);
    }
    let Some(refresh) = refresh else { return Ok(access) };
    // Uma renovação por vez por servidor, e no máximo uma tentativa a cada 10 min (inclusive após falha).
    let claimed = sqlx::query(
        "UPDATE server_payout_accounts SET refresh_attempt_at = now()
          WHERE server_id = $1 AND (refresh_attempt_at IS NULL OR refresh_attempt_at < now() - interval '10 minutes')",
    )
    .bind(server_id)
    .execute(&state.pool)
    .await?
    .rows_affected()
        == 1;
    if !claimed {
        return if expires > Utc::now() { Ok(access) } else { Err(ApiError::bad("A conta Mercado Pago do servidor precisa ser reconectada pelo dono.")) };
    }
    let refresh = crypto::open(key, context, &refresh)?;
    match token_request(
        state,
        json!({ "client_id": client_id, "client_secret": client_secret, "grant_type": "refresh_token", "refresh_token": refresh }),
    )
    .await
    {
        Ok(tokens) => {
            // A conta renovada tem que ser a mesma que já estava conectada.
            if mp_user_id(tokens.user_id.as_ref()).is_some_and(|id| id != mp_user) {
                return Err(ApiError::Internal(eyre::eyre!("renovação devolveu outra conta Mercado Pago")));
            }
            save_tokens(state, server_id, None, &mp_user, &tokens).await?;
            Ok(tokens.access_token)
        }
        Err(error) if expires > Utc::now() => {
            tracing::warn!(%server_id, "não foi possível renovar o token do dono: {error:?}");
            Ok(access)
        }
        Err(error) => Err(error),
    }
}

// ------------------------------------------------------------------ conectar conta (dono)

pub async fn connect(State(state): State<AppState>, AuthUser(user): AuthUser, Path(slug): Path<String>) -> ApiResult<Json<serde_json::Value>> {
    let (client_id, _, _) = oauth_ready(&state)?;
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_owner(&state.pool, server_id, &user).await?;
    sqlx::query("DELETE FROM mp_oauth_states WHERE expires_at < now()").execute(&state.pool).await?;
    let open: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mp_oauth_states WHERE server_id = $1").bind(server_id).fetch_one(&state.pool).await?;
    if open >= 5 {
        return Err(ApiError::bad("Muitas tentativas de conexão. Espere alguns minutos."));
    }
    let oauth_state = random_token(32);
    let verifier = random_token(48);
    sqlx::query(
        "INSERT INTO mp_oauth_states (state_sha256, server_id, user_id, code_verifier, expires_at)
         VALUES ($1, $2, $3, $4, now() + make_interval(mins => $5))",
    )
    .bind(sha256_hex(&oauth_state))
    .bind(server_id)
    .bind(user.id)
    .bind(&verifier)
    .bind(STATE_TTL_MINUTES as i32)
    .execute(&state.pool)
    .await?;
    let mut url = reqwest::Url::parse(&state.config.mp_auth_url).map_err(|e| ApiError::Internal(e.into()))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("platform_id", "mp")
        .append_pair("state", &oauth_state)
        .append_pair("redirect_uri", &state.config.mp_oauth_redirect_uri)
        .append_pair("code_challenge", &code_challenge(&verifier))
        .append_pair("code_challenge_method", "S256");
    Ok(Json(json!({ "url": url.to_string() })))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

fn safe_param(value: &str) -> bool {
    !value.is_empty() && value.len() <= 512 && value.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Volta do Mercado Pago depois que o dono autoriza. Sempre termina no painel.
pub async fn oauth_callback(State(state): State<AppState>, Query(query): Query<CallbackQuery>) -> Response {
    let panel = state.config.panel_url.clone();
    let fail = |reason: &str| Redirect::to(&format!("{panel}/#/?mp=erro&motivo={reason}")).into_response();
    let (Some(code), Some(oauth_state)) = (query.code.filter(|c| safe_param(c)), query.state.filter(|s| safe_param(s))) else {
        return fail("cancelado");
    };
    // Uso único: o DELETE garante que dois callbacks com o mesmo state não passam.
    let row: Option<(Uuid, Uuid, String, DateTime<Utc>)> = match sqlx::query_as(
        "DELETE FROM mp_oauth_states WHERE state_sha256 = $1 RETURNING server_id, user_id, code_verifier, expires_at",
    )
    .bind(sha256_hex(&oauth_state))
    .fetch_optional(&state.pool)
    .await
    {
        Ok(row) => row,
        Err(error) => {
            tracing::error!("falha ao ler o state OAuth: {error:?}");
            return fail("interno");
        }
    };
    let Some((server_id, user_id, verifier, _)) = row.filter(|r| r.3 > Utc::now()) else {
        return fail("expirado");
    };
    match finish_connect(&state, server_id, user_id, &code, &verifier).await {
        Ok(slug) => Redirect::to(&format!("{panel}/#/s/{slug}?tab=pro&mp=ok")).into_response(),
        Err(ApiError::Forbidden) => fail("sem-permissao"),
        Err(error) => {
            tracing::warn!(%server_id, "conexão Mercado Pago falhou: {error:?}");
            fail("falhou")
        }
    }
}

async fn finish_connect(state: &AppState, server_id: Uuid, user_id: Uuid, code: &str, verifier: &str) -> ApiResult<String> {
    let (client_id, client_secret, _) = oauth_ready(state)?;
    // Continua dono? (pode ter sido removido nos minutos entre o clique e a volta)
    let user: User = sqlx::query_as(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1")).bind(user_id).fetch_one(&state.pool).await?;
    require_owner(&state.pool, server_id, &user).await?;
    let tokens = token_request(
        state,
        json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": state.config.mp_oauth_redirect_uri,
            "code_verifier": verifier,
        }),
    )
    .await?;
    let mp_user = mp_user_id(tokens.user_id.as_ref()).ok_or_else(|| ApiError::Internal(eyre::eyre!("OAuth sem user_id")))?;
    // Conta de teste do Mercado Pago aprova "pagamentos" sem dinheiro: em produção não vale.
    if state.config.production && tokens.live_mode == Some(false) {
        return Err(ApiError::Forbidden);
    }
    save_tokens(state, server_id, Some(user_id), &mp_user, &tokens).await?;
    audit(&state.pool, Some(server_id), Some(user_id), "payout.connected", json!({ "mp_user": mask(&mp_user) })).await?;
    let slug: String = sqlx::query_scalar("SELECT slug FROM servers WHERE id = $1").bind(server_id).fetch_one(&state.pool).await?;
    Ok(slug)
}

fn mask(id: &str) -> String {
    let tail: String = id.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("•••{tail}")
}

pub async fn disconnect(State(state): State<AppState>, AuthUser(user): AuthUser, Path(slug): Path<String>) -> ApiResult<StatusCode> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_owner(&state.pool, server_id, &user).await?;
    sqlx::query("DELETE FROM server_payout_accounts WHERE server_id = $1").bind(server_id).execute(&state.pool).await?;
    audit(&state.pool, Some(server_id), Some(user.id), "payout.disconnected", json!({})).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SettingsInput {
    support_enabled: bool,
}

pub async fn settings(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Json(input): Json<SettingsInput>,
) -> ApiResult<StatusCode> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_owner(&state.pool, server_id, &user).await?;
    let done = sqlx::query("UPDATE server_payout_accounts SET support_enabled = $2 WHERE server_id = $1")
        .bind(server_id)
        .bind(input.support_enabled)
        .execute(&state.pool)
        .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::bad("Conecte a conta do Mercado Pago primeiro."));
    }
    audit(&state.pool, Some(server_id), Some(user.id), "payout.settings", json!({ "support_enabled": input.support_enabled })).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ painel: apoios recebidos

#[derive(Serialize, sqlx::FromRow)]
struct ReceivedSupport {
    id: Uuid,
    supporter: Option<String>,
    amount_cents: i32,
    fee_cents: i32,
    message: Option<String>,
    status: String,
    paid_at: Option<DateTime<Utc>>,
}

/// Situação do apoio no servidor (admin+): conta, totais e últimos apoios.
pub async fn admin_view(State(state): State<AppState>, AuthUser(user): AuthUser, Path(slug): Path<String>) -> ApiResult<Json<serde_json::Value>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Admin).await?;
    let is_owner = require_owner(&state.pool, server_id, &user).await.is_ok();
    let account: Option<(String, bool, bool, DateTime<Utc>)> = sqlx::query_as(
        "SELECT mp_user_id, support_enabled, live_mode, connected_at FROM server_payout_accounts WHERE server_id = $1",
    )
    .bind(server_id)
    .fetch_optional(&state.pool)
    .await?;
    let (count, gross, fee): (i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(amount_cents), 0)::BIGINT, COALESCE(SUM(fee_cents), 0)::BIGINT
           FROM orders WHERE server_id = $1 AND kind = 'support' AND status = 'approved' AND paid_at > now() - interval '30 days'",
    )
    .bind(server_id)
    .fetch_one(&state.pool)
    .await?;
    // Anônimo continua anônimo também para a equipe do servidor.
    let recent = sqlx::query_as::<_, ReceivedSupport>(
        "SELECT o.id, CASE WHEN o.anonymous THEN NULL ELSE u.minecraft_name END AS supporter, o.amount_cents, o.fee_cents,
                o.message, o.status, o.paid_at
           FROM orders o LEFT JOIN users u ON u.id = o.user_id
          WHERE o.server_id = $1 AND o.kind = 'support' AND o.status IN ('approved', 'refunded')
          ORDER BY o.paid_at DESC NULLS LAST LIMIT 50",
    )
    .bind(server_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({
        "available": oauth_ready(&state).is_ok(),
        "can_manage": is_owner,
        "fee_percent": f64::from(state.config.support_fee_bps) / 100.0,
        "account": account.map(|(id, enabled, live, at)| json!({
            "mp_user": mask(&id), "support_enabled": enabled, "live_mode": live, "connected_at": at,
        })),
        "last_30_days": { "count": count, "gross_cents": gross, "fee_cents": fee, "net_cents": gross - fee },
        "recent": recent,
    })))
}

// ------------------------------------------------------------------ jogador: apoiar

#[derive(Serialize, sqlx::FromRow)]
struct PublicSupport {
    supporter: Option<String>,
    supporter_uuid: Option<Uuid>,
    message: Option<String>,
    paid_at: Option<DateTime<Utc>>,
}

/// Mural público de apoiadores (sem valores).
pub async fn supporters(State(state): State<AppState>, Path(slug): Path<String>) -> ApiResult<Json<serde_json::Value>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT user_id) FROM orders WHERE server_id = $1 AND kind = 'support' AND status = 'approved' AND paid_at > now() - interval '30 days'",
    )
    .bind(server_id)
    .fetch_one(&state.pool)
    .await?;
    let recent = sqlx::query_as::<_, PublicSupport>(
        "SELECT CASE WHEN o.anonymous THEN NULL ELSE u.minecraft_name END AS supporter,
                CASE WHEN o.anonymous THEN NULL ELSE u.minecraft_uuid END AS supporter_uuid,
                o.message, o.paid_at
           FROM orders o LEFT JOIN users u ON u.id = o.user_id
          WHERE o.server_id = $1 AND o.kind = 'support' AND o.status = 'approved'
          ORDER BY o.paid_at DESC LIMIT 20",
    )
    .bind(server_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({ "supporters_30d": count, "recent": recent })))
}

#[derive(Deserialize)]
pub struct SupportInput {
    amount_cents: i32,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    anonymous: bool,
}

fn clean_message(message: Option<&str>) -> ApiResult<Option<String>> {
    let Some(text) = message.map(|m| m.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|m| !m.is_empty()) else {
        return Ok(None);
    };
    if text.chars().count() > 140 {
        return Err(ApiError::bad("A mensagem pode ter até 140 caracteres."));
    }
    if text.chars().any(char::is_control) || moderation::has_link(&text) {
        return Err(ApiError::bad("A mensagem não pode ter links."));
    }
    if moderation::blocked_term(&text).is_some() {
        return Err(ApiError::bad("A mensagem contém termos não permitidos no Conexão."));
    }
    Ok(Some(text))
}

pub async fn checkout(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Json(input): Json<SupportInput>,
) -> ApiResult<Json<serde_json::Value>> {
    oauth_ready(&state)?;
    let c = &state.config;
    if !(c.support_min_cents..=c.support_max_cents).contains(&input.amount_cents) {
        return Err(ApiError::bad(format!(
            "O apoio vai de R$ {:.2} a R$ {:.2}.",
            f64::from(c.support_min_cents) / 100.0,
            f64::from(c.support_max_cents) / 100.0
        )));
    }
    let message = clean_message(input.message.as_deref())?;
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    let (name, approval): (String, String) =
        sqlx::query_as("SELECT name, approval FROM servers WHERE id = $1").bind(server_id).fetch_one(&state.pool).await?;
    let account: Option<(String, bool)> =
        sqlx::query_as("SELECT mp_user_id, support_enabled AND (live_mode OR NOT $2) FROM server_payout_accounts WHERE server_id = $1")
            .bind(server_id)
            .bind(c.production)
            .fetch_optional(&state.pool)
            .await?;
    let Some((seller, true)) = account.filter(|_| approval == "approved") else {
        return Err(ApiError::bad("Este servidor não está recebendo apoios agora."));
    };
    billing::ensure_pending_room(&state, user.id).await?;
    let token = seller_token(&state, server_id).await?;

    let id = Uuid::new_v4();
    let fee = billing::support_fee(input.amount_cents, c.support_fee_bps);
    sqlx::query(
        "INSERT INTO orders (id, user_id, kind, sku, server_id, pay_with, amount_cents, fee_cents, seller_mp_user_id, message, anonymous)
         VALUES ($1, $2, 'support', 'apoio', $3, 'money', $4, $5, $6, $7, $8)",
    )
    .bind(id)
    .bind(user.id)
    .bind(server_id)
    .bind(input.amount_cents)
    .bind(fee)
    .bind(&seller)
    .bind(&message)
    .bind(input.anonymous)
    .execute(&state.pool)
    .await?;

    let back = format!("{}/pagamento?pedido={id}", c.public_url);
    let (from, to) = billing::expiration_window();
    let mut preference = json!({
        "items": [{
            "id": "apoio",
            "title": format!("Apoio ao servidor {name}"),
            "quantity": 1,
            "unit_price": f64::from(input.amount_cents) / 100.0,
            "currency_id": "BRL",
            "category_id": "services",
        }],
        // Taxa do Conexão (valor em reais, descontado do que o dono recebe).
        "marketplace_fee": f64::from(fee) / 100.0,
        "external_reference": id.to_string(),
        "notification_url": format!("{}/v1/payments/mercadopago/webhook/apoio/{id}", c.public_url),
        "back_urls": { "success": back, "pending": back, "failure": back },
        "statement_descriptor": "Conexão Apoio",
        "metadata": { "order_id": id.to_string() },
        "expires": true,
        "expiration_date_from": from,
        "expiration_date_to": to,
    });
    if back.starts_with("https://") {
        preference["auto_return"] = json!("approved");
    }
    match billing::create_preference(&state, &token, id, &preference).await {
        Ok((preference_id, url)) => {
            sqlx::query("UPDATE orders SET mp_preference_id = $2 WHERE id = $1").bind(id).bind(preference_id).execute(&state.pool).await?;
            audit(&state.pool, Some(server_id), Some(user.id), "support.checkout", json!({ "order": id, "cents": input.amount_cents })).await?;
            Ok(Json(json!({ "order_id": id, "checkout_url": url, "fee_cents": fee })))
        }
        Err(error) => {
            sqlx::query("UPDATE orders SET status = 'cancelled' WHERE id = $1").bind(id).execute(&state.pool).await?;
            Err(error)
        }
    }
}

#[derive(Deserialize, Default)]
pub struct WebhookQuery {
    #[serde(rename = "data.id")]
    pub data_id: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

/// Aviso de pagamento de um apoio. O id do pedido vem na URL (criada por nós);
/// o pagamento é sempre buscado com o token do dono e conferido (referência, conta, valor).
pub async fn webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(order_id): Path<Uuid>,
    Query(query): Query<WebhookQuery>,
    body: Bytes,
) -> StatusCode {
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    let kind = query.kind.clone().or_else(|| json["type"].as_str().map(str::to_string)).unwrap_or_default();
    let data_id = query
        .data_id
        .or_else(|| json["data"]["id"].as_str().map(str::to_string))
        .or_else(|| json["data"]["id"].as_i64().map(|v| v.to_string()));
    let Some(data_id) = data_id.filter(|id| billing::valid_payment_id(id)) else {
        return StatusCode::BAD_REQUEST;
    };
    if let Some(secret) = &state.config.mp_webhook_secret {
        let header = headers.get("x-signature").and_then(|v| v.to_str().ok()).unwrap_or_default();
        let request_id = headers.get("x-request-id").and_then(|v| v.to_str().ok()).unwrap_or_default();
        if !crate::routes::featured::signature_ok(secret, header, request_id, &data_id.to_lowercase()) {
            // Sem assinatura não consultamos nada (evita usar a API como amplificador contra o
            // Mercado Pago). Se um aviso legítimo cair aqui, o launcher concilia sozinho.
            tracing::warn!(%order_id, "aviso de apoio com assinatura inválida (data.id {data_id})");
            return StatusCode::UNAUTHORIZED;
        }
    }
    if kind == "payment" {
        tokio::spawn(async move {
            if let Err(error) = billing::process_support_payment(&state, order_id, &data_id).await {
                tracing::error!(%order_id, "falha ao processar o apoio {data_id}: {error:?}");
            }
        });
    }
    StatusCode::OK
}

// ------------------------------------------------------------------ PRO: estatísticas

#[derive(Serialize, sqlx::FromRow)]
struct DailyRow {
    day: chrono::NaiveDate,
    whitelist: i64,
    stars: i64,
    comments: i64,
    supports: i64,
    support_net_cents: i64,
}

/// Últimos 30 dias, dia a dia (só PRO).
pub async fn stats(State(state): State<AppState>, AuthUser(user): AuthUser, Path(slug): Path<String>) -> ApiResult<Json<serde_json::Value>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Admin).await?;
    let pro: bool = sqlx::query_scalar("SELECT COALESCE(pro_until > now(), FALSE) FROM servers WHERE id = $1")
        .bind(server_id)
        .fetch_one(&state.pool)
        .await?;
    if !pro && !user.is_staff {
        return Err(ApiError::bad("As estatísticas fazem parte do plano PRO."));
    }
    let days = sqlx::query_as::<_, DailyRow>(
        "WITH d AS (SELECT generate_series((now() - interval '29 days')::date, now()::date, interval '1 day')::date AS day)
         SELECT d.day,
           (SELECT COUNT(*) FROM whitelist_entries w WHERE w.server_id = $1 AND w.created_at::date = d.day) AS whitelist,
           (SELECT COUNT(*) FROM server_stars s WHERE s.server_id = $1 AND s.created_at::date = d.day) AS stars,
           (SELECT COUNT(*) FROM server_comments c WHERE c.server_id = $1 AND c.created_at::date = d.day) AS comments,
           (SELECT COUNT(*) FROM orders o WHERE o.server_id = $1 AND o.kind = 'support' AND o.status = 'approved' AND o.paid_at::date = d.day) AS supports,
           (SELECT COALESCE(SUM(o.amount_cents - o.fee_cents), 0)::BIGINT FROM orders o
             WHERE o.server_id = $1 AND o.kind = 'support' AND o.status = 'approved' AND o.paid_at::date = d.day) AS support_net_cents
         FROM d ORDER BY d.day",
    )
    .bind(server_id)
    .fetch_all(&state.pool)
    .await?;
    let (members, approved): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE status = 'approved') FROM whitelist_entries WHERE server_id = $1",
    )
    .bind(server_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(json!({ "days": days, "whitelist_total": members, "whitelist_approved": approved })))
}

/// Apoio feito por este usuário nos últimos 30 dias (selo "Apoiador" no perfil).
pub async fn is_recent_supporter(state: &AppState, user_id: Uuid) -> ApiResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM orders WHERE user_id = $1 AND kind = 'support' AND status = 'approved' AND paid_at > now() - interval '30 days')",
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_rfc7636() {
        // Exemplo do RFC 7636, apêndice B.
        assert_eq!(code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn random_tokens_are_long_and_unique() {
        let a = random_token(32);
        assert!(a.len() >= 43);
        assert_ne!(a, random_token(32));
        assert!(safe_param(&a));
    }

    #[test]
    fn rejects_bad_params_and_ids() {
        assert!(!safe_param(""));
        assert!(!safe_param("abc def"));
        assert!(!safe_param("a&b=c"));
        assert!(!safe_param(&"a".repeat(600)));
        assert_eq!(mp_user_id(Some(&json!(123456))), Some("123456".into()));
        assert_eq!(mp_user_id(Some(&json!("12a"))), None);
        assert_eq!(mp_user_id(None), None);
        assert_eq!(mask("123456789"), "•••6789");
    }

    #[test]
    fn support_messages() {
        assert_eq!(clean_message(Some("  valeu   pelo servidor ")).unwrap(), Some("valeu pelo servidor".into()));
        assert_eq!(clean_message(Some("   ")).unwrap(), None);
        assert!(clean_message(Some("entra em https://golpe.com")).is_err());
        assert!(clean_message(Some(&"a".repeat(141))).is_err());
    }
}
