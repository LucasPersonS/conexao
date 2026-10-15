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
}
