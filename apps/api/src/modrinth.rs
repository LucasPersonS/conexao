//! Reconhecimento de arquivos pelo hash no Modrinth.
//!
//! Mods (e resource/shader packs) publicados no Modrinth viram links para o
//! CDN deles: o Conexão não re-hospeda o arquivo, os jogadores baixam direto
//! de lá, e ainda sabemos nome, ícone e se o mod é só do cliente ou do servidor.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::ApiResult;
use crate::state::AppState;

const BATCH: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedFile {
    pub sha1: String,
    pub sha512: String,
    pub size: u64,
    pub url: String,
    pub filename: String,
    pub project_id: String,
    pub project_slug: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub version_number: String,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    /// `required`, `optional`, `unsupported` ou `unknown` (padrão do Modrinth).
    pub client_side: String,
    pub server_side: String,
}

impl ResolvedFile {
    /// Mod que não roda no cliente: fica fora do modpack dos jogadores.
    pub fn server_only(&self) -> bool {
        self.client_side == "unsupported"
    }

    pub fn client_only(&self) -> bool {
        self.server_side == "unsupported" && self.client_side != "unsupported"
    }
}

#[derive(Deserialize)]
struct MrVersion {
    project_id: String,
    version_number: String,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    game_versions: Vec<String>,
    files: Vec<MrFile>,
}

#[derive(Deserialize)]
struct MrFile {
    hashes: HashMap<String, String>,
    url: String,
    filename: String,
    size: u64,
}

#[derive(Deserialize)]
struct MrProject {
    id: String,
    slug: String,
    title: String,
    icon_url: Option<String>,
    #[serde(default)]
    client_side: String,
    #[serde(default)]
    server_side: String,
}

async fn lookup(state: &AppState, hashes: &[String]) -> eyre::Result<HashMap<String, ResolvedFile>> {
    let api = &state.config.modrinth_api_url;
    let mut versions: HashMap<String, MrVersion> = HashMap::new();
    for chunk in hashes.chunks(BATCH) {
        let found: HashMap<String, MrVersion> = state
            .http
            .post(format!("{api}/version_files"))
            .timeout(Duration::from_secs(30))
            .json(&serde_json::json!({ "hashes": chunk, "algorithm": "sha1" }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        versions.extend(found);
    }
    let project_ids: Vec<String> =
        versions.values().map(|v| v.project_id.clone()).collect::<HashSet<_>>().into_iter().collect();
    let mut projects: HashMap<String, MrProject> = HashMap::new();
    for chunk in project_ids.chunks(100) {
        let ids = serde_json::to_string(chunk)?;
        let found: Vec<MrProject> = state
            .http
            .get(format!("{api}/projects"))
            .query(&[("ids", ids)])
            .timeout(Duration::from_secs(30))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        projects.extend(found.into_iter().map(|p| (p.id.clone(), p)));
    }
    let mut resolved = HashMap::new();
    for (sha1, version) in versions {
        let Some(file) = version.files.iter().find(|f| f.hashes.get("sha1") == Some(&sha1)) else { continue };
        let Some(sha512) = file.hashes.get("sha512").filter(|h| h.len() == 128) else { continue };
        if !file.url.starts_with("https://") {
            continue;
        }
        let project = projects.get(&version.project_id);
        resolved.insert(
            sha1.clone(),
            ResolvedFile {
                sha1: sha1.clone(),
                sha512: sha512.clone(),
                size: file.size,
                url: file.url.clone(),
                filename: file.filename.clone(),
                project_id: version.project_id.clone(),
                project_slug: project.map(|p| p.slug.clone()).unwrap_or_default(),
                title: project.map(|p| p.title.clone()).unwrap_or_else(|| file.filename.clone()),
                icon_url: project.and_then(|p| p.icon_url.clone()),
                version_number: version.version_number.clone(),
                loaders: version.loaders.clone(),
                game_versions: version.game_versions.clone(),
                client_side: project.map(|p| p.client_side.clone()).unwrap_or_else(|| "unknown".into()),
                server_side: project.map(|p| p.server_side.clone()).unwrap_or_else(|| "unknown".into()),
            },
        );
    }
    Ok(resolved)
}

/// Resolve os SHA-1 no Modrinth, com cache no banco. Falhas de rede não
/// impedem a publicação: os arquivos só deixam de ser reconhecidos.
pub async fn resolve(state: &AppState, hashes: &[String]) -> ApiResult<HashMap<String, ResolvedFile>> {
    let unique: Vec<String> = hashes.iter().cloned().collect::<HashSet<_>>().into_iter().collect();
    if unique.is_empty() {
        return Ok(HashMap::new());
    }
    let cached: Vec<(String, bool, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT sha1, found, data FROM modrinth_files
          WHERE sha1 = ANY($1)
            AND checked_at > now() - CASE WHEN found THEN interval '7 days' ELSE interval '1 day' END",
    )
    .bind(&unique)
    .fetch_all(&state.pool)
    .await?;
    let mut result = HashMap::new();
    let mut known = HashSet::new();
    for (sha1, found, data) in cached {
        known.insert(sha1.clone());
        if found && let Some(file) = data.and_then(|d| serde_json::from_value::<ResolvedFile>(d).ok()) {
            result.insert(sha1, file);
        }
    }
    let pending: Vec<String> = unique.into_iter().filter(|h| !known.contains(h)).collect();
    if pending.is_empty() {
        return Ok(result);
    }
    match lookup(state, &pending).await {
        Ok(found) => {
            for sha1 in &pending {
                let data = found.get(sha1).map(serde_json::to_value).transpose().map_err(|e| eyre::eyre!(e))?;
                sqlx::query(
                    "INSERT INTO modrinth_files (sha1, found, data, checked_at) VALUES ($1, $2, $3, now())
                     ON CONFLICT (sha1) DO UPDATE SET found = EXCLUDED.found, data = EXCLUDED.data, checked_at = now()",
                )
                .bind(sha1)
                .bind(data.is_some())
                .bind(data)
                .execute(&state.pool)
                .await?;
            }
            result.extend(found);
        }
        Err(error) => tracing::warn!("Modrinth indisponível, seguindo sem reconhecer {} arquivo(s): {error}", pending.len()),
    }
    Ok(result)
}
