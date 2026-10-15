use std::path::PathBuf;

use eyre::{WrapErr, eyre};

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: String,
    pub database_url: String,
    pub database_max_connections: u32,
    /// Segredo HS256 dos JWT emitidos para launcher e painel.
    pub jwt_secret: String,
    /// URL pública da API/CDN (ex.: `https://api.conexao.gg`), usada nos downloads.
    pub public_url: String,
    /// Endereço do painel web (links de vínculo do Companion).
    pub panel_url: String,
    /// Pasta da VPS onde os arquivos dos modpacks ficam, endereçados por SHA-1.
    pub storage_dir: PathBuf,
    /// Token da equipe Conexão (aprovar servidores, destaque pago).
    pub staff_token: Option<String>,
    pub max_upload_bytes: usize,
    /// Quantos servidores ainda não aprovados uma conta pode ter ao mesmo tempo.
    pub max_pending_servers: i64,
    /// Origens permitidas para o painel web (CORS). Vazio = qualquer origem.
    pub cors_origins: Vec<String>,
    /// UUIDs Minecraft da equipe Conexão; viram `is_staff` ao fazer login.
    pub staff_uuids: Vec<uuid::Uuid>,
    /// Endpoint de perfil da Minecraft Services (configurável para testes).
    pub minecraft_profile_url: String,
    pub mojang_lookup_url: String,
    /// Chave Ed25519 que assina os manifestos (`CONEXAO_SIGNING_KEY`, semente base64).
    pub signing_key: Option<conexao_types::signing::SigningKeyHandle>,
    /// Metadados de versões (Minecraft e loaders): os mesmos que o launcher usa.
    pub launcher_meta_url: String,
    /// API do Modrinth, usada para reconhecer mods pelo hash e linkar o download.
    pub modrinth_api_url: String,
    /// Espaço que cada servidor pode ocupar com arquivos hospedados no Conexão
    /// (o que não existe no Modrinth: configs, scripts, mods próprios).
    pub server_quota_bytes: u64,
    /// Cota maior para servidores em destaque (plano pago).
    pub featured_quota_bytes: u64,
    /// Permite detectar servidores em IPs de rede local (só para testes locais).
    pub allow_private_ping: bool,
    /// Link fixo do instalador; sem ele, `/baixar` usa o `latest.json` do storage.
    pub download_url: Option<String>,
    /// Itens premium do perfil só para quem tem Conexão+ (desligado no lançamento).
    pub premium_enforced: bool,
    /// Contas criadas antes desta data ganham a insígnia "Pioneiro".
    pub pioneer_until: Option<chrono::DateTime<chrono::Utc>>,
    /// Mercado Pago (destaque pago). Sem token, a compra fica desativada.
    pub mp_access_token: Option<String>,
    pub mp_webhook_secret: Option<String>,
    pub mp_api_url: String,
    /// Planos de destaque: dias → preço em centavos.
    pub featured_plans: Vec<(i32, i32)>,
    /// Produção = `CONEXAO_PUBLIC_URL` em https fora de localhost. Liga as checagens de segurança.
    pub production: bool,
    /// Webhook (Discord) para avisos de disco cheio e backup atrasado.
    pub alert_webhook: Option<String>,
    /// Acima disto (% do disco em uso) o monitor avisa no webhook.
    pub disk_warn_percent: f64,
    /// Acima disto o `/health` responde 503 (o UptimeRobot avisa).
    pub disk_alert_percent: f64,
    /// Pasta com os dumps do serviço `backup` (montada só para leitura).
    pub backup_dir: Option<PathBuf>,
}

/// Lê `CONEXAO_*`; se não existir, aceita o nome antigo (`CENTRAL_*`) para quem ainda não migrou.
pub(crate) fn var(name: &str) -> Option<String> {
    let read = |key: &str| std::env::var(key).ok().filter(|v| !v.trim().is_empty());
    read(name).or_else(|| name.strip_prefix("CONEXAO_").and_then(|rest| read(&format!("CENTRAL_{rest}"))))
}

/// Produção = endereço público em https que não é a própria máquina.
pub fn is_production_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', ':']).next().unwrap_or_default();
    !matches!(host, "localhost" | "127.0.0.1" | "[::1]") && !host.ends_with(".localhost")
}

