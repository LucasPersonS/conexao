//! Registro local: qual instância do theseus pertence a qual servidor do Conexão.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledServer {
    pub instance_id: String,
    pub version: String,
    pub etag: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    pub servers: BTreeMap<String, InstalledServer>,
}

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("conexao-servers.json")
}

pub async fn load(data_dir: &Path) -> Registry {
    match tokio::fs::read(path(data_dir)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => Registry::default(),
    }
}

pub async fn save(data_dir: &Path, registry: &Registry) -> std::io::Result<()> {
    tokio::fs::create_dir_all(data_dir).await?;
    let tmp = data_dir.join("conexao-servers.json.tmp");
    tokio::fs::write(&tmp, serde_json::to_vec_pretty(registry).unwrap_or_default()).await?;
    tokio::fs::rename(&tmp, path(data_dir)).await
}
