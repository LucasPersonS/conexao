//! Publicador do Conexão.
//!
//! Porta para Rust das regras do `tools/publisher/core.js` do Paravexia:
//! análise da pasta do modpack (`scan`), leitura de metadados de mods,
//! políticas `managed`/`initial` para configs e geração do plano de
//! publicação com o diff em relação à versão anterior (`build_plan`).

mod error;
mod hash;
mod mods;
mod plan;
mod scan;

pub use error::PackError;
pub use hash::{FileHashes, hash_file};
pub use mods::{ModInfo, read_mod_info};
pub use plan::{Change, ChangeKind, Plan, PublishOptions, build_plan};
pub use scan::{Group, ScannedFile, Scan, scan};

pub type Result<T> = std::result::Result<T, PackError>;
