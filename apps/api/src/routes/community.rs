//! Estrelas e comentários dos jogadores nos servidores.
//!
//! Estrela: qualquer jogador logado marca (uma por servidor) e o total aparece nos
//! cards do launcher. Comentário: só quem tem whitelist aprovada no servidor ou conta
//! cadastrada pelo Discord (além da equipe do servidor). A equipe (moderador ou
//! acima) apaga comentários pelo painel; o autor pode apagar o próprio.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::{AuthUser, MaybeUser, Role, User, audit, require_role};
use crate::error::{ApiError, ApiResult};
use crate::moderation;
use crate::state::AppState;

const MAX_COMMENT_CHARS: usize = 500;
const MAX_COMMENT_LINES: usize = 8;
/// Por jogador, em cada servidor, a cada 24 horas.
const MAX_COMMENTS_PER_DAY: i64 = 3;
/// Intervalo mínimo entre dois comentários do mesmo jogador (qualquer servidor).
const COMMENT_COOLDOWN_SECS: i64 = 30;
const PAGE_DEFAULT: i64 = 30;
const PAGE_MAX: i64 = 100;

/// Servidor visível no launcher (aprovado). Estrelas e comentários só valem para esses.
async fn approved_server(pool: &PgPool, slug: &str) -> ApiResult<Uuid> {
    sqlx::query_scalar("SELECT id FROM servers WHERE slug = $1 AND approval = 'approved'")
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)
}

/// Servidor por slug para a equipe: aparece no painel mesmo em análise.
async fn team_server(pool: &PgPool, slug: &str, user: Option<&User>) -> ApiResult<Uuid> {
    let (id, approval): (Uuid, String) = sqlx::query_as("SELECT id, approval FROM servers WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    if approval != "approved" {
        let user = user.ok_or(ApiError::NotFound)?;
        require_role(pool, id, user, Role::Moderator).await.map_err(|_| ApiError::NotFound)?;
    }
    Ok(id)
}

// ---------------------------------------------------------------- estrelas

#[derive(Serialize)]
pub struct StarState {
    pub stars: i64,
    pub starred: bool,
}

async fn star_state(pool: &PgPool, server_id: Uuid, user_id: Uuid) -> ApiResult<StarState> {
    let (stars, starred): (i64, bool) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(bool_or(user_id = $2), FALSE) FROM server_stars WHERE server_id = $1",
    )
    .bind(server_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(StarState { stars, starred })
}

