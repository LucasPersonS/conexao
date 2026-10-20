//! API central do Conexão.
//!
//! Fonte única de verdade para o diretório de servidores parceiros, as
//! versões publicadas de cada modpack, a whitelist e os agentes que rodam ao
//! lado dos servidores Minecraft.

mod agents;
mod auth;
mod billing;
#[cfg(test)]
mod billing_db_tests;
#[cfg(test)]
mod page_db_tests;
#[cfg(test)]
mod central_db_tests;
mod cleanup;
mod config;
mod crypto;
mod error;
mod moderation;
mod monitor;
mod modrinth;
mod netguard;
mod pairing;
mod policy;
mod ratelimit;
mod routes;
mod state;
mod status;
mod storage;

use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("gerar-chave-assinatura") {
        let mut seed = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut seed);
        let encoded = conexao_types::signing::seed_from_bytes(seed);
        let key = conexao_types::signing::signing_key(&encoded)?;
        println!("CONEXAO_SIGNING_KEY={encoded}   # só na VPS (segredo)");
        println!("CONEXAO_MANIFEST_PUBKEY={}   # no build do launcher", conexao_types::signing::public_key_b64(&key));
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("gerar-chave-segredos") {
        use base64::Engine;
        let mut key = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut key);
        println!("CONEXAO_SECRETS_KEY={}   # só na VPS (segredo; guarde um backup: sem ela as contas conectadas precisam reconectar)", base64::engine::general_purpose::STANDARD.encode(key));
        return Ok(());
    }
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("conexao_api=info,tower_http=info")),
        )
        .init();

    let config = Config::from_env()?;
    // Testa uma conexão direta antes do pool: assim o erro real (senha, TLS,
    // porta) aparece, em vez de um "pool timed out" genérico.
    {
        use sqlx::Connection;
        match tokio::time::timeout(Duration::from_secs(10), sqlx::PgConnection::connect(&config.database_url)).await {
            Ok(Ok(conn)) => {
                let _ = conn.close().await;
            }
            Ok(Err(error)) => return Err(eyre::eyre!("não foi possível conectar ao PostgreSQL ({}): {error}", redact(&config.database_url))),
            Err(_) => return Err(eyre::eyre!("o PostgreSQL não respondeu em 10s ({}); confira se a porta está acessível", redact(&config.database_url))),
        }
    }
    let pool = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&config.database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    storage::ensure_dirs(&config.storage_dir).await?;

    let bind = config.bind.clone();
    let state = AppState::new(config, pool);
    tokio::spawn(status::run(state.clone()));
    tokio::spawn(monitor::run(state.clone()));
    tokio::spawn(cleanup::run(state.clone()));

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("Conexão API ouvindo em {bind}");
    if state.config.production {
        tracing::info!("modo produção: checagens de segurança ativas");
    } else if state.config.signing_key.is_none() {
        tracing::warn!("CONEXAO_SIGNING_KEY não definida: manifestos sairão sem assinatura (só em desenvolvimento)");
    }
    let app = routes::router(state).into_make_service_with_connect_info::<std::net::SocketAddr>();
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await?;
    Ok(())
}

/// Ctrl+C ou SIGTERM (`docker stop`): para de aceitar conexões e termina as que estão em andamento.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("encerrando a API");
}

/// Esconde a senha da URL do banco nas mensagens de erro.
fn redact(url: &str) -> String {
    match (url.find("://"), url.rfind('@')) {
        (Some(start), Some(at)) if at > start => {
            let creds = &url[start + 3..at];
            let user = creds.split(':').next().unwrap_or_default();
            format!("{}{user}:***{}", &url[..start + 3], &url[at..])
        }
        _ => url.to_string(),
    }
}
