//! Análise de um modpack enviado pelo painel web (ou fotografado pelo Companion).
//!
//! O navegador calcula o SHA-1 de cada arquivo e chama esta rota com a lista
//! `caminho → sha1`. A API:
//! 1. reconhece no Modrinth o que estiver publicado lá (vira link, não é hospedado);
//! 2. tira do pacote os mods que só rodam no servidor;
//! 3. exige no storage apenas o resto (configs, scripts, mods próprios), dentro da cota;
//! 4. roda as mesmas regras do `conexao-publish` (crate `conexao-pack`) e devolve o
//!    manifesto, o diff com a versão estável e os avisos. Publicar é `POST /versions`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use axum::Json;
use axum::extract::{Path as UrlPath, State};
use conexao_pack::{ChangeKind, FileHashes, Group, ModInfo, PublishOptions, Scan, ScannedFile};
use conexao_types::{FilePolicy, FileSide, Loader, LoaderSpec, MinecraftVersion, PackManifest};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthUser, Role, require_role};
use crate::error::{ApiError, ApiResult};
use crate::modrinth::{self, ResolvedFile};
use crate::policy;
use crate::routes::servers::{parse_loader, server_id_by_slug};
use crate::state::AppState;
use crate::storage;

const MAX_FILES: usize = 20_000;

#[derive(Deserialize, Clone)]
pub struct PlanFile {
    pub path: String,
    pub sha1: String,
    /// Download externo declarado pelo próprio pacote (`.mrpack`).
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha512: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Deserialize)]
pub struct PlanInput {
    pub version: String,
    pub minecraft: Option<String>,
    pub loader: Option<LoaderSpec>,
    pub files: Vec<PlanFile>,
    /// Configs que devem sempre sobrescrever a cópia do jogador.
    #[serde(default)]
    pub managed: Vec<String>,
    #[serde(default)]
    pub excluded: Vec<String>,
    /// Mods marcados no Modrinth como "só servidor" que o admin quer mandar mesmo assim.
    #[serde(default)]
    pub include_server_only: Vec<String>,
}

#[derive(Serialize)]
pub struct PlanChange {
    pub kind: &'static str,
    pub path: String,
    pub name: String,
}

#[derive(Serialize, Clone)]
pub struct PlanFileInfo {
    pub path: String,
    /// `modrinth` (link do Modrinth), `external` (link declarado pelo pacote) ou `hosted` (no Conexão).
    pub source: &'static str,
    pub size: u64,
    pub title: Option<String>,
    pub icon_url: Option<String>,
    pub project_slug: Option<String>,
    pub side: FileSide,
}

#[derive(Serialize)]
pub struct PlanResponse {
    pub manifest: PackManifest,
    pub previous_version: Option<String>,
    pub changes: Vec<PlanChange>,
    pub warnings: Vec<String>,
    pub files: Vec<PlanFileInfo>,
    /// Mods só do servidor que ficaram de fora do pacote dos jogadores.
    pub server_only: Vec<PlanFileInfo>,
    pub hosted_bytes: u64,
    pub quota_bytes: u64,
}

pub fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 400
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.chars().any(|c| c.is_control() || c == ':')
        && path.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}

