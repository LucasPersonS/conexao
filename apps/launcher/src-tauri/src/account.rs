//! Conta do jogador.
//!
//! - **Original**: conta Microsoft/Minecraft, usando o fluxo de login do `theseus`.
//! - **Não original**: conta Conexão criada pelo Discord, com um nick escolhido no
//!   primeiro acesso. Só joga em servidores que aceitam contas não originais
//!   (`online-mode=false` + passe de entrada do Companion).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{Manager, UserAttentionType};
use theseus::minecraft_auth;
use theseus::prelude::Credentials;
use uuid::Uuid;

use crate::AppContext;
use crate::error::{CmdError, CmdResult};

#[derive(Serialize)]
pub struct Account {
    pub uuid: Uuid,
    pub name: String,
    /// `microsoft` (original) ou `offline` (não original, login pelo Discord).
    pub kind: &'static str,
}

/// Conta não original salva no computador do jogador.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineAccount {
    pub uuid: Uuid,
    pub name: String,
    /// Segredo da sessão longa no Conexão (trocado por JWT quando precisa).
    pub session: String,
}

impl OfflineAccount {
    fn account(&self) -> Account {
        Account { uuid: self.uuid, name: self.name.clone(), kind: "offline" }
    }
}

const OFFLINE_FILE: &str = "conexao-conta-discord.json";

fn offline_path(data_dir: &Path) -> PathBuf {
    data_dir.join(OFFLINE_FILE)
}

