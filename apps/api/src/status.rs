//! Coleta periódica de status (online, jogadores, ping) dos servidores aprovados.
//!
//! A API pinga os servidores, não cada launcher: um ping por servidor a cada
//! ciclo, independente de quantos jogadores abrirem a home.

use std::time::{Duration, Instant};

use futures::StreamExt;
use uuid::Uuid;

use crate::state::AppState;

const INTERVAL: Duration = Duration::from_secs(90);
const TIMEOUT: Duration = Duration::from_secs(5);

pub fn split_address(address: &str) -> (String, Option<u16>) {
    match address.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => match port.parse() {
            Ok(port) => (host.to_string(), Some(port)),
            Err(_) => (address.to_string(), None),
        },
        _ => (address.to_string(), None),
    }
}

struct Ping {
    online: bool,
    players: i32,
    max: i32,
    latency: Option<i32>,
}

async fn ping(address: &str, allow_private: bool) -> Ping {
    let offline = Ping { online: false, players: 0, max: 0, latency: None };
    // Mesmo filtro da detecção: um servidor aprovado que troca o endereço para a rede
    // interna não vira um scanner da VPS.
    let Ok(target) = crate::netguard::resolve(address, allow_private).await else {
        return offline;
    };
    let config = crate::netguard::ping_config(&target, TIMEOUT);
    let started = Instant::now();
    let result = tokio::time::timeout(TIMEOUT * 2, async {
        let connection = config.connect().await?;
        connection.status().await
    })
    .await;
    match result {
        Ok(Ok(status)) => Ping {
            online: true,
            players: status.status.players.online,
            max: status.status.players.max,
            latency: i32::try_from(started.elapsed().as_millis()).ok(),
        },
        _ => offline,
    }
}

pub async fn run(state: AppState) {
    let mut interval = tokio::time::interval(INTERVAL);
    loop {
        interval.tick().await;
        let servers: Vec<(Uuid, String)> = match sqlx::query_as(
            "SELECT id, address FROM servers WHERE approval = 'approved'",
        )
        .fetch_all(&state.pool)
        .await
        {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!("falha ao listar servidores para ping: {error}");
                continue;
            }
        };
        futures::stream::iter(servers)
            .for_each_concurrent(16, |(id, address)| {
                let state = state.clone();
                async move {
                    let result = ping(&address, state.config.allow_private_ping).await;
                    if let Err(error) = sqlx::query(
                        "UPDATE servers SET status_online = $2, status_players = $3, status_max_players = $4,
                                status_latency_ms = $5, status_checked_at = now() WHERE id = $1",
                    )
                    .bind(id)
                    .bind(result.online)
                    .bind(result.players)
                    .bind(result.max)
                    .bind(result.latency)
                    .execute(&state.pool)
                    .await
                    {
                        tracing::warn!("falha ao salvar status de {address}: {error}");
                    }
                }
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::split_address;

    #[test]
    fn addresses() {
        assert_eq!(split_address("jogar.paravexia.com"), ("jogar.paravexia.com".into(), None));
        assert_eq!(split_address("1.2.3.4:25570"), ("1.2.3.4".into(), Some(25570)));
    }
}
