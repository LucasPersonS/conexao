//! Protocolo WebSocket entre a API e o agente que roda ao lado do servidor.
//!
//! As mensagens são JSON com um campo `type`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::manifest::Loader;
use crate::whitelist::WhitelistPlayer;

/// API → agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToAgent {
    /// Lista completa de jogadores liberados; o agente substitui o `whitelist.json`.
    WhitelistSync {
        players: Vec<WhitelistPlayer>,
        /// O servidor exige whitelist? O Companion liga/desliga a whitelist do servidor.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        required: Option<bool>,
    },
    /// Pede ao Companion os arquivos (por SHA-1) que não estão no Modrinth.
    UploadFiles { sha1: Vec<String> },
    /// Executa um comando de console via RCON (apenas comandos permitidos).
    Command { id: Uuid, command: String },
    Ping,
}

/// Agente → API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FromAgent {
    Hello { agent_version: String },
    WhitelistApplied { count: usize },
    CommandResult { id: Uuid, ok: bool, output: String },
    Pong,
    Error { message: String },
    /// Companion: dados do servidor em execução (versões exatas, MOTD, vagas).
    ServerInfo(ServerInfo),
    /// Companion: mods instalados no servidor (`mods/*.jar`).
    ModsSnapshot { files: Vec<SnapshotFile> },
    /// Companion: resultado de um `UploadFiles`.
    FilesUploaded {
        sha1: Vec<String>,
        #[serde(default)]
        failed: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ServerInfo {
    pub minecraft: String,
    pub loader: Option<Loader>,
    pub loader_version: Option<String>,
    #[serde(default)]
    pub motd: String,
    #[serde(default)]
    pub max_players: u32,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub online_mode: bool,
    #[serde(default)]
    pub whitelist: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFile {
    pub path: String,
    pub sha1: String,
    pub size: u64,
}

/// Comandos de console que a API pode pedir ao agente.
pub const ALLOWED_COMMAND_PREFIXES: &[&str] = &["whitelist ", "kick ", "say ", "list"];

pub fn is_allowed_command(command: &str) -> bool {
    let command = command.trim();
    !command.contains(['\n', '\r'])
        && ALLOWED_COMMAND_PREFIXES
            .iter()
            .any(|prefix| command == prefix.trim() || command.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_shape() {
        let msg = ToAgent::WhitelistSync { players: vec![], required: None };
        assert_eq!(
            serde_json::to_string(&msg).unwrap(),
            r#"{"type":"whitelist_sync","players":[]}"#
        );
    }

    #[test]
    fn companion_messages() {
        let info: FromAgent = serde_json::from_str(
            r#"{"type":"server_info","minecraft":"1.21.1","loader":"neoforge","loader_version":"21.1.77","motd":"Oi","max_players":50,"port":25565,"online_mode":true,"whitelist":true}"#,
        )
        .unwrap();
        assert!(matches!(info, FromAgent::ServerInfo(ServerInfo { max_players: 50, .. })));
        let sync = ToAgent::WhitelistSync { players: vec![], required: Some(true) };
        assert_eq!(serde_json::to_string(&sync).unwrap(), r#"{"type":"whitelist_sync","players":[],"required":true}"#);
    }

    #[test]
    fn command_allowlist() {
        assert!(is_allowed_command("whitelist reload"));
        assert!(is_allowed_command("list"));
        assert!(!is_allowed_command("op Notch"));
        assert!(!is_allowed_command("say oi\nop Notch"));
        assert!(!is_allowed_command("stop"));
    }
}
