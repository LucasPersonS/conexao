//! Assinatura Ed25519 dos manifestos.
//!
//! A API assina os bytes exatos do manifesto que envia (cabeçalho
//! `X-Conexao-Signature`, base64) e o launcher confere com a chave pública
//! embutida no build. Assim, mesmo que alguém intercepte a conexão ou invada o
//! CDN, não consegue trocar os arquivos que os jogadores instalam.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
pub use ed25519_dalek::SigningKey;

/// Apelido usado pela configuração da API.
pub type SigningKeyHandle = SigningKey;

pub const SIGNATURE_HEADER: &str = "x-conexao-signature";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SigningError {
    #[error("chave inválida")]
    InvalidKey,
    #[error("assinatura ausente ou malformada")]
    InvalidSignature,
    #[error("a assinatura do manifesto não confere")]
    Mismatch,
}

/// Chave de assinatura a partir da semente de 32 bytes em base64.
pub fn signing_key(seed_b64: &str) -> Result<SigningKey, SigningError> {
    let bytes = STANDARD.decode(seed_b64.trim()).map_err(|_| SigningError::InvalidKey)?;
    let seed: [u8; 32] = bytes.try_into().map_err(|_| SigningError::InvalidKey)?;
    Ok(SigningKey::from_bytes(&seed))
}

pub fn public_key_b64(key: &SigningKey) -> String {
    STANDARD.encode(key.verifying_key().to_bytes())
}

pub fn sign(key: &SigningKey, body: &[u8]) -> String {
    STANDARD.encode(key.sign(body).to_bytes())
}

pub fn verify(public_key_b64: &str, body: &[u8], signature_b64: &str) -> Result<(), SigningError> {
    let key_bytes: [u8; 32] = STANDARD
        .decode(public_key_b64.trim())
        .map_err(|_| SigningError::InvalidKey)?
        .try_into()
        .map_err(|_| SigningError::InvalidKey)?;
    let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| SigningError::InvalidKey)?;
    let sig_bytes: [u8; 64] = STANDARD
        .decode(signature_b64.trim())
        .map_err(|_| SigningError::InvalidSignature)?
        .try_into()
        .map_err(|_| SigningError::InvalidSignature)?;
    key.verify(body, &Signature::from_bytes(&sig_bytes)).map_err(|_| SigningError::Mismatch)
}

/// Gera uma semente nova (base64) a partir de 32 bytes aleatórios.
pub fn seed_from_bytes(bytes: [u8; 32]) -> String {
    STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify() {
        let seed = seed_from_bytes([7; 32]);
        let key = signing_key(&seed).unwrap();
        let public = public_key_b64(&key);
        let body = br#"{"version":"1.0.0"}"#;
        let signature = sign(&key, body);
        assert_eq!(verify(&public, body, &signature), Ok(()));
        assert_eq!(verify(&public, br#"{"version":"6.6.6"}"#, &signature), Err(SigningError::Mismatch));
        assert_eq!(verify(&public, body, "lixo"), Err(SigningError::InvalidSignature));
        assert!(signing_key("curta").is_err());
    }
}
