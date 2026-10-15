//! Armazenamento dos arquivos dos modpacks no disco da VPS.
//!
//! Cada arquivo é guardado uma vez, pelo SHA-1 (`{storage}/files/{sha1}`),
//! e servido em `/files/{sha1}`. Em produção, Caddy/Nginx pode servir essa
//! pasta diretamente com cache de CDN.

use std::path::{Path, PathBuf};

pub fn is_sha1(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

pub fn files_dir(root: &Path) -> PathBuf {
    root.join("files")
}

pub fn file_path(root: &Path, sha1: &str) -> PathBuf {
    files_dir(root).join(sha1)
}

pub async fn ensure_dirs(root: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(files_dir(root)).await?;
    tokio::fs::create_dir_all(root.join("images")).await?;
    tokio::fs::create_dir_all(root.join("tmp")).await
}

/// Arquivo temporário novo dentro do storage (mesmo disco, para o `rename` ser atômico).
pub fn tmp_path(root: &Path, label: &str) -> PathBuf {
    root.join("tmp").join(format!("{label}.{}", uuid::Uuid::new_v4()))
}

/// Move um temporário já conferido para o lugar definitivo (ou descarta, se já existir).
pub async fn commit_tmp(root: &Path, sha1: &str, tmp: &Path) -> std::io::Result<()> {
    let target = file_path(root, sha1);
    if tokio::fs::try_exists(&target).await? {
        let _ = tokio::fs::remove_file(tmp).await;
        return Ok(());
    }
    tokio::fs::rename(tmp, &target).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha1_shape() {
        assert!(super::is_sha1("da39a3ee5e6b4b0d3255bfef95601890afd80709"));
        assert!(!super::is_sha1("DA39A3EE5E6B4B0D3255BFEF95601890AFD80709"));
        assert!(!super::is_sha1("../etc/passwd"));
    }
}
