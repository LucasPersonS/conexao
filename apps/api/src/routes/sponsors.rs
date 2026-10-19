//! Banners de patrocinadores no launcher.
//!
//! * A equipe cadastra no painel (imagem enviada e limpa pelo `images.rs`, link https,
//!   posições, período e peso na rotação).
//! * O launcher pede os banners ativos de uma posição e conta visualizações e cliques.
//! * Contagem confiável para o relatório: uma visualização e um clique por pessoa, por dia
//!   e por posição; só conta banner que está no ar naquela posição.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthUser, Staff, audit};
use crate::error::{ApiError, ApiResult};
use crate::moderation;
use crate::routes::images::{MAX_BANNER, store_image};
use crate::state::AppState;

pub const PLACEMENTS: &[&str] = &["home", "server", "loading"];
/// Máximo de banners contados por requisição.
const MAX_EVENT_IDS: usize = 10;

fn valid_placement(value: &str) -> bool {
    PLACEMENTS.contains(&value)
}

/// Link de destino: só https, sem usuário/senha embutidos, até 500 caracteres.
pub fn clean_link(value: &str) -> ApiResult<String> {
    let value = value.trim();
    let bad = || ApiError::bad("O link precisa ser um endereço https:// válido.");
    if value.len() > 500 || value.chars().any(char::is_whitespace) {
        return Err(bad());
    }
    let url = reqwest::Url::parse(value).map_err(|_| bad())?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(bad());
    }
    let host = url.host_str().ok_or_else(bad)?;
    if !host.contains('.') || host.ends_with(".localhost") {
        return Err(bad());
    }
    Ok(url.to_string())
}

/// Imagem: só as enviadas ao próprio Conexão (`{public_url}/images/<sha1>.<ext>`).
fn clean_image(state: &AppState, value: &str) -> ApiResult<String> {
    let prefix = format!("{}/images/", state.config.public_url);
    let name = value.trim().strip_prefix(&prefix).ok_or_else(|| ApiError::bad("Envie a imagem pelo painel."))?;
    let (sha1, ext) = name.split_once('.').ok_or_else(|| ApiError::bad("Imagem inválida."))?;
    if !crate::storage::is_sha1(sha1) || !matches!(ext, "png" | "jpg") {
        return Err(ApiError::bad("Imagem inválida."));
    }
    Ok(value.trim().to_string())
}

// ------------------------------------------------------------------ launcher

#[derive(Serialize, sqlx::FromRow)]
pub struct PublicSponsor {
    id: Uuid,
    name: String,
    image_url: String,
    image_side_url: Option<String>,
    link_url: String,
    alt_text: Option<String>,
    weight: i32,
}

const LIVE: &str = "active AND starts_at <= now() AND (ends_at IS NULL OR ends_at > now())";

/// Banners no ar numa posição (público; o launcher sorteia a ordem pelo peso).
pub async fn list(State(state): State<AppState>, Path(placement): Path<String>) -> ApiResult<Json<Vec<PublicSponsor>>> {
    if !valid_placement(&placement) {
        return Err(ApiError::bad("Posição inválida."));
    }
    Ok(Json(
        sqlx::query_as::<_, PublicSponsor>(&format!(
            "SELECT id, name, image_url, image_side_url, link_url, alt_text, weight FROM sponsors
              WHERE {LIVE} AND $1 = ANY(placements) ORDER BY weight DESC, created_at LIMIT 20"
        ))
        .bind(&placement)
        .fetch_all(&state.pool)
        .await?,
    ))
}

#[derive(Deserialize)]
pub struct EventsInput {
    placement: String,
    #[serde(default)]
    views: Vec<Uuid>,
    #[serde(default)]
    click: Option<Uuid>,
}

async fn count(state: &AppState, ids: &[Uuid], user: Uuid, placement: &str, kind: &str) -> ApiResult<()> {
    if ids.is_empty() {
        return Ok(());
    }
    // Primeiro marca "já contou hoje" (sem repetir); só o que entrou agora soma nos totais.
    sqlx::query(&format!(
        "WITH fresh AS (
            INSERT INTO sponsor_seen (sponsor_id, user_id, day, placement, kind)
            SELECT s.id, $2, current_date, $3, $4 FROM sponsors s
             WHERE s.id = ANY($1) AND {LIVE} AND $3 = ANY(s.placements)
            ON CONFLICT DO NOTHING
            RETURNING sponsor_id
         )
         INSERT INTO sponsor_stats (sponsor_id, day, placement, views, clicks)
         SELECT sponsor_id, current_date, $3, CASE WHEN $4 = 'view' THEN 1 ELSE 0 END, CASE WHEN $4 = 'click' THEN 1 ELSE 0 END
           FROM fresh
         ON CONFLICT (sponsor_id, day, placement)
         DO UPDATE SET views = sponsor_stats.views + EXCLUDED.views, clicks = sponsor_stats.clicks + EXCLUDED.clicks"
    ))
    .bind(ids)
    .bind(user)
    .bind(placement)
    .bind(kind)
    .execute(&state.pool)
    .await?;
    Ok(())
}

