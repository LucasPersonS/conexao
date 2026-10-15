use axum::Json;
use axum::extract::{Path, Query, State};
use conexao_types::{AccessResponse, WhitelistStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::agents::push_whitelist;
use crate::auth::{AuthUser, Role, audit, lookup_minecraft_name, require_role, upsert_user};
use crate::error::{ApiError, ApiResult};
use crate::routes::servers::server_id_by_slug;
use crate::state::AppState;

/// O jogador pode entrar? Usado pelo launcher para liberar o botão Jogar.
pub async fn access(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<AccessResponse>> {
    let (server_id, whitelist_required): (Uuid, bool) =
        sqlx::query_as("SELECT id, whitelist_required FROM servers WHERE slug = $1")
            .bind(&slug)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(ApiError::NotFound)?;
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM whitelist_entries WHERE server_id = $1 AND user_id = $2")
            .bind(server_id)
            .bind(user.id)
            .fetch_optional(&state.pool)
            .await?;
    let status = status.as_deref().and_then(WhitelistStatus::parse);
    let can_play = match status {
        Some(WhitelistStatus::Banned) => false,
        Some(WhitelistStatus::Approved) => true,
        _ => !whitelist_required,
    };
    Ok(Json(AccessResponse { server: slug, whitelist_required, status, can_play }))
}

#[derive(Deserialize, Default)]
pub struct ApplyInput {
    #[serde(default)]
    pub note: Option<String>,
}

/// Pedido de entrada na whitelist (fica pendente até a equipe decidir).
pub async fn apply(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    body: Option<Json<ApplyInput>>,
) -> ApiResult<Json<AccessResponse>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    let note = body.and_then(|b| b.0.note).map(|n| n.chars().take(1000).collect::<String>());
    sqlx::query(
        "INSERT INTO whitelist_entries (server_id, user_id, status, note) VALUES ($1, $2, 'pending', $3)
         ON CONFLICT (server_id, user_id) DO NOTHING",
    )
    .bind(server_id)
    .bind(user.id)
    .bind(note)
    .execute(&state.pool)
    .await?;
    access(State(state), AuthUser(user), Path(slug)).await
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Entry {
    #[serde(skip)]
    pub user_id: Uuid,
    pub minecraft_uuid: Uuid,
    pub minecraft_name: String,
    pub status: String,
    pub note: Option<String>,
    pub decided_by_name: Option<String>,
    pub updated_at: DateTime<Utc>,
    /// Perfil do jogador (pronomes, bio, Discord conforme a visibilidade escolhida por ele).
    #[sqlx(skip)]
    pub profile: Option<crate::routes::profiles::PublicProfile>,
}

pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<Vec<Entry>>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Moderator).await?;
    let rows = sqlx::query_as::<_, Entry>(
        "SELECT u.id AS user_id, u.minecraft_uuid, u.minecraft_name, w.status, w.note, d.minecraft_name AS decided_by_name, w.updated_at
           FROM whitelist_entries w
           JOIN users u ON u.id = w.user_id
           LEFT JOIN users d ON d.id = w.decided_by
          WHERE w.server_id = $1 AND ($2::text IS NULL OR w.status = $2)
          ORDER BY w.updated_at DESC LIMIT 500",
    )
    .bind(server_id)
    .bind(query.status)
    .fetch_all(&state.pool)
    .await?;
    let mut rows = rows;
    for row in &mut rows {
        row.profile = crate::routes::profiles::public_by_id(&state, row.user_id, &user).await.ok();
    }
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct SetInput {
    pub minecraft_name: String,
    pub status: WhitelistStatus,
    pub note: Option<String>,
}

/// Aprova, recusa ou bane um jogador pelo nick. Notifica o agente na hora.
pub async fn set_by_name(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Json(input): Json<SetInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Moderator).await?;
    let (uuid, name) = lookup_minecraft_name(&state, input.minecraft_name.trim()).await?;
    let target = upsert_user(&state.pool, uuid, &name, false).await?;
    sqlx::query(
        "INSERT INTO whitelist_entries (server_id, user_id, status, note, decided_by)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (server_id, user_id) DO UPDATE
            SET status = EXCLUDED.status, note = COALESCE(EXCLUDED.note, whitelist_entries.note),
                decided_by = EXCLUDED.decided_by, updated_at = now()",
    )
    .bind(server_id)
    .bind(target.id)
    .bind(input.status.as_str())
    .bind(&input.note)
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    audit(
        &state.pool,
        Some(server_id),
        Some(user.id),
        "whitelist.set",
        json!({ "player": name, "uuid": uuid, "status": input.status }),
    )
    .await?;
    push_whitelist(&state, server_id).await?;
    if input.status == WhitelistStatus::Banned {
        state.agents.send(
            server_id,
            conexao_types::agent::ToAgent::Command { id: Uuid::new_v4(), command: format!("kick {name}") },
        );
    }
    Ok(Json(json!({ "minecraft_uuid": uuid, "minecraft_name": name, "status": input.status })))
}
