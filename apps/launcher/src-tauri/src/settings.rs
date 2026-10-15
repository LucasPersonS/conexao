//! Configurações do jogo e do launcher: memória, Java, argumentos da JVM, janela,
//! variáveis de ambiente, comandos de inicialização e downloads.
//!
//! As opções de jogo ficam nas configurações globais do theseus (valem para todas as
//! instâncias); as do próprio Conexão ficam em `conexao-prefs.json` (ver `prefs`).

use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;
use theseus::prelude::{Hooks, WindowSize};

use crate::error::{CmdError, CmdResult};
use crate::play::installed_instance;
use crate::prefs::{self, LauncherPrefs};

#[derive(Serialize, Deserialize)]
pub struct GameSettings {
    /// Memória máxima do Minecraft (modo manual), em MiB.
    pub memory_mb: u32,
    pub fullscreen: bool,
    /// Tamanho da janela do jogo quando não está em tela cheia.
    pub resolution_width: u16,
    pub resolution_height: u16,
    /// Argumentos extras da JVM, como o jogador digitou (separados por espaço, aspas valem).
    pub jvm_args: String,
    /// Variáveis de ambiente, uma por linha: `NOME=valor`.
    pub env_vars: String,
    /// Comando rodado antes de abrir o jogo.
    pub pre_launch: String,
    /// Comando que "embrulha" o Java (ex.: `prime-run`, `gamemoderun`).
    pub wrapper: String,
    /// Comando rodado depois que o jogo fecha.
    pub post_exit: String,
    /// Downloads simultâneos ao instalar/atualizar um modpack.
    pub max_downloads: usize,
}

#[derive(Serialize)]
pub struct SettingsView {
    #[serde(flatten)]
    pub game: GameSettings,
    /// RAM total do computador, em MiB (limite do controle deslizante).
    pub system_memory_mb: u32,
    pub recommended_memory_mb: u32,
    pub launcher: LauncherPrefs,
    /// Flags que o Conexão adiciona com "Otimizações do Java" ligado.
    pub optimized_flags: Vec<String>,
    /// O build tem o app do Conexão no Discord (sem ele a opção some).
    pub discord_available: bool,
}

#[tauri::command]
pub async fn get_settings(app: tauri::AppHandle) -> CmdResult<SettingsView> {
    let settings = theseus::settings::get().await?;
    let system_kib = theseus::jre::get_max_memory().await?;
    Ok(SettingsView {
        game: GameSettings {
            memory_mb: settings.memory.maximum,
            fullscreen: settings.force_fullscreen,
            resolution_width: settings.game_resolution.0,
            resolution_height: settings.game_resolution.1,
            jvm_args: join_args(&settings.extra_launch_args),
            env_vars: settings.custom_env_vars.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("\n"),
            pre_launch: settings.hooks.pre_launch.unwrap_or_default(),
            wrapper: settings.hooks.wrapper.unwrap_or_default(),
            post_exit: settings.hooks.post_exit.unwrap_or_default(),
            max_downloads: settings.max_concurrent_downloads,
        },
        system_memory_mb: u32::try_from(system_kib / 1024).unwrap_or(u32::MAX),
        recommended_memory_mb: theseus::jre::default_memory_max_mb(),
        launcher: prefs::load(&app.path().app_data_dir()?).await,
        optimized_flags: crate::performance::JVM_FLAGS.iter().map(|f| (*f).to_string()).collect(),
        discord_available: crate::presence::available(),
    })
}

#[tauri::command]
pub async fn set_settings(app: tauri::AppHandle, game: GameSettings, launcher: LauncherPrefs) -> CmdResult<()> {
    // Valida tudo antes de gravar qualquer coisa: ou salva tudo, ou nada.
    let system_mb = crate::performance::system_memory_mb().await;
    if game.memory_mb < 1024 {
        return Err(CmdError::new("Use pelo menos 1 GB de memória."));
    }
    if game.memory_mb > system_mb {
        return Err(CmdError::new("A memória não pode passar da RAM do computador."));
    }
    if game.resolution_width < 320 || game.resolution_height < 240 {
        return Err(CmdError::new("A janela do jogo precisa ter pelo menos 320 × 240."));
    }
    if !(1..=50).contains(&game.max_downloads) {
        return Err(CmdError::new("Downloads simultâneos: escolha entre 1 e 50."));
    }
    let jvm_args = split_args(&game.jvm_args)?;
    if let Some(arg) = jvm_args.iter().find(|a| a.starts_with("-Xmx")) {
        return Err(CmdError::new(format!(
            "Tire o \"{arg}\" dos argumentos: a memória máxima é ajustada no controle de memória."
        )));
    }
    if let Some(arg) = jvm_args.iter().find(|a| !a.starts_with('-')) {
        return Err(CmdError::new(format!(
            "\"{arg}\" não parece um argumento da JVM (todos começam com \"-\")."
        )));
    }
    let env_vars = parse_env(&game.env_vars)?;
    let java_path = match launcher.java_path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(path) => Some(resolve_java(path).await?.path),
        None => None,
    };

    let dir = app.path().app_data_dir()?;
    let saved = prefs::load(&dir).await;
    // Os lembretes são gerenciados à parte (set_reminder).
    prefs::save(&dir, &LauncherPrefs { reminders: saved.reminders, java_path, ..launcher }).await?;

    let mut settings = theseus::settings::get().await?;
    settings.memory.maximum = game.memory_mb;
    settings.force_fullscreen = game.fullscreen;
    settings.game_resolution = WindowSize(game.resolution_width, game.resolution_height);
    settings.extra_launch_args = jvm_args;
    settings.custom_env_vars = env_vars;
    settings.hooks = Hooks {
        pre_launch: non_empty(game.pre_launch),
        wrapper: non_empty(game.wrapper),
        post_exit: non_empty(game.post_exit),
    };
    settings.max_concurrent_downloads = game.max_downloads;
    theseus::settings::set(settings).await?;
    Ok(())
}

