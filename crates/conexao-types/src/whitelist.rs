use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WhitelistStatus {
    Pending,
    Approved,
    Rejected,
    Banned,
}

impl WhitelistStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            WhitelistStatus::Pending => "pending",
            WhitelistStatus::Approved => "approved",
            WhitelistStatus::Rejected => "rejected",
            WhitelistStatus::Banned => "banned",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "pending" => WhitelistStatus::Pending,
            "approved" => WhitelistStatus::Approved,
            "rejected" => WhitelistStatus::Rejected,
            "banned" => WhitelistStatus::Banned,
            _ => return None,
        })
    }
}

/// Entrada no formato do `whitelist.json` do servidor Minecraft.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WhitelistPlayer {
    pub uuid: Uuid,
    pub name: String,
}

/// Resposta para o launcher: este jogador pode entrar neste servidor?
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessResponse {
    pub server: String,
    pub whitelist_required: bool,
    pub status: Option<WhitelistStatus>,
    pub can_play: bool,
    /// O servidor aceita contas não originais (`online-mode=false` + passe de entrada).
    #[serde(default)]
    pub allow_offline: bool,
    /// Onde pedir o passe de entrada (endereço só IPv4 da API). Só com `allow_offline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join_url: Option<String>,
    /// Motivo para mostrar ao jogador quando `can_play` é falso por outra razão que a whitelist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Resposta de `POST /v1/servers/{slug}/join-ticket`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinTicketResponse {
    /// Nick com que o jogo deve entrar.
    pub name: String,
    pub expires_in: u64,
    /// O Companion do servidor está conectado ao Conexão agora?
    pub companion_online: bool,
}
