//! Agente do Conexão.
//!
//! Roda na mesma máquina do servidor Minecraft (NeoForge, Forge, Fabric ou
//! Paper), conecta na API por WebSocket (só conexão de saída) e:
//! - grava o `whitelist.json` sempre que a equipe aprova ou remove alguém;
//! - recarrega a whitelist pelo RCON (`whitelist reload`);
//! - executa comandos permitidos (kick, say) pedidos pela API.

mod rcon;
mod whitelist;

use std::path::PathBuf;
use std::time::Duration;

use conexao_types::agent::{FromAgent, ToAgent, is_allowed_command};
use clap::Parser;
use eyre::{WrapErr, eyre};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "conexao-agent", version, about = "Agente do Conexão para servidores Minecraft")]
struct Args {
    /// Arquivo de configuração.
    #[arg(long, short, default_value = "conexao-agent.toml")]
    config: PathBuf,
}

#[derive(Deserialize)]
struct Config {
    /// Ex.: `wss://api.conexao.gg/v1/agent/connect`
    api_url: String,
    /// Token gerado no painel (Servidor → Agente → Gerar token).
    token: String,
    /// Pasta do servidor (onde ficam `server.properties` e `whitelist.json`).
    server_dir: PathBuf,
    rcon: Option<RconConfig>,
}

#[derive(Deserialize, Clone)]
struct RconConfig {
    address: String,
    password: String,
}

async fn rcon_command(config: &Option<RconConfig>, command: &str) -> eyre::Result<String> {
    let rcon = config.as_ref().ok_or_else(|| eyre!("RCON não configurado"))?;
    let mut client = rcon::Rcon::connect(&rcon.address, &rcon.password).await?;
    client.command(command).await
}

async fn handle(config: &Config, message: ToAgent) -> Option<FromAgent> {
    match message {
        ToAgent::Ping => Some(FromAgent::Pong),
        ToAgent::WhitelistSync { players, .. } => {
            if let Err(error) = whitelist::write(&config.server_dir, &players).await {
                return Some(FromAgent::Error { message: format!("falha ao gravar whitelist.json: {error}") });
            }
            if config.rcon.is_some()
                && let Err(error) = rcon_command(&config.rcon, "whitelist reload").await
            {
                tracing::warn!("whitelist gravada, mas o reload via RCON falhou: {error:#}");
            }
            tracing::info!(count = players.len(), "whitelist aplicada");
            Some(FromAgent::WhitelistApplied { count: players.len() })
        }
        // Uploads são do Companion (mod); o agente não guarda a lista de mods.
        ToAgent::UploadFiles { sha1 } => Some(FromAgent::FilesUploaded { sha1: Vec::new(), failed: sha1 }),
        ToAgent::Command { id, command } => {
            if !is_allowed_command(&command) {
                return Some(FromAgent::CommandResult { id, ok: false, output: "comando não permitido".into() });
            }
            Some(match rcon_command(&config.rcon, &command).await {
                Ok(output) => FromAgent::CommandResult { id, ok: true, output },
                Err(error) => FromAgent::CommandResult { id, ok: false, output: format!("{error:#}") },
            })
        }
    }
}

async fn session(config: &Config) -> eyre::Result<()> {
    let mut request = config.api_url.as_str().into_client_request()?;
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", config.token).parse().wrap_err("token inválido")?,
    );
    let (socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .wrap_err("não foi possível conectar à API do Conexão")?;
    tracing::info!("conectado à API do Conexão");
    let (mut sink, mut stream) = socket.split();
    let hello = FromAgent::Hello { agent_version: env!("CARGO_PKG_VERSION").into() };
    sink.send(Message::Text(serde_json::to_string(&hello)?.into())).await?;

    while let Some(message) = stream.next().await {
        let text = match message? {
            Message::Text(text) => text,
            Message::Ping(payload) => {
                sink.send(Message::Pong(payload)).await?;
                continue;
            }
            Message::Close(_) => break,
            _ => continue,
        };
        let parsed: ToAgent = match serde_json::from_str(&text) {
            Ok(parsed) => parsed,
            Err(error) => {
                tracing::warn!("mensagem desconhecida da API: {error}");
                continue;
            }
        };
        if let Some(reply) = handle(config, parsed).await {
            sink.send(Message::Text(serde_json::to_string(&reply)?.into())).await?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("conexao_agent=info")))
        .init();
    let args = Args::parse();
    let text = std::fs::read_to_string(&args.config)
        .wrap_err_with(|| format!("não foi possível ler {}", args.config.display()))?;
    let config: Config = toml::from_str(&text).wrap_err("configuração inválida")?;
    if !config.api_url.starts_with("wss://") && !config.api_url.starts_with("ws://localhost") {
        return Err(eyre!("use wss:// em api_url (ws:// só para localhost)"));
    }
    match std::fs::read_to_string(config.server_dir.join("server.properties")) {
        Ok(properties) => {
            for warning in whitelist::check_properties(&properties) {
                tracing::warn!("{warning}");
            }
        }
        Err(_) => tracing::warn!("server.properties não encontrado em {}", config.server_dir.display()),
    }

    let mut backoff = Duration::from_secs(1);
    loop {
        tokio::select! {
            result = session(&config) => {
                match result {
                    Ok(()) => {
                        tracing::warn!("conexão encerrada pela API; reconectando");
                        backoff = Duration::from_secs(1);
                    }
                    Err(error) => tracing::warn!("{error:#}; nova tentativa em {}s", backoff.as_secs()),
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("encerrando");
                return Ok(());
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(60));
    }
}
