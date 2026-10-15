//! Atualização diferencial do modpack.
//!
//! Quando o servidor publica uma versão nova com o mesmo Minecraft e o mesmo loader,
//! o launcher compara o manifesto novo com o último aplicado e só baixa o que mudou,
//! direto do CDN, sem refazer a instalação inteira pelo theseus (que faz backup da
//! instância, remove e recopia todos os mods e roda os processadores do loader).
//!
//! O último manifesto aplicado fica em `manifests/<slug>.json` na pasta do launcher.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use conexao_types::{FilePolicy, PackFile, PackManifest};
use futures::StreamExt;

use crate::error::{CmdError, CmdResult};

/// Downloads simultâneos.
const PARALLEL: usize = 6;

#[derive(Debug, Default)]
pub struct Plan<'a> {
    /// Arquivos novos ou alterados (só do lado cliente).
    pub download: Vec<&'a PackFile>,
    /// Arquivos `managed` que saíram do modpack.
    pub remove: Vec<String>,
}

impl Plan<'_> {
    pub fn bytes(&self) -> u64 {
        self.download.iter().map(|f| f.size).sum()
    }
}

/// O que a instância precisa para ir de `old` para `new`. `None` quando o Minecraft
/// ou o loader mudaram: aí a instalação completa é necessária.
pub fn plan<'a>(old: &PackManifest, new: &'a PackManifest, instance_dir: &Path) -> Option<Plan<'a>> {
    if old.minecraft != new.minecraft || old.loader != new.loader {
        return None;
    }
    let mut plan = removals(old, new);
    let previous: HashMap<&str, &PackFile> =
        old.files.iter().filter(|f| f.side.on_client()).map(|f| (f.path.as_str(), f)).collect();
    for file in new.files.iter().filter(|f| f.side.on_client()) {
        let Some(target) = safe_join(instance_dir, &file.path) else { continue };
        let on_disk = std::fs::metadata(&target).ok().filter(|m| m.is_file());
        let needed = match file.policy {
            // Configs iniciais: só se o jogador ainda não tem.
            FilePolicy::Initial => on_disk.is_none(),
            FilePolicy::Managed => {
                let unchanged = previous.get(file.path.as_str()).is_some_and(|p| p.sha1 == file.sha1);
                !(unchanged && on_disk.is_some_and(|m| m.len() == file.size))
            }
        };
        if needed {
            plan.download.push(file);
        }
    }
    Some(plan)
}

/// Arquivos `managed` do manifesto antigo que não existem mais no novo.
pub fn removals<'a>(old: &PackManifest, new: &PackManifest) -> Plan<'a> {
    let current: std::collections::HashSet<&str> =
        new.files.iter().filter(|f| f.side.on_client()).map(|f| f.path.as_str()).collect();
    Plan {
        download: Vec::new(),
        remove: old
            .files
            .iter()
            .filter(|f| f.side.on_client() && f.policy == FilePolicy::Managed && !current.contains(f.path.as_str()))
            .map(|f| f.path.clone())
            .collect(),
    }
}

/// Junta um caminho do manifesto à pasta da instância, recusando `..`, raiz e unidades.
pub fn safe_join(base: &Path, relative: &str) -> Option<PathBuf> {
    let path = Path::new(relative);
    if relative.is_empty() || path.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(base.join(path))
}

pub async fn remove_files(instance_dir: &Path, paths: &[String]) {
    for relative in paths {
        if let Some(target) = safe_join(instance_dir, relative) {
            match tokio::fs::remove_file(&target).await {
                Ok(()) => tracing::info!("removido do modpack: {relative}"),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => tracing::warn!("não foi possível remover {relative}: {e}"),
            }
        }
    }
}

/// Baixa os arquivos do plano, confere o SHA-1 e troca cada um de forma atômica.
/// `progress(baixado, total)` é chamado a cada arquivo concluído.
pub async fn apply(
    http: &reqwest::Client,
    cdn: &str,
    instance_dir: &Path,
    plan: &Plan<'_>,
    progress: impl Fn(u64, u64) + Send + Sync,
) -> CmdResult<()> {
    let total = plan.bytes();
    let done = Arc::new(AtomicU64::new(0));
    progress(0, total);
    // Dados próprios em cada tarefa (o executor do Tauri exige futures `Send` sem empréstimos).
    #[allow(clippy::needless_collect)] // sem o Vec, o future deixa de ser `Send`
    let files: Vec<PackFile> = plan.download.iter().map(|f| (*f).clone()).collect();
    let mut tasks = futures::stream::iter(files.into_iter().map(|file| {
        let done = Arc::clone(&done);
        let (http, cdn, dir) = (http.clone(), cdn.to_string(), instance_dir.to_path_buf());
        async move {
            fetch_one(&http, &cdn, &dir, &file).await?;
            Ok::<u64, CmdError>(done.fetch_add(file.size, Ordering::Relaxed) + file.size)
        }
    }))
    .buffer_unordered(PARALLEL);
    while let Some(result) = tasks.next().await {
        progress(result?, total);
    }
    drop(tasks);
    remove_files(instance_dir, &plan.remove).await;
    Ok(())
}

