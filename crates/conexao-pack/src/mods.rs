use std::fs::File;
use std::io::Read;
use std::path::Path;

use conexao_types::Loader;

use crate::error::{fail, io_err};

const MAX_METADATA: u64 = 1024 * 1024;

/// Metadados extraídos de um JAR de mod.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModInfo {
    pub mod_ids: Vec<String>,
    pub name: String,
    pub version: String,
    /// Loader para o qual o JAR foi feito; `None` quando o `mods.toml`
    /// pode servir tanto Forge quanto NeoForge antigo (1.20.1).
    pub loader: Option<Loader>,
    pub supports: Vec<Loader>,
}

fn read_entry(archive: &mut zip::ZipArchive<File>, name: &str) -> crate::Result<Option<String>> {
    let Ok(mut entry) = archive.by_name(name) else {
        return Ok(None);
    };
    if entry.size() > MAX_METADATA {
        return fail(format!("Metadados {name} excedem 1 MB."));
    }
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .map_err(|_| crate::PackError::Invalid(format!("{name} não é texto UTF-8 válido.")))?;
    Ok(Some(text))
}

fn valid_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 100
        && version.bytes().all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
}

fn valid_mod_id(id: &str) -> bool {
    let mut bytes = id.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && (2..=64).contains(&id.len())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Lê `neoforge.mods.toml`, `mods.toml` ou `fabric.mod.json` de um JAR.
pub fn read_mod_info(path: &Path) -> crate::Result<ModInfo> {
    let file = File::open(path).map_err(io_err(path))?;
    let mut archive = zip::ZipArchive::new(file)?;

    let neoforge = read_entry(&mut archive, "META-INF/neoforge.mods.toml")?;
    let forge = if neoforge.is_none() {
        read_entry(&mut archive, "META-INF/mods.toml")?
    } else {
        None
    };
    if let Some((text, is_neo)) = neoforge.map(|t| (t, true)).or(forge.map(|t| (t, false))) {
        let data: toml::Value = toml::from_str(&text)
            .map_err(|e| crate::PackError::Invalid(format!("mods.toml inválido: {e}")))?;
        let Some(mods) = data.get("mods").and_then(|m| m.as_array()).filter(|m| !m.is_empty()) else {
            return fail("Metadados do mod sem a lista [[mods]].");
        };
        let mut mod_ids = Vec::new();
        for entry in mods {
            match entry.get("modId").and_then(|v| v.as_str()) {
                Some(id) if valid_mod_id(id) => mod_ids.push(id.to_string()),
                _ => return fail("Metadados do mod sem modId válido."),
            }
        }
        let first = mods.first();
        let mut version = first
            .and_then(|m| m.get("version"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if version.contains("${") {
            version = read_entry(&mut archive, "META-INF/MANIFEST.MF")?
                .and_then(|manifest| {
                    manifest.lines().find_map(|line| {
                        line.strip_prefix("Implementation-Version:").map(|v| v.trim().to_string())
                    })
                })
                .unwrap_or_else(|| "0.0.0".to_string());
        }
        if !valid_version(&version) {
            return fail("Versão do mod contém caracteres incompatíveis.");
        }
        let name = first
            .and_then(|m| m.get("displayName"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| mod_ids.first().cloned().unwrap_or_default());
        let (loader, supports) = if is_neo {
            (Some(Loader::NeoForge), vec![Loader::NeoForge])
        } else {
            // mods.toml: Forge, e NeoForge até 20.4 também usava este nome.
            (None, vec![Loader::Forge, Loader::NeoForge])
        };
        return Ok(ModInfo { mod_ids, name, version, loader, supports });
    }

    if let Some(text) = read_entry(&mut archive, "fabric.mod.json")? {
        let data: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| crate::PackError::Invalid(format!("fabric.mod.json inválido: {e}")))?;
        let id = data.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let version = data
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0")
            .to_string();
        let name = data
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| id.clone());
        return Ok(ModInfo {
            mod_ids: if id.is_empty() { vec![] } else { vec![id] },
            name,
            version,
            loader: Some(Loader::Fabric),
            supports: vec![Loader::Fabric],
        });
    }

    if read_entry(&mut archive, "quilt.mod.json")?.is_some() {
        return fail("Mod Quilt: o Conexão suporta apenas Fabric, Forge e NeoForge.");
    }
    fail("O JAR não possui metadados de mod NeoForge, Forge ou Fabric.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_rules() {
        assert!(valid_mod_id("create"));
        assert!(valid_mod_id("jei_addon"));
        assert!(!valid_mod_id("Create"));
        assert!(!valid_mod_id("1abc"));
        assert!(valid_version("1.20.1-0.5.1.f"));
        assert!(!valid_version("1.0 beta"));
    }
}
