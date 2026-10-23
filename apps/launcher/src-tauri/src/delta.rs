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
use std::time::Duration;

use conexao_types::{FilePolicy, PackFile, PackManifest};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;

use crate::error::{CmdError, CmdResult};

/// Downloads simultâneos (cada um na sua conexão HTTP/1.1, ver `ConexaoApi::downloads`).
const PARALLEL: usize = 6;
/// Tentativas por arquivo; a partir da segunda, continua de onde parou (Range).
const ATTEMPTS: u32 = 5;

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
/// `progress(baixado, total)` é chamado várias vezes por segundo, em bytes, inclusive
/// no meio de cada arquivo (antes só andava quando um arquivo inteiro terminava).
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
        async move { fetch_with_retry(&http, &cdn, &dir, &file, &done).await }
    }))
    .buffer_unordered(PARALLEL);
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            next = tasks.next() => match next {
                Some(result) => result?,
                None => break,
            },
            _ = tick.tick() => progress(done.load(Ordering::Relaxed).min(total), total),
        }
    }
    drop(tasks);
    progress(total, total);
    remove_files(instance_dir, &plan.remove).await;
    Ok(())
}

/// Como terminou uma tentativa de baixar um arquivo.
enum Attempt {
    /// Conexão caiu, servidor ocupado etc.: tenta de novo (continuando de onde parou).
    Retry(String),
    /// Chegou diferente do manifesto: apaga o parcial e baixa do zero.
    Corrupt,
    /// Não adianta tentar de novo (arquivo não existe no servidor, disco cheio...).
    Fatal(CmdError),
}

/// Arquivo parcial ao lado do destino. O SHA-1 no nome garante que um parcial de outra
/// versão do mesmo mod nunca seja continuado.
fn partial_path(target: &Path, sha1: &str) -> PathBuf {
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let short = sha1.get(..12).unwrap_or(sha1);
    target.with_file_name(format!("{name}.{short}.conexao-tmp"))
}

async fn fetch_with_retry(
    http: &reqwest::Client,
    cdn: &str,
    instance_dir: &Path,
    file: &PackFile,
    done: &AtomicU64,
) -> CmdResult<()> {
    let target = safe_join(instance_dir, &file.path).ok_or_else(|| CmdError::new("caminho inválido no modpack"))?;
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let url = PackManifest::file_url(file, cdn);
    let partial = partial_path(&target, &file.sha1);
    let mut last = String::new();
    for attempt in 0..ATTEMPTS {
        if attempt > 0 {
            // 2 s, 4 s, 8 s, 16 s: dá tempo de uma queda rápida de internet voltar.
            tokio::time::sleep(Duration::from_secs(1 << attempt.min(4))).await;
        }
        // Bytes que esta tentativa somou ao progresso: saem de volta se ela falhar.
        let mut counted = 0u64;
        let outcome = fetch_once(http, &url, &partial, file, done, &mut counted).await;
        if outcome.is_err() {
            done.fetch_sub(counted, Ordering::Relaxed);
        }
        match outcome {
            Ok(()) => {
                // Escreve ao lado e renomeia: nunca altera o conteúdo de um arquivo que o
                // theseus possa ter ligado (hard link) ao seu repositório de conteúdo.
                if let Err(error) = tokio::fs::rename(&partial, &target).await {
                    let _ = tokio::fs::remove_file(&partial).await;
                    return Err(error.into());
                }
                return Ok(());
            }
            Err(Attempt::Retry(reason)) => {
                tracing::warn!(path = %file.path, attempt, "download falhou, tentando de novo: {reason}");
                last = reason;
            }
            Err(Attempt::Corrupt) => {
                tracing::warn!(path = %file.path, attempt, "arquivo veio diferente do manifesto");
                let _ = tokio::fs::remove_file(&partial).await;
                last = "arquivo corrompido".into();
            }
            Err(Attempt::Fatal(error)) => return Err(error),
        }
    }
    tracing::warn!(path = %file.path, "desistindo do download: {last}");
    Err(CmdError::new(format!(
        "A conexão caiu várias vezes ao baixar {}. Verifique sua internet e clique de novo: \
         o download continua de onde parou.",
        file.path
    )))
}

async fn fetch_once(
    http: &reqwest::Client,
    url: &str,
    partial: &Path,
    file: &PackFile,
    done: &AtomicU64,
    counted: &mut u64,
) -> Result<(), Attempt> {
    let io = |error: std::io::Error| Attempt::Fatal(error.into());
    let net = |error: reqwest::Error| Attempt::Retry(error.to_string());

    // O que já chegou numa tentativa anterior (ou num clique anterior em "Jogar").
    let mut hasher = sha1_smol::Sha1::new();
    let mut have = 0u64;
    if let Ok(existing) = tokio::fs::read(partial).await
        && !existing.is_empty()
        && (existing.len() as u64) < file.size
    {
        hasher.update(&existing);
        have = existing.len() as u64;
    }

    let mut request = http.get(url);
    if have > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let mut response = request.send().await.map_err(net)?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::FORBIDDEN {
        return Err(Attempt::Fatal(CmdError::new(format!(
            "O arquivo {} não está disponível no Conexão. Avise a equipe do servidor.",
            file.path
        ))));
    }
    if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        // Parcial maior/estranho: recomeça do zero.
        return Err(Attempt::Corrupt);
    }
    if !status.is_success() {
        return Err(Attempt::Retry(format!("HTTP {status}")));
    }
    let resumed = have > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;
    if !resumed {
        // O servidor mandou o arquivo inteiro: descarta o parcial.
        hasher = sha1_smol::Sha1::new();
        have = 0;
    }

    let out = if resumed {
        tokio::fs::OpenOptions::new().append(true).open(partial).await.map_err(io)?
    } else {
        tokio::fs::File::create(partial).await.map_err(io)?
    };
    let mut out = tokio::io::BufWriter::with_capacity(256 * 1024, out);
    done.fetch_add(have, Ordering::Relaxed);
    *counted += have;

    // Mesmo em erro de rede, o que já foi recebido fica salvo para a próxima tentativa.
    let received = async {
        while let Some(chunk) = response.chunk().await.map_err(net)? {
            have += chunk.len() as u64;
            if have > file.size {
                return Err(Attempt::Corrupt);
            }
            hasher.update(&chunk);
            out.write_all(&chunk).await.map_err(io)?;
            done.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            *counted += chunk.len() as u64;
        }
        Ok(())
    }
    .await;
    out.flush().await.map_err(io)?;
    drop(out);
    received?;

    if have != file.size || !hasher.digest().to_string().eq_ignore_ascii_case(&file.sha1) {
        return Err(Attempt::Corrupt);
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
