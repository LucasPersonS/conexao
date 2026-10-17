use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use conexao_types::{Channel, PackManifest, PackVersionInfo};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::{AuthUser, MaybeUser, Role, audit, require_role};
use crate::error::{ApiError, ApiResult};
use crate::routes::servers::{loader_str, server_id_by_slug};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ChannelQuery {
    #[serde(default)]
    pub channel: Channel,
}

/// Manifesto da versão ativa no canal. O launcher usa o `ETag` para saber se mudou.
pub async fn manifest(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path(slug): Path<String>,
    Query(query): Query<ChannelQuery>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    // Lista de espera: o modpack só sai para contas liberadas (o launcher manda o token).
    crate::routes::waitlist::require_session_access(&state, user.as_ref())?;
    let (server_id, approval): (Uuid, String) =
        sqlx::query_as("SELECT id, approval FROM servers WHERE slug = $1")
            .bind(&slug)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(ApiError::NotFound)?;
    if approval != "approved" {
        let user = user.ok_or(ApiError::NotFound)?;
        require_role(&state.pool, server_id, &user, Role::Moderator)
            .await
            .map_err(|_| ApiError::NotFound)?;
    }
    let (manifest, sha): (serde_json::Value, String) = sqlx::query_as(
        "SELECT pv.manifest, pv.manifest_sha256
           FROM server_channels sc JOIN pack_versions pv ON pv.id = sc.version_id
          WHERE sc.server_id = $1 AND sc.channel = $2",
    )
    .bind(server_id)
    .bind(query.channel.as_str())
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;

    let etag = format!("\"{sha}\"");
    if headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(etag.as_str()) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let body = serde_json::to_vec(&manifest).map_err(|e| ApiError::Internal(e.into()))?;
    let signature = state.config.signing_key.as_ref().map(|key| conexao_types::signing::sign(key, &body));
    let mut response = ([(header::CONTENT_TYPE, "application/json")], body).into_response();
    if let Ok(value) = HeaderValue::from_str(&etag) {
        response.headers_mut().insert(header::ETAG, value);
    }
    if let Some(signature) = signature.and_then(|s| HeaderValue::from_str(&s).ok()) {
        response.headers_mut().insert(conexao_types::signing::SIGNATURE_HEADER, signature);
    }
    Ok(response)
}

#[derive(Serialize)]
pub struct VersionListItem {
    #[serde(flatten)]
    pub info: PackVersionInfo,
    pub channels: Vec<Channel>,
    pub file_count: usize,
}

#[derive(sqlx::FromRow)]
struct VersionRow {
    id: Uuid,
    version: String,
    manifest: serde_json::Value,
    manifest_sha256: String,
    published_by: Option<Uuid>,
    published_at: DateTime<Utc>,
    channels: Vec<String>,
}

pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<Vec<VersionListItem>>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Moderator).await?;
    let rows = sqlx::query_as::<_, VersionRow>(
        "SELECT pv.id, pv.version, pv.manifest, pv.manifest_sha256, pv.published_by, pv.published_at,
                COALESCE(array_agg(sc.channel) FILTER (WHERE sc.channel IS NOT NULL), '{}') AS channels
           FROM pack_versions pv
           LEFT JOIN server_channels sc ON sc.version_id = pv.id
          WHERE pv.server_id = $1
          GROUP BY pv.id ORDER BY pv.published_at DESC",
    )
    .bind(server_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| {
                let channels: Vec<Channel> = row
                    .channels
                    .iter()
                    .map(|c| if c == "beta" { Channel::Beta } else { Channel::Stable })
                    .collect();
                VersionListItem {
                    file_count: row.manifest["files"].as_array().map_or(0, Vec::len),
                    info: PackVersionInfo {
                        id: row.id,
                        version: row.version,
                        channel: channels.first().copied().unwrap_or_default(),
                        manifest_sha256: row.manifest_sha256,
                        published_by: row.published_by,
                        published_at: row.published_at,
                    },
                    channels,
                }
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct PublishInput {
    pub manifest: PackManifest,
    /// Canal que passa a apontar para esta versão. `null` = só registra.
    pub channel: Option<Channel>,
}

/// Registra uma nova versão. Todos os arquivos sem URL externa precisam já
/// estar no storage (o publicador sobe com `PUT /v1/files/{sha1}` antes).
pub async fn publish(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Json(input): Json<PublishInput>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Admin).await?;
    let manifest = input.manifest;

    if manifest.server != slug {
        return Err(ApiError::bad("O manifesto pertence a outro servidor."));
    }
    if manifest.format != conexao_types::manifest::MANIFEST_FORMAT {
        return Err(ApiError::bad("Formato de manifesto não suportado; atualize o publicador."));
    }
    if !manifest.minecraft.is_supported() {
        return Err(ApiError::bad("O Conexão suporta apenas Minecraft 1.18.2 ou mais novo."));
    }
    let version = semver::Version::parse(&manifest.version)
        .map_err(|_| ApiError::bad("Versão inválida; use semver, como 1.2.0."))?;
    let existing: Vec<String> = sqlx::query_scalar("SELECT version FROM pack_versions WHERE server_id = $1")
        .bind(server_id)
        .fetch_all(&state.pool)
        .await?;
    if let Some(latest) = existing.iter().filter_map(|v| semver::Version::parse(v).ok()).max()
        && version <= latest
    {
        return Err(ApiError::bad(format!("Escolha uma versão maior que {latest}.")));
    }

    let mut paths = std::collections::HashSet::new();
    let mut hosted = Vec::new();
    for file in &manifest.files {
        let path = file.path.as_str();
        if path.is_empty()
            || path.starts_with('/')
            || path.contains('\\')
            || path.split('/').any(|p| p.is_empty() || p == "." || p == "..")
            || path.chars().any(|c| c.is_control() || c == ':')
        {
            return Err(ApiError::bad(format!("Caminho inseguro no manifesto: {path}")));
        }
        if !paths.insert(path.to_lowercase()) {
            return Err(ApiError::bad(format!("Caminho duplicado no manifesto: {path}")));
        }
        if file.sha512.len() != 128 || !crate::storage::is_sha1(&file.sha1) {
            return Err(ApiError::bad(format!("Hashes inválidos para {path}.")));
        }
        crate::policy::check_path(path).map_err(ApiError::bad)?;
        match &file.url {
            Some(url) if !crate::policy::external_url_allowed(url) => {
                return Err(ApiError::bad(format!(
                    "Link externo não permitido para {path}. Aceitos: {}.",
                    crate::policy::EXTERNAL_HOSTS.join(", ")
                )));
            }
            Some(_) => {}
            None => hosted.push(file.sha1.clone()),
        }
    }
    let quota = crate::policy::quota_for(&state, server_id).await?;
    // Tamanho real do storage (o manifesto vem do cliente).
    let hosted_bytes: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(size), 0)::BIGINT FROM stored_files WHERE sha1 = ANY($1)")
            .bind(&hosted)
            .fetch_one(&state.pool)
            .await?;
    let hosted_bytes = u64::try_from(hosted_bytes).unwrap_or(0);
    if hosted_bytes > quota {
        return Err(ApiError::bad(format!(
            "Arquivos hospedados no Conexão somam {}, acima da cota de {}.",
            crate::policy::megabytes(hosted_bytes),
            crate::policy::megabytes(quota)
        )));
    }
    // Servidor em análise: todas as versões juntas cabem na cota (não dá para ir
    // acumulando versões enquanto a equipe não aprova).
    let approval: String = sqlx::query_scalar("SELECT approval FROM servers WHERE id = $1")
        .bind(server_id)
        .fetch_one(&state.pool)
        .await?;
    if approval != "approved" {
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(sf.size), 0)::BIGINT FROM stored_files sf
              WHERE sf.sha1 = ANY($2)
                 OR EXISTS (SELECT 1 FROM pack_versions pv
                             WHERE pv.server_id = $1
                               AND pv.manifest -> 'files' @> jsonb_build_array(jsonb_build_object('sha1', sf.sha1)))",
        )
        .bind(server_id)
        .bind(&hosted)
        .fetch_one(&state.pool)
        .await?;
        let total = u64::try_from(total).unwrap_or(0);
        if total > quota {
            return Err(ApiError::bad(format!(
                "Enquanto o servidor está em análise, todas as versões juntas podem hospedar até {} (com esta, {}). Espere a aprovação para publicar mais.",
                crate::policy::megabytes(quota),
                crate::policy::megabytes(total)
            )));
        }
    }
    // Conferir conta como uso: a limpeza diária não apaga estes arquivos no meio da publicação.
    let present: Vec<String> =
        sqlx::query_scalar("UPDATE stored_files SET last_used_at = now() WHERE sha1 = ANY($1) RETURNING sha1")
        .bind(&hosted)
        .fetch_all(&state.pool)
        .await?;
    let missing: Vec<&String> = hosted.iter().filter(|h| !present.contains(h)).collect();
    if !missing.is_empty() {
        return Err(ApiError::Conflict {
            message: format!("{} arquivo(s) ainda não foram enviados.", missing.len()),
            details: json!({ "missing": missing }),
        });
    }

    let manifest_json = serde_json::to_value(&manifest).map_err(|e| ApiError::Internal(e.into()))?;
    let sha = hex::encode(Sha256::digest(manifest_json.to_string().as_bytes()));
    let version_id = Uuid::new_v4();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO pack_versions (id, server_id, version, manifest, manifest_sha256, published_by)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(version_id)
    .bind(server_id)
    .bind(&manifest.version)
    .bind(&manifest_json)
    .bind(&sha)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    if let Some(channel) = input.channel {
        point_channel(&mut tx, server_id, channel, version_id).await?;
        if channel == Channel::Stable {
            // A home mostra versão e loader da versão estável.
            sqlx::query("UPDATE servers SET minecraft_version = $2, loader = $3, loader_version = $4 WHERE id = $1")
                .bind(server_id)
                .bind(manifest.minecraft.to_string())
                .bind(loader_str(manifest.loader.kind))
                .bind(&manifest.loader.version)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    audit(
        &state.pool,
        Some(server_id),
        Some(user.id),
        "version.published",
        json!({ "version": manifest.version, "channel": input.channel, "files": manifest.files.len() }),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": version_id, "version": manifest.version, "manifest_sha256": sha }))))
}

async fn point_channel(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    server_id: Uuid,
    channel: Channel,
    version_id: Uuid,
) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO server_channels (server_id, channel, version_id) VALUES ($1, $2, $3)
         ON CONFLICT (server_id, channel) DO UPDATE SET version_id = EXCLUDED.version_id, updated_at = now()",
    )
    .bind(server_id)
    .bind(channel.as_str())
    .bind(version_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct SetChannelInput {
    pub version: String,
}

/// Promove ou faz rollback: aponta o canal para uma versão já publicada.
pub async fn set_channel(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((slug, channel)): Path<(String, String)>,
    Json(input): Json<SetChannelInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let channel = match channel.as_str() {
        "stable" => Channel::Stable,
        "beta" => Channel::Beta,
        _ => return Err(ApiError::NotFound),
    };
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Admin).await?;
    let (version_id, manifest): (Uuid, serde_json::Value) =
        sqlx::query_as("SELECT id, manifest FROM pack_versions WHERE server_id = $1 AND version = $2")
            .bind(server_id)
            .bind(&input.version)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::bad(format!("A versão {} não existe.", input.version)))?;
    let mut tx = state.pool.begin().await?;
    point_channel(&mut tx, server_id, channel, version_id).await?;
    if channel == Channel::Stable
        && let Ok(manifest) = serde_json::from_value::<PackManifest>(manifest)
    {
        sqlx::query("UPDATE servers SET minecraft_version = $2, loader = $3, loader_version = $4 WHERE id = $1")
            .bind(server_id)
            .bind(manifest.minecraft.to_string())
            .bind(loader_str(manifest.loader.kind))
            .bind(&manifest.loader.version)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    audit(
        &state.pool,
        Some(server_id),
        Some(user.id),
        "channel.set",
        json!({ "channel": channel, "version": input.version }),
    )
    .await?;
    Ok(Json(json!({ "channel": channel, "version": input.version })))
}

/// Chave pública usada pelo launcher para conferir os manifestos.
pub async fn signing_key(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "public_key": state.config.signing_key.as_ref().map(conexao_types::signing::public_key_b64),
        "algorithm": "ed25519",
        "header": conexao_types::signing::SIGNATURE_HEADER,
    }))
}