async fn fetch_one(http: &reqwest::Client, cdn: &str, instance_dir: &Path, file: &PackFile) -> CmdResult<()> {
    let target = safe_join(instance_dir, &file.path).ok_or_else(|| CmdError::new("caminho inválido no modpack"))?;
    let url = PackManifest::file_url(file, cdn);
    let bytes = http.get(&url).send().await?.error_for_status()?.bytes().await?;
    let sha1 = sha1_smol::Sha1::from(&bytes).hexdigest();
    if bytes.len() as u64 != file.size || !sha1.eq_ignore_ascii_case(&file.sha1) {
        return Err(CmdError::new(format!("O arquivo {} veio corrompido. Tente de novo.", file.path)));
    }
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    // Escreve ao lado e renomeia: nunca altera o conteúdo de um arquivo que o theseus
    // possa ter ligado (hard link) ao seu repositório de conteúdo.
    let tmp = target.with_extension("conexao-tmp");
    tokio::fs::write(&tmp, &bytes).await?;
    if let Err(error) = tokio::fs::rename(&tmp, &target).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(error.into());
    }
    Ok(())
}

// ---------------------------------------------------------------- manifesto salvo

fn saved_path(data_dir: &Path, slug: &str) -> PathBuf {
    data_dir.join("manifests").join(format!("{slug}.json"))
}

pub async fn load_applied(data_dir: &Path, slug: &str) -> Option<PackManifest> {
    let bytes = tokio::fs::read(saved_path(data_dir, slug)).await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub async fn save_applied(data_dir: &Path, slug: &str, manifest: &PackManifest) -> CmdResult<()> {
    let path = saved_path(data_dir, slug);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, serde_json::to_vec(manifest)?).await?;
    tokio::fs::rename(&tmp, &path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use conexao_types::{FileSide, Loader, LoaderSpec};

    use super::*;

    fn file(path: &str, sha1: &str, size: u64, policy: FilePolicy) -> PackFile {
        PackFile {
            path: path.into(),
            sha1: sha1.into(),
            sha512: String::new(),
            size,
            policy,
            side: FileSide::Both,
            url: None,
        }
    }

    fn manifest(files: Vec<PackFile>, loader: &str) -> PackManifest {
        PackManifest {
            format: 1,
            server: "s".into(),
            name: "S".into(),
            version: "1".into(),
            minecraft: "1.21.1".parse().unwrap(),
            loader: LoaderSpec { kind: Loader::NeoForge, version: loader.into() },
            files,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn downloads_only_what_changed() {
        let dir = std::env::temp_dir().join(format!("conexao-delta-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("mods")).unwrap();
        std::fs::write(dir.join("mods/a.jar"), b"aaaa").unwrap();
        std::fs::write(dir.join("mods/b.jar"), b"bbbb").unwrap();
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::write(dir.join("config/x.toml"), b"meu").unwrap();

        let old = manifest(
            vec![
                file("mods/a.jar", "a1", 4, FilePolicy::Managed),
                file("mods/b.jar", "b1", 4, FilePolicy::Managed),
                file("mods/gone.jar", "g1", 4, FilePolicy::Managed),
                file("config/x.toml", "x1", 3, FilePolicy::Initial),
            ],
            "21.1.1",
        );
        let new = manifest(
            vec![
                file("mods/a.jar", "a1", 4, FilePolicy::Managed), // igual: não baixa
                file("mods/b.jar", "b2", 5, FilePolicy::Managed), // mudou
                file("mods/c.jar", "c1", 4, FilePolicy::Managed), // novo
                file("config/x.toml", "x2", 3, FilePolicy::Initial), // jogador já tem
                file("config/y.toml", "y1", 3, FilePolicy::Initial), // não tem
            ],
            "21.1.1",
        );
        let plan = plan(&old, &new, &dir).unwrap();
        let paths: Vec<_> = plan.download.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["mods/b.jar", "mods/c.jar", "config/y.toml"]);
        assert_eq!(plan.remove, ["mods/gone.jar"]);

        // Arquivo apagado pelo jogador volta a ser baixado.
        std::fs::remove_file(dir.join("mods/a.jar")).unwrap();
        let again = super::plan(&old, &new, &dir).unwrap();
        assert!(again.download.iter().any(|f| f.path == "mods/a.jar"));

        // Loader diferente: instalação completa.
        assert!(super::plan(&old, &manifest(vec![], "21.1.2"), &dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_unsafe_paths() {
        let base = Path::new("/tmp/i");
        assert!(safe_join(base, "../x").is_none());
        assert!(safe_join(base, "/etc/x").is_none());
        assert!(safe_join(base, "mods/../../x").is_none());
        assert!(safe_join(base, "").is_none());
        assert!(safe_join(base, "mods/a.jar").is_some());
    }
}
