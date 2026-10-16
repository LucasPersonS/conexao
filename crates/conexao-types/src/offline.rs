//! Contas não originais (servidores em `online-mode=false`).

use uuid::Uuid;

/// UUID que o servidor Minecraft atribui a um jogador em `online-mode=false`:
/// `UUID.nameUUIDFromBytes("OfflinePlayer:" + nick)` (MD5, versão 3).
pub fn offline_uuid(name: &str) -> Uuid {
    let mut bytes = md5::compute(format!("OfflinePlayer:{name}").as_bytes()).0;
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Nick válido no Minecraft: 3 a 16 caracteres, letras, números e `_`.
pub fn valid_nick(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_java_name_uuid_from_bytes() {
        // Valores conferidos com `UUID.nameUUIDFromBytes` do Java.
        assert_eq!(offline_uuid("Notch").to_string(), "b50ad385-829d-3141-a216-7e7d7539ba7f");
        assert_eq!(offline_uuid("jeb_").to_string(), "a762f560-4fce-3236-812a-b80efff0b62b");
    }

    #[test]
    fn nick_rules() {
        assert!(valid_nick("Lucas_01"));
        assert!(!valid_nick("ab"));
        assert!(!valid_nick("nome com espaço"));
        assert!(!valid_nick("abcdefghijklmnopq"));
    }
}
