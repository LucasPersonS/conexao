//! Conexões WebSocket com os agentes dos servidores.

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Json;
use conexao_types::WhitelistPlayer;
use conexao_types::agent::{FromAgent, JoinTicket, ToAgent};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use rand::RngCore;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::auth::{AuthUser, Role, audit, require_role};
use crate::error::{ApiError, ApiResult};
use crate::routes::servers::server_id_by_slug;
use crate::state::AppState;

#[derive(Default)]
pub struct AgentHub {
    connections: DashMap<Uuid, (Uuid, mpsc::UnboundedSender<ToAgent>)>,
}

impl AgentHub {
    pub fn send(&self, server_id: Uuid, message: ToAgent) -> bool {
        self.connections
            .get(&server_id)
            .is_some_and(|entry| entry.1.send(message).is_ok())
    }

    pub fn is_connected(&self, server_id: Uuid) -> bool {
        self.connections.contains_key(&server_id)
    }
}

pub fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

pub async fn approved_players(pool: &PgPool, server_id: Uuid) -> ApiResult<Vec<WhitelistPlayer>> {
    let allow_offline: bool = sqlx::query_scalar("SELECT allow_offline FROM servers WHERE id = $1")
        .bind(server_id)
        .fetch_one(pool)
        .await?;
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT u.minecraft_uuid, u.minecraft_name
           FROM whitelist_entries w JOIN users u ON u.id = w.user_id
          WHERE w.server_id = $1 AND w.status = 'approved'
          ORDER BY u.minecraft_name",
    )
    .bind(server_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(uuid, name)| whitelist_player(allow_offline, uuid, name)).collect())
}

/// Em `online-mode=false` o servidor identifica todo mundo (original ou não) pelo UUID offline do nick.
pub fn whitelist_player(allow_offline: bool, uuid: Uuid, name: String) -> WhitelistPlayer {
    if allow_offline {
        WhitelistPlayer { uuid: conexao_types::offline::offline_uuid(&name), name }
    } else {
        WhitelistPlayer { uuid, name }
    }
}

/// Estado do passe de entrada para o Companion: liga/desliga e passes ainda válidos.
pub async fn join_tickets_message(pool: &PgPool, server_id: Uuid) -> ApiResult<ToAgent> {
    let enforce: bool = sqlx::query_scalar("SELECT allow_offline FROM servers WHERE id = $1")
        .bind(server_id)
        .fetch_one(pool)
        .await?;
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT player_name, ip, EXTRACT(EPOCH FROM (expires_at - now()))::BIGINT
           FROM join_tickets WHERE server_id = $1 AND expires_at > now()",
    )
    .bind(server_id)
    .fetch_all(pool)
    .await?;
    let tickets = rows
        .into_iter()
        .map(|(name, ip, secs)| JoinTicket { name, ip, expires_in: u64::try_from(secs).unwrap_or(0) })
        .collect();
    Ok(ToAgent::JoinTickets { enforce, tickets })
}

/// Avisa o Companion que a opção de contas não originais mudou.
pub async fn push_join_tickets(state: &AppState, server_id: Uuid) -> ApiResult<()> {
    if state.agents.is_connected(server_id) {
        let message = join_tickets_message(&state.pool, server_id).await?;
        state.agents.send(server_id, message);
    }
    Ok(())
}

pub async fn whitelist_message(pool: &PgPool, server_id: Uuid) -> ApiResult<ToAgent> {
    let required: bool = sqlx::query_scalar("SELECT whitelist_required FROM servers WHERE id = $1")
        .bind(server_id)
        .fetch_one(pool)
        .await?;
    Ok(ToAgent::WhitelistSync { players: approved_players(pool, server_id).await?, required: Some(required) })
}

/// Envia a whitelist atualizada ao agente, se ele estiver conectado.
pub async fn push_whitelist(state: &AppState, server_id: Uuid) -> ApiResult<()> {
    if state.agents.is_connected(server_id) {
        let message = whitelist_message(&state.pool, server_id).await?;
        state.agents.send(server_id, message);
    }
    Ok(())
}

/// Gera um token novo para o agente/Companion do servidor (substitui o anterior).
pub async fn issue_agent_token(pool: &PgPool, server_id: Uuid) -> ApiResult<String> {
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    let token = format!("cagt_{}", hex::encode(raw));
    sqlx::query(
        "INSERT INTO agents (server_id, token_sha256) VALUES ($1, $2)
         ON CONFLICT (server_id) DO UPDATE SET token_sha256 = EXCLUDED.token_sha256, created_at = now()",
    )
    .bind(server_id)
    .bind(sha256_hex(&token))
    .execute(pool)
    .await?;
    Ok(token)
}

/// Gera (ou rotaciona) o token do agente. O token só é mostrado uma vez.
pub async fn create_token(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Owner).await?;
    let token = issue_agent_token(&state.pool, server_id).await?;
    audit(&state.pool, Some(server_id), Some(user.id), "agent.token_rotated", json!({})).await?;
    Ok(Json(json!({ "token": token })))
}

