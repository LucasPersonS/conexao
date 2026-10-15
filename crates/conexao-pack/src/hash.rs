use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha512};

use crate::error::io_err;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHashes {
    pub sha1: String,
    pub sha512: String,
    pub size: u64,
}

/// Calcula SHA-1 e SHA-512 (os dois hashes exigidos pelo formato `.mrpack`)
/// lendo o arquivo uma única vez.
pub fn hash_file(path: &Path) -> crate::Result<FileHashes> {
    let mut file = File::open(path).map_err(io_err(path))?;
    let mut sha1 = sha1_smol::Sha1::new();
    let mut sha512 = Sha512::new();
    let mut size = 0u64;
    let mut buffer = vec![0u8; 256 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(io_err(path))?;
        if read == 0 {
            break;
        }
        let chunk = buffer.get(..read).unwrap_or_default();
        sha1.update(chunk);
        sha512.update(chunk);
        size += read as u64;
    }
    Ok(FileHashes {
        sha1: sha1.digest().to_string(),
        sha512: hex::encode(sha512.finalize()),
        size,
    })
}
