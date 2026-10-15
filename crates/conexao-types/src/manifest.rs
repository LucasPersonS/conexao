//! Manifesto de uma versão publicada do modpack de um servidor.
//!
//! O manifesto é a fonte de verdade publicada pela API. O launcher o converte
//! num índice `.mrpack` (formato do Modrinth) para que o `theseus` instale o
//! loader, as bibliotecas e os arquivos.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::minecraft::MinecraftVersion;

pub const MANIFEST_FORMAT: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Fabric,
    Forge,
    #[serde(alias = "neo-forge")]
    NeoForge,
}

impl Loader {
    /// Chave usada em `dependencies` do `modrinth.index.json`.
    pub fn mrpack_key(self) -> &'static str {
        match self {
            Loader::Fabric => "fabric-loader",
            Loader::Forge => "forge",
            Loader::NeoForge => "neoforge",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Loader::Fabric => "Fabric",
            Loader::Forge => "Forge",
            Loader::NeoForge => "NeoForge",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderSpec {
    pub kind: Loader,
    pub version: String,
}

/// Como o launcher trata um arquivo já existente no computador do jogador.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilePolicy {
    /// Sempre sobrescreve para ficar igual ao publicado.
    #[default]
    Managed,
    /// Só instala se o arquivo não existir (preserva configs do jogador).
    Initial,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileSide {
    #[default]
    Both,
    Client,
    Server,
}

impl FileSide {
    pub fn on_client(self) -> bool {
        matches!(self, FileSide::Both | FileSide::Client)
    }
    pub fn on_server(self) -> bool {
        matches!(self, FileSide::Both | FileSide::Server)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackFile {
    /// Caminho relativo à pasta da instância, sempre com `/`.
    pub path: String,
    pub sha1: String,
    pub sha512: String,
    pub size: u64,
    #[serde(default)]
    pub policy: FilePolicy,
    #[serde(default)]
    pub side: FileSide,
    /// URL externa (Modrinth/CurseForge). Sem ela, o arquivo é servido pelo CDN do Conexão.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackManifest {
    pub format: u32,
    pub server: String,
    pub name: String,
    pub version: String,
    pub minecraft: MinecraftVersion,
    pub loader: LoaderSpec,
    pub files: Vec<PackFile>,
    pub created_at: DateTime<Utc>,
}

impl PackManifest {
    /// URL de download de um arquivo: externa, ou `{cdn}/files/{sha1}`.
    pub fn file_url(file: &PackFile, cdn_base: &str) -> String {
        file.url
            .clone()
            .unwrap_or_else(|| format!("{}/files/{}", cdn_base.trim_end_matches('/'), file.sha1))
    }

    /// Gera o `modrinth.index.json` consumido pelo `theseus`.
    ///
    /// `skip` permite ao launcher omitir arquivos `initial` que o jogador já tem.
    pub fn to_mrpack_index(
        &self,
        cdn_base: &str,
        mut skip: impl FnMut(&PackFile) -> bool,
    ) -> serde_json::Value {
        let files: Vec<_> = self
            .files
            .iter()
            .filter(|file| file.side.on_client() && !skip(file))
            .map(|file| {
                serde_json::json!({
                    "path": file.path,
                    "hashes": { "sha1": file.sha1, "sha512": file.sha512 },
                    "env": {
                        "client": if file.side.on_client() { "required" } else { "unsupported" },
                        "server": if file.side.on_server() { "required" } else { "unsupported" },
                    },
                    "downloads": [Self::file_url(file, cdn_base)],
                    "fileSize": file.size,
                })
            })
            .collect();
        let mut dependencies = BTreeMap::new();
        dependencies.insert("minecraft", self.minecraft.to_string());
        dependencies.insert(self.loader.kind.mrpack_key(), self.loader.version.clone());
        serde_json::json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": self.version,
            "name": self.name,
            "summary": format!("Conexão · {}", self.server),
            "files": files,
            "dependencies": dependencies,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> PackManifest {
        PackManifest {
            format: MANIFEST_FORMAT,
            server: "paravexia".into(),
            name: "Paravexia".into(),
            version: "1.0.0".into(),
            minecraft: "1.21.1".parse().unwrap(),
            loader: LoaderSpec { kind: Loader::NeoForge, version: "21.1.256".into() },
            files: vec![
                PackFile {
                    path: "mods/a.jar".into(),
                    sha1: "aa".into(),
                    sha512: "bb".into(),
                    size: 10,
                    policy: FilePolicy::Managed,
                    side: FileSide::Both,
                    url: None,
                },
                PackFile {
                    path: "config/a.toml".into(),
                    sha1: "cc".into(),
                    sha512: "dd".into(),
                    size: 1,
                    policy: FilePolicy::Initial,
                    side: FileSide::Client,
                    url: None,
                },
                PackFile {
                    path: "mods/server-only.jar".into(),
                    sha1: "ee".into(),
                    sha512: "ff".into(),
                    size: 1,
                    policy: FilePolicy::Managed,
                    side: FileSide::Server,
                    url: Some("https://cdn.modrinth.com/x.jar".into()),
                },
            ],
            created_at: Utc::now(),
        }
    }

    #[test]
    fn mrpack_index_has_loader_and_cdn_urls() {
        let index = manifest().to_mrpack_index("https://cdn.conexao.gg/", |_| false);
        assert_eq!(index["dependencies"]["neoforge"], "21.1.256");
        assert_eq!(index["dependencies"]["minecraft"], "1.21.1");
        let files = index["files"].as_array().unwrap();
        // arquivo só de servidor não vai para o cliente
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["downloads"][0], "https://cdn.conexao.gg/files/aa");
    }

    #[test]
    fn launcher_can_skip_initial_files() {
        let index = manifest().to_mrpack_index("https://cdn", |f| f.policy == FilePolicy::Initial);
        assert_eq!(index["files"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn manifest_roundtrip() {
        let m = manifest();
        let json = serde_json::to_string(&m).unwrap();
        let back: PackManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
        assert!(json.contains("\"neoforge\""));
    }
}