pub async fn connect(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> ApiResult<Response> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;
    let server_id: Uuid = sqlx::query_scalar("SELECT server_id FROM agents WHERE token_sha256 = $1")
        .bind(sha256_hex(token.trim()))
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    Ok(ws.on_upgrade(move |socket| async move {
        if let Err(error) = session(state, server_id, socket).await {
            tracing::warn!(%server_id, "agente desconectado com erro: {error:?}");
        }
    }))
}

async fn session(state: AppState, server_id: Uuid, socket: WebSocket) -> ApiResult<()> {
    let connection_id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::unbounded_channel();
    state.agents.connections.insert(server_id, (connection_id, tx));
    tracing::info!(%server_id, "agente conectado");

    let (mut sink, mut stream) = socket.split();
    let initial = whitelist_message(&state.pool, server_id).await?;
    let tickets = join_tickets_message(&state.pool, server_id).await?;
    let mut ping = tokio::time::interval(Duration::from_secs(30));

    let send = |message: &ToAgent| Message::Text(serde_json::to_string(message).unwrap_or_default().into());
    let result: ApiResult<()> = async {
        sink.send(send(&initial)).await.map_err(|e| ApiError::Internal(e.into()))?;
        sink.send(send(&tickets)).await.map_err(|e| ApiError::Internal(e.into()))?;
        loop {
            tokio::select! {
                outgoing = rx.recv() => {
                    let Some(message) = outgoing else { break };
                    sink.send(send(&message)).await.map_err(|e| ApiError::Internal(e.into()))?;
                }
                _ = ping.tick() => {
                    sink.send(send(&ToAgent::Ping)).await.map_err(|e| ApiError::Internal(e.into()))?;
                }
                incoming = stream.next() => {
                    let Some(Ok(message)) = incoming else { break };
                    let Message::Text(text) = message else { continue };
                    match serde_json::from_str::<FromAgent>(&text) {
                        Ok(FromAgent::Hello { agent_version }) => {
                            sqlx::query("UPDATE agents SET agent_version = $2, last_seen_at = now() WHERE server_id = $1")
                                .bind(server_id).bind(agent_version).execute(&state.pool).await?;
                        }
                        Ok(FromAgent::Pong) => {
                            sqlx::query("UPDATE agents SET last_seen_at = now() WHERE server_id = $1")
                                .bind(server_id).execute(&state.pool).await?;
                        }
                        Ok(FromAgent::WhitelistApplied { count }) => {
                            tracing::info!(%server_id, count, "whitelist aplicada pelo agente");
                        }
                        Ok(FromAgent::CommandResult { id, ok, output }) => {
                            audit(&state.pool, Some(server_id), None, "agent.command_result",
                                json!({ "id": id, "ok": ok, "output": output })).await?;
                        }
                        Ok(FromAgent::Error { message }) => {
                            tracing::warn!(%server_id, "erro reportado pelo agente: {message}");
                        }
                        Ok(FromAgent::ServerInfo(info)) => {
                            let info = serde_json::to_value(info).map_err(|e| ApiError::Internal(e.into()))?;
                            sqlx::query(
                                "INSERT INTO server_snapshots (server_id, info, reported_at) VALUES ($1, $2, now())
                                 ON CONFLICT (server_id) DO UPDATE SET info = EXCLUDED.info, reported_at = now()",
                            )
                            .bind(server_id).bind(info).execute(&state.pool).await?;
                        }
                        Ok(FromAgent::ModsSnapshot { files }) => {
                            let files: Vec<_> = files
                                .into_iter()
                                .filter(|f| crate::storage::is_sha1(&f.sha1) && crate::routes::plan::safe_relative(&f.path))
                                .take(5_000)
                                .collect();
                            tracing::info!(%server_id, mods = files.len(), "fotografia de mods recebida");
                            let mods = serde_json::to_value(files).map_err(|e| ApiError::Internal(e.into()))?;
                            sqlx::query(
                                "INSERT INTO server_snapshots (server_id, mods, reported_at) VALUES ($1, $2, now())
                                 ON CONFLICT (server_id) DO UPDATE SET mods = EXCLUDED.mods, reported_at = now()",
                            )
                            .bind(server_id).bind(mods).execute(&state.pool).await?;
                        }
                        Ok(FromAgent::FilesUploaded { sha1, failed }) => {
                            tracing::info!(%server_id, ok = sha1.len(), failed = failed.len(), "uploads do Companion");
                        }
                        Err(error) => tracing::warn!(%server_id, "mensagem inválida do agente: {error}"),
                    }
                }
            }
        }
        Ok(())
    }
    .await;

    state.agents.connections.remove_if(&server_id, |_, (id, _)| *id == connection_id);
    tracing::info!(%server_id, "agente saiu");
    result
}
