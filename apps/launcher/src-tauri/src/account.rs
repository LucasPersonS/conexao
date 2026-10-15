//! Conta Microsoft/Minecraft, usando o fluxo de login do `theseus`.

use std::time::Duration;

use serde::Serialize;
use tauri::{Manager, UserAttentionType};
use theseus::minecraft_auth;
use theseus::prelude::Credentials;
use uuid::Uuid;

use crate::error::{CmdError, CmdResult};

#[derive(Serialize)]
pub struct Account {
    pub uuid: Uuid,
    pub name: String,
}

pub async fn default_credentials() -> CmdResult<Option<Credentials>> {
    Ok(minecraft_auth::get_default_credentials().await?)
}

#[tauri::command]
pub async fn current_account() -> CmdResult<Option<Account>> {
    Ok(default_credentials().await?.map(|c| Account { uuid: c.offline_profile.id, name: c.offline_profile.name }))
}

/// Abre a janela de login da Microsoft e conclui o fluxo quando ela redireciona.
#[tauri::command]
pub async fn login_microsoft(app: tauri::AppHandle) -> CmdResult<Option<Account>> {
    let flow = minecraft_auth::begin_login().await?;
    if let Some(window) = app.get_webview_window("signin") {
        window.close()?;
    }
    let url = flow
        .auth_request_uri
        .parse()
        .map_err(|_| CmdError::new("URL de login inválida"))?;
    let window = tauri::WebviewWindowBuilder::new(&app, "signin", tauri::WebviewUrl::External(url))
        .title("Entrar com Microsoft — Conexão")
        .always_on_top(true)
        .inner_size(520.0, 680.0)
        .center()
        .focused(true)
        .build()?;
    window.request_user_attention(Some(UserAttentionType::Critical))?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(600);
    while tokio::time::Instant::now() < deadline {
        if window.title().is_err() {
            return Ok(None); // janela fechada pelo jogador
        }
        if let Ok(current) = window.url()
            && current.as_str().starts_with("https://login.live.com/oauth20_desktop.srf")
            && let Some((_, code)) = current.query_pairs().find(|(k, _)| k == "code")
        {
            window.close()?;
            let credentials = minecraft_auth::finish_login(&code, flow).await?;
            return Ok(Some(Account {
                uuid: credentials.offline_profile.id,
                name: credentials.offline_profile.name,
            }));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    window.close()?;
    Ok(None)
}

#[tauri::command]
pub async fn logout(ctx: tauri::State<'_, std::sync::Arc<crate::AppContext>>) -> CmdResult<()> {
    if let Some(user) = minecraft_auth::get_default_user().await? {
        minecraft_auth::remove_user(user).await?;
    }
    ctx.api.clear_token();
    Ok(())
}
