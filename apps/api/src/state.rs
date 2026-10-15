use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;

use crate::agents::AgentHub;
use crate::config::Config;
use crate::pairing::Pairings;
use crate::ratelimit::RateLimiter;
use crate::routes::meta::MetaCache;

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub config: Config,
    pub pool: PgPool,
    pub agents: AgentHub,
    pub pairings: Pairings,
    pub http: reqwest::Client,
    pub meta: MetaCache,
    pub limiter: RateLimiter,
}

impl AppState {
    pub fn new(config: Config, pool: PgPool) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("conexao-api/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self(Arc::new(Inner { config, pool, agents: AgentHub::default(), pairings: Pairings::default(),
            http,
            meta: MetaCache::default(),
            limiter: RateLimiter::default(),
        }))
    }
}

impl std::ops::Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}
