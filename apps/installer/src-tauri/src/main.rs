//! Instalador do Conexão.
//!
//! Uma janela pequena, no visual do launcher, que:
//! 1. lê `latest.json` da API (a mesma fonte da atualização automática);
//! 2. baixa o instalador NSIS da versão mais nova, mostrando o progresso;
//! 3. confere a assinatura com a chave do atualizador (nada sem assinatura é executado);
//! 4. roda o NSIS em modo silencioso (instala só para o usuário, sem pedir administrador);
//! 5. cria o atalho na área de trabalho e abre o launcher.
//!
//! O NSIS continua sendo o instalador "de verdade": o atualizador do launcher usa ele.

#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

const API_URL: &str = match option_env!("CONEXAO_API_URL") {
    Some(url) => url,
    None => "https://api.conexaosmps.com.br",
};
const UPDATER_PUBKEY: &str = env!("CONEXAO_UPDATER_PUBKEY");
/// Pasta e executável que o NSIS do launcher cria (instalação "só para mim").
const PRODUCT_DIR: &str = "Conexao";
const LAUNCHER_EXE: &str = "conexao.exe";
const SHORTCUT_NAME: &str = "Conexao.lnk";

type CmdResult<T> = Result<T, String>;

#[derive(Deserialize)]
struct LatestJson {
    version: String,
    #[serde(default)]
    notes: String,
    platforms: std::collections::HashMap<String, Platform>,
}

#[derive(Deserialize, Clone)]
struct Platform {
    url: String,
    signature: String,
}

#[derive(Serialize)]
pub struct ReleaseInfo {
    version: String,
    notes: String,
    size: Option<u64>,
    installed: bool,
}

#[derive(Serialize, Clone)]
#[serde(tag = "stage", rename_all = "snake_case")]
enum Progress {
    Download { downloaded: u64, total: Option<u64>, bytes_per_sec: u64 },
    Verify,
    Install,
    Shortcut,
}

fn launcher_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join(PRODUCT_DIR).join(LAUNCHER_EXE))
}

fn http() -> CmdResult<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("ConexaoInstalador/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Não foi possível iniciar a conexão: {e}"))
}

async fn latest(client: &reqwest::Client) -> CmdResult<(LatestJson, Platform)> {
    let url = format!("{API_URL}/launcher/latest.json");
    let response = client
        .get(&url)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|_| "Sem conexão com o Conexão. Confira a sua internet e tente de novo.".to_string())?;
    if !response.status().is_success() {
        return Err(format!("O servidor do Conexão respondeu {} ao buscar a versão mais nova.", response.status()));
    }
    let latest: LatestJson = response.json().await.map_err(|_| "Resposta inválida do servidor do Conexão.".to_string())?;
    let platform = ["windows-x86_64-nsis", "windows-x86_64"]
        .iter()
        .find_map(|key| latest.platforms.get(*key))
        .cloned()
        .ok_or("Ainda não há uma versão do Conexão para Windows.")?;
    check_url(&platform.url)?;
    Ok((latest, platform))
}

/// Só baixa do próprio domínio da API, em https.
fn check_url(raw: &str) -> CmdResult<()> {
    let url = url::Url::parse(raw).map_err(|_| "Link de download inválido.".to_string())?;
    let api = url::Url::parse(API_URL).map_err(|_| "Endereço da API inválido.".to_string())?;
    if url.scheme() == "https" && url.host_str().is_some() && url.host_str() == api.host_str() {
        Ok(())
    } else {
        Err("Link de download fora do domínio do Conexão; instalação cancelada.".into())
    }
}

/// Assinatura do atualizador do Tauri (minisign), a mesma que o launcher confere ao se atualizar.
fn verify(data: &[u8], signature_b64: &str) -> CmdResult<()> {
    let decode = |b64: &str| -> Option<String> { String::from_utf8(BASE64.decode(b64.trim()).ok()?).ok() };
    let public_key = decode(UPDATER_PUBKEY)
        .and_then(|text| minisign_verify::PublicKey::decode(&text).ok())
        .ok_or("Chave pública do instalador inválida.")?;
    let signature = decode(signature_b64)
        .and_then(|text| minisign_verify::Signature::decode(&text).ok())
        .ok_or("Assinatura do instalador inválida.")?;
    public_key
        .verify(data, &signature, true)
        .map_err(|_| "A assinatura do arquivo baixado não confere. Por segurança, nada foi instalado.".to_string())
}

