//! Downloads do Conexão Companion por versão do Minecraft e loader.
//!
//! O repositório `conexao-companion` gera um JAR por combinação (Fabric, Forge e NeoForge,
//! 1.18.2 até a mais nova) e um `manifest.json` dizendo qual JAR serve para qual versão.
//! O `deploy/atualizar.sh` copia tudo para `<storage>/launcher/companion/`; aqui a API só
//! lê o manifesto e entrega a lista (com CORS, para o painel) ou redireciona para o JAR certo.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::Redirect;
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanionBuild {
    /// Versão do Minecraft para a qual o JAR foi compilado.
    pub minecraft: String,
    /// Versões em que o JAR funciona (no Fabric pode ser uma faixa; nos Forges, só a exata).
    #[serde(default)]
    pub supports: Vec<String>,
    pub loader: String,
    pub java: u32,
    pub file: String,
    pub size: u64,
    pub sha1: String,
    /// Preenchido aqui: endereço público do JAR.
    #[serde(default, skip_deserializing)]
    pub url: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CompanionBuilds {
    /// Versão do Companion publicada (`None` se nada foi publicado ainda).
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub builds: Vec<CompanionBuild>,
}

fn safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.ends_with(".jar")
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
}

async fn load(state: &AppState) -> ApiResult<CompanionBuilds> {
    let path = state.config.storage_dir.join("launcher").join("companion").join("manifest.json");
    let text = match tokio::fs::read_to_string(&path).await {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(CompanionBuilds::default()),
        Err(e) => return Err(eyre::eyre!("lendo {}: {e}", path.display()).into()),
    };
    let mut manifest: CompanionBuilds =
        serde_json::from_str(&text).map_err(|e| eyre::eyre!("manifest.json do Companion inválido: {e}"))?;
    let base = state.config.public_url.trim_end_matches('/');
    manifest.builds.retain(|b| safe_file_name(&b.file));
    for build in &mut manifest.builds {
        if build.supports.is_empty() {
            build.supports.push(build.minecraft.clone());
        }
        build.url = format!("{base}/launcher/companion/{}", build.file);
    }
    Ok(manifest)
}

/// O JAR certo para um servidor: mesmo loader e, de preferência, a mesma versão de compilação.
pub fn pick<'a>(builds: &'a [CompanionBuild], minecraft: &str, loader: &str) -> Option<&'a CompanionBuild> {
    let same_loader = || builds.iter().filter(move |b| b.loader.eq_ignore_ascii_case(loader));
    same_loader()
        .find(|b| b.minecraft == minecraft)
        .or_else(|| same_loader().find(|b| b.supports.iter().any(|s| s == minecraft)))
}

/// `GET /v1/companion/builds`: todos os JARs publicados (público, sem login).
pub async fn builds(State(state): State<AppState>) -> ApiResult<Json<CompanionBuilds>> {
    Ok(Json(load(&state).await?))
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    pub minecraft: String,
    pub loader: String,
}

/// `GET /v1/companion/download?minecraft=1.20.1&loader=forge`: redireciona para o JAR certo.
/// Serve para links diretos (Discord, documentação, mensagem no console).
pub async fn download(State(state): State<AppState>, Query(query): Query<DownloadQuery>) -> ApiResult<Redirect> {
    let manifest = load(&state).await?;
    match pick(&manifest.builds, query.minecraft.trim(), query.loader.trim()) {
        Some(build) => Ok(Redirect::temporary(&build.url)),
        None => Err(ApiError::bad(format!(
            "Ainda não há Conexão Companion para {} {}.",
            query.loader.trim(),
            query.minecraft.trim()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(minecraft: &str, supports: &[&str], loader: &str) -> CompanionBuild {
        CompanionBuild {
            minecraft: minecraft.into(),
            supports: supports.iter().map(|s| s.to_string()).collect(),
            loader: loader.into(),
            java: 21,
            file: format!("conexao-companion-{loader}-{minecraft}.jar"),
            size: 1,
            sha1: String::new(),
            url: String::new(),
        }
    }

    #[test]
    fn picks_exact_then_range() {
        let builds = vec![
            build("1.21.1", &["1.21", "1.21.1"], "fabric"),
            build("1.21.1", &["1.21.1"], "neoforge"),
            build("1.20.1", &["1.20", "1.20.1"], "forge"),
        ];
        assert_eq!(pick(&builds, "1.21.1", "neoforge").unwrap().loader, "neoforge");
        assert_eq!(pick(&builds, "1.21", "fabric").unwrap().minecraft, "1.21.1");
        assert_eq!(pick(&builds, "1.20.1", "Forge").unwrap().minecraft, "1.20.1");
        assert!(pick(&builds, "1.21", "neoforge").is_none());
        assert!(pick(&builds, "1.19.2", "forge").is_none());
    }

    #[test]
    fn rejects_paths() {
        assert!(safe_file_name("conexao-companion-fabric-1.21.1.jar"));
        assert!(!safe_file_name("../segredo.jar"));
        assert!(!safe_file_name("a/b.jar"));
        assert!(!safe_file_name("x.exe"));
    }
}
