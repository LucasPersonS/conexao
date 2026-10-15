//! Notícias e eventos dos servidores.
//!
//! A equipe do servidor (moderador ou acima) publica pelo painel; o launcher mostra
//! na página do servidor e os próximos eventos na home. Só texto: sem HTML e sem
//! links (o filtro de palavras dos perfis também vale aqui).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthUser, Role, audit, require_role};
use crate::error::{ApiError, ApiResult};
use crate::moderation;
use crate::routes::servers::server_id_by_slug;
use crate::state::AppState;

/// Eventos continuam listados até 3 horas depois do início (estão acontecendo).
const EVENT_GRACE_HOURS: i64 = 3;
const MAX_POSTS_PER_DAY: i64 = 20;

#[derive(Serialize, sqlx::FromRow)]
pub struct Post {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub starts_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub author_name: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct EventItem {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub starts_at: DateTime<Utc>,
    pub slug: String,
    pub server_name: String,
    pub icon_url: Option<String>,
}

const POST_SELECT: &str = "SELECT p.id, p.kind, p.title, p.body, p.starts_at, p.created_at, u.minecraft_name AS author_name
    FROM server_posts p LEFT JOIN users u ON u.id = p.author_id";

pub async fn list(State(state): State<AppState>, Path(slug): Path<String>) -> ApiResult<Json<serde_json::Value>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    let news = sqlx::query_as::<_, Post>(&format!(
        "{POST_SELECT} WHERE p.server_id = $1 AND p.kind = 'news' ORDER BY p.created_at DESC LIMIT 20"
    ))
    .bind(server_id)
    .fetch_all(&state.pool)
    .await?;
    let events = sqlx::query_as::<_, Post>(&format!(
        "{POST_SELECT} WHERE p.server_id = $1 AND p.kind = 'event' AND p.starts_at > now() - make_interval(hours => $2)
         ORDER BY p.starts_at LIMIT 20"
    ))
    .bind(server_id)
    .bind(EVENT_GRACE_HOURS as i32)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({ "news": news, "events": events })))
}

/// Próximos eventos de todos os servidores aprovados (home do launcher).
pub async fn upcoming(State(state): State<AppState>) -> ApiResult<Json<Vec<EventItem>>> {
    let events = sqlx::query_as::<_, EventItem>(
        "SELECT p.id, p.title, p.body, p.starts_at, s.slug, s.name AS server_name, s.icon_url
           FROM server_posts p JOIN servers s ON s.id = p.server_id
          WHERE p.kind = 'event' AND s.approval = 'approved'
            AND p.starts_at > now() - make_interval(hours => $1) AND p.starts_at < now() + interval '14 days'
          ORDER BY p.starts_at LIMIT 40",
    )
    .bind(EVENT_GRACE_HOURS as i32)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(events))
}

#[derive(Deserialize)]
pub struct PostInput {
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub starts_at: Option<DateTime<Utc>>,
}

struct Clean {
    kind: &'static str,
    title: String,
    body: String,
    starts_at: Option<DateTime<Utc>>,
}

fn check_text(value: &str, label: &str) -> ApiResult<()> {
    if value.chars().any(|c| c.is_control() && c != '\n') {
        return Err(ApiError::bad(format!("{label}: caracteres inválidos.")));
    }
    if moderation::blocked_term(value).is_some() {
        return Err(ApiError::bad(format!("{label}: contém termos não permitidos no Conexão.")));
    }
    Ok(())
}

