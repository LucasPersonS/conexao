//! Login do painel web pelo launcher (código de pareamento).
//!
//! O painel não consegue fazer o login Microsoft do Minecraft sozinho, então:
//! 1. o painel pede um código (`POST /v1/auth/pair`) e mostra ao admin;
//! 2. o admin digita o código no launcher, que já está logado (`POST /v1/auth/pair/approve`);
//! 3. o painel, consultando `POST /v1/auth/pair/poll`, recebe o JWT do Conexão.

use std::time::{Duration, Instant};

use std::net::SocketAddr;

use axum::Json;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use dashmap::DashMap;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::{AuthUser, User, issue_token};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const TTL: Duration = Duration::from_secs(300);
/// Sem 0/O, 1/I/L: fácil de ler e digitar.
const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

struct Pending {
    poll_hash: String,
    created: Instant,
    approved_by: Option<Uuid>,
    /// De onde veio o pedido: o launcher mostra antes de aprovar (contra golpe do código).
    origin: Origin,
}

#[derive(Clone, Serialize)]
pub struct Origin {
    pub ip: String,
    /// "Chrome no Windows", por exemplo.
    pub browser: String,
}

/// Resumo legível do User-Agent do navegador que pediu o código.
pub fn describe_browser(user_agent: &str) -> String {
    let ua = user_agent;
    let browser = if ua.contains("Edg/") {
        "Edge"
    } else if ua.contains("OPR/") || ua.contains("Opera") {
        "Opera"
    } else if ua.contains("Firefox/") {
        "Firefox"
    } else if ua.contains("Chrome/") {
        "Chrome"
    } else if ua.contains("Safari/") {
        "Safari"
    } else {
        "Navegador desconhecido"
    };
    let system = if ua.contains("Windows") {
        Some("Windows")
    } else if ua.contains("Android") {
        Some("Android")
    } else if ua.contains("iPhone") || ua.contains("iPad") {
        Some("iOS")
    } else if ua.contains("Mac OS X") {
        Some("macOS")
    } else if ua.contains("Linux") {
        Some("Linux")
    } else {
        None
    };
    match system {
        Some(system) => format!("{browser} no {system}"),
        None => browser.to_string(),
    }
}

#[derive(Default)]
pub struct Pairings {
    by_code: DashMap<String, Pending>,
}

fn sha(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

pub fn normalize(code: &str) -> String {
    code.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_uppercase()).collect()
}

fn new_code() -> String {
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| char::from(*ALPHABET.get(rng.gen_range(0..ALPHABET.len())).unwrap_or(&b'A')))
        .collect()
}

#[derive(Serialize)]
pub struct PairStart {
    /// Código exibido ao admin, no formato `ABCD-EFGH`.
    pub code: String,
    /// Segredo que só o painel conhece, usado para buscar o token.
    pub poll_token: String,
    pub expires_in: u64,
}

pub async fn start(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> ApiResult<Json<PairStart>> {
    state.pairings.by_code.retain(|_, p| p.created.elapsed() < TTL);
    if state.pairings.by_code.len() > 10_000 {
        return Err(ApiError::bad("Muitos pareamentos em aberto; tente de novo em instantes."));
    }
    let code = new_code();
    let poll_token = format!("{:032x}", rand::thread_rng().r#gen::<u128>());
    state.pairings.by_code.insert(
        code.clone(),
        Pending {
            poll_hash: sha(&poll_token),
            created: Instant::now(),
            approved_by: None,
            origin: Origin {
                ip: crate::ratelimit::client_ip(peer.ip(), &headers).to_string(),
                browser: describe_browser(
                    headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or_default(),
                ),
            },
        },
    );
    Ok(Json(PairStart {
        code: format!("{}-{}", &code[..4], &code[4..]),
        poll_token,
        expires_in: TTL.as_secs(),
    }))
}

#[derive(Serialize)]
pub struct PairInfo {
    pub ip: String,
    pub browser: String,
    /// Há quantos segundos o código foi gerado.
    pub age_secs: u64,
}

/// O launcher consulta quem pediu o código ANTES de aprovar e mostra ao jogador.
pub async fn info(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(code): Path<String>,
) -> ApiResult<Json<PairInfo>> {
    let code = normalize(&code);
    let entry = state
        .pairings
        .by_code
        .get(&code)
        .filter(|p| p.created.elapsed() < TTL && p.approved_by.is_none())
        .ok_or_else(|| ApiError::bad("Código inválido ou expirado. Gere um novo no painel."))?;
    Ok(Json(PairInfo {
        ip: entry.origin.ip.clone(),
        browser: entry.origin.browser.clone(),
        age_secs: entry.created.elapsed().as_secs(),
    }))
}

#[derive(Deserialize)]
pub struct ApproveInput {
    pub code: String,
}

/// Chamado pelo launcher (usuário já autenticado) para liberar o painel.
pub async fn approve(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(input): Json<ApproveInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let code = normalize(&input.code);
    let mut entry = state
        .pairings
        .by_code
        .get_mut(&code)
        .filter(|p| p.created.elapsed() < TTL)
        .ok_or_else(|| ApiError::bad("Código inválido ou expirado. Gere um novo no painel."))?;
    if entry.approved_by.is_some() {
        return Err(ApiError::bad("Este código já foi usado."));
    }
    entry.approved_by = Some(user.id);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct PollInput {
    pub poll_token: String,
}

#[derive(Serialize)]
pub struct PollResponse {
    pub status: &'static str,
    pub token: Option<String>,
    pub user: Option<User>,
}

pub async fn poll(
    State(state): State<AppState>,
    Json(input): Json<PollInput>,
) -> ApiResult<(StatusCode, Json<PollResponse>)> {
    let hash = sha(&input.poll_token);
    let found = state
        .pairings
        .by_code
        .iter()
        .find(|e| e.poll_hash == hash)
        .map(|e| (e.key().clone(), e.created.elapsed() >= TTL, e.approved_by));
    let Some((code, expired, approved_by)) = found else {
        return Ok((StatusCode::GONE, Json(PollResponse { status: "expired", token: None, user: None })));
    };
    if expired {
        state.pairings.by_code.remove(&code);
        return Ok((StatusCode::GONE, Json(PollResponse { status: "expired", token: None, user: None })));
    }
    let Some(user_id) = approved_by else {
        return Ok((StatusCode::ACCEPTED, Json(PollResponse { status: "pending", token: None, user: None })));
    };
    state.pairings.by_code.remove(&code);
    let user = sqlx::query_as::<_, User>(&format!("SELECT {} FROM users WHERE id = $1", crate::auth::USER_COLUMNS))
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;
    let token = issue_token(&state, &user)?;
    Ok((StatusCode::OK, Json(PollResponse { status: "approved", token: Some(token), user: Some(user) })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_readable() {
        let code = new_code();
        assert_eq!(code.len(), 8);
        assert!(code.bytes().all(|b| ALPHABET.contains(&b)));
        assert_eq!(normalize("abcd-efgh "), "ABCDEFGH");
    }

    #[test]
    fn describes_browsers() {
        let chrome = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";
        let edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130.0 Safari/537.36 Edg/130.0";
        let iphone = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 Version/17.0 Mobile Safari/604.1";
        assert_eq!(describe_browser(chrome), "Chrome no Windows");
        assert_eq!(describe_browser(edge), "Edge no Windows");
        assert_eq!(describe_browser(iphone), "Safari no iOS");
        assert_eq!(describe_browser(""), "Navegador desconhecido");
    }
}