/// Visualizações e cliques vindos do launcher (precisa de login: é o que impede inflar números).
pub async fn events(State(state): State<AppState>, AuthUser(user): AuthUser, Json(input): Json<EventsInput>) -> ApiResult<StatusCode> {
    if !valid_placement(&input.placement) {
        return Err(ApiError::bad("Posição inválida."));
    }
    record(&state, user.id, &input.placement, input.views, input.click).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn record(state: &AppState, user: Uuid, placement: &str, mut views: Vec<Uuid>, click: Option<Uuid>) -> ApiResult<()> {
    views.sort_unstable();
    views.dedup();
    views.truncate(MAX_EVENT_IDS);
    count(state, &views, user, placement, "view").await?;
    if let Some(click) = click {
        // Clique também conta como visto (o banner estava na tela).
        count(state, &[click], user, placement, "view").await?;
        count(state, &[click], user, placement, "click").await?;
    }
    Ok(())
}

// ------------------------------------------------------------------ equipe

#[derive(Serialize, sqlx::FromRow)]
pub struct StaffSponsor {
    id: Uuid,
    name: String,
    image_url: String,
    image_side_url: Option<String>,
    link_url: String,
    alt_text: Option<String>,
    placements: Vec<String>,
    weight: i32,
    starts_at: DateTime<Utc>,
    ends_at: Option<DateTime<Utc>>,
    active: bool,
    live: bool,
    views_30d: i64,
    clicks_30d: i64,
    created_at: DateTime<Utc>,
}

pub async fn staff_list(State(state): State<AppState>, Staff(_): Staff) -> ApiResult<Json<Vec<StaffSponsor>>> {
    Ok(Json(
        sqlx::query_as::<_, StaffSponsor>(&format!(
            "SELECT s.id, s.name, s.image_url, s.image_side_url, s.link_url, s.alt_text, s.placements, s.weight,
                    s.starts_at, s.ends_at, s.active, ({LIVE}) AS live,
                    COALESCE(SUM(st.views), 0)::BIGINT AS views_30d, COALESCE(SUM(st.clicks), 0)::BIGINT AS clicks_30d,
                    s.created_at
               FROM sponsors s
               LEFT JOIN sponsor_stats st ON st.sponsor_id = s.id AND st.day > current_date - 30
              GROUP BY s.id
              ORDER BY ({LIVE}) DESC, s.created_at DESC"
        ))
        .fetch_all(&state.pool)
        .await?,
    ))
}

#[derive(Deserialize)]
pub struct SponsorInput {
    name: String,
    image_url: String,
    #[serde(default)]
    image_side_url: Option<String>,
    link_url: String,
    #[serde(default)]
    alt_text: Option<String>,
    placements: Vec<String>,
    #[serde(default = "one")]
    weight: i32,
    #[serde(default)]
    starts_at: Option<DateTime<Utc>>,
    #[serde(default)]
    ends_at: Option<DateTime<Utc>>,
    #[serde(default = "yes")]
    active: bool,
}

const fn one() -> i32 {
    1
}
const fn yes() -> bool {
    true
}

struct Clean {
    name: String,
    image_url: String,
    image_side_url: Option<String>,
    link_url: String,
    alt_text: Option<String>,
    placements: Vec<String>,
    weight: i32,
    starts_at: DateTime<Utc>,
    ends_at: Option<DateTime<Utc>>,
    active: bool,
}

fn short_text(value: &str, max: usize, field: &str) -> ApiResult<String> {
    let text: String = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > max || text.chars().any(char::is_control) {
        return Err(ApiError::bad(format!("{field}: até {max} caracteres, sem quebras de linha.")));
    }
    Ok(text)
}

fn validate(state: &AppState, input: SponsorInput) -> ApiResult<Clean> {
    let name = short_text(&input.name, 80, "Nome")?;
    if name.is_empty() {
        return Err(ApiError::bad("Dê um nome ao patrocinador."));
    }
    let alt_text = input.alt_text.as_deref().map(|t| short_text(t, 140, "Descrição")).transpose()?.filter(|t| !t.is_empty());
    if alt_text.as_deref().is_some_and(|t| moderation::blocked_term(t).is_some()) || moderation::blocked_term(&name).is_some() {
        return Err(ApiError::bad("O texto contém termos não permitidos no Conexão."));
    }
    let mut placements: Vec<String> = input.placements.into_iter().filter(|p| valid_placement(p)).collect();
    placements.sort_unstable();
    placements.dedup();
    if placements.is_empty() {
        return Err(ApiError::bad("Escolha pelo menos uma posição (home, servidor ou carregamento)."));
    }
    if !(1..=10).contains(&input.weight) {
        return Err(ApiError::bad("O peso vai de 1 a 10."));
    }
    let starts_at = input.starts_at.unwrap_or_else(Utc::now);
    if input.ends_at.is_some_and(|end| end <= starts_at) {
        return Err(ApiError::bad("O fim precisa ser depois do início."));
    }
    Ok(Clean {
        name,
        image_url: clean_image(state, &input.image_url)?,
        image_side_url: input.image_side_url.as_deref().filter(|u| !u.trim().is_empty()).map(|u| clean_image(state, u)).transpose()?,
        link_url: clean_link(&input.link_url)?,
        alt_text,
        placements,
        weight: input.weight,
        starts_at,
        ends_at: input.ends_at,
        active: input.active,
    })
}

pub async fn staff_create(State(state): State<AppState>, Staff(staff): Staff, Json(input): Json<SponsorInput>) -> ApiResult<Json<serde_json::Value>> {
    let c = validate(&state, input)?;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO sponsors (id, name, image_url, image_side_url, link_url, alt_text, placements, weight, starts_at, ends_at, active, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(id)
    .bind(&c.name)
    .bind(&c.image_url)
    .bind(&c.image_side_url)
    .bind(&c.link_url)
    .bind(&c.alt_text)
    .bind(&c.placements)
    .bind(c.weight)
    .bind(c.starts_at)
    .bind(c.ends_at)
    .bind(c.active)
    .bind(staff.as_ref().map(|s| s.id))
    .execute(&state.pool)
    .await?;
    audit(&state.pool, None, staff.map(|s| s.id), "sponsor.created", json!({ "id": id, "name": c.name, "link": c.link_url })).await?;
    Ok(Json(json!({ "id": id })))
}

pub async fn staff_update(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Path(id): Path<Uuid>,
    Json(input): Json<SponsorInput>,
) -> ApiResult<StatusCode> {
    let c = validate(&state, input)?;
    let done = sqlx::query(
        "UPDATE sponsors SET name = $2, image_url = $3, image_side_url = $4, link_url = $5, alt_text = $6, placements = $7,
                weight = $8, starts_at = $9, ends_at = $10, active = $11, updated_at = now()
          WHERE id = $1",
    )
    .bind(id)
    .bind(&c.name)
    .bind(&c.image_url)
    .bind(&c.image_side_url)
    .bind(&c.link_url)
    .bind(&c.alt_text)
    .bind(&c.placements)
    .bind(c.weight)
    .bind(c.starts_at)
    .bind(c.ends_at)
    .bind(c.active)
    .execute(&state.pool)
    .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    audit(&state.pool, None, staff.map(|s| s.id), "sponsor.updated", json!({ "id": id, "link": c.link_url, "active": c.active })).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn staff_delete(State(state): State<AppState>, Staff(staff): Staff, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let done = sqlx::query("DELETE FROM sponsors WHERE id = $1").bind(id).execute(&state.pool).await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    audit(&state.pool, None, staff.map(|s| s.id), "sponsor.deleted", json!({ "id": id })).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Envio da imagem (PNG, JPEG ou WebP; limpa metadados). Devolve a URL para o cadastro.
pub async fn staff_image(State(state): State<AppState>, Staff(_): Staff, body: Bytes) -> ApiResult<Json<serde_json::Value>> {
    if body.len() > MAX_BANNER {
        return Err(ApiError::bad(format!("Imagem grande demais (máximo {} MB).", MAX_BANNER / 1024 / 1024)));
    }
    Ok(Json(json!({ "url": store_image(&state, &body).await? })))
}

#[derive(Serialize, sqlx::FromRow)]
struct DailyStat {
    day: NaiveDate,
    placement: String,
    views: i64,
    clicks: i64,
}

/// Relatório diário (30 dias) para mandar ao patrocinador.
pub async fn staff_stats(State(state): State<AppState>, Staff(_): Staff, Path(id): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    let rows = sqlx::query_as::<_, DailyStat>(
        "SELECT day, placement, views, clicks FROM sponsor_stats
          WHERE sponsor_id = $1 AND day > current_date - 30 ORDER BY day, placement",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({ "days": rows })))
}

/// Limpa as marcações de "já contou" com mais de 2 dias (os totais ficam).
pub async fn prune_seen(state: &AppState) -> ApiResult<u64> {
    Ok(sqlx::query("DELETE FROM sponsor_seen WHERE day < current_date - 2").execute(&state.pool).await?.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::clean_link;

    #[test]
    fn links_must_be_plain_https() {
        assert_eq!(clean_link(" https://loja.exemplo.com.br/promo?utm=conexao ").unwrap(), "https://loja.exemplo.com.br/promo?utm=conexao");
        for bad in [
            "http://exemplo.com",
            "javascript:alert(1)",
            "https://user:senha@exemplo.com",
            "https://localhost/x",
            "https://intranet/x",
            "https://exemplo .com",
            "file:///C:/Windows",
            "conexao://servidor/x",
        ] {
            assert!(clean_link(bad).is_err(), "{bad}");
        }
        assert!(clean_link(&format!("https://exemplo.com/{}", "a".repeat(600))).is_err());
    }
}
