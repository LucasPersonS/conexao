use std::collections::HashMap;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthUser, Uploader, User};
use crate::error::{ApiError, ApiResult};
use crate::modrinth::{self, ResolvedFile};
use crate::state::AppState;
use crate::storage;

#[derive(Deserialize)]
pub struct MissingInput {
    pub sha1: Vec<String>,
}

#[derive(Serialize)]
pub struct MissingResponse {
    pub missing: Vec<String>,
}

#[derive(Serialize)]
pub struct ResolveResponse {
    /// Arquivos reconhecidos no Modrinth: viram link, não precisam ser enviados.
    pub resolved: HashMap<String, ResolvedFile>,
    /// Não estão no Modrinth nem no storage: o painel envia só estes.
    pub missing: Vec<String>,
}

async fn can_upload(state: &AppState, user: &User) -> ApiResult<()> {
    if user.is_staff {
        return Ok(());
    }
    let is_admin: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM server_members WHERE user_id = $1 AND role IN ('owner', 'admin'))",
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    if is_admin { Ok(()) } else { Err(ApiError::Forbidden) }
}

fn check_hashes(list: &[String]) -> ApiResult<()> {
    if list.len() > 20_000 || !list.iter().all(|h| storage::is_sha1(h)) {
        return Err(ApiError::bad("Lista de SHA-1 inválida."));
    }
    Ok(())
}

async fn stored(state: &AppState, hashes: &[String]) -> ApiResult<Vec<String>> {
    Ok(sqlx::query_scalar("SELECT sha1 FROM stored_files WHERE sha1 = ANY($1)")
        .bind(hashes)
        .fetch_all(&state.pool)
        .await?)
}

/// Quais destes arquivos ainda não estão no storage?
pub async fn missing(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(input): Json<MissingInput>,
) -> ApiResult<Json<MissingResponse>> {
    can_upload(&state, &user).await?;
    check_hashes(&input.sha1)?;
    let present = stored(&state, &input.sha1).await?;
    let mut missing: Vec<String> = input.sha1.into_iter().filter(|h| !present.contains(h)).collect();
    missing.sort();
    missing.dedup();
    Ok(Json(MissingResponse { missing }))
}

/// Reconhece os arquivos no Modrinth e diz quais ainda precisam ser enviados.
pub async fn resolve(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(input): Json<MissingInput>,
) -> ApiResult<Json<ResolveResponse>> {
    can_upload(&state, &user).await?;
    check_hashes(&input.sha1)?;
    let resolved = modrinth::resolve(&state, &input.sha1).await?;
    let present = stored(&state, &input.sha1).await?;
    let mut missing: Vec<String> = input
        .sha1
        .into_iter()
        .filter(|h| !resolved.contains_key(h) && !present.contains(h))
        .collect();
    missing.sort();
    missing.dedup();
    Ok(Json(ResolveResponse { resolved, missing }))
}

/// Quanto esta conta ainda pode enviar nas últimas 24 horas. `None` = sem limite (equipe).
///
/// Quem é dono ou admin de um servidor em destaque tem a cota do destaque; os demais,
/// a cota normal de um servidor. Assim uma conta qualquer não lota o disco da VPS.
async fn user_budget(state: &AppState, user: &User) -> ApiResult<Option<u64>> {
    if user.is_staff {
        return Ok(None);
    }
    let featured: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM server_members m JOIN servers s ON s.id = m.server_id
          WHERE m.user_id = $1 AND m.role IN ('owner', 'admin')
            AND s.featured_until IS NOT NULL AND s.featured_until > now())",
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    let used: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(size), 0)::BIGINT FROM stored_files
          WHERE uploaded_by = $1 AND created_at > now() - interval '24 hours'",
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    let quota = if featured { state.config.featured_quota_bytes } else { state.config.server_quota_bytes };
    Ok(Some(quota.saturating_sub(u64::try_from(used).unwrap_or(0))))
}

/// O mesmo limite diário para o agente/Companion de um servidor.
async fn agent_budget(state: &AppState, server_id: Uuid) -> ApiResult<u64> {
    let used: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(size), 0)::BIGINT FROM stored_files
          WHERE uploaded_by_server = $1 AND created_at > now() - interval '24 hours'",
    )
    .bind(server_id)
    .fetch_one(&state.pool)
    .await?;
    let quota = crate::policy::quota_for(state, server_id).await?;
    Ok(quota.saturating_sub(u64::try_from(used).unwrap_or(0)))
}