/// `PUT /servers/{slug}/star`: marca a estrela (repetir não muda nada).
pub async fn star(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<StarState>> {
    let server_id = approved_server(&state.pool, &slug).await?;
    sqlx::query("INSERT INTO server_stars (server_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(server_id)
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(star_state(&state.pool, server_id, user.id).await?))
}

/// `DELETE /servers/{slug}/star`: tira a estrela.
pub async fn unstar(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<StarState>> {
    let server_id = approved_server(&state.pool, &slug).await?;
    sqlx::query("DELETE FROM server_stars WHERE server_id = $1 AND user_id = $2")
        .bind(server_id)
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(star_state(&state.pool, server_id, user.id).await?))
}

/// `GET /me/stars`: slugs dos servidores em que o jogador deu estrela.
pub async fn my_stars(State(state): State<AppState>, AuthUser(user): AuthUser) -> ApiResult<Json<Vec<String>>> {
    let slugs = sqlx::query_scalar(
        "SELECT s.slug FROM server_stars st JOIN servers s ON s.id = st.server_id
          WHERE st.user_id = $1 ORDER BY st.created_at DESC",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(slugs))
}

// ---------------------------------------------------------------- comentários

#[derive(sqlx::FromRow)]
struct CommentRow {
    id: Uuid,
    body: String,
    created_at: DateTime<Utc>,
    author_id: Uuid,
    author_name: String,
    author_uuid: Uuid,
    author_role: Option<String>,
    author_offline: bool,
}

#[derive(Serialize)]
pub struct Comment {
    pub id: Uuid,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub author_name: String,
    pub author_uuid: Uuid,
    /// Papel do autor na equipe do servidor (`owner`, `admin`, `moderator`), se tiver.
    pub author_role: Option<String>,
    /// Conta não original (cadastrada pelo Discord).
    pub author_offline: bool,
    /// O comentário é de quem está vendo.
    pub mine: bool,
    /// Quem está vendo pode apagar (autor ou equipe do servidor).
    pub can_delete: bool,
}

#[derive(Serialize)]
pub struct Viewer {
    pub can_comment: bool,
    /// Por que não pode comentar (mensagem para o jogador).
    pub reason: Option<String>,
    /// Equipe do servidor: pode apagar qualquer comentário.
    pub can_moderate: bool,
}

#[derive(Serialize)]
pub struct CommentPage {
    pub total: i64,
    pub stars: i64,
    pub comments: Vec<Comment>,
    pub has_more: bool,
    pub viewer: Viewer,
}

#[derive(Deserialize)]
pub struct PageQuery {
    /// Comentários anteriores a esta data (paginação do painel).
    pub before: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

const NOT_ALLOWED: &str =
    "Para comentar, você precisa ter whitelist aprovada neste servidor ou uma conta cadastrada pelo Discord.";

/// Quem pode comentar: whitelist aprovada no servidor, conta cadastrada pelo Discord
/// ou equipe do servidor. Banidos do servidor e perfis bloqueados pela equipe Conexão não.
async fn comment_permission(pool: &PgPool, server_id: Uuid, user: &User) -> ApiResult<Result<(), String>> {
    let (whitelist, discord, member, profile_banned): (Option<String>, bool, bool, bool) = sqlx::query_as(
        "SELECT (SELECT status FROM whitelist_entries WHERE server_id = $1 AND user_id = u.id),
                u.discord_id IS NOT NULL,
                EXISTS (SELECT 1 FROM server_members WHERE server_id = $1 AND user_id = u.id),
                u.profile_banned_at IS NOT NULL
           FROM users u WHERE u.id = $2",
    )
    .bind(server_id)
    .bind(user.id)
    .fetch_one(pool)
    .await?;
    if profile_banned {
        return Ok(Err("Sua conta está bloqueada para publicar no Conexão.".into()));
    }
    if whitelist.as_deref() == Some("banned") {
        return Ok(Err("Você foi banido deste servidor e não pode comentar.".into()));
    }
    if user.is_staff || member || discord || whitelist.as_deref() == Some("approved") {
        Ok(Ok(()))
    } else {
        Ok(Err(NOT_ALLOWED.into()))
    }
}

async fn is_moderator(pool: &PgPool, server_id: Uuid, user: Option<&User>) -> bool {
    match user {
        Some(user) => require_role(pool, server_id, user, Role::Moderator).await.is_ok(),
        None => false,
    }
}

/// `GET /servers/{slug}/comments`: mais novos primeiro.
pub async fn list(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path(slug): Path<String>,
    Query(page): Query<PageQuery>,
) -> ApiResult<Json<CommentPage>> {
    let server_id = team_server(&state.pool, &slug, user.as_ref()).await?;
    let can_moderate = is_moderator(&state.pool, server_id, user.as_ref()).await;
    let limit = page.limit.unwrap_or(PAGE_DEFAULT).clamp(1, PAGE_MAX);
    let mut rows = sqlx::query_as::<_, CommentRow>(
        "SELECT c.id, c.body, c.created_at, c.author_id, u.minecraft_name AS author_name,
                u.minecraft_uuid AS author_uuid, m.role AS author_role,
                u.account_type = 'offline' AS author_offline
           FROM server_comments c
           JOIN users u ON u.id = c.author_id
           LEFT JOIN server_members m ON m.server_id = c.server_id AND m.user_id = c.author_id
          WHERE c.server_id = $1 AND ($2::timestamptz IS NULL OR c.created_at < $2)
          ORDER BY c.created_at DESC
          LIMIT $3",
    )
    .bind(server_id)
    .bind(page.before)
    .bind(limit + 1)
    .fetch_all(&state.pool)
    .await?;
    let has_more = rows.len() as i64 > limit;
    rows.truncate(usize::try_from(limit).unwrap_or(PAGE_DEFAULT as usize));
    let (total, stars): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM server_comments WHERE server_id = $1),
                (SELECT COUNT(*) FROM server_stars WHERE server_id = $1)",
    )
    .bind(server_id)
    .fetch_one(&state.pool)
    .await?;
    let viewer_id = user.as_ref().map(|u| u.id);
    let comments = rows
        .into_iter()
        .map(|row| {
            let mine = viewer_id == Some(row.author_id);
            Comment {
                id: row.id,
                body: row.body,
                created_at: row.created_at,
                author_name: row.author_name,
                author_uuid: row.author_uuid,
                author_role: row.author_role,
                author_offline: row.author_offline,
                mine,
                can_delete: mine || can_moderate,
            }
        })
        .collect();
    let viewer = match &user {
        Some(user) => match comment_permission(&state.pool, server_id, user).await? {
            Ok(()) => Viewer { can_comment: true, reason: None, can_moderate },
            Err(reason) => Viewer { can_comment: false, reason: Some(reason), can_moderate },
        },
        None => Viewer { can_comment: false, reason: Some("Entre com sua conta para comentar.".into()), can_moderate },
    };
    Ok(Json(CommentPage { total, stars, comments, has_more, viewer }))
}

#[derive(Deserialize)]
pub struct CommentInput {
    pub body: String,
}

/// Limpa e confere o texto do comentário. Mensagens para o jogador.
fn clean_body(raw: &str) -> ApiResult<String> {
    let body = raw.lines().map(str::trim_end).collect::<Vec<_>>().join("\n").trim().to_string();
    // Sem linhas em branco repetidas (no máximo uma entre parágrafos).
    let mut out = String::with_capacity(body.len());
    let mut blank = 0;
    for line in body.lines() {
        blank = if line.trim().is_empty() { blank + 1 } else { 0 };
        if blank <= 1 {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
    }
    let chars = out.chars().count();
    if chars < 2 {
        return Err(ApiError::bad("Escreva seu comentário."));
    }
    if chars > MAX_COMMENT_CHARS {
        return Err(ApiError::bad(format!("O comentário pode ter no máximo {MAX_COMMENT_CHARS} caracteres.")));
    }
    if out.lines().count() > MAX_COMMENT_LINES {
        return Err(ApiError::bad(format!("O comentário pode ter no máximo {MAX_COMMENT_LINES} linhas.")));
    }
    if out.chars().any(|c| c.is_control() && c != '\n') {
        return Err(ApiError::bad("O comentário tem caracteres inválidos."));
    }
    if moderation::has_link(&out) {
        return Err(ApiError::bad("Comentários não podem ter links."));
    }
    if moderation::blocked_term(&out).is_some() {
        return Err(ApiError::bad("O comentário contém termos não permitidos no Conexão."));
    }
    Ok(out)
}

/// `POST /servers/{slug}/comments`
pub async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Json(input): Json<CommentInput>,
) -> ApiResult<(StatusCode, Json<Comment>)> {
    let server_id = approved_server(&state.pool, &slug).await?;
    if let Err(reason) = comment_permission(&state.pool, server_id, &user).await? {
        return Err(ApiError::bad(reason));
    }
    let body = clean_body(&input.body)?;
    let (today, last): (i64, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM server_comments
                  WHERE server_id = $1 AND author_id = $2 AND created_at > now() - interval '1 day'),
                (SELECT MAX(created_at) FROM server_comments WHERE author_id = $2)",
    )
    .bind(server_id)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    if today >= MAX_COMMENTS_PER_DAY {
        return Err(ApiError::bad(format!(
            "Você já comentou {MAX_COMMENTS_PER_DAY} vezes neste servidor nas últimas 24 horas. Volte amanhã."
        )));
    }
    if let Some(last) = last {
        let wait = COMMENT_COOLDOWN_SECS - (Utc::now() - last).num_seconds();
        if wait > 0 {
            return Err(ApiError::bad(format!("Espere {wait} s para comentar de novo.")));
        }
    }
    let id = Uuid::new_v4();
    let created_at: DateTime<Utc> = sqlx::query_scalar(
        "INSERT INTO server_comments (id, server_id, author_id, body) VALUES ($1, $2, $3, $4) RETURNING created_at",
    )
    .bind(id)
    .bind(server_id)
    .bind(user.id)
    .bind(&body)
    .fetch_one(&state.pool)
    .await?;
    let author_role: Option<String> =
        sqlx::query_scalar("SELECT role FROM server_members WHERE server_id = $1 AND user_id = $2")
            .bind(server_id)
            .bind(user.id)
            .fetch_optional(&state.pool)
            .await?;
    Ok((
        StatusCode::CREATED,
        Json(Comment {
            id,
            body,
            created_at,
            author_offline: user.is_offline(),
            author_name: user.minecraft_name,
            author_uuid: user.minecraft_uuid,
            author_role,
            mine: true,
            can_delete: true,
        }),
    ))
}