/// `"7:29.90,15:49.90"` → [(7, 2990), (15, 4990)].
pub fn parse_plans(value: &str) -> eyre::Result<Vec<(i32, i32)>> {
    value
        .split(',')
        .filter(|p| !p.trim().is_empty())
        .map(|plan| {
            let (days, price) = plan.trim().split_once(':').ok_or_else(|| eyre!("plano inválido: {plan}"))?;
            let days: i32 = days.trim().parse().wrap_err_with(|| format!("dias inválidos em {plan}"))?;
            let price: f64 = price.trim().replace(',', ".").parse().wrap_err_with(|| format!("preço inválido em {plan}"))?;
            let cents = (price * 100.0).round() as i32;
            if days <= 0 || days > 365 || cents <= 0 {
                return Err(eyre!("plano fora dos limites: {plan}"));
            }
            Ok((days, cents))
        })
        .collect()
}

fn percent(name: &str, default: f64) -> eyre::Result<f64> {
    let value = var(name).map(|v| v.parse::<f64>()).transpose().wrap_err_with(|| format!("{name} inválido"))?;
    let value = value.unwrap_or(default);
    if !(1.0..=100.0).contains(&value) {
        return Err(eyre!("{name} deve ficar entre 1 e 100"));
    }
    Ok(value)
}

fn megabytes(name: &str, default: u64) -> eyre::Result<u64> {
    let value = var(name).map(|v| v.parse::<u64>()).transpose().wrap_err_with(|| format!("{name} inválido"))?;
    Ok(value.unwrap_or(default) * 1024 * 1024)
}

impl Config {
    pub fn from_env() -> eyre::Result<Self> {
        let config = Self::read_env()?;
        config.validate_production()?;
        Ok(config)
    }

    /// Em produção a API não sobe com configuração insegura.
    pub fn validate_production(&self) -> eyre::Result<()> {
        if !self.production {
            return Ok(());
        }
        let mut missing = Vec::new();
        if self.signing_key.is_none() {
            missing.push("CONEXAO_SIGNING_KEY (gere com: conexao-api gerar-chave-assinatura)");
        }
        if self.cors_origins.is_empty() {
            missing.push("CONEXAO_CORS_ORIGINS (ex.: https://painel.seudominio)");
        }
        if self.mp_access_token.is_some() && self.mp_webhook_secret.is_none() {
            missing.push("CONEXAO_MP_WEBHOOK_SECRET (obrigatória junto com CONEXAO_MP_ACCESS_TOKEN)");
        }
        if !self.panel_url.starts_with("https://") {
            missing.push("CONEXAO_PANEL_URL em https");
        }
        let external = [&self.minecraft_profile_url, &self.mojang_lookup_url, &self.mp_api_url, &self.modrinth_api_url];
        if external.iter().any(|url| !url.starts_with("https://")) {
            missing.push("URLs de serviços externos (CONEXAO_*_URL) em https");
        }
        if self.allow_private_ping {
            missing.push("CONEXAO_ALLOW_PRIVATE_PING desligado (só para testes locais)");
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(eyre!("configuração de produção incompleta:\n  - {}", missing.join("\n  - ")))
        }
    }

