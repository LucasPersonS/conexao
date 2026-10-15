//! Painel da equipe → Armazenamento: quanto cada servidor ocupa no disco da VPS e ações
//! para liberar espaço (apagar versões, derrubar arquivos, rodar a limpeza na hora).
//!
//! "Hospedado" = arquivos guardados no Conexão (sem link do Modrinth/CurseForge), contados
//! uma vez por SHA-1. "Exclusivo" = o que só aquele servidor usa: o espaço que volta se as
//! versões dele forem apagadas (arquivos iguais em outros servidores continuam).

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{Staff, audit};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::storage;

/// Pares (servidor, arquivo hospedado) de todas as versões publicadas.
const REFS: &str = "refs AS (
    SELECT DISTINCT pv.server_id, f ->> 'sha1' AS sha1
      FROM pack_versions pv, jsonb_array_elements(pv.manifest -> 'files') f
     WHERE f ->> 'url' IS NULL
)";

#[derive(Serialize)]
pub struct Disk {
    pub total: u64,
    pub free: u64,
    pub used_percent: Option<f64>,
    pub alert_percent: f64,
}

#[derive(Serialize)]
pub struct Totals {
    pub files: i64,
    pub bytes: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ServerUsage {
    pub slug: String,
    pub name: String,
    pub approval: String,
    pub featured: bool,
    pub versions: i64,
    pub files: i64,
    pub hosted_bytes: i64,
    pub exclusive_bytes: i64,
    pub current_bytes: i64,
    pub uploaded_24h: i64,
    pub last_published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct BlockedFile {
    pub sha1: String,
    pub size: i64,
    pub path: String,
    pub reason: String,
    pub blocked_by: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
pub struct Overview {
    pub disk: Disk,
    pub stored: Totals,
    /// Arquivos que nenhuma versão usa (a limpeza diária apaga depois de 48 h sem uso).
    pub unused: Totals,
    pub quota_bytes: u64,
    pub featured_quota_bytes: u64,
    pub servers: Vec<ServerUsage>,
    pub blocked: Vec<BlockedFile>,
}

pub async fn overview(State(state): State<AppState>, _staff: Staff) -> ApiResult<Json<Overview>> {
    let root = &state.config.storage_dir;
    let disk = Disk {
        total: fs4::total_space(root).unwrap_or(0),
        free: fs4::available_space(root).unwrap_or(0),
        used_percent: crate::monitor::disk_used_percent(root),
        alert_percent: state.config.disk_alert_percent,
    };
    let (files, bytes): (i64, i64) =
        sqlx::query_as("SELECT count(*), COALESCE(SUM(size), 0)::BIGINT FROM stored_files")
            .fetch_one(&state.pool)
            .await?;
    let (unused_files, unused_bytes): (i64, i64) = sqlx::query_as(
        "SELECT count(*), COALESCE(SUM(sf.size), 0)::BIGINT FROM stored_files sf
          WHERE NOT EXISTS (SELECT 1 FROM pack_versions pv
                             WHERE pv.manifest -> 'files' @> jsonb_build_array(jsonb_build_object('sha1', sf.sha1)))
            AND NOT EXISTS (SELECT 1 FROM server_snapshots ss
                             WHERE ss.mods @> jsonb_build_array(jsonb_build_object('sha1', sf.sha1)))",
    )
    .fetch_one(&state.pool)
    .await?;
    let servers = sqlx::query_as::<_, ServerUsage>(&format!(
        "WITH {REFS},
         sizes AS (
            SELECT r.server_id, r.sha1, sf.size,
                   (SELECT count(DISTINCT o.server_id) FROM refs o WHERE o.sha1 = r.sha1) AS users
              FROM refs r JOIN stored_files sf ON sf.sha1 = r.sha1
         ),
         current_files AS (
            SELECT DISTINCT sc.server_id, f ->> 'sha1' AS sha1
              FROM server_channels sc
              JOIN pack_versions pv ON pv.id = sc.version_id,
                   jsonb_array_elements(pv.manifest -> 'files') f
             WHERE sc.channel = 'stable' AND f ->> 'url' IS NULL
         )
         SELECT s.slug, s.name, s.approval,
                COALESCE(s.featured_until > now(), false) AS featured,
                (SELECT count(*) FROM pack_versions pv WHERE pv.server_id = s.id) AS versions,
                (SELECT count(*) FROM sizes z WHERE z.server_id = s.id) AS files,
                COALESCE((SELECT SUM(z.size) FROM sizes z WHERE z.server_id = s.id), 0)::BIGINT AS hosted_bytes,
                COALESCE((SELECT SUM(z.size) FROM sizes z WHERE z.server_id = s.id AND z.users = 1), 0)::BIGINT
                    AS exclusive_bytes,
                COALESCE((SELECT SUM(sf.size) FROM current_files c JOIN stored_files sf ON sf.sha1 = c.sha1
                           WHERE c.server_id = s.id), 0)::BIGINT AS current_bytes,
                COALESCE((SELECT SUM(sf.size) FROM stored_files sf
                           WHERE sf.created_at > now() - interval '24 hours'
                             AND (sf.uploaded_by_server = s.id
                                  OR sf.uploaded_by IN (SELECT m.user_id FROM server_members m
                                                         WHERE m.server_id = s.id AND m.role IN ('owner', 'admin')))),
                         0)::BIGINT AS uploaded_24h,
                (SELECT max(pv.published_at) FROM pack_versions pv WHERE pv.server_id = s.id) AS last_published_at
           FROM servers s
          ORDER BY hosted_bytes DESC, s.name"
    ))
    .fetch_all(&state.pool)
    .await?;
    let blocked = sqlx::query_as::<_, BlockedFile>(
        "SELECT b.sha1, b.size, b.path, b.reason, u.minecraft_name AS blocked_by, b.created_at
           FROM blocked_files b LEFT JOIN users u ON u.id = b.blocked_by
          ORDER BY b.created_at DESC LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(Overview {
        disk,
        stored: Totals { files, bytes },
        unused: Totals { files: unused_files, bytes: unused_bytes },
        quota_bytes: state.config.server_quota_bytes,
        featured_quota_bytes: state.config.featured_quota_bytes,
        servers,
        blocked,
    }))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct VersionUsage {
    pub id: Uuid,
    pub version: String,
    pub published_at: chrono::DateTime<chrono::Utc>,
    pub channels: Vec<String>,
    pub files: i64,
    pub hosted_bytes: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct FileUsage {
    pub sha1: String,
    pub path: String,
    pub size: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub versions: Vec<String>,
    /// Outros servidores que usam o mesmo arquivo (derrubar afeta todos).
    pub other_servers: Vec<String>,
}

#[derive(Serialize)]
pub struct ServerStorage {
    pub versions: Vec<VersionUsage>,
    pub files: Vec<FileUsage>,
}

pub async fn server_detail(
    State(state): State<AppState>,
    _staff: Staff,
    Path(slug): Path<String>,
) -> ApiResult<Json<ServerStorage>> {
    let server_id = crate::routes::servers::server_id_by_slug(&state.pool, &slug).await?;
    let versions = sqlx::query_as::<_, VersionUsage>(
        "SELECT pv.id, pv.version, pv.published_at,
                ARRAY(SELECT sc.channel FROM server_channels sc WHERE sc.version_id = pv.id ORDER BY sc.channel) AS channels,
                (SELECT count(*) FROM stored_files sf
                  WHERE pv.manifest -> 'files' @> jsonb_build_array(jsonb_build_object('sha1', sf.sha1))) AS files,
                COALESCE((SELECT SUM(sf.size) FROM stored_files sf
                  WHERE pv.manifest -> 'files' @> jsonb_build_array(jsonb_build_object('sha1', sf.sha1))), 0)::BIGINT
                    AS hosted_bytes
           FROM pack_versions pv
          WHERE pv.server_id = $1
          ORDER BY pv.published_at DESC",
    )
    .bind(server_id)
    .fetch_all(&state.pool)
    .await?;
    let files = sqlx::query_as::<_, FileUsage>(&format!(
        "WITH {REFS},
         entries AS (
            SELECT f ->> 'sha1' AS sha1, f ->> 'path' AS path, pv.version, pv.published_at
              FROM pack_versions pv, jsonb_array_elements(pv.manifest -> 'files') f
             WHERE pv.server_id = $1 AND f ->> 'url' IS NULL
         )
         SELECT sf.sha1,
                (SELECT e.path FROM entries e WHERE e.sha1 = sf.sha1 ORDER BY e.published_at DESC LIMIT 1) AS path,
                sf.size, sf.created_at,
                ARRAY(SELECT DISTINCT e.version FROM entries e WHERE e.sha1 = sf.sha1) AS versions,
                ARRAY(SELECT DISTINCT s.slug FROM refs r JOIN servers s ON s.id = r.server_id
                       WHERE r.sha1 = sf.sha1 AND r.server_id <> $1) AS other_servers
           FROM stored_files sf
          WHERE sf.sha1 IN (SELECT e.sha1 FROM entries e)
          ORDER BY sf.size DESC
          LIMIT 300"
    ))
    .bind(server_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(ServerStorage { versions, files }))
}

/// Apaga uma versão (só se não for a ativa de um canal) e, na hora, os arquivos dela que
/// ficaram sem uso. Margem de 1 h: um arquivo consultado por quem está publicando agora fica.
pub async fn delete_version(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Path((slug, id)): Path<(String, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    let server_id = crate::routes::servers::server_id_by_slug(&state.pool, &slug).await?;
    let channels: Vec<String> =
        sqlx::query_scalar("SELECT channel FROM server_channels WHERE version_id = $1 AND server_id = $2")
            .bind(id)
            .bind(server_id)
            .fetch_all(&state.pool)
            .await?;
    if !channels.is_empty() {
        return Err(ApiError::bad(format!(
            "Esta versão está ativa no canal {}. Peça ao servidor para publicar ou voltar para outra versão antes, ou suspenda o servidor.",
            channels.join(" e ")
        )));
    }
    let hashes: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT f ->> 'sha1' FROM pack_versions pv, jsonb_array_elements(pv.manifest -> 'files') f
          WHERE pv.id = $1 AND pv.server_id = $2 AND f ->> 'url' IS NULL",
    )
    .bind(id)
    .bind(server_id)
    .fetch_all(&state.pool)
    .await?;
    let version: Option<String> =
        sqlx::query_scalar("DELETE FROM pack_versions WHERE id = $1 AND server_id = $2 RETURNING version")
            .bind(id)
            .bind(server_id)
            .fetch_optional(&state.pool)
            .await?;
    let version = version.ok_or(ApiError::NotFound)?;
    let (files, bytes) = crate::cleanup::remove_if_unused(&state, &hashes, 1).await.map_err(ApiError::Internal)?;
    audit(
        &state.pool,
        Some(server_id),
        staff.map(|s| s.id),
        "storage.version_deleted",
        json!({ "version": version, "files_freed": files, "bytes_freed": bytes }),
    )
    .await?;
    Ok(Json(json!({ "deleted": version, "files_freed": files, "bytes_freed": bytes })))
}

#[derive(Deserialize)]
pub struct BlockInput {
    #[serde(default)]
    pub reason: String,
}

/// Derruba um arquivo: some do disco na hora e não pode ser enviado de novo.
/// As versões que o usam deixam de instalar até o servidor publicar sem ele.
pub async fn block_file(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Path(sha1): Path<String>,
    Json(input): Json<BlockInput>,
) -> ApiResult<Json<serde_json::Value>> {
    if !storage::is_sha1(&sha1) {
        return Err(ApiError::bad("SHA-1 inválido."));
    }
    let reason: String = input.reason.trim().chars().take(300).collect();
    let affected: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT DISTINCT s.id, s.slug, pv.version FROM pack_versions pv JOIN servers s ON s.id = pv.server_id
          WHERE pv.manifest -> 'files' @> jsonb_build_array(jsonb_build_object('sha1', $1::text))",
    )
    .bind(&sha1)
    .fetch_all(&state.pool)
    .await?;
    let path: String = sqlx::query_scalar(
        "SELECT f ->> 'path' FROM pack_versions pv, jsonb_array_elements(pv.manifest -> 'files') f
          WHERE f ->> 'sha1' = $1 ORDER BY pv.published_at DESC LIMIT 1",
    )
    .bind(&sha1)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or_default();

    let mut tx = state.pool.begin().await?;
    let size: Option<i64> = sqlx::query_scalar("DELETE FROM stored_files WHERE sha1 = $1 RETURNING size")
        .bind(&sha1)
        .fetch_optional(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO blocked_files (sha1, size, path, reason, blocked_by) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (sha1) DO UPDATE SET reason = EXCLUDED.reason",
    )
    .bind(&sha1)
    .bind(size.unwrap_or(0))
    .bind(&path)
    .bind(&reason)
    .bind(staff.as_ref().map(|s| s.id))
    .execute(&mut *tx)
    .await?;
    match tokio::fs::remove_file(storage::file_path(&state.config.storage_dir, &sha1)).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    tx.commit().await?;

    let versions: Vec<String> = affected.iter().map(|(_, slug, version)| format!("{slug} {version}")).collect();
    let servers: std::collections::BTreeSet<Uuid> = affected.iter().map(|(id, _, _)| *id).collect();
    let details = json!({ "sha1": sha1, "path": path, "size": size, "reason": reason });
    if servers.is_empty() {
        audit(&state.pool, None, staff.as_ref().map(|s| s.id), "storage.file_blocked", details.clone()).await?;
    }
    for server in servers {
        audit(&state.pool, Some(server), staff.as_ref().map(|s| s.id), "storage.file_blocked", details.clone()).await?;
    }
    Ok(Json(json!({ "sha1": sha1, "freed": size.unwrap_or(0), "affected_versions": versions })))
}

/// Libera o arquivo para ser enviado de novo (não restaura o conteúdo).
pub async fn unblock_file(
    State(state): State<AppState>,
    Staff(staff): Staff,
    Path(sha1): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let removed = sqlx::query("DELETE FROM blocked_files WHERE sha1 = $1").bind(&sha1).execute(&state.pool).await?;
    if removed.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    audit(&state.pool, None, staff.map(|s| s.id), "storage.file_unblocked", json!({ "sha1": sha1 })).await?;
    Ok(Json(json!({ "sha1": sha1 })))
}

/// Roda a limpeza diária agora (mesmas regras: 48 h de margem, recusados há 7 dias).
pub async fn run_cleanup(State(state): State<AppState>, Staff(staff): Staff) -> ApiResult<Json<serde_json::Value>> {
    let report = crate::cleanup::collect(&state).await.map_err(ApiError::Internal)?;
    audit(&state.pool, None, staff.map(|s| s.id), "storage.cleanup", json!({ "files": report.files, "bytes": report.bytes }))
        .await?;
    Ok(Json(json!({
        "versions": report.versions,
        "files": report.files,
        "bytes": report.bytes,
        "temporaries": report.temporaries,
    })))
}