/// `DELETE /servers/{slug}/comments/{id}`: o autor ou a equipe do servidor (moderador ou acima).
pub async fn remove(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((slug, id)): Path<(String, Uuid)>,
) -> ApiResult<StatusCode> {
    let server_id = team_server(&state.pool, &slug, Some(&user)).await?;
    let (author_id, author_name, body): (Uuid, String, String) = sqlx::query_as(
        "SELECT c.author_id, u.minecraft_name, c.body FROM server_comments c JOIN users u ON u.id = c.author_id
          WHERE c.id = $1 AND c.server_id = $2",
    )
    .bind(id)
    .bind(server_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    let own = author_id == user.id;
    if !own {
        require_role(&state.pool, server_id, &user, Role::Moderator).await?;
    }
    sqlx::query("DELETE FROM server_comments WHERE id = $1").bind(id).execute(&state.pool).await?;
    if !own {
        // Fica no histórico do painel: quem apagou, de quem era e o começo do texto.
        let excerpt: String = body.chars().take(120).collect();
        audit(
            &state.pool,
            Some(server_id),
            Some(user.id),
            "comment.deleted",
            json!({ "id": id, "author": author_name, "excerpt": excerpt }),
        )
        .await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_comments() {
        assert_eq!(clean_body("  Servidor muito bom!  ").unwrap(), "Servidor muito bom!");
        assert_eq!(clean_body("Linha 1\n\n\n\nLinha 2").unwrap(), "Linha 1\n\nLinha 2");
        assert!(clean_body("").is_err());
        assert!(clean_body("x").is_err());
        assert!(clean_body(&"a".repeat(MAX_COMMENT_CHARS + 1)).is_err());
        assert!(clean_body("1\n2\n3\n4\n5\n6\n7\n8\n9").is_err());
        assert!(clean_body("entra no discord.gg/abc").is_err());
        assert!(clean_body("https://exemplo.com").is_err());
        assert!(clean_body("servidor de nazista").is_err());
        assert!(clean_body("Staff atenciosa, RP medieval excelente. Recomendo! 💚").is_ok());
    }
}
