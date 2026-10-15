//! Versões de Minecraft e dos loaders, para os seletores do painel.
//!
//! A fonte é o mesmo servidor de metadados que o launcher (theseus) usa para
//! instalar: tudo o que aparece no seletor é instalável pelo launcher.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Path, State};
use conexao_types::{Loader, MinecraftVersion};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const TTL: Duration = Duration::from_secs(60 * 60);

#[derive(Default)]
pub struct MetaCache {
    entries: RwLock<BTreeMap<&'static str, (Instant, serde_json::Value)>>,
}

#[derive(Deserialize)]
struct LoaderManifest {
    #[serde(rename = "gameVersions")]
    game_versions: Vec<GameVersion>,
}

#[derive(Deserialize)]
struct GameVersion {
    id: String,
    #[serde(default)]
    loaders: Vec<LoaderVersion>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct LoaderVersion {
    pub id: String,
    #[serde(default)]
    pub stable: bool,
}

#[derive(Deserialize)]
struct MinecraftManifest {
    versions: Vec<MinecraftEntry>,
}

#[derive(Deserialize)]
struct MinecraftEntry {
    id: String,
    #[serde(rename = "type")]
    kind: String,
}

/// Placeholder do manifesto do Fabric: a lista de loaders vale para todas as versões.
const FABRIC_ANY: &str = "${modrinth.gameVersion}";

fn path_for(key: &str) -> &'static str {
    match key {
        "minecraft" => "minecraft/v0/manifest.json",
        "neoforge" => "neo/v0/manifest.json",
        "forge" => "forge/v0/manifest.json",
        _ => "fabric/v0/manifest.json",
    }
}

async fn fetch(state: &AppState, key: &'static str) -> ApiResult<serde_json::Value> {
    if let Some((at, value)) = state.meta.entries.read().await.get(key)
        && at.elapsed() < TTL
    {
        return Ok(value.clone());
    }
    let url = format!("{}/{}", state.config.launcher_meta_url, path_for(key));
    let fetched = async {
        let response = state.http.get(&url).timeout(Duration::from_secs(20)).send().await?.error_for_status()?;
        response.json::<serde_json::Value>().await
    }
    .await;
    match fetched {
        Ok(value) => {
            state.meta.entries.write().await.insert(key, (Instant::now(), value.clone()));
            Ok(value)
        }
        // Sem rede: usa a cópia antiga, se houver.
        Err(error) => match state.meta.entries.read().await.get(key) {
            Some((_, value)) => Ok(value.clone()),
            None => Err(ApiError::Internal(eyre::eyre!("metadados indisponíveis ({url}): {error}"))),
        },
    }
}

async fn loader_manifest(state: &AppState, loader: Loader) -> ApiResult<LoaderManifest> {
    let key = match loader {
        Loader::NeoForge => "neoforge",
        Loader::Forge => "forge",
        Loader::Fabric => "fabric",
    };
    serde_json::from_value(fetch(state, key).await?).map_err(|e| ApiError::Internal(e.into()))
}

#[derive(Serialize)]
pub struct MinecraftOption {
    pub id: String,
    pub loaders: Vec<Loader>,
}

/// Releases suportadas (1.18.2+), da mais nova para a mais antiga, com os loaders disponíveis.
pub async fn minecraft(State(state): State<AppState>) -> ApiResult<Json<Vec<MinecraftOption>>> {
    let manifest: MinecraftManifest =
        serde_json::from_value(fetch(&state, "minecraft").await?).map_err(|e| ApiError::Internal(e.into()))?;
    let mut by_loader: Vec<(Loader, Vec<String>)> = Vec::new();
    for loader in [Loader::NeoForge, Loader::Forge, Loader::Fabric] {
        let manifest = loader_manifest(&state, loader).await?;
        let ids = manifest
            .game_versions
            .into_iter()
            .filter(|g| loader == Loader::Fabric || !g.loaders.is_empty())
            .map(|g| g.id)
            .collect();
        by_loader.push((loader, ids));
    }
    let mut options: Vec<(MinecraftVersion, MinecraftOption)> = manifest
        .versions
        .into_iter()
        .filter(|v| v.kind == "release")
        .filter_map(|v| {
            let parsed = MinecraftVersion::parse_supported(&v.id).ok()?;
            let loaders: Vec<Loader> =
                by_loader.iter().filter(|(_, ids)| ids.contains(&v.id)).map(|(l, _)| *l).collect();
            (!loaders.is_empty()).then_some((parsed, MinecraftOption { id: v.id, loaders }))
        })
        .collect();
    options.sort_by_key(|o| std::cmp::Reverse(o.0));
    Ok(Json(options.into_iter().map(|(_, o)| o).collect()))
}

#[derive(Serialize)]
pub struct LoaderOption {
    pub id: String,
    pub stable: bool,
    /// Sugestão do painel: a versão estável mais nova (ou a mais nova, se nenhuma for estável).
    pub recommended: bool,
}

pub fn parse_loader_kind(value: &str) -> ApiResult<Loader> {
    match value {
        "neoforge" => Ok(Loader::NeoForge),
        "forge" => Ok(Loader::Forge),
        "fabric" => Ok(Loader::Fabric),
        _ => Err(ApiError::bad("Loader inválido (use neoforge, forge ou fabric).")),
    }
}

/// Versões do loader para uma versão do Minecraft, da mais nova para a mais antiga.
pub async fn loader_versions_for(state: &AppState, loader: Loader, minecraft: &str) -> ApiResult<Vec<LoaderOption>> {
    let manifest = loader_manifest(state, loader).await?;
    let versions: Vec<LoaderVersion> = match loader {
        Loader::Fabric => {
            let known = manifest.game_versions.iter().any(|g| g.id == minecraft);
            if !known {
                return Err(ApiError::NotFound);
            }
            manifest
                .game_versions
                .into_iter()
                .find(|g| g.id == FABRIC_ANY)
                .map(|g| g.loaders)
                .unwrap_or_default()
        }
        _ => manifest
            .game_versions
            .into_iter()
            .find(|g| g.id == minecraft)
            .map(|g| g.loaders)
            .ok_or(ApiError::NotFound)?,
    };
    let recommended = versions
        .iter()
        .position(|v| v.stable)
        .or_else(|| versions.iter().position(|v| !v.id.contains("beta") && !v.id.contains("alpha")))
        .unwrap_or(0);
    Ok(versions
        .into_iter()
        .enumerate()
        .map(|(i, v)| LoaderOption { id: v.id, stable: v.stable, recommended: i == recommended })
        .collect())
}

pub async fn loader_versions(
    State(state): State<AppState>,
    Path((loader, minecraft)): Path<(String, String)>,
) -> ApiResult<Json<Vec<LoaderOption>>> {
    let loader = parse_loader_kind(&loader)?;
    Ok(Json(loader_versions_for(&state, loader, &minecraft).await?))
}
