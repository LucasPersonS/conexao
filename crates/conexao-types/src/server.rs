use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::manifest::LoaderSpec;
use crate::minecraft::MinecraftVersion;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Stable,
    Beta,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerStatus {
    pub online: bool,
    pub players_online: u32,
    pub players_max: u32,
    pub latency_ms: Option<u32>,
    pub checked_at: Option<DateTime<Utc>>,
}

/// Cartão de servidor exibido na home do launcher (lista de parceiros).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSummary {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub address: String,
    pub minecraft: MinecraftVersion,
    pub loader: LoaderSpec,
    pub tags: Vec<String>,
    /// Destaque pago na home (gratuito para parceiros, destaque é pago).
    pub featured: bool,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub discord_url: Option<String>,
    pub whitelist_required: bool,
    pub status: ServerStatus,
    /// Versão estável atual do modpack, se já houver alguma publicada.
    pub current_version: Option<String>,
    /// Tamanho total do modpack estável em bytes (mostrado no botão "Instalar").
    #[serde(default)]
    pub pack_size: Option<u64>,
    /// Aceita contas não originais (login pelo Discord).
    #[serde(default)]
    pub allow_offline: bool,
    /// Estrelas dadas pelos jogadores no launcher.
    #[serde(default)]
    pub stars: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackVersionInfo {
    pub id: Uuid,
    pub version: String,
    pub channel: Channel,
    pub manifest_sha256: String,
    pub published_by: Option<Uuid>,
    pub published_at: DateTime<Utc>,
}
