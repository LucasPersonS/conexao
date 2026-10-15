//! Detecção automática pelo endereço: o painel digita `play.servidor.com` e a
//! API pergunta ao próprio servidor (o mesmo ping da lista de multiplayer) a
//! versão, o MOTD, o ícone, o número de vagas e, quando o loader informa, os mods.

use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Query, State};
use conexao_types::{Loader, MinecraftVersion};
use serde::{Deserialize, Serialize};

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::netguard::{self, ResolveError};

#[derive(Deserialize)]
pub struct DetectQuery {
    pub address: String,
}

#[derive(Serialize, Default, Debug, PartialEq, Eq)]
pub struct DetectedMod {
    pub id: String,
    pub version: Option<String>,
}

#[derive(Serialize, Default, Debug)]
pub struct Detected {
    pub online: bool,
    pub address: String,
    pub version_name: Option<String>,
    /// Versão do Minecraft deduzida do nome da versão, se suportada.
    pub minecraft: Option<String>,
    pub loader: Option<Loader>,
    pub motd: Option<String>,
    /// `data:image/png;base64,...`
    pub favicon: Option<String>,
    pub players_online: i32,
    pub players_max: i32,
    pub latency_ms: Option<u32>,
    /// Mods anunciados pelo servidor (Forge/NeoForge costumam anunciar).
    pub mods: Vec<DetectedMod>,
    pub error: Option<String>,
}

/// Junta o texto de um componente de chat (`text` + `extra`), sem códigos de cor.
fn chat_text(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::String(s) => out.push_str(s),
        serde_json::Value::Array(items) => items.iter().for_each(|i| chat_text(i, out)),
        serde_json::Value::Object(map) => {
            if let Some(text) = map.get("text") {
                chat_text(text, out);
            }
            if let Some(extra) = map.get("extra") {
                chat_text(extra, out);
            }
        }
        _ => {}
    }
}

pub fn strip_formatting(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// "Paper 1.21.1", "NeoForge 1.21.1", "1.20.1" → primeira versão suportada encontrada.
pub fn minecraft_from_name(name: &str) -> Option<String> {
    name.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .filter(|t| t.contains('.'))
        .find_map(|t| MinecraftVersion::parse_supported(t.trim_matches('.')).ok())
        .map(|v| v.to_string())
}

/// Extrai o que o ping traz além do básico.
pub fn parse_raw(raw: &serde_json::Value, detected: &mut Detected) {
    if let Some(description) = raw.get("description") {
        let mut text = String::new();
        chat_text(description, &mut text);
        let text = strip_formatting(&text);
        if !text.is_empty() {
            detected.motd = Some(text);
        }
    }
    let name = raw.pointer("/version/name").and_then(|v| v.as_str()).unwrap_or_default();
    let lower = name.to_lowercase();
    if lower.contains("neoforge") {
        detected.loader = Some(Loader::NeoForge);
    } else if lower.contains("forge") {
        detected.loader = Some(Loader::Forge);
    } else if lower.contains("fabric") || lower.contains("quilt") {
        detected.loader = Some(Loader::Fabric);
    }
    // Forge 1.13+ (`forgeData.mods`) e FML antigo (`modinfo.modList`).
    let mods = raw
        .pointer("/forgeData/mods")
        .and_then(|v| v.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|m| {
                    Some(DetectedMod {
                        id: m.get("modId")?.as_str()?.to_string(),
                        version: m.get("modmarker").and_then(|v| v.as_str()).map(str::to_string),
                    })
                })
                .collect::<Vec<_>>()
        })
        .or_else(|| {
            raw.pointer("/modinfo/modList").and_then(|v| v.as_array()).map(|list| {
                list.iter()
                    .filter_map(|m| {
                        Some(DetectedMod {
                            id: m.get("modid")?.as_str()?.to_string(),
                            version: m.get("version").and_then(|v| v.as_str()).map(str::to_string),
                        })
                    })
                    .collect()
            })
        })
        .unwrap_or_default();
    if raw.get("forgeData").is_some() && detected.loader.is_none() {
        let neo = mods.iter().any(|m| m.id == "neoforge");
        detected.loader = Some(if neo { Loader::NeoForge } else { Loader::Forge });
    }
    detected.mods = mods
        .into_iter()
        .filter(|m| !["minecraft", "forge", "neoforge", "fml", "mcp"].contains(&m.id.as_str()))
        .collect();
}

pub async fn detect(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Query(query): Query<DetectQuery>,
) -> ApiResult<Json<Detected>> {
    let address = query.address.trim().trim_start_matches("minecraft://").to_string();
    if address.is_empty() || address.len() > 255 || address.contains(['/', ' ', '@']) {
        return Err(ApiError::bad("Endereço inválido. Use algo como play.seuservidor.com ou 1.2.3.4:25565."));
    }
    let mut detected = Detected { address: address.clone(), ..Default::default() };
    let target = match netguard::resolve(&address, state.config.allow_private_ping).await {
        Ok(target) => target,
        Err(ResolveError::Private) => {
            return Err(ApiError::bad("Use o endereço público do servidor (IPs de rede local não são aceitos)."));
        }
        Err(ResolveError::NotFound) => {
            detected.error = Some("Não encontramos esse endereço. Confira se está escrito certo.".into());
            return Ok(Json(detected));
        }
    };
    let config = netguard::ping_config(&target, Duration::from_secs(5));
    let started = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(10), async { config.connect().await?.status().await }).await;
    match result {
        Ok(Ok(connection)) => {
            detected.online = true;
            detected.latency_ms = u32::try_from(started.elapsed().as_millis()).ok();
            let status = &connection.status;
            detected.version_name = Some(status.version.name.clone());
            detected.minecraft = minecraft_from_name(&status.version.name);
            detected.players_online = status.players.online;
            detected.players_max = status.players.max;
            detected.favicon = status.favicon.clone().filter(|f| f.starts_with("data:image/png;base64,") && f.len() < 200_000);
            if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&connection.raw) {
                parse_raw(&raw, &mut detected);
            }
        }
        // Sem o erro bruto: ele revelaria se a porta existe (mapa da rede).
        Ok(Err(_)) => detected.error = Some("O servidor não respondeu ao ping. Ele está ligado e com a porta aberta?".into()),
        Err(_) => detected.error = Some("O servidor não respondeu em 10 segundos. Ele está ligado?".into()),
    }
    Ok(Json(detected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_versions_from_names() {
        assert_eq!(minecraft_from_name("NeoForge 1.21.1").as_deref(), Some("1.21.1"));
        assert_eq!(minecraft_from_name("Paper 1.20.4").as_deref(), Some("1.20.4"));
        assert_eq!(minecraft_from_name("1.12.2"), None);
        assert_eq!(minecraft_from_name("Velocity 3.3.0"), None);
    }

    #[test]
    fn parses_forge_status() {
        let raw = serde_json::json!({
            "version": {"name": "1.20.1", "protocol": 763},
            "description": {"text": "", "extra": [{"text": "§aCidade "}, {"text": "Alta RP"}]},
            "forgeData": {"mods": [{"modId": "forge", "modmarker": "47.3.0"}, {"modId": "create", "modmarker": "0.5.1.f"}]}
        });
        let mut detected = Detected::default();
        parse_raw(&raw, &mut detected);
        assert_eq!(detected.motd.as_deref(), Some("Cidade Alta RP"));
        assert_eq!(detected.loader, Some(Loader::Forge));
        assert_eq!(detected.mods, vec![DetectedMod { id: "create".into(), version: Some("0.5.1.f".into()) }]);
    }
}