/// Monta a pasta temporária com hardlinks (ou cópias) dos arquivos do storage.
fn materialize(storage_root: &Path, target: &Path, files: &[(String, String)]) -> std::io::Result<()> {
    for (path, sha1) in files {
        let destination = target.join(path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let source = storage::file_path(storage_root, sha1);
        if std::fs::hard_link(&source, &destination).is_err() {
            std::fs::copy(&source, &destination)?;
        }
    }
    Ok(())
}

fn loader_from_modrinth(name: &str) -> Option<Loader> {
    match name {
        "neoforge" => Some(Loader::NeoForge),
        "forge" => Some(Loader::Forge),
        "fabric" | "quilt" => Some(Loader::Fabric),
        _ => None,
    }
}

/// Arquivo que não precisa estar no disco da API: os dados vêm do Modrinth ou do `.mrpack`.
fn virtual_file(path: &str, group: Group, hashes: FileHashes, resolved: Option<&ResolvedFile>) -> ScannedFile {
    let mod_info = resolved.filter(|_| group == Group::Mods).and_then(|r| {
        let supports: Vec<Loader> = r.loaders.iter().filter_map(|l| loader_from_modrinth(l)).collect();
        (!supports.is_empty()).then(|| ModInfo {
            mod_ids: vec![r.project_slug.clone()],
            name: r.title.clone(),
            version: r.version_number.clone(),
            loader: None,
            supports,
        })
    });
    ScannedFile {
        relative: path.to_string(),
        absolute: PathBuf::from(path),
        group,
        hashes,
        mod_info,
        warnings: Vec::new(),
    }
}

/// Núcleo da análise, usado pelo painel e pelo "atualizar a partir do servidor".
/// `only` restringe as pastas analisadas; as demais continuam como na versão estável.
pub async fn build(
    state: &AppState,
    server_id: Uuid,
    slug: &str,
    input: PlanInput,
    only: Option<&[Group]>,
) -> ApiResult<PlanResponse> {
    if input.files.is_empty() {
        return Err(ApiError::bad("Nenhum arquivo do modpack foi enviado."));
    }
    if input.files.len() > MAX_FILES {
        return Err(ApiError::bad(format!("O modpack passa de {MAX_FILES} arquivos.")));
    }
    let mut seen = HashSet::new();
    let mut files: Vec<(PlanFile, Group)> = Vec::with_capacity(input.files.len());
    for file in input.files {
        if !safe_relative(&file.path) {
            return Err(ApiError::bad(format!("Caminho inválido: {}", file.path)));
        }
        let top = file.path.split('/').next().unwrap_or_default();
        let Some(group) = Group::from_folder(top) else {
            continue; // pastas que o Conexão não gerencia (saves, logs...) são ignoradas
        };
        if only.is_some_and(|groups| !groups.contains(&group)) {
            continue;
        }
        policy::check_path(&file.path).map_err(ApiError::bad)?;
        if !storage::is_sha1(&file.sha1) {
            return Err(ApiError::bad(format!("SHA-1 inválido para {}", file.path)));
        }
        if !seen.insert(file.path.to_lowercase()) {
            return Err(ApiError::bad(format!("Nomes que colidem no Windows: {}", file.path)));
        }
        files.push((file, group));
    }
    if files.is_empty() {
        return Err(ApiError::bad(
            "Nenhuma pasta reconhecida (mods, config, defaultconfigs, resourcepacks, shaderpacks, kubejs).",
        ));
    }

    // 1. Modrinth
    let hashes: Vec<String> = files.iter().map(|(f, _)| f.sha1.clone()).collect();
    let resolved = modrinth::resolve(state, &hashes).await?;

    // 2. Classifica cada arquivo
    let include_server_only: HashSet<&str> = input.include_server_only.iter().map(String::as_str).collect();
    let mut virtual_files = Vec::new();
    let mut local: Vec<(String, String)> = Vec::new();
    let mut external_urls: HashMap<String, String> = HashMap::new();
    let mut sides: HashMap<String, FileSide> = HashMap::new();
    let mut infos: HashMap<String, PlanFileInfo> = HashMap::new();
    let mut server_only = Vec::new();
    for (file, group) in &files {
        let found = resolved.get(&file.sha1);
        let side = match found {
            Some(r) if r.client_only() => FileSide::Client,
            _ => FileSide::Both,
        };
        let info = |source: &'static str, size: u64| PlanFileInfo {
            path: file.path.clone(),
            source,
            size,
            title: found.map(|r| r.title.clone()),
            icon_url: found.and_then(|r| r.icon_url.clone()),
            project_slug: found.map(|r| r.project_slug.clone()),
            side,
        };
        if let Some(r) = found
            && r.server_only()
            && !include_server_only.contains(file.path.as_str())
        {
            server_only.push(info("modrinth", r.size));
            continue;
        }
        sides.insert(file.path.clone(), side);
        if let Some(r) = found {
            external_urls.insert(file.sha1.clone(), r.url.clone());
            infos.insert(file.path.clone(), info("modrinth", r.size));
            let hashes = FileHashes { sha1: r.sha1.clone(), sha512: r.sha512.clone(), size: r.size };
            virtual_files.push(virtual_file(&file.path, *group, hashes, Some(r)));
        } else if let Some(url) = &file.url {
            if !policy::external_url_allowed(url) {
                return Err(ApiError::bad(format!(
                    "{}: link externo não permitido. Aceitos: {}.",
                    file.path,
                    policy::EXTERNAL_HOSTS.join(", ")
                )));
            }
            let (Some(sha512), Some(size)) = (file.sha512.clone(), file.size) else {
                return Err(ApiError::bad(format!("{}: informe sha512 e tamanho do link externo.", file.path)));
            };
            if sha512.len() != 128 || !sha512.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(ApiError::bad(format!("{}: SHA-512 inválido.", file.path)));
            }
            external_urls.insert(file.sha1.clone(), url.clone());
            infos.insert(file.path.clone(), info("external", size));
            let hashes = FileHashes { sha1: file.sha1.clone(), sha512, size };
            virtual_files.push(virtual_file(&file.path, *group, hashes, None));
        } else {
            local.push((file.path.clone(), file.sha1.clone()));
        }
    }

    // 3. O resto precisa estar no storage
    let local_hashes: Vec<String> = local.iter().map(|(_, h)| h.clone()).collect();
    let stored: HashMap<String, i64> =
        sqlx::query_as::<_, (String, i64)>(
            "UPDATE stored_files SET last_used_at = now() WHERE sha1 = ANY($1) RETURNING sha1, size",
        )
            .bind(&local_hashes)
            .fetch_all(&state.pool)
            .await?
            .into_iter()
            .collect();
    let mut missing: Vec<&String> = local_hashes.iter().filter(|h| !stored.contains_key(*h)).collect();
    missing.sort();
    missing.dedup();
    if !missing.is_empty() {
        return Err(ApiError::Conflict {
            message: format!("{} arquivo(s) ainda não foram enviados.", missing.len()),
            details: json!({ "missing": missing }),
        });
    }
    let quota = policy::quota_for(state, server_id).await?;
    let mut unique = HashSet::new();
    let hosted: u64 = local
        .iter()
        .filter(|(_, h)| unique.insert(h.clone()))
        .map(|(_, h)| u64::try_from(stored.get(h).copied().unwrap_or(0)).unwrap_or(0))
        .sum();
    if hosted > quota {
        return Err(ApiError::bad(format!(
            "Os arquivos que não estão no Modrinth somam {}, acima da cota de {} do servidor. \
             Tire arquivos grandes (mods próprios, resource packs) ou publique-os no Modrinth.",
            policy::megabytes(hosted),
            policy::megabytes(quota)
        )));
    }
    for (path, sha1) in &local {
        let size = u64::try_from(stored.get(sha1).copied().unwrap_or(0)).unwrap_or(0);
        infos.insert(
            path.clone(),
            PlanFileInfo {
                path: path.clone(),
                source: "hosted",
                size,
                title: None,
                icon_url: None,
                project_slug: None,
                side: FileSide::Both,
            },
        );
    }

    // 4. Regras do publicador
    let previous: Option<PackManifest> = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT pv.manifest FROM server_channels sc JOIN pack_versions pv ON pv.id = sc.version_id
          WHERE sc.server_id = $1 AND sc.channel = 'stable'",
    )
    .bind(server_id)
    .fetch_optional(&state.pool)
    .await?
    .and_then(|value| serde_json::from_value(value).ok());
    let (name, mc, loader, loader_version): (String, String, String, String) = sqlx::query_as(
        "SELECT name, minecraft_version, loader, loader_version FROM servers WHERE id = $1",
    )
    .bind(server_id)
    .fetch_one(&state.pool)
    .await?;

    let minecraft = match &input.minecraft {
        Some(value) => MinecraftVersion::parse_supported(value).map_err(|e| ApiError::bad(e.to_string()))?,
        None => previous
            .as_ref()
            .map(|p| p.minecraft)
            .or_else(|| mc.parse().ok())
            .unwrap_or(conexao_types::MIN_SUPPORTED_MINECRAFT),
    };
    let loader = input.loader.clone().unwrap_or_else(|| {
        previous
            .as_ref()
            .map(|p| p.loader.clone())
            .unwrap_or(LoaderSpec { kind: parse_loader(&loader), version: loader_version })
    });
    let options = PublishOptions {
        server: slug.to_string(),
        name,
        version: input.version.trim().to_string(),
        minecraft,
        loader,
        excluded: input.excluded.into_iter().collect(),
        policies: input.managed.into_iter().map(|p| (p, FilePolicy::Managed)).collect::<HashMap<_, _>>(),
        sides,
        external_urls,
    };

    let groups: Vec<Group> = match only {
        Some(groups) => groups.to_vec(),
        None => {
            let mut groups: Vec<Group> = files.iter().map(|(_, g)| *g).collect();
            groups.sort();
            groups.dedup();
            groups
        }
    };
    let storage_root = state.config.storage_dir.clone();
    let work: PathBuf = storage_root.join("tmp").join(format!("plan-{}", Uuid::new_v4()));
    let previous_for_plan = previous.clone();
    let result = tokio::task::spawn_blocking(move || {
        let outcome = (|| {
            let mut scanned = if local.is_empty() {
                Scan { root: work.clone(), groups: groups.clone(), files: Vec::new() }
            } else {
                materialize(&storage_root, &work, &local).map_err(|e| ApiError::Internal(e.into()))?;
                // Só as pastas que existem no disco; o resto veio do Modrinth/`.mrpack`.
                let mut local_groups: Vec<Group> = local
                    .iter()
                    .filter_map(|(p, _)| Group::from_folder(p.split('/').next().unwrap_or_default()))
                    .collect();
                local_groups.sort();
                local_groups.dedup();
                conexao_pack::scan(&work, &local_groups, |_| {}).map_err(|e| ApiError::bad(e.to_string()))?
            };
            scanned.groups = groups;
            scanned.files.extend(virtual_files);
            scanned.files.sort_by(|a, b| a.relative.cmp(&b.relative));
            conexao_pack::build_plan(previous_for_plan.as_ref(), &scanned, &options)
                .map_err(|e| ApiError::bad(e.to_string()))
        })();
        let _ = std::fs::remove_dir_all(&work);
        outcome
    })
    .await
    .map_err(|e| ApiError::Internal(e.into()))??;

    let mut warnings = result.warnings;
    if !server_only.is_empty() {
        warnings.insert(
            0,
            format!(
                "{} mod(s) só do servidor ficaram de fora do pacote dos jogadores: {}.",
                server_only.len(),
                server_only.iter().map(|f| f.title.clone().unwrap_or_else(|| f.path.clone())).collect::<Vec<_>>().join(", ")
            ),
        );
    }
    let mut file_infos: Vec<PlanFileInfo> = result
        .manifest
        .files
        .iter()
        .filter_map(|f| infos.get(&f.path).cloned())
        .collect();
    file_infos.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(PlanResponse {
        previous_version: previous.map(|p| p.version),
        changes: result
            .changes
            .into_iter()
            .map(|c| PlanChange {
                kind: match c.kind {
                    ChangeKind::Added => "added",
                    ChangeKind::Changed => "changed",
                    ChangeKind::Unchanged => "unchanged",
                    ChangeKind::Removed => "removed",
                },
                path: c.path,
                name: c.name,
            })
            .collect(),
        warnings,
        hosted_bytes: policy::hosted_bytes(&result.manifest.files),
        quota_bytes: quota,
        files: file_infos,
        server_only,
        manifest: result.manifest,
    })
}

pub async fn plan(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    UrlPath(slug): UrlPath<String>,
    Json(input): Json<PlanInput>,
) -> ApiResult<Json<PlanResponse>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Admin).await?;
    Ok(Json(build(&state, server_id, &slug, input, None).await?))
}

#[cfg(test)]
mod tests {
    use super::safe_relative;

    #[test]
    fn rejects_unsafe_paths() {
        assert!(safe_relative("mods/create.jar"));
        assert!(!safe_relative("../etc/passwd"));
        assert!(!safe_relative("mods//x.jar"));
        assert!(!safe_relative("C:/x"));
        assert!(!safe_relative("/abs"));
    }
}
