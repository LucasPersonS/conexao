//! Atualização automática do launcher (feature `updater`, ligada no build de release).
//!
//! O CI publica o instalador assinado e o `latest.json` na VPS
//! (`/launcher/latest.json`); o launcher confere a assinatura minisign com a
//! chave pública do `tauri.release.conf.json` antes de instalar.

use serde::Serialize;

use crate::error::CmdResult;

#[derive(Serialize)]
pub struct UpdateInfo {
    pub version: String,
    /// Versão instalada agora.
    pub current: String,
    pub notes: Option<String>,
}

/// Evento `update-progress` enquanto a atualização baixa e instala.
#[derive(Serialize, Clone)]
#[cfg_attr(not(feature = "updater"), allow(dead_code))]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    /// Download terminou; o instalador está rodando (o launcher vai fechar e reabrir).
    pub installing: bool,
}

#[cfg(feature = "updater")]
mod imp {
    use tauri::Emitter;
    use tauri_plugin_updater::UpdaterExt;

    use super::{UpdateInfo, UpdateProgress};
    use crate::error::{CmdError, CmdResult};

    fn err(error: impl std::fmt::Display) -> CmdError {
        CmdError::new(format!("Falha na atualização: {error}"))
    }

    pub async fn check(app: &tauri::AppHandle) -> CmdResult<Option<UpdateInfo>> {
        let update = app.updater().map_err(err)?.check().await.map_err(err)?;
        Ok(update.map(|u| UpdateInfo { version: u.version.clone(), current: u.current_version.clone(), notes: u.body }))
    }

    pub async fn install(app: &tauri::AppHandle) -> CmdResult<()> {
        let Some(update) = app.updater().map_err(err)?.check().await.map_err(err)? else {
            return Ok(());
        };
        // Progresso para o aviso de atualização (no máximo ~8 vezes por segundo).
        let mut downloaded: u64 = 0;
        let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
        let progress_app = app.clone();
        let finished_app = app.clone();
        update
            .download_and_install(
                move |chunk, total| {
                    downloaded += chunk as u64;
                    if last.elapsed() >= std::time::Duration::from_millis(120) {
                        last = std::time::Instant::now();
                        let _ = progress_app.emit("update-progress", UpdateProgress { downloaded, total, installing: false });
                    }
                },
                move || {
                    let _ = finished_app.emit(
                        "update-progress",
                        UpdateProgress { downloaded: 0, total: None, installing: true },
                    );
                },
            )
            .await
            .map_err(err)?;
        // No Windows o instalador fecha e reabre o launcher sozinho; nos outros, reinicia aqui.
        app.restart();
    }
}

#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> CmdResult<Option<UpdateInfo>> {
    #[cfg(feature = "updater")]
    {
        match imp::check(&app).await {
            Ok(info) => Ok(info),
            Err(error) => {
                tracing::warn!("verificação de atualização falhou: {}", error.message);
                Ok(None)
            }
        }
    }
    #[cfg(not(feature = "updater"))]
    {
        let _ = app;
        Ok(None)
    }
}

#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> CmdResult<()> {
    #[cfg(feature = "updater")]
    {
        imp::install(&app).await
    }
    #[cfg(not(feature = "updater"))]
    {
        let _ = app;
        Ok(())
    }
}