#[tauri::command]
async fn release_info() -> CmdResult<ReleaseInfo> {
    let client = http()?;
    let (latest, platform) = latest(&client).await?;
    let size = client
        .head(&platform.url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .ok()
        .filter(|r| r.status().is_success())
        .and_then(|r| r.content_length());
    Ok(ReleaseInfo {
        version: latest.version,
        notes: latest.notes,
        size,
        installed: launcher_path().is_some_and(|p| p.exists()),
    })
}

#[tauri::command]
async fn install(app: AppHandle, desktop_shortcut: bool) -> CmdResult<()> {
    let client = http()?;
    let (_, platform) = latest(&client).await?;

    // 1. Download, com progresso a cada ~120 ms.
    let response = client
        .get(&platform.url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| "Não foi possível baixar o Conexão. Confira a sua internet e tente de novo.".to_string())?;
    let total = response.content_length();
    let mut data = Vec::with_capacity(usize::try_from(total.unwrap_or(0)).unwrap_or(0));
    let mut stream = response.bytes_stream();
    let started = Instant::now();
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "O download foi interrompido. Tente de novo.".to_string())?;
        data.extend_from_slice(&chunk);
        if last_emit.elapsed() >= Duration::from_millis(120) {
            last_emit = Instant::now();
            let downloaded = data.len() as u64;
            let secs = started.elapsed().as_secs_f64().max(0.001);
            let _ = app.emit(
                "progress",
                Progress::Download { downloaded, total, bytes_per_sec: (downloaded as f64 / secs) as u64 },
            );
        }
    }
    let downloaded = data.len() as u64;
    let _ = app.emit("progress", Progress::Download { downloaded, total: Some(downloaded), bytes_per_sec: 0 });

    // 2. Assinatura antes de gravar qualquer coisa executável.
    let _ = app.emit("progress", Progress::Verify);
    verify(&data, &platform.signature)?;

    let name = platform.url.rsplit('/').next().filter(|n| n.ends_with(".exe")).unwrap_or("Conexao-setup.exe");
    let dir = std::env::temp_dir().join("conexao-instalador");
    tokio::fs::create_dir_all(&dir).await.map_err(|e| format!("Não foi possível usar a pasta temporária: {e}"))?;
    let setup = dir.join(name);
    let mut file =
        tokio::fs::File::create(&setup).await.map_err(|e| format!("Não foi possível salvar o instalador: {e}"))?;
    file.write_all(&data).await.map_err(|e| format!("Não foi possível salvar o instalador: {e}"))?;
    file.flush().await.map_err(|e| format!("Não foi possível salvar o instalador: {e}"))?;
    drop(file);
    drop(data);

    // 3. NSIS silencioso (instala em %LOCALAPPDATA%\Conexao, sem pedir administrador).
    let _ = app.emit("progress", Progress::Install);
    let shortcut = dirs::desktop_dir().map(|d| d.join(SHORTCUT_NAME));
    let shortcut_existed = shortcut.as_ref().is_some_and(|p| p.exists());
    let status = tokio::process::Command::new(&setup)
        .arg("/S")
        .status()
        .await
        .map_err(|e| format!("Não foi possível iniciar a instalação: {e}"))?;
    let _ = tokio::fs::remove_file(&setup).await;
    if !status.success() {
        return Err(format!(
            "A instalação terminou com erro (código {}). Feche o Conexão se ele estiver aberto e tente de novo.",
            status.code().unwrap_or(-1)
        ));
    }
    let exe = launcher_path().filter(|p| p.exists()).ok_or("A instalação terminou, mas o Conexão não foi encontrado.")?;

    // 4. Atalho na área de trabalho (ou nenhum, se a pessoa desmarcou).
    let _ = app.emit("progress", Progress::Shortcut);
    if let Some(path) = shortcut {
        if desktop_shortcut {
            create_shortcut(&exe, &path);
        } else if !shortcut_existed {
            let _ = std::fs::remove_file(&path);
        }
    }
    Ok(())
}

#[cfg(windows)]
fn create_shortcut(target: &std::path::Path, path: &std::path::Path) {
    if let Ok(mut link) = mslnk::ShellLink::new(target) {
        link.set_name(Some("Conexão".into()));
        if let Some(dir) = target.parent() {
            link.set_working_dir(Some(dir.to_string_lossy().into_owned()));
        }
        let _ = link.create_lnk(path);
    }
}

#[cfg(not(windows))]
fn create_shortcut(_target: &std::path::Path, _path: &std::path::Path) {}

#[tauri::command]
fn launch(app: AppHandle) -> CmdResult<()> {
    let exe = launcher_path().filter(|p| p.exists()).ok_or("O Conexão não está instalado.")?;
    std::process::Command::new(&exe)
        .current_dir(exe.parent().unwrap_or(&exe))
        .spawn()
        .map_err(|e| format!("Não foi possível abrir o Conexão: {e}"))?;
    app.exit(0);
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![release_info, install, launch])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o instalador do Conexão");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_downloads_from_api_domain() {
        let host = url::Url::parse(API_URL).unwrap();
        let ok = format!("https://{}/launcher/Conexao_0.1.0_x64-setup.exe", host.host_str().unwrap());
        assert!(check_url(&ok).is_ok());
        assert!(check_url("https://evil.example/Conexao.exe").is_err());
        assert!(check_url(&ok.replace("https://", "http://")).is_err());
    }

    #[test]
    fn rejects_bad_signature() {
        assert!(verify(b"conteudo", "bm90LWEtc2lnbmF0dXJl").is_err());
    }

    #[test]
    fn pubkey_decodes() {
        let text = String::from_utf8(BASE64.decode(UPDATER_PUBKEY).unwrap()).unwrap();
        assert!(minisign_verify::PublicKey::decode(&text).is_ok());
    }
}
