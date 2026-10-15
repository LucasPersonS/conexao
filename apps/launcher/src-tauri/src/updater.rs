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
    pub notes: Option<String>,
}

#[cfg(feature = "updater")]
mod imp {
    use tauri_plugin_updater::UpdaterExt;

    use super::UpdateInfo;
    use crate::error::{CmdError, CmdResult};

    fn err(error: impl std::fmt::Display) -> CmdError {
        CmdError::new(format!("Falha na atualização: {error}"))
    }

    pub async fn check(app: &tauri::AppHandle) -> CmdResult<Option<UpdateInfo>> {
        let update = app.updater().map_err(err)?.check().await.map_err(err)?;
        Ok(update.map(|u| UpdateInfo { version: u.version.clone(), notes: u.body }))
    }

    pub async fn install(app: &tauri::AppHandle) -> CmdResult<()> {
        let Some(update) = app.updater().map_err(err)?.check().await.map_err(err)? else {
            return Ok(());
        };
        update.download_and_install(|_, _| {}, || {}).await.map_err(err)?;
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
