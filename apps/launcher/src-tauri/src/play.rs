//! Botão "Jogar": checa acesso, sincroniza o modpack e inicia o jogo
//! já conectando no servidor.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use conexao_types::{FilePolicy, PackManifest};
use dashmap::DashMap;
use serde::Serialize;
use tauri::{Emitter, Manager};
use theseus::install::{InstallJobStatus, InstallPhaseId};
use theseus::instance::QuickPlayType;
use theseus::pack::install_from::CreatePackLocation;
use theseus::server_address::ServerAddress;

use crate::AppContext;
use crate::conexao_api::ManifestFetch;
use crate::error::{CmdError, CmdResult};
use crate::registry::{self, InstalledServer};
use crate::{delta, performance, prefs, presence};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    #[default]
    Idle,
    Checking,
    Installing,
    Launching,
    Running,
    Error,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PlayProgress {
    pub stage: Stage,
    pub message: String,
    pub current: u64,
    pub total: u64,
    /// Etapa mostrada na tela: 0 acesso, 1 Minecraft e Java, 2 loader,
    /// 3 mods e arquivos, 4 abrindo o jogo, 5 em jogo.
    pub step: u8,
}

#[derive(Default)]
pub struct PlayTracker {
    progress: DashMap<String, PlayProgress>,
}

impl PlayTracker {
    fn set(&self, slug: &str, stage: Stage, message: impl Into<String>) {
        let step = match stage {
            Stage::Idle | Stage::Checking => 0,
            Stage::Installing => 1,
            Stage::Launching => 4,
            Stage::Running => 5,
            // Mantém a etapa em que o erro aconteceu.
            Stage::Error => self.progress.get(slug).map_or(0, |p| p.step),
        };
        self.progress
            .insert(slug.to_string(), PlayProgress { stage, message: message.into(), current: 0, total: 0, step });
    }

    fn busy(&self, slug: &str) -> bool {
        self.progress
            .get(slug)
            .is_some_and(|p| matches!(p.stage, Stage::Checking | Stage::Installing | Stage::Launching | Stage::Running))
    }
}

fn phase_step(phase: InstallPhaseId) -> u8 {
    match phase {
        InstallPhaseId::ResolvingMinecraft | InstallPhaseId::DownloadingMinecraft | InstallPhaseId::PreparingJava => 1,
        InstallPhaseId::ResolvingLoader | InstallPhaseId::RunningLoaderProcessors => 2,
        InstallPhaseId::PreparingInstance
        | InstallPhaseId::ResolvingPack
        | InstallPhaseId::ReadingPackManifest
        | InstallPhaseId::DownloadingPackFile
        | InstallPhaseId::DownloadingContent
        | InstallPhaseId::ExtractingOverrides
        | InstallPhaseId::Finalizing
        | InstallPhaseId::RollingBack => 3,
    }
}

fn phase_text(phase: InstallPhaseId) -> &'static str {
    match phase {
        InstallPhaseId::PreparingInstance => "Preparando a instância",
        InstallPhaseId::ResolvingPack | InstallPhaseId::ReadingPackManifest => "Lendo o modpack",
        InstallPhaseId::DownloadingPackFile | InstallPhaseId::DownloadingContent => "Baixando mods e arquivos",
        InstallPhaseId::ExtractingOverrides => "Aplicando configurações",
        InstallPhaseId::ResolvingMinecraft | InstallPhaseId::DownloadingMinecraft => "Baixando o Minecraft",
        InstallPhaseId::ResolvingLoader => "Preparando o loader",
        InstallPhaseId::PreparingJava => "Instalando o Java",
        InstallPhaseId::RunningLoaderProcessors => "Instalando o loader",
        InstallPhaseId::Finalizing => "Finalizando",
        InstallPhaseId::RollingBack => "Desfazendo a instalação",
    }
}

/// Monta um `.mrpack` contendo só o `modrinth.index.json`: os arquivos são
/// baixados pelo theseus a partir do CDN do Conexão.
fn write_mrpack(
    path: &Path,
    manifest: &PackManifest,
    cdn: &str,
    instance_dir: Option<&Path>,
) -> CmdResult<()> {
    let index = manifest.to_mrpack_index(cdn, |file| {
        // Configs `initial` não sobrescrevem o que o jogador já tem.
        file.policy == FilePolicy::Initial && instance_dir.is_some_and(|dir| dir.join(&file.path).exists())
    });
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path)?);
    zip.start_file("modrinth.index.json", zip::write::SimpleFileOptions::default())?;
    zip.write_all(serde_json::to_string_pretty(&index)?.as_bytes())?;
    zip.finish()?;
    Ok(())
}

