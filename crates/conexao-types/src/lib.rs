//! Tipos compartilhados do Conexão.
//!
//! Tudo que trafega entre launcher, API, painel e agente vive aqui, para que
//! os quatro lados concordem sobre o formato dos dados.

pub mod agent;
pub mod manifest;
pub mod minecraft;
pub mod server;
pub mod signing;
pub mod whitelist;

pub use manifest::{FilePolicy, FileSide, Loader, LoaderSpec, PackFile, PackManifest};
pub use minecraft::{MinecraftVersion, MIN_SUPPORTED_MINECRAFT};
pub use server::{Channel, PackVersionInfo, ServerStatus, ServerSummary};
pub use whitelist::{AccessResponse, WhitelistPlayer, WhitelistStatus};
