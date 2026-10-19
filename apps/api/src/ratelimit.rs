//! Limite de requisições por IP (balde de fichas em memória).
//!
//! Cada rota cai numa classe com o seu próprio limite: login, pareamento,
//! Companion e detecção de servidor são as mais caras ou as mais visadas, então
//! têm limites curtos; o resto da API divide um limite geral folgado.
//!
//! O IP vem do `X-Forwarded-For` só quando a conexão chega de um endereço de rede
//! interna (o Caddy, na mesma rede do Docker); de fora, vale o IP da conexão,
//! para ninguém escapar do limite inventando o cabeçalho.

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use dashmap::DashMap;
use serde_json::json;

use crate::state::AppState;

/// Classes de limite. `per_minute` = fichas repostas por minuto; `burst` = máximo acumulado.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    Login,
    PairStart,
    PairPoll,
    PairApprove,
    CompanionHello,
    CompanionPoll,
    Detect,
    CreateServer,
    Upload,
    Report,
    Comment,
    /// Criar link de pagamento ou gastar moedas.
    Checkout,
    /// Começar a conectar a conta do Mercado Pago.
    Payout,
    /// Avisos do Mercado Pago.
    Webhook,
    General,
}

impl Class {
    const fn limits(self) -> (f64, f64) {
        // (por minuto, rajada)
        match self {
            Class::Login => (10.0, 10.0),
            Class::PairStart => (6.0, 6.0),
            Class::PairPoll => (60.0, 30.0),
            Class::PairApprove => (10.0, 5.0),
            // Vários servidores de uma mesma hospedagem dividem o IP.
            Class::CompanionHello => (20.0, 20.0),
            Class::CompanionPoll => (60.0, 30.0),
            Class::Detect => (10.0, 10.0),
            // 5 por hora
            Class::CreateServer => (5.0 / 60.0, 5.0),
            Class::Upload => (240.0, 120.0),
            Class::Report => (5.0, 5.0),
            Class::Comment => (6.0, 3.0),
            Class::Checkout => (10.0, 10.0),
            Class::Payout => (2.0, 4.0),
            Class::Webhook => (120.0, 60.0),
            Class::General => (300.0, 150.0),
        }
    }
}

/// Em que classe cai a requisição (caminho já com o prefixo `/v1`).
pub fn classify(method: &Method, path: &str) -> Class {
    let path = path.strip_prefix("/v1").unwrap_or(path);
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    match (method, segments.as_slice()) {
        (&Method::POST, ["auth", "minecraft"]) => Class::Login,
        (&Method::POST, ["auth", "discord"]) => Class::Login,
        (&Method::POST, ["auth", "offline", "register" | "session" | "logout"]) => Class::Login,
        (&Method::POST, ["servers", _, "join-ticket"]) => Class::Login,
        (&Method::POST, ["auth", "publish-token"]) => Class::Login,
        (&Method::POST, ["waitlist", "me"]) | (&Method::GET, ["waitlist", "discord", "callback"]) => Class::Login,
        (&Method::POST, ["auth", "pair"]) => Class::PairStart,
        (&Method::POST, ["auth", "pair", "poll"]) => Class::PairPoll,
        (&Method::POST, ["auth", "pair", "approve"]) | (&Method::GET, ["auth", "pair", _]) => Class::PairApprove,
        (&Method::POST, ["companion", "hello"]) => Class::CompanionHello,
        (&Method::POST, ["companion", "poll"]) => Class::CompanionPoll,
        (_, ["companion", "links", _]) => Class::PairApprove,
        (&Method::GET, ["detect", "server"]) => Class::Detect,
        (&Method::POST, ["servers"]) => Class::CreateServer,
        (&Method::PUT, ["files", _]) => Class::Upload,
        (&Method::POST, ["servers", _, "page", "media"]) => Class::Upload,
        (&Method::POST, ["profiles", _, "report"]) => Class::Report,
        (&Method::POST, ["servers", _, "comments"]) => Class::Comment,
        (&Method::POST, ["shop", "checkout" | "spend"] | ["servers", _, "support", "checkout"]) => Class::Checkout,
        (&Method::POST, ["servers", _, "payout", "connect"]) | (&Method::GET, ["payments", "mercadopago", "oauth", "callback"]) => Class::Payout,
        (&Method::POST, ["staff", "coins"]) => Class::Report,
        (&Method::POST, ["payments", "mercadopago", "webhook", ..]) => Class::Webhook,
        _ => Class::General,
    }
}

