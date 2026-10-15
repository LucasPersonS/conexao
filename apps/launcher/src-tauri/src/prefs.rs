//! Preferências do próprio Conexão (as do jogo ficam no theseus).
//! Arquivo `conexao-prefs.json` na pasta de dados do launcher.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherPrefs {
    /// Memória calculada pelo tamanho do modpack e pela RAM do computador.
    pub memory_auto: bool,
    /// Flags de JVM testadas para modpacks (G1GC com pausas curtas).
    pub jvm_optimized: bool,
    /// Minimiza o launcher enquanto o jogo está aberto (e para o que roda em segundo plano).
    pub minimize_on_play: bool,
    /// Mostra no Discord em qual servidor o jogador está, com botão "Jogar também".
    pub discord_presence: bool,
    /// Notificações do sistema (vaga aberta, eventos lembrados).
    pub notifications: bool,
    /// Java escolhido pelo jogador (vazio: o Conexão baixa o certo para cada versão).
    pub java_path: Option<String>,
    /// Eventos que o jogador pediu para lembrar.
    pub reminders: Vec<Reminder>,
}

impl Default for LauncherPrefs {
    fn default() -> Self {
        Self {
            memory_auto: true,
            jvm_optimized: true,
            minimize_on_play: true,
            discord_presence: true,
            notifications: true,
            java_path: None,
            reminders: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub server_name: String,
    pub starts_at: DateTime<Utc>,
}

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("conexao-prefs.json")
}

pub async fn load(data_dir: &Path) -> LauncherPrefs {
    match tokio::fs::read(path(data_dir)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => LauncherPrefs::default(),
    }
}

pub async fn save(data_dir: &Path, prefs: &LauncherPrefs) -> std::io::Result<()> {
    tokio::fs::create_dir_all(data_dir).await?;
    let tmp = data_dir.join("conexao-prefs.json.tmp");
    tokio::fs::write(&tmp, serde_json::to_vec_pretty(prefs).unwrap_or_default()).await?;
    tokio::fs::rename(&tmp, path(data_dir)).await
}