fn validate(input: PostInput, now: DateTime<Utc>) -> ApiResult<Clean> {
    let kind = match input.kind.as_str() {
        "news" => "news",
        "event" => "event",
        _ => return Err(ApiError::bad("Tipo inválido (news ou event).")),
    };
    let title = input.title.split_whitespace().collect::<Vec<_>>().join(" ");
    let title_len = title.chars().count();
    if !(3..=80).contains(&title_len) {
        return Err(ApiError::bad("O título precisa ter de 3 a 80 caracteres."));
    }
    let body = input.body.lines().map(str::trim_end).collect::<Vec<_>>().join("\n").trim().to_string();
    if body.chars().count() > 1000 {
        return Err(ApiError::bad("O texto pode ter no máximo 1000 caracteres."));
    }
    if body.lines().count() > 15 {
        return Err(ApiError::bad("O texto pode ter no máximo 15 linhas."));
    }
    check_text(&title, "Título")?;
    check_text(&body, "Texto")?;
    let starts_at = match kind {
        "event" => {
            let at = input.starts_at.ok_or_else(|| ApiError::bad("Informe a data e a hora do evento."))?;
            if at < now - TimeDelta::minutes(5) {
                return Err(ApiError::bad("A data do evento já passou."));
            }
            if at > now + TimeDelta::days(180) {
                return Err(ApiError::bad("Eventos podem ser marcados com até 6 meses de antecedência."));
            }
            Some(at)
        }
        _ => None,
    };
    Ok(Clean { kind, title, body, starts_at })
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Json(input): Json<PostInput>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Moderator).await?;
    let post = validate(input, Utc::now())?;
    let today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM server_posts WHERE server_id = $1 AND created_at > now() - interval '1 day'",
    )
    .bind(server_id)
    .fetch_one(&state.pool)
    .await?;
    if today >= MAX_POSTS_PER_DAY {
        return Err(ApiError::bad("Limite de publicações por dia atingido."));
    }
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO server_posts (id, server_id, kind, title, body, starts_at, author_id) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(server_id)
    .bind(post.kind)
    .bind(&post.title)
    .bind(&post.body)
    .bind(post.starts_at)
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    audit(&state.pool, Some(server_id), Some(user.id), "post.created", json!({ "id": id, "kind": post.kind, "title": post.title })).await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

pub async fn update(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((slug, id)): Path<(String, Uuid)>,
    Json(input): Json<PostInput>,
) -> ApiResult<StatusCode> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Moderator).await?;
    let post = validate(input, Utc::now())?;
    let done = sqlx::query(
        "UPDATE server_posts SET kind = $3, title = $4, body = $5, starts_at = $6, updated_at = now()
          WHERE id = $1 AND server_id = $2",
    )
    .bind(id)
    .bind(server_id)
    .bind(post.kind)
    .bind(&post.title)
    .bind(&post.body)
    .bind(post.starts_at)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if done == 0 {
        return Err(ApiError::NotFound);
    }
    audit(&state.pool, Some(server_id), Some(user.id), "post.updated", json!({ "id": id, "title": post.title })).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((slug, id)): Path<(String, Uuid)>,
) -> ApiResult<StatusCode> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Moderator).await?;
    let done = sqlx::query("DELETE FROM server_posts WHERE id = $1 AND server_id = $2")
        .bind(id)
        .bind(server_id)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if done == 0 {
        return Err(ApiError::NotFound);
    }
    audit(&state.pool, Some(server_id), Some(user.id), "post.deleted", json!({ "id": id })).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(kind: &str, title: &str, starts_at: Option<DateTime<Utc>>) -> PostInput {
        PostInput { kind: kind.into(), title: title.into(), body: "Todos na praça às 20h.".into(), starts_at }
    }

    #[test]
    fn validates_posts() {
        let now = Utc::now();
        assert!(validate(input("news", "Atualização 1.2 no ar", None), now).is_ok());
        assert!(validate(input("event", "Coroação do Rei", Some(now + TimeDelta::days(2))), now).is_ok());
        assert!(validate(input("event", "Sem data", None), now).is_err());
        assert!(validate(input("event", "Passado", Some(now - TimeDelta::days(1))), now).is_err());
        assert!(validate(input("news", "x", None), now).is_err());
        assert!(validate(input("party", "Tipo errado", None), now).is_err());
        assert!(validate(input("news", "Evento nazista", None), now).is_err());
    }
}