struct Bucket {
    tokens: f64,
    updated: Instant,
}

#[derive(Default)]
pub struct RateLimiter {
    buckets: DashMap<(Class, IpAddr), Bucket>,
}

impl RateLimiter {
    /// Gasta uma ficha. `Err(segundos)` = quanto esperar.
    pub fn check(&self, class: Class, ip: IpAddr) -> Result<(), u64> {
        self.check_at(class, ip, Instant::now())
    }

    fn check_at(&self, class: Class, ip: IpAddr, now: Instant) -> Result<(), u64> {
        let (per_minute, burst) = class.limits();
        let per_second = per_minute / 60.0;
        if self.buckets.len() > 100_000 {
            // Limpeza: quem não aparece há 10 minutos já teria o balde cheio de novo.
            self.buckets.retain(|_, b| now.duration_since(b.updated) < Duration::from_secs(600));
        }
        let mut bucket = self.buckets.entry((class, ip)).or_insert(Bucket { tokens: burst, updated: now });
        let elapsed = now.saturating_duration_since(bucket.updated).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * per_second).min(burst);
        bucket.updated = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            let wait = ((1.0 - bucket.tokens) / per_second).ceil();
            Err(wait.max(1.0) as u64)
        }
    }
}

fn is_internal(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00,
    }
}

/// IP real do cliente: o último do `X-Forwarded-For` quando quem conectou é o proxy interno.
pub fn client_ip(peer: IpAddr, headers: &HeaderMap) -> IpAddr {
    if !is_internal(peer) {
        return peer;
    }
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .and_then(|ip| ip.trim().parse().ok())
        .unwrap_or(peer)
}

pub async fn middleware(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or(IpAddr::from([127, 0, 0, 1]), |c| c.0.ip());
    let ip = client_ip(peer, request.headers());
    let class = classify(request.method(), request.uri().path());
    match state.limiter.check(class, ip) {
        Ok(()) => next.run(request).await,
        Err(wait) => {
            let mut response = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({
                    "error": "rate_limited",
                    "message": format!("Muitas tentativas seguidas. Espere {wait} s e tente de novo."),
                    "details": null,
                })),
            )
                .into_response();
            if let Ok(value) = HeaderValue::from_str(&wait.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
            response
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_routes() {
        assert_eq!(classify(&Method::POST, "/v1/auth/minecraft"), Class::Login);
        assert_eq!(classify(&Method::POST, "/v1/auth/pair"), Class::PairStart);
        assert_eq!(classify(&Method::POST, "/v1/auth/pair/poll"), Class::PairPoll);
        assert_eq!(classify(&Method::GET, "/v1/auth/pair/ABCDEFGH"), Class::PairApprove);
        assert_eq!(classify(&Method::PUT, "/v1/files/da39a3ee5e6b4b0d3255bfef95601890afd80709"), Class::Upload);
        assert_eq!(classify(&Method::POST, "/v1/servers"), Class::CreateServer);
        assert_eq!(classify(&Method::GET, "/v1/servers"), Class::General);
        assert_eq!(classify(&Method::POST, "/v1/servers/paravexia/comments"), Class::Comment);
        assert_eq!(classify(&Method::GET, "/v1/servers/paravexia/comments"), Class::General);
        assert_eq!(classify(&Method::GET, "/health"), Class::General);
    }

    #[test]
    fn bucket_empties_and_refills() {
        let limiter = RateLimiter::default();
        let ip: IpAddr = "203.0.113.7".parse().unwrap();
        let start = Instant::now();
        for _ in 0..10 {
            assert!(limiter.check_at(Class::Login, ip, start).is_ok());
        }
        let wait = limiter.check_at(Class::Login, ip, start).unwrap_err();
        assert!((1..=6).contains(&wait));
        // Outro IP não é afetado.
        assert!(limiter.check_at(Class::Login, "203.0.113.8".parse().unwrap(), start).is_ok());
        // Depois de 6 s volta uma ficha (10 por minuto).
        assert!(limiter.check_at(Class::Login, ip, start + Duration::from_secs(6)).is_ok());
    }

    #[test]
    fn forwarded_only_from_internal_proxy() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("1.1.1.1, 198.51.100.4"));
        let caddy: IpAddr = "172.18.0.3".parse().unwrap();
        let outsider: IpAddr = "203.0.113.9".parse().unwrap();
        assert_eq!(client_ip(caddy, &headers), "198.51.100.4".parse::<IpAddr>().unwrap());
        assert_eq!(client_ip(outsider, &headers), outsider);
    }
}