async fn wait_job(tracker: &PlayTracker, slug: &str, job_id: &str) -> CmdResult<Option<String>> {
    let job_id: uuid::Uuid = job_id.parse().map_err(|_| CmdError::new("instalação inválida"))?;
    loop {
        let job = theseus::install::get_job(job_id).await?;
        let (current, total) = job.progress.as_ref().map_or((0, 0), |p| (p.current, p.total));
        tracker.progress.insert(
            slug.to_string(),
            PlayProgress {
                stage: Stage::Installing,
                message: phase_text(job.phase).to_string(),
                current,
                total,
                step: phase_step(job.phase),
            },
        );
        match job.status {
            InstallJobStatus::Succeeded => return Ok(job.instance_id),
            InstallJobStatus::Failed | InstallJobStatus::Canceled | InstallJobStatus::Interrupted => {
                let message = job
                    .error
                    .map(|e| e.message)
                    .unwrap_or_else(|| format!("instalação {}", job.status.as_str()));
                return Err(CmdError::new(format!("Falha ao instalar o modpack: {message}")));
            }
            InstallJobStatus::Queued | InstallJobStatus::Running => {}
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

async fn run_play(app: tauri::AppHandle, ctx: Arc<AppContext>, slug: String, address: String) -> CmdResult<()> {
    let tracker = &ctx.plays;
    tracker.set(&slug, Stage::Checking, "Verificando acesso");
    let access = ctx.api.access(&slug).await?;
    if !access.can_play {
        return Err(CmdError::new(match access.status {
            Some(conexao_types::WhitelistStatus::Banned) => "Você está banido deste servidor.",
            Some(conexao_types::WhitelistStatus::Pending) => "Seu pedido de whitelist ainda está em análise.",
            _ => "Este servidor exige whitelist. Peça acesso antes de jogar.",
        }));
    }

    let data_dir = app.path().app_data_dir()?;
    let mut registry = registry::load(&data_dir).await;
    let installed = registry.servers.get(&slug).cloned();
    let instance_exists = match &installed {
        Some(entry) => theseus::instance::get(&entry.instance_id).await?.is_some(),
        None => false,
    };

    tracker.set(&slug, Stage::Checking, "Procurando atualizações");
    let etag = installed.as_ref().filter(|_| instance_exists).and_then(|e| e.etag.clone());
    let instance_id = match ctx.api.manifest(&slug, etag.as_deref()).await? {
        ManifestFetch::NotModified => installed.map(|e| e.instance_id).ok_or_else(|| CmdError::new("instância ausente"))?,
        ManifestFetch::Manifest { manifest, etag } => {
            let instance_dir = match (&installed, instance_exists) {
                (Some(entry), true) => Some(theseus::instance::get_full_path(&entry.instance_id).await?),
                _ => None,
            };
            let previous = match &instance_dir {
                Some(_) => delta::load_applied(&data_dir, &slug).await,
                None => None,
            };
            // Mesmo Minecraft e loader: baixa só o que mudou, sem reinstalar tudo.
            if let (Some(entry), Some(dir), Some(old)) = (&installed, &instance_dir, &previous)
                && let Some(plan) = delta::plan(old, &manifest, dir)
            {
                let count = plan.download.len();
                let message = match count {
                    0 => "Aplicando a atualização".to_string(),
                    1 => "Baixando 1 arquivo novo".to_string(),
                    n => format!("Baixando {n} arquivos novos"),
                };
                tracing::info!(slug, count, removed = plan.remove.len(), "atualização diferencial");
                delta::apply(ctx.api.http(), ctx.api.base(), dir, &plan, |current, total| {
                    tracker.progress.insert(
                        slug.clone(),
                        PlayProgress { stage: Stage::Installing, message: message.clone(), current, total, step: 3 },
                    );
                })
                .await?;
                delta::save_applied(&data_dir, &slug, &manifest).await?;
                registry.servers.insert(
                    slug.clone(),
                    InstalledServer { instance_id: entry.instance_id.clone(), version: manifest.version.clone(), etag },
                );
                registry::save(&data_dir, &registry).await?;
                return launch(&app, &ctx, &slug, &address, &data_dir, entry.instance_id.clone()).await;
            }
            // Instalação completa (primeira vez, ou Minecraft/loader mudou). Antes, tira o
            // que saiu do modpack: o theseus só conhece os arquivos da primeira instalação.
            if let (Some(dir), Some(old)) = (&instance_dir, &previous) {
                delta::remove_files(dir, &delta::removals(old, &manifest).remove).await;
            }
            let packs = data_dir.join("packs");
            tokio::fs::create_dir_all(&packs).await?;
            // O nome do arquivo vira o nome da instância no theseus.
            let pack_path: PathBuf = packs.join(format!("{}.mrpack", sanitize(&manifest.name)));
            write_mrpack(&pack_path, &manifest, ctx.api.base(), instance_dir.as_deref())?;

            tracker.set(&slug, Stage::Installing, "Preparando instalação");
            let location = CreatePackLocation::FromFile { path: pack_path };
            let job = match (&installed, instance_exists) {
                (Some(entry), true) => {
                    theseus::install::install_pack_to_existing_instance(entry.instance_id.clone(), location, None)
                        .await?
                }
                _ => theseus::install::create_modpack_instance(location, None).await?,
            };
            let instance_id = wait_job(tracker, &slug, &job.job_id)
                .await?
                .or(job.instance_id)
                .or_else(|| installed.as_ref().map(|e| e.instance_id.clone()))
                .ok_or_else(|| CmdError::new("A instalação terminou sem criar a instância."))?;
            registry.servers.insert(
                slug.clone(),
                InstalledServer { instance_id: instance_id.clone(), version: manifest.version.clone(), etag },
            );
            registry::save(&data_dir, &registry).await?;
            delta::save_applied(&data_dir, &slug, &manifest).await?;
            instance_id
        }
    };
    launch(&app, &ctx, &slug, &address, &data_dir, instance_id).await
}

async fn launch(
    app: &tauri::AppHandle,
    ctx: &Arc<AppContext>,
    slug: &str,
    address: &str,
    data_dir: &Path,
    instance_id: String,
) -> CmdResult<()> {
    let tracker = &ctx.plays;
    tracker.set(slug, Stage::Launching, "Otimizando para o seu computador");
    let launcher_prefs = prefs::load(data_dir).await;
    if let Err(error) = performance::apply(&instance_id, &launcher_prefs).await {
        // Não impede o jogo de abrir: usa as configurações globais.
        tracing::warn!("não foi possível aplicar memória/JVM: {}", error.message);
    }

    tracker.set(slug, Stage::Launching, "Abrindo o Minecraft");
    let process =
        theseus::instance::run(&instance_id, QuickPlayType::Server(ServerAddress::Unresolved(address.to_string())))
            .await?;
    tracker.set(slug, Stage::Running, "Em jogo");
    game_started(app, ctx, slug, &launcher_prefs).await;
    let _ = theseus::process::wait_for(process.uuid).await;
    game_stopped(app, ctx, slug, &launcher_prefs);
    tracker.set(slug, Stage::Idle, "");
    Ok(())
}

#[derive(Clone, Serialize)]
struct GameEvent {
    slug: String,
    running: bool,
}

/// Jogo aberto: o launcher sai do caminho (minimiza e o frontend para o que roda em
/// segundo plano) e o Discord mostra o servidor.
async fn game_started(app: &tauri::AppHandle, ctx: &AppContext, slug: &str, launcher_prefs: &prefs::LauncherPrefs) {
    let _ = app.emit("game", GameEvent { slug: slug.to_string(), running: true });
    if launcher_prefs.minimize_on_play
        && let Some(window) = app.get_webview_window("main")
    {
        let _ = window.minimize();
    }
    if launcher_prefs.discord_presence && presence::available() {
        let server = ctx.api.server(slug).await.ok();
        ctx.presence.playing(presence::Playing {
            server_name: server.as_ref().map_or_else(|| slug.to_string(), |s| s.name.clone()),
            icon_url: server.and_then(|s| s.icon_url),
            invite_url: Some(format!("{}/convite/{slug}", ctx.api.base())),
            started_at: chrono::Utc::now().timestamp(),
        });
    }
}

fn game_stopped(app: &tauri::AppHandle, ctx: &AppContext, slug: &str, launcher_prefs: &prefs::LauncherPrefs) {
    ctx.presence.clear();
    let _ = app.emit("game", GameEvent { slug: slug.to_string(), running: false });
    if launcher_prefs.minimize_on_play
        && let Some(window) = app.get_webview_window("main")
    {
        let _ = window.unminimize();
    }
}

fn sanitize(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let clean = clean.trim();
    if clean.is_empty() { "Servidor Conexão".into() } else { clean.chars().take(48).collect() }
}

#[tauri::command]
pub async fn play(
    app: tauri::AppHandle,
    ctx: tauri::State<'_, Arc<AppContext>>,
    slug: String,
    address: String,
) -> CmdResult<()> {
    crate::deeplink::ensure_slug(&slug)?;
    if ctx.plays.busy(&slug) {
        return Ok(());
    }
    let ctx = Arc::clone(&ctx);
    ctx.plays.set(&slug, Stage::Checking, "Iniciando");
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_play(app, Arc::clone(&ctx), slug.clone(), address).await {
            tracing::warn!("falha ao jogar em {slug}: {}", error.message);
            ctx.plays.set(&slug, Stage::Error, error.message);
        }
    });
    Ok(())
}

/// Se o modpack do servidor já está instalado e em qual versão.
#[derive(Serialize)]
pub struct InstallState {
    pub installed: bool,
    pub version: Option<String>,
}

pub async fn installed_instance(app: &tauri::AppHandle, slug: &str) -> CmdResult<Option<InstalledServer>> {
    let registry = registry::load(&app.path().app_data_dir()?).await;
    match registry.servers.get(slug) {
        Some(entry) if theseus::instance::get(&entry.instance_id).await?.is_some() => Ok(Some(entry.clone())),
        _ => Ok(None),
    }
}

#[tauri::command]
pub async fn install_state(app: tauri::AppHandle, slug: String) -> CmdResult<InstallState> {
    crate::deeplink::ensure_slug(&slug)?;
    let entry = installed_instance(&app, &slug).await?;
    Ok(InstallState { installed: entry.is_some(), version: entry.map(|e| e.version) })
}

#[tauri::command]
pub fn play_progress(ctx: tauri::State<'_, Arc<AppContext>>, slug: String) -> PlayProgress {
    ctx.plays.progress.get(&slug).map(|p| p.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use conexao_types::{FileSide, Loader, LoaderSpec, PackFile};
    use theseus::pack::install_from::{PackDependency, PackFormat};

    use super::*;

    #[test]
    fn mrpack_index_is_accepted_by_theseus() {
        let manifest = PackManifest {
            format: 1,
            server: "paravexia".into(),
            name: "Paravexia".into(),
            version: "1.0.0".into(),
            minecraft: "1.21.1".parse().unwrap(),
            loader: LoaderSpec { kind: Loader::NeoForge, version: "21.1.256".into() },
            files: vec![PackFile {
                path: "mods/create.jar".into(),
                sha1: "2c38cc2b74245ac1bd8bf1869b4696b947973ca4".into(),
                sha512: "a".repeat(128),
                size: 42,
                policy: FilePolicy::Managed,
                side: FileSide::Both,
                url: None,
            }],
            created_at: chrono_now(),
        };
        let dir = std::env::temp_dir().join(format!("conexao-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pack = dir.join("Paravexia.mrpack");
        write_mrpack(&pack, &manifest, "https://api.conexao.gg", None).unwrap();

        let mut zip = zip::ZipArchive::new(std::fs::File::open(&pack).unwrap()).unwrap();
        let index: PackFormat = serde_json::from_reader(zip.by_name("modrinth.index.json").unwrap()).unwrap();
        assert_eq!(index.game, "minecraft");
        assert_eq!(index.dependencies.get(&PackDependency::NeoForge).map(String::as_str), Some("21.1.256"));
        assert_eq!(index.files.len(), 1);
        assert_eq!(
            index.files[0].downloads,
            ["https://api.conexao.gg/files/2c38cc2b74245ac1bd8bf1869b4696b947973ca4"]
        );
        std::fs::remove_dir_all(dir).ok();
    }

    fn chrono_now() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }

    #[test]
    fn sanitizes_instance_names() {
        assert_eq!(sanitize("Paravexia RP"), "Paravexia RP");
        assert_eq!(sanitize("../evil"), "___evil");
        assert_eq!(sanitize("  "), "Servidor Conexão");
    }
}