    fn read_env() -> eyre::Result<Self> {
        let jwt_secret = var("CONEXAO_JWT_SECRET").ok_or_else(|| eyre!("defina CONEXAO_JWT_SECRET"))?;
        if jwt_secret.len() < 32 {
            return Err(eyre!("CONEXAO_JWT_SECRET deve ter pelo menos 32 caracteres"));
        }
        let public_url = var("CONEXAO_PUBLIC_URL")
            .unwrap_or_else(|| "http://localhost:8080".into())
            .trim_end_matches('/')
            .to_string();
        let production = is_production_url(&public_url);
        Ok(Self {
            bind: var("CONEXAO_BIND").unwrap_or_else(|| "0.0.0.0:8080".into()),
            database_url: var("DATABASE_URL").ok_or_else(|| eyre!("defina DATABASE_URL"))?,
            database_max_connections: var("DATABASE_MAX_CONNECTIONS")
                .map(|v| v.parse())
                .transpose()
                .wrap_err("DATABASE_MAX_CONNECTIONS inválido")?
                .unwrap_or(10),
            jwt_secret,
            public_url,
            panel_url: var("CONEXAO_PANEL_URL")
                .unwrap_or_else(|| "http://localhost:5174".into())
                .trim_end_matches('/')
                .to_string(),
            storage_dir: var("CONEXAO_STORAGE_DIR").unwrap_or_else(|| "./data".into()).into(),
            staff_token: var("CONEXAO_STAFF_TOKEN"),
            max_upload_bytes: var("CONEXAO_MAX_UPLOAD_MB")
                .map(|v| v.parse::<usize>())
                .transpose()
                .wrap_err("CONEXAO_MAX_UPLOAD_MB inválido")?
                .unwrap_or(100)
                * 1024
                * 1024,
            max_pending_servers: var("CONEXAO_MAX_PENDING_SERVERS")
                .map(|v| v.parse::<i64>())
                .transpose()
                .wrap_err("CONEXAO_MAX_PENDING_SERVERS inválido")?
                .unwrap_or(3),
            cors_origins: var("CONEXAO_CORS_ORIGINS")
                .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                .unwrap_or_default(),
            staff_uuids: var("CONEXAO_STAFF_UUIDS")
                .map(|v| v.split(',').filter_map(|s| uuid::Uuid::parse_str(s.trim()).ok()).collect())
                .unwrap_or_default(),
            minecraft_profile_url: var("CONEXAO_MINECRAFT_PROFILE_URL")
                .unwrap_or_else(|| "https://api.minecraftservices.com/minecraft/profile".into()),
            mojang_lookup_url: var("CONEXAO_MOJANG_LOOKUP_URL")
                .unwrap_or_else(|| "https://api.mojang.com/users/profiles/minecraft".into()),
            launcher_meta_url: var("CONEXAO_LAUNCHER_META_URL")
                .unwrap_or_else(|| "https://launcher-meta.modrinth.com".into())
                .trim_end_matches('/')
                .to_string(),
            modrinth_api_url: var("CONEXAO_MODRINTH_API_URL")
                .unwrap_or_else(|| "https://api.modrinth.com/v2".into())
                .trim_end_matches('/')
                .to_string(),
            server_quota_bytes: megabytes("CONEXAO_SERVER_QUOTA_MB", 300)?,
            featured_quota_bytes: megabytes("CONEXAO_FEATURED_QUOTA_MB", 1024)?,
            download_url: var("CONEXAO_DOWNLOAD_URL"),
            premium_enforced: var("CONEXAO_PREMIUM_ENFORCED").is_some_and(|v| v == "1" || v == "true"),
            pioneer_until: var("CONEXAO_PIONEER_UNTIL")
                .map(|v| chrono::DateTime::parse_from_rfc3339(&v).map(|d| d.with_timezone(&chrono::Utc)))
                .transpose()
                .wrap_err("CONEXAO_PIONEER_UNTIL inválido (use 2027-01-01T00:00:00Z)")?,
            mp_access_token: var("CONEXAO_MP_ACCESS_TOKEN"),
            mp_webhook_secret: var("CONEXAO_MP_WEBHOOK_SECRET"),
            mp_api_url: var("CONEXAO_MP_API_URL")
                .unwrap_or_else(|| "https://api.mercadopago.com".into())
                .trim_end_matches('/')
                .to_string(),
            featured_plans: parse_plans(&var("CONEXAO_FEATURED_PLANS").unwrap_or_else(|| "7:29.90,15:49.90,30:89.90".into()))?,
            allow_private_ping: var("CONEXAO_ALLOW_PRIVATE_PING").is_some_and(|v| v == "1" || v == "true"),
            signing_key: var("CONEXAO_SIGNING_KEY")
                .map(|seed| conexao_types::signing::signing_key(&seed))
                .transpose()
                .map_err(|_| eyre!("CONEXAO_SIGNING_KEY inválida (use: conexao-api gerar-chave-assinatura)"))?,
            production,
            alert_webhook: var("CONEXAO_ALERT_WEBHOOK")
                .map(|url| {
                    if url.starts_with("https://") { Ok(url) } else { Err(eyre!("CONEXAO_ALERT_WEBHOOK precisa ser https")) }
                })
                .transpose()?,
            disk_warn_percent: percent("CONEXAO_DISK_WARN_PERCENT", 85.0)?,
            disk_alert_percent: percent("CONEXAO_DISK_ALERT_PERCENT", 90.0)?,
            backup_dir: var("CONEXAO_BACKUP_DIR").map(PathBuf::from),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::is_production_url;

    #[test]
    fn production_is_public_https() {
        assert!(is_production_url("https://api.conexaosmp.com.br"));
        assert!(is_production_url("https://api.exemplo.gg:8443/base"));
        assert!(!is_production_url("http://api.exemplo.gg"));
        assert!(!is_production_url("https://localhost:8080"));
        assert!(!is_production_url("https://127.0.0.1"));
        assert!(!is_production_url("https://api.localhost"));
    }
}
