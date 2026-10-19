//! Segredos guardados no banco (tokens OAuth do Mercado Pago dos donos de servidor).
//!
//! AES-256-GCM com chave fora do banco (`CONEXAO_SECRETS_KEY`) e nonce aleatório por valor.
//! O `context` (ex.: id do servidor) entra como dado associado: um token copiado para a
//! linha de outro servidor não decifra, então trocar linhas no banco não adianta.

use aes_gcm::aead::{Aead, KeyInit, OsRng, Payload, rand_core::RngCore};
use aes_gcm::{Aes256Gcm, Nonce};

use crate::error::{ApiError, ApiResult};

const NONCE_LEN: usize = 12;
const VERSION: u8 = 1;

pub fn seal(key: &[u8; 32], context: &[u8], plaintext: &str) -> ApiResult<Vec<u8>> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let sealed = cipher
        .encrypt(&Nonce::from(nonce), Payload { msg: plaintext.as_bytes(), aad: context })
        .map_err(|_| ApiError::Internal(eyre::eyre!("falha ao criptografar segredo")))?;
    let mut out = Vec::with_capacity(1 + NONCE_LEN + sealed.len());
    out.push(VERSION);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

pub fn open(key: &[u8; 32], context: &[u8], data: &[u8]) -> ApiResult<String> {
    let fail = || ApiError::Internal(eyre::eyre!("segredo ilegível (chave trocada ou dado adulterado)"));
    if data.len() <= 1 + NONCE_LEN || data[0] != VERSION {
        return Err(fail());
    }
    let (nonce, sealed) = data[1..].split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| fail())?;
    let plain = Aes256Gcm::new(key.into())
        .decrypt(&Nonce::from(nonce), Payload { msg: sealed, aad: context })
        .map_err(|_| fail())?;
    String::from_utf8(plain).map_err(|_| fail())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_tamper() {
        let key = [7u8; 32];
        let sealed = seal(&key, b"server-a", "APP_USR-segredo").unwrap();
        assert_eq!(open(&key, b"server-a", &sealed).unwrap(), "APP_USR-segredo");
        // outro contexto (linha trocada), outra chave ou um byte alterado: recusa
        assert!(open(&key, b"server-b", &sealed).is_err());
        assert!(open(&[8u8; 32], b"server-a", &sealed).is_err());
        let mut bad = sealed.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(open(&key, b"server-a", &bad).is_err());
        // nonce novo a cada vez
        assert_ne!(seal(&key, b"server-a", "x").unwrap(), seal(&key, b"server-a", "x").unwrap());
    }
}
