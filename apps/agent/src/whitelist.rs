use std::path::Path;

use conexao_types::WhitelistPlayer;
use serde::Serialize;

#[derive(Serialize)]
struct Entry<'a> {
    uuid: String,
    name: &'a str,
}

/// Conteúdo do `whitelist.json` no formato do servidor vanilla.
pub fn render(players: &[WhitelistPlayer]) -> String {
    let mut players: Vec<_> = players.iter().collect();
    players.sort_by_key(|p| p.name.to_lowercase());
    players.dedup_by_key(|p| p.uuid);
    let entries: Vec<_> = players
        .iter()
        .map(|p| Entry { uuid: p.uuid.hyphenated().to_string(), name: &p.name })
        .collect();
    serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".into())
}

/// Escreve de forma atômica (arquivo temporário + rename).
pub async fn write(server_dir: &Path, players: &[WhitelistPlayer]) -> std::io::Result<()> {
    let target = server_dir.join("whitelist.json");
    let tmp = server_dir.join(".whitelist.json.conexao-tmp");
    tokio::fs::write(&tmp, render(players)).await?;
    tokio::fs::rename(&tmp, &target).await
}

/// Avisos sobre o `server.properties` (whitelist desligada não protege nada).
pub fn check_properties(text: &str) -> Vec<String> {
    let get = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(key).and_then(|rest| rest.strip_prefix('=')))
            .map(str::trim)
    };
    let mut warnings = Vec::new();
    if get("white-list") != Some("true") {
        warnings.push("server.properties: defina white-list=true para a whitelist do Conexão valer.".into());
    }
    if get("enforce-whitelist") != Some("true") {
        warnings.push("server.properties: defina enforce-whitelist=true para expulsar quem sair da whitelist.".into());
    }
    if get("enable-rcon") != Some("true") {
        warnings.push("server.properties: RCON desligado; a whitelist só será recarregada no próximo restart.".into());
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn renders_vanilla_format() {
        let uuid = Uuid::parse_str("069a79f444e94726a5befca90e38aaf5").unwrap();
        let json = render(&[WhitelistPlayer { uuid, name: "Notch".into() }]);
        assert!(json.contains("\"uuid\": \"069a79f4-44e9-4726-a5be-fca90e38aaf5\""));
        assert!(json.contains("\"name\": \"Notch\""));
    }

    #[test]
    fn property_warnings() {
        let ok = "white-list=true\nenforce-whitelist=true\nenable-rcon=true\n";
        assert!(check_properties(ok).is_empty());
        assert_eq!(check_properties("white-list=false").len(), 3);
    }

    #[tokio::test]
    async fn writes_atomically() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), &[]).await.unwrap();
        assert_eq!(std::fs::read_to_string(dir.path().join("whitelist.json")).unwrap(), "[]");
    }
}
