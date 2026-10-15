//! Configurações simples para o jogador (memória, tela cheia, pasta do jogo).
//! Ficam guardadas nas configurações globais do theseus, usadas por todas as instâncias.

use serde::{Deserialize, Serialize};
use tauri_plugin_opener::OpenerExt;

use tauri::Manager;

use crate::error::{CmdError, CmdResult};
use crate::play::installed_instance;
use crate::prefs::{self, LauncherPrefs};

#[derive(Serialize, Deserialize)]
pub struct GameSettings {
    /// Memória máxima do Minecraft, em MiB.
    pub memory_mb: u32,
    pub fullscreen: bool,
}

#[derive(Serialize)]
pub struct SettingsView {
    #[serde(flatten)]
    pub game: GameSettings,
    /// RAM total do computador, em MiB (limite do controle deslizante).
    pub system_memory_mb: u32,
    pub recommended_memory_mb: u32,
    pub launcher: LauncherPrefs,
    /// O build tem o app do Conexão no Discord (sem ele a opção some).
    pub discord_available: bool,
}

#[tauri::command]
pub async fn get_settings(app: tauri::AppHandle) -> CmdResult<SettingsView> {
    let settings = theseus::settings::get().await?;
    let system_kib = theseus::jre::get_max_memory().await?;
    Ok(SettingsView {
        game: GameSettings { memory_mb: settings.memory.maximum, fullscreen: settings.force_fullscreen },
        system_memory_mb: u32::try_from(system_kib / 1024).unwrap_or(u32::MAX),
        recommended_memory_mb: theseus::jre::default_memory_max_mb(),
        launcher: prefs::load(&app.path().app_data_dir()?).await,
        discord_available: crate::presence::available(),
    })
}

#[tauri::command]
pub async fn set_settings(app: tauri::AppHandle, game: GameSettings, launcher: LauncherPrefs) -> CmdResult<()> {
    let dir = app.path().app_data_dir()?;
    let saved = prefs::load(&dir).await;
    // Os lembretes são gerenciados à parte (set_reminder).
    prefs::save(&dir, &LauncherPrefs { reminders: saved.reminders, ..launcher }).await?;

    if game.memory_mb < 1024 {
        return Err(CmdError::new("Use pelo menos 1 GB de memória."));
    }
    let mut settings = theseus::settings::get().await?;
    settings.memory.maximum = game.memory_mb;
    settings.force_fullscreen = game.fullscreen;
    theseus::settings::set(settings).await?;
    Ok(())
}

/// Abre no Explorer a pasta da instância do servidor (mods, prints, logs).
#[tauri::command]
pub async fn open_game_folder(app: tauri::AppHandle, slug: String) -> CmdResult<()> {
    crate::deeplink::ensure_slug(&slug)?;
    let entry = installed_instance(&app, &slug)
        .await?
        .ok_or_else(|| CmdError::new("Este servidor ainda não foi instalado."))?;
    let path = theseus::instance::get_full_path(&entry.instance_id).await?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| CmdError::new(format!("Não foi possível abrir a pasta: {e}")))
}