/// Recebe o conteúdo de um arquivo e confere o SHA-1 antes de guardar.
///
/// O corpo é gravado direto no disco enquanto chega (nunca inteiro na memória) e a
/// gravação para assim que passa do limite: o tamanho máximo por arquivo e o que
/// ainda resta da cota diária de quem envia.
pub async fn upload(
    State(state): State<AppState>,
    uploader: Uploader,
    Path(sha1): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    if !storage::is_sha1(&sha1) {
        return Err(ApiError::bad("SHA-1 inválido."));
    }
    let (user_id, server_id, budget): (Option<Uuid>, Option<Uuid>, Option<u64>) = match uploader {
        Uploader::User(user) => {
            can_upload(&state, &user).await?;
            let budget = user_budget(&state, &user).await?;
            (Some(user.id), None, budget)
        }
        Uploader::Agent(server_id) => {
            // Agentes só enviam o que a API pediu (o arquivo precisa estar na última fotografia).
            let requested: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM server_snapshots s, jsonb_array_elements(s.mods) m
                  WHERE s.server_id = $1 AND m->>'sha1' = $2)",
            )
            .bind(server_id)
            .bind(&sha1)
            .fetch_one(&state.pool)
            .await?;
            if !requested {
                return Err(ApiError::Forbidden);
            }
            (None, Some(server_id), Some(agent_budget(&state, server_id).await?))
        }
    };
    let url = format!("{}/files/{sha1}", state.config.public_url);
    // Já guardado (por qualquer servidor): nada a receber.
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM stored_files WHERE sha1 = $1)")
        .bind(&sha1)
        .fetch_one(&state.pool)
        .await?;
    if exists {
        return Ok((StatusCode::OK, Json(json!({ "sha1": sha1, "url": url }))));
    }

    let max_file = state.config.max_upload_bytes as u64;
    let limit = budget.map_or(max_file, |b| b.min(max_file));
    let declared: Option<u64> = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());
    let too_big = |size: u64| -> ApiError {
        if size > max_file {
            ApiError::bad(format!("Arquivo grande demais (máximo {}).", crate::policy::megabytes(max_file)))
        } else {
            ApiError::bad(format!(
                "Cota diária de envio esgotada (restam {}). Tente de novo amanhã ou fale com a equipe Conexão.",
                crate::policy::megabytes(limit)
            ))
        }
    };
    if let Some(size) = declared
        && size > limit
    {
        return Err(too_big(size));
    }

    let tmp = storage::tmp_path(&state.config.storage_dir, &sha1);
    let received = receive(body, &tmp, limit).await;
    let size = match received {
        Ok(Received { size, sha1: actual }) if actual == sha1 => size,
        Ok(_) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(ApiError::bad("O conteúdo não corresponde ao SHA-1 informado."));
        }
        Err(error) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(match error {
                ReceiveError::TooBig(size) => too_big(size),
                ReceiveError::Aborted => ApiError::bad("O envio foi interrompido. Tente de novo."),
                ReceiveError::Io(error) => ApiError::from(error),
            });
        }
    };
    storage::commit_tmp(&state.config.storage_dir, &sha1, &tmp).await?;
    sqlx::query(
        "INSERT INTO stored_files (sha1, size, uploaded_by, uploaded_by_server) VALUES ($1, $2, $3, $4)
         ON CONFLICT (sha1) DO NOTHING",
    )
    .bind(&sha1)
    .bind(i64::try_from(size).unwrap_or(i64::MAX))
    .bind(user_id)
    .bind(server_id)
    .execute(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(json!({ "sha1": sha1, "url": url }))))
}

struct Received {
    size: u64,
    sha1: String,
}

enum ReceiveError {
    TooBig(u64),
    Aborted,
    Io(std::io::Error),
}

/// Grava o corpo em `tmp` calculando o SHA-1, parando ao passar de `limit` bytes.
async fn receive(body: Body, tmp: &std::path::Path, limit: u64) -> Result<Received, ReceiveError> {
    let mut file = tokio::fs::File::create(tmp).await.map_err(ReceiveError::Io)?;
    let mut hasher = sha1_smol::Sha1::new();
    let mut size: u64 = 0;
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ReceiveError::Aborted)?;
        size += chunk.len() as u64;
        if size > limit {
            return Err(ReceiveError::TooBig(size));
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(ReceiveError::Io)?;
    }
    file.sync_all().await.map_err(ReceiveError::Io)?;
    Ok(Received { size, sha1: hasher.digest().to_string() })
}
