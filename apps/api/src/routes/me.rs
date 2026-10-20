use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{AuthUser, USER_COLUMNS, User, issue_publish_token, issue_token, upsert_user, verify_minecraft_token};
use crate::error::ApiResult;
use crate::routes::waitlist;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct MinecraftLogin {
    /// Access token da Minecraft Services obtido pelo launcher.
    pub access_token: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: User,
}

pub async fn login_minecraft(
    State(state): State<AppState>,
    Json(body): Json<MinecraftLogin>,
) -> ApiResult<Json<LoginResponse>> {
    let (uuid, name) = verify_minecraft_token(&state, &body.access_token).await?;
    let existing = sqlx::query_as::<_, User>(&format!("SELECT {USER_COLUMNS} FROM users WHERE minecraft_uuid = $1"))
        .bind(uuid)
        .fetch_optional(&state.pool)
        .await?;
    waitlist::require_launcher_access(&state, existing.as_ref(), Some(uuid), None).await?;
    let user = upsert_user(&state.pool, uuid, &name, true).await?;
    let user = waitlist::grant(&state, user).await?;
    let mut user = waitlist::adopt_origin(&state, user).await?;
    if !user.is_staff && state.config.staff_uuids.contains(&uuid) {
        sqlx::query("UPDATE users SET is_staff = TRUE WHERE id = $1").bind(user.id).execute(&state.pool).await?;
        user.is_staff = true;
    }
    let token = issue_token(&state, &user)?;
    Ok(Json(LoginResponse { token, user }))
}

#[derive(Serialize)]
pub struct PublishToken {
    pub token: String,
    pub expires_in: u64,
}

/// Token curto (2 h) que só publica modpacks, para o `conexao-publish`. Exige a sessão completa.
pub async fn publish_token(State(state): State<AppState>, AuthUser(user): AuthUser) -> ApiResult<Json<PublishToken>> {
    Ok(Json(PublishToken { token: issue_publish_token(&state, &user)?, expires_in: 2 * 3600 }))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Membership {
    pub server_id: Uuid,
    pub slug: String,
    pub name: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct MeResponse {
    pub user: User,
    pub memberships: Vec<Membership>,
}

pub async fn me(State(state): State<AppState>, AuthUser(user): AuthUser) -> ApiResult<Json<MeResponse>> {
    let memberships = sqlx::query_as::<_, Membership>(
        "SELECT s.id AS server_id, s.slug, s.name, m.role
           FROM server_members m JOIN servers s ON s.id = m.server_id
          WHERE m.user_id = $1 ORDER BY s.name",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(MeResponse { user, memberships }))
}
