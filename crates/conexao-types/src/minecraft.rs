use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Versão mínima aceita pelo Conexão (decisão de produto: 1.18.2 ou mais nova).
pub const MIN_SUPPORTED_MINECRAFT: MinecraftVersion = MinecraftVersion {
    major: 1,
    minor: 18,
    patch: 2,
};

/// Versão *release* do Minecraft (`1.20.1`, `1.21`, `1.21.1`, `26.1`, `26.1.2`).
///
/// Snapshots, pre-releases e release candidates não são aceitos: servidores
/// parceiros devem rodar uma release estável.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MinecraftVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MinecraftVersionError {
    #[error("versão do Minecraft inválida: {0}")]
    Invalid(String),
    #[error("o Conexão suporta apenas Minecraft {min} ou mais novo (recebido {got})")]
    Unsupported { got: String, min: MinecraftVersion },
}

impl MinecraftVersion {
    /// Faz o parse e já valida a versão mínima suportada.
    pub fn parse_supported(value: &str) -> Result<Self, MinecraftVersionError> {
        let version: Self = value.parse()?;
        if version < MIN_SUPPORTED_MINECRAFT {
            return Err(MinecraftVersionError::Unsupported {
                got: value.to_string(),
                min: MIN_SUPPORTED_MINECRAFT,
            });
        }
        Ok(version)
    }

    pub fn is_supported(&self) -> bool {
        *self >= MIN_SUPPORTED_MINECRAFT
    }

    /// Versão principal de Java exigida pela Mojang para esta versão.
    pub fn required_java(&self) -> u32 {
        // 26.x exige Java 25; 1.20.5+ exige Java 21; 1.18 até 1.20.4 exige Java 17.
        // (Informativo: o theseus usa o `javaVersion` do JSON oficial da versão.)
        if self.major >= 26 {
            25
        } else if (self.minor, self.patch) >= (20, 5) || self.minor >= 21 {
            21
        } else {
            17
        }
    }
}

impl FromStr for MinecraftVersion {
    type Err = MinecraftVersionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = || MinecraftVersionError::Invalid(value.to_string());
        let mut parts = value.trim().split('.');
        let mut next = |required: bool| -> Result<Option<u32>, MinecraftVersionError> {
            match parts.next() {
                Some(part) if !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()) => {
                    part.parse().map(Some).map_err(|_| invalid())
                }
                None if !required => Ok(None),
                _ => Err(invalid()),
            }
        };
        let major = next(true)?.ok_or_else(invalid)?;
        let minor = next(true)?.ok_or_else(invalid)?;
        let patch = next(false)?.unwrap_or(0);
        // Releases antigas são 1.x.y; desde 2026 a Mojang numera por ano (26.1, 26.1.2).
        if parts.next().is_some() || !(major == 1 || (26..=99).contains(&major)) {
            return Err(invalid());
        }
        Ok(Self { major, minor, patch })
    }
}

impl TryFrom<String> for MinecraftVersion {
    type Error = MinecraftVersionError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<MinecraftVersion> for String {
    fn from(value: MinecraftVersion) -> Self {
        value.to_string()
    }
}

impl fmt::Display for MinecraftVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.patch == 0 {
            write!(f, "{}.{}", self.major, self.minor)
        } else {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        }
    }
}

impl PartialOrd for MinecraftVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MinecraftVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_releases() {
        assert_eq!("1.21".parse::<MinecraftVersion>().unwrap().to_string(), "1.21");
        assert_eq!("1.20.1".parse::<MinecraftVersion>().unwrap().patch, 1);
    }

    #[test]
    fn rejects_snapshots_and_garbage() {
        for bad in ["24w14a", "1.20.5-pre1", "1.20.", "", "2.0", "1.x", "26.4-snapshot-3", "26.3-rc-1"] {
            assert!(bad.parse::<MinecraftVersion>().is_err(), "{bad}");
        }
    }

    #[test]
    fn enforces_minimum() {
        assert!(MinecraftVersion::parse_supported("1.18.2").is_ok());
        assert!(MinecraftVersion::parse_supported("1.21.1").is_ok());
        assert!(matches!(
            MinecraftVersion::parse_supported("1.18.1"),
            Err(MinecraftVersionError::Unsupported { .. })
        ));
        assert!(MinecraftVersion::parse_supported("1.12.2").is_err());
    }

    #[test]
    fn java_requirements() {
        let java = |v: &str| v.parse::<MinecraftVersion>().unwrap().required_java();
        assert_eq!(java("1.18.2"), 17);
        assert_eq!(java("1.20.4"), 17);
        assert_eq!(java("1.20.6"), 21);
        assert_eq!(java("1.21.1"), 21);
        assert_eq!(java("26.1"), 25);
    }

    #[test]
    fn year_versions() {
        let v: MinecraftVersion = "26.1.2".parse().unwrap();
        assert_eq!(v.to_string(), "26.1.2");
        assert!(v > "1.21.11".parse().unwrap());
        assert!(v.is_supported());
    }
}