pub fn load_offline(data_dir: &Path) -> Option<OfflineAccount> {
    let text = std::fs::read_to_string(offline_path(data_dir)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_offline(data_dir: &Path, account: &OfflineAccount) -> CmdResult<()> {
    std::fs::create_dir_all(data_dir)?;
    let target = offline_path(data_dir);
    let tmp = target.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(account)?)?;
    std::fs::rename(tmp, target)?;
    Ok(())
}

pub fn remove_offline(data_dir: &Path) {
    let _ = std::fs::remove_file(offline_path(data_dir));
}

pub async fn default_credentials() -> CmdResult<Option<Credentials>> {
    Ok(minecraft_auth::get_default_credentials().await?)
}

#[tauri::command]
pub async fn current_account(ctx: tauri::State<'_, Arc<AppContext>>) -> CmdResult<Option<Account>> {
    if let Some(offline) = ctx.api.offline_account() {
        return Ok(Some(offline.account()));
    }
    Ok(default_credentials()
        .await?
        .map(|c| Account { uuid: c.offline_profile.id, name: c.offline_profile.name, kind: "microsoft" }))
}

/// Abre uma janela de login e devolve a URL de retorno quando ela chega em `redirect_prefix`.
/// `None` = o jogador fechou a janela ou o tempo acabou.
async fn signin_window(
    app: &tauri::AppHandle,
    url: &str,
    title: &str,
    redirect_prefix: &str,
) -> CmdResult<Option<url::Url>> {
    if let Some(window) = app.get_webview_window("signin") {
        window.close()?;
    }
    let url = url.parse().map_err(|_| CmdError::new("URL de login inválida"))?;
    let window = tauri::WebviewWindowBuilder::new(app, "signin", tauri::WebviewUrl::External(url))
        .title(title)
        .always_on_top(true)
        .inner_size(520.0, 720.0)
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
            && current.as_str().starts_with(redirect_prefix)
        {
            window.close()?;
            return Ok(Some(current));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    window.close()?;
    Ok(None)
}

/// Abre a janela de login da Microsoft e conclui o fluxo quando ela redireciona.
#[tauri::command]
pub async fn login_microsoft(app: tauri::AppHandle) -> CmdResult<Option<Account>> {
    let flow = minecraft_auth::begin_login().await?;
    let Some(redirect) = signin_window(
        &app,
        &flow.auth_request_uri,
        "Entrar com Microsoft — Conexão",
        "https://login.live.com/oauth20_desktop.srf",
    )
    .await?
    else {
        return Ok(None);
    };
    let Some((_, code)) = redirect.query_pairs().find(|(k, _)| k == "code") else {
        return Ok(None);
    };
    let credentials = minecraft_auth::finish_login(&code, flow).await?;
    // Uma conta por vez: entrar com a original encerra a não original.
    if let Some(ctx) = app.try_state::<Arc<AppContext>>() {
        ctx.api.forget_offline().await;
        ctx.api.clear_token();
    }
    Ok(Some(Account {
        uuid: credentials.offline_profile.id,
        name: credentials.offline_profile.name,
        kind: "microsoft",
    }))
}

/// Resultado do login pelo Discord para o frontend.
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DiscordLogin {
    /// O jogador fechou a janela.
    Cancelled,
    /// Conta pronta.
    Ok { account: Account },
    /// Primeiro acesso: o frontend pede o nick e chama `register_offline`.
    NeedsNick { signup_token: String, discord_name: String, suggested_nick: Option<String> },
}

/// Login de conta não original: Discord → Conexão.
#[tauri::command]
pub async fn login_discord(app: tauri::AppHandle, ctx: tauri::State<'_, Arc<AppContext>>) -> CmdResult<DiscordLogin> {
    let config = ctx.api.discord_config().await?;
    let (true, Some(client_id)) = (config.enabled, config.client_id) else {
        return Err(CmdError::new("O login pelo Discord ainda não está disponível."));
    };
    let state = Uuid::new_v4().simple().to_string();
    let mut url = url::Url::parse("https://discord.com/oauth2/authorize").map_err(|e| CmdError::new(e.to_string()))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &client_id)
        .append_pair("scope", &config.scope)
        .append_pair("redirect_uri", &config.redirect_uri)
        .append_pair("state", &state)
        .append_pair("prompt", "none");
    let Some(redirect) = signin_window(&app, url.as_str(), "Entrar com Discord — Conexão", &config.redirect_uri).await?
    else {
        return Ok(DiscordLogin::Cancelled);
    };
    let query: std::collections::HashMap<_, _> = redirect.query_pairs().into_owned().collect();
    if query.get("state") != Some(&state) {
        return Err(CmdError::new("O login no Discord não confere. Tente de novo."));
    }
    let Some(code) = query.get("code") else {
        // `error=access_denied`: o jogador clicou em Cancelar no Discord.
        return Ok(DiscordLogin::Cancelled);
    };
    match ctx.api.discord_login(code).await? {
        crate::conexao_api::DiscordLoginResponse::Ok(login) => {
            let account = ctx.api.store_offline_login(login).await?;
            Ok(DiscordLogin::Ok { account: account.account() })
        }
        crate::conexao_api::DiscordLoginResponse::NeedsNick { signup_token, discord_name, suggested_nick } => {
            Ok(DiscordLogin::NeedsNick { signup_token, discord_name, suggested_nick })
        }
    }
}

/// Cria a conta não original com o nick escolhido.
#[tauri::command]
pub async fn register_offline(
    ctx: tauri::State<'_, Arc<AppContext>>,
    signup_token: String,
    nick: String,
) -> CmdResult<Account> {
    let nick = nick.trim().to_string();
    if !conexao_types::offline::valid_nick(&nick) {
        return Err(CmdError::new("O nick precisa ter de 3 a 16 caracteres: letras, números e _."));
    }
    let login = ctx.api.register_offline(&signup_token, &nick).await?;
    Ok(ctx.api.store_offline_login(login).await?.account())
}

#[tauri::command]
pub async fn logout(ctx: tauri::State<'_, Arc<AppContext>>) -> CmdResult<()> {
    ctx.api.forget_offline().await;
    if let Some(user) = minecraft_auth::get_default_user().await? {
        minecraft_auth::remove_user(user).await?;
    }
    ctx.api.clear_token();
    Ok(())
}
