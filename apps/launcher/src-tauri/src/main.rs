//! Conexão — launcher de Minecraft para a comunidade de Roleplay.
//!
//! O shell Tauri expõe comandos para o frontend Vue. A instalação de Java,
//! loaders (Fabric, Forge, NeoForge) e o launch do jogo são feitos pelo
//! `theseus` (app-lib do Modrinth, GPL-3.0), vendorizado em `vendor/theseus`.

#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod account;
mod conexao_api;
mod deeplink;
mod delta;
mod error;
mod notify;
mod performance;
mod play;
mod prefs;
mod presence;
mod registry;
mod settings;
mod updater;

use std::sync::Arc;

use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;

use crate::conexao_api::ConexaoApi;
use crate::error::CmdResult;
use crate::play::PlayTracker;

pub struct AppContext {
    pub api: ConexaoApi,
    pub plays: PlayTracker,
    pub presence: presence::Presence,
    pub watchers: notify::Watchers,
}

#[tauri::command]
async fn initialize(app: tauri::AppHandle) -> CmdResult<()> {
    theseus::EventState::init().await?;
    theseus::State::init(app.config().identifier.clone()).await?;
    Ok(())
}

#[tauri::command]
fn api_base(ctx: tauri::State<'_, Arc<AppContext>>) -> String {
    ctx.api.base().to_string()
}

/// Instalações feitas quando o launcher ainda se chamava CENTRAL (`gg.central.launcher`):
/// move os dados (conta, instâncias, Java, preferências e o armazenamento da janela)
/// para a pasta nova antes de qualquer coisa abrir, para ninguém perder nada na troca de nome.
fn migrate_legacy_data(identifier: &str) {
    const LEGACY: &str = "gg.central.launcher";
    const FILES: [(&str, &str); 2] =
        [("central-prefs.json", "conexao-prefs.json"), ("central-servers.json", "conexao-servers.json")];
    for base in [dirs::data_dir(), dirs::data_local_dir()].into_iter().flatten() {
        let (old, new) = (base.join(LEGACY), base.join(identifier));
        if old.is_dir() && !new.exists() {
            match std::fs::rename(&old, &new) {
                Ok(()) => tracing::info!("dados do CENTRAL movidos para {}", new.display()),
                Err(error) => tracing::warn!("não foi possível mover {}: {error}", old.display()),
            }
        }
        for (from, to) in FILES {
            let (from, to) = (new.join(from), new.join(to));
            if from.is_file() && !to.exists() {
                let _ = std::fs::rename(from, to);
            }
        }
    }
}

fn main() {
    tracing::info!("iniciando Conexão");
    let tauri_context = tauri::generate_context!();
    migrate_legacy_data(&tauri_context.config().identifier);
    let context = Arc::new(AppContext {
        api: ConexaoApi::from_env(),
        plays: PlayTracker::default(),
        presence: presence::Presence::default(),
        watchers: notify::Watchers::default(),
    });

    let builder = tauri::Builder::default();
    #[cfg(feature = "updater")]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    builder
        // O single-instance precisa vir primeiro: com o launcher aberto, um convite
        // clicado no navegador chega aqui em vez de abrir uma segunda janela.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(url) = args.iter().find(|a| a.starts_with("conexao://")) {
                deeplink::handle(app, url);
            } else if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Arc::clone(&context))
        .manage(deeplink::PendingInvite::default())
        .setup(move |app| {
            // Em desenvolvimento o instalador não registrou o esquema conexao://.
            #[cfg(all(debug_assertions, any(windows, target_os = "linux")))]
            if let Err(error) = app.deep_link().register_all() {
                tracing::warn!("não foi possível registrar conexao://: {error}");
            }
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    deeplink::handle(&handle, url.as_str());
                }
            });
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    deeplink::handle(app.handle(), url.as_str());
                }
            }
            tauri::async_runtime::spawn(notify::restore(app.handle().clone(), Arc::clone(&context)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            initialize,
            api_base,
            deeplink::take_invite,
            account::current_account,
            account::login_microsoft,
            account::logout,
            conexao_api::conexao_session,
            conexao_api::list_servers,
            conexao_api::api_json,
            conexao_api::upload_profile_background,
            conexao_api::my_roles,
            conexao_api::server_access,
            conexao_api::apply_whitelist,
            conexao_api::publish_token,
            conexao_api::approve_panel,
            conexao_api::pairing_info,
            play::play,
            play::play_progress,
            play::install_state,
            settings::get_settings,
            settings::set_settings,
            settings::open_game_folder,
            settings::open_launcher_folder,
            settings::list_java,
            settings::test_java,
            notify::watch_slot,
            notify::unwatch_slot,
            notify::watched_slots,
            notify::set_reminder,
            notify::reminders,
            updater::check_update,
            updater::install_update,
        ])
        .run(tauri_context)
        .expect("falha ao iniciar o Conexão");
}