#[derive(Serialize)]
pub struct JavaInfo {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub architecture: String,
}

impl From<theseus::prelude::JavaVersion> for JavaInfo {
    fn from(java: theseus::prelude::JavaVersion) -> Self {
        Self { path: java.path, version: java.version, major: java.parsed_version, architecture: java.architecture }
    }
}

/// Javas encontrados no computador (os baixados pelo Conexão e os instalados no sistema).
#[tauri::command]
pub async fn list_java() -> CmdResult<Vec<JavaInfo>> {
    let mut seen = HashSet::new();
    let mut found: Vec<JavaInfo> = Vec::new();
    let managed = theseus::jre::get_java_versions().await?.into_iter().map(|(_, java)| java);
    let system = theseus::jre::find_filtered_jres(None).await.unwrap_or_default();
    for java in managed.chain(system) {
        if seen.insert(java.path.to_lowercase()) {
            found.push(java.into());
        }
    }
    found.sort_by(|a, b| b.major.cmp(&a.major).then_with(|| a.path.cmp(&b.path)));
    Ok(found)
}

/// Confere se o caminho tem um Java que funciona (aceita a pasta do Java, a `bin` ou o executável).
#[tauri::command]
pub async fn test_java(path: String) -> CmdResult<JavaInfo> {
    resolve_java(path.trim()).await
}

async fn resolve_java(path: &str) -> CmdResult<JavaInfo> {
    let path = PathBuf::from(path.trim_matches('"'));
    for candidate in [path.clone(), path.join("bin")] {
        if let Ok(java) = theseus::jre::check_jre(candidate).await {
            return Ok(java.into());
        }
    }
    Err(CmdError::new(format!("Não encontrei um Java funcionando em {}.", path.display())))
}

/// Abre no Explorer a pasta da instância do servidor (mods, prints, logs).
#[tauri::command]
pub async fn open_game_folder(app: tauri::AppHandle, slug: String) -> CmdResult<()> {
    crate::deeplink::ensure_slug(&slug)?;
    let entry = installed_instance(&app, &slug)
        .await?
        .ok_or_else(|| CmdError::new("Este servidor ainda não foi instalado."))?;
    let path = theseus::instance::get_full_path(&entry.instance_id).await?;
    open(&app, path)
}

/// Abre a pasta de dados do launcher (`data`) ou a de logs do launcher (`logs`).
#[tauri::command]
pub async fn open_launcher_folder(app: tauri::AppHandle, kind: String) -> CmdResult<()> {
    let state = theseus::State::get().await?;
    let path = match kind.as_str() {
        "data" => state.directories.config_dir.clone(),
        "logs" => state
            .directories
            .launcher_logs_dir()
            .ok_or_else(|| CmdError::new("Pasta de logs não encontrada."))?,
        _ => return Err(CmdError::new("Pasta desconhecida.")),
    };
    tokio::fs::create_dir_all(&path).await?;
    open(&app, path)
}

fn open(app: &tauri::AppHandle, path: PathBuf) -> CmdResult<()> {
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| CmdError::new(format!("Não foi possível abrir a pasta: {e}")))
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// Separa argumentos como um terminal: espaços separam, aspas simples ou duplas agrupam.
pub fn split_args(input: &str) -> CmdResult<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in input.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (None, c) => {
                current.push(c);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return Err(CmdError::new("Os argumentos da JVM têm aspas abertas sem fechar."));
    }
    if started {
        args.push(current);
    }
    Ok(args)
}

/// Junta argumentos de volta para mostrar no campo de texto (com aspas onde precisa).
fn join_args(args: &[String]) -> String {
    args.iter()
        .map(|a| if a.is_empty() || a.chars().any(char::is_whitespace) { format!("\"{a}\"") } else { a.clone() })
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_env(input: &str) -> CmdResult<Vec<(String, String)>> {
    let mut vars = Vec::new();
    for line in input.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (name, value) = line
            .split_once('=')
            .ok_or_else(|| CmdError::new(format!("Variável sem \"=\": {line}")))?;
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(CmdError::new(format!("Nome de variável inválido: \"{name}\".")));
        }
        vars.push((name.to_string(), value.trim().to_string()));
    }
    Ok(vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_joins_args() {
        let args = split_args(r#"-Xms2G  -Dfoo="a b" '-Dbar=c d'"#).unwrap();
        assert_eq!(args, ["-Xms2G", "-Dfoo=a b", "-Dbar=c d"]);
        assert_eq!(split_args(&join_args(&args)).unwrap(), args);
        assert!(split_args("-Dx=\"aberto").is_err());
        assert!(split_args("   ").unwrap().is_empty());
    }

    #[test]
    fn parses_env_lines() {
        let vars = parse_env("# comentário\nFOO=1\n\n BAR = a=b \n").unwrap();
        assert_eq!(vars, [("FOO".to_string(), "1".to_string()), ("BAR".to_string(), "a=b".to_string())]);
        assert!(parse_env("SEMIGUAL").is_err());
        assert!(parse_env("COM ESPACO=1").is_err());
    }
}
