//! Regras de segurança e de espaço para o que entra num modpack.

use std::collections::HashSet;

use conexao_types::PackFile;
use uuid::Uuid;

use crate::error::ApiResult;
use crate::state::AppState;

/// Hosts aceitos para downloads externos (os mesmos permitidos pelo formato `.mrpack`).
pub const EXTERNAL_HOSTS: &[&str] = &[
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
    "edge.forgecdn.net",
    "mediafilez.forgecdn.net",
];

pub fn external_url_allowed(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else { return false };
    parsed.scheme() == "https"
        && parsed.username().is_empty()
        && parsed.port().is_none()
        && parsed.host_str().is_some_and(|host| EXTERNAL_HOSTS.contains(&host))
}

/// Extensões que nunca entram num modpack (executáveis e scripts do sistema).
const BLOCKED_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "bat", "cmd", "ps1", "psm1", "sh", "bash", "vbs", "vbe", "msi", "scr", "com", "pif",
    "lnk", "reg", "hta", "app", "command", "jnlp", "class",
];

/// Confere se o arquivo pode entrar no modpack. Retorna o motivo da recusa.
pub fn check_path(path: &str) -> Result<(), String> {
    let lower = path.to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or_default();
    let extension = name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or_default();
    if BLOCKED_EXTENSIONS.contains(&extension) {
        return Err(format!("{path}: arquivos .{extension} não são permitidos em modpacks."));
    }
    let top = lower.split('/').next().unwrap_or_default();
    if extension == "jar" && top != "mods" {
        return Err(format!("{path}: arquivos .jar só podem ficar na pasta mods/."));
    }
    if top == "mods" && lower.matches('/').count() == 1 && extension != "jar" {
        return Err(format!("{path}: a pasta mods/ só aceita arquivos .jar."));
    }
    Ok(())
}

/// Bytes hospedados no Conexão (arquivos sem link externo), contando cada arquivo uma vez.
pub fn hosted_bytes(files: &[PackFile]) -> u64 {
    let mut seen = HashSet::new();
    files
        .iter()
        .filter(|f| f.url.is_none() && seen.insert(f.sha1.as_str()))
        .map(|f| f.size)
        .sum()
}

/// Cota do servidor: maior para quem está em destaque (plano pago).
pub async fn quota_for(state: &AppState, server_id: Uuid) -> ApiResult<u64> {
    let featured: bool =
        sqlx::query_scalar("SELECT featured_until IS NOT NULL AND featured_until > now() FROM servers WHERE id = $1")
            .bind(server_id)
            .fetch_one(&state.pool)
            .await?;
    Ok(if featured { state.config.featured_quota_bytes } else { state.config.server_quota_bytes })
}

pub fn megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / 1024.0 / 1024.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_hosts() {
        assert!(external_url_allowed("https://cdn.modrinth.com/data/abc/versions/1/x.jar"));
        assert!(!external_url_allowed("http://cdn.modrinth.com/x.jar"));
        assert!(!external_url_allowed("https://cdn.modrinth.com.evil.io/x.jar"));
        assert!(!external_url_allowed("https://user@cdn.modrinth.com/x.jar"));
    }

    #[test]
    fn paths() {
        assert!(check_path("mods/create.jar").is_ok());
        assert!(check_path("config/create-client.toml").is_ok());
        assert!(check_path("kubejs/server_scripts/main.js").is_ok());
        assert!(check_path("mods/run.bat").is_err());
        assert!(check_path("config/evil.exe").is_err());
        assert!(check_path("config/sneaky.jar").is_err());
        assert!(check_path("mods/notes.txt").is_err());
    }
}
