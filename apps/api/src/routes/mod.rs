pub mod community;
pub mod companion;
pub mod detect;
pub mod discord;
pub mod featured;
pub mod files;
pub mod images;
pub mod invite;
pub mod me;
pub mod meta;
pub mod plan;
pub mod posts;
pub mod profiles;
pub mod servers;
pub mod shop;
pub mod storage_admin;
pub mod support;
pub mod versions;
pub mod waitlist;
pub mod whitelist;

use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, DefaultBodyLimit, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::{Router, middleware};
use axum::routing::{delete, get, post, put};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::{agents, monitor, pairing, ratelimit};
use crate::state::AppState;
use crate::storage;

pub fn router(state: AppState) -> Router {
    // Vazio só em desenvolvimento: em produção a API não sobe sem a lista (config.rs).
    let cors = if state.config.cors_origins.is_empty() {
        CorsLayer::new().allow_origin(Any)
    } else {
        CorsLayer::new().allow_origin(AllowOrigin::list(
            state.config.cors_origins.iter().filter_map(|o| HeaderValue::from_str(o).ok()),
        ))
    }
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
    .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    let api = Router::new()
        .route("/auth/minecraft", post(me::login_minecraft))
        .route("/auth/discord", post(discord::login))
        .route("/auth/discord/config", get(discord::config))
        .route("/auth/discord/callback", get(discord::callback))
        .route("/auth/offline/register", post(discord::register))
        .route("/auth/offline/session", post(discord::session))
        .route("/auth/offline/logout", post(discord::logout))
        .route("/auth/pair", post(pairing::start))
        .route("/auth/pair/approve", post(pairing::approve))
        .route("/auth/pair/poll", post(pairing::poll))
        .route("/auth/pair/{code}", get(pairing::info))
        .route("/auth/publish-token", post(me::publish_token))
        // Lista de espera: servida no domínio do site (o Caddy repassa), com cookie próprio.
        .route("/waitlist/me", get(waitlist::me).post(waitlist::join))
        .route("/waitlist/sair", post(waitlist::logout))
        .route("/waitlist/comunidade", get(waitlist::community))
        .route("/waitlist/discord", get(waitlist::discord_start))
        .route("/waitlist/discord/callback", get(waitlist::discord_callback))
        .route("/staff/waitlist", get(waitlist::staff_list))
        .route("/staff/waitlist/next", post(waitlist::staff_approve_next))
        .route("/staff/waitlist/manual", post(waitlist::staff_manual))
        .route("/staff/waitlist/{id}", post(waitlist::staff_decide))
        .route("/signing-key", get(versions::signing_key))
        .route("/meta/minecraft", get(meta::minecraft))
        .route("/meta/loaders/{loader}/{minecraft}", get(meta::loader_versions))
        .route("/detect/server", get(detect::detect))
        .route("/me", get(me::me))
        .route("/me/access/{slug}", get(whitelist::access))
        .route("/servers", get(servers::list).post(servers::create))
        .route("/servers/{slug}", get(servers::get_one).patch(servers::update))
        .route("/servers/{slug}/members", get(servers::members).put(servers::set_member))
        .route("/servers/{slug}/audit", get(servers::audit_log))
        .route("/servers/{slug}/agent", get(servers::agent_status))
        .route("/servers/{slug}/manifest", get(versions::manifest))
        .route("/servers/{slug}/versions", get(versions::list).post(versions::publish))
        .route("/servers/{slug}/versions/plan", post(plan::plan))
        .route("/servers/{slug}/channels/{channel}", put(versions::set_channel))
        .route("/servers/{slug}/whitelist", get(whitelist::list).post(whitelist::set_by_name))
        .route("/servers/{slug}/whitelist/apply", post(whitelist::apply))
        .route("/servers/{slug}/join-ticket", post(whitelist::join_ticket))
        .route("/servers/{slug}/agent/token", post(agents::create_token))
        .route("/agent/connect", get(agents::connect))
        .route("/companion/hello", post(companion::hello))
        .route("/companion/poll", post(companion::poll))
        .route("/companion/links/{code}", get(companion::get_link).post(companion::confirm_link))
        .route("/servers/{slug}/snapshot", get(companion::snapshot))
        .route("/servers/{slug}/versions/plan-from-server", post(companion::plan_from_server))
        .route(
            "/servers/{slug}/images/{kind}",
            post(images::upload).layer(DefaultBodyLimit::max(images::MAX_BANNER + 1024)),
        )
        .route("/servers/{slug}/posts", get(posts::list).post(posts::create))
        .route("/servers/{slug}/posts/{id}", put(posts::update).delete(posts::remove))
        .route("/events", get(posts::upcoming))
        .route("/servers/{slug}/star", put(community::star).delete(community::unstar))
        .route("/me/stars", get(community::my_stars))
        .route("/servers/{slug}/comments", get(community::list).post(community::create))
        .route("/servers/{slug}/comments/{id}", delete(community::remove))
        .route("/profile/catalog", get(profiles::get_catalog))
        .route("/me/profile", get(profiles::get_me).put(profiles::put_me))
        .route(
            "/me/profile/background",
            post(profiles::upload_background).layer(DefaultBodyLimit::max(profiles::MAX_BACKGROUND + 1024)),
        )
        .route("/profiles/{name}", get(profiles::get_public))
        .route("/profiles/{name}/report", post(profiles::report))
        .route("/staff/moderation", get(profiles::staff_queue))
        .route("/staff/profiles/{name}/background", post(profiles::staff_background))
        .route("/staff/profiles/{name}/ban", post(profiles::staff_ban))
        .route("/staff/backgrounds/seen", post(profiles::staff_backgrounds_seen))
        .route("/staff/reports/{id}", post(profiles::staff_report))
        .route("/featured/plans", get(featured::plans))
        .route("/servers/{slug}/featured/checkout", post(featured::checkout))
        .route("/servers/{slug}/featured/purchases", get(featured::purchases))
        .route("/servers/{slug}/featured/purchases/{id}", get(featured::purchase))
        .route("/payments/mercadopago/webhook", post(featured::webhook))
        // Monetização: Moedas, Conexão+, PRO e apoio a servidores
        .route("/shop/catalog", get(shop::catalog))
        .route("/shop/checkout", post(shop::checkout))
        .route("/shop/spend", post(shop::spend))
        .route("/me/wallet", get(shop::wallet))
        .route("/me/orders", get(shop::my_orders))
        .route("/me/orders/{id}", get(shop::my_order))
        .route("/servers/{slug}/supporters", get(support::supporters))
        .route("/servers/{slug}/support", get(support::admin_view))
        .route("/servers/{slug}/support/checkout", post(support::checkout))
        .route("/servers/{slug}/payout", put(support::settings).delete(support::disconnect))
        .route("/servers/{slug}/payout/connect", post(support::connect))
        .route("/servers/{slug}/stats", get(support::stats))
        .route("/payments/mercadopago/oauth/callback", get(support::oauth_callback))
        .route("/payments/mercadopago/webhook/apoio/{order_id}", post(support::webhook))
        .route("/staff/coins", post(shop::staff_grant))
        .route("/staff/orders", get(shop::staff_orders))
        .route("/files/missing", post(files::missing))
        .route("/files/resolve", post(files::resolve))
        // Corpo gravado em streaming; o limite por arquivo e a cota são conferidos no handler.
        .route("/files/{sha1}", put(files::upload).layer(DefaultBodyLimit::disable()))
        .route("/staff/servers", get(servers::staff_list))
        .route("/staff/servers/{slug}/approval", post(servers::set_approval))
        .route("/staff/servers/{slug}/featured", post(servers::set_featured))
        .route("/staff/storage", get(storage_admin::overview))
        .route("/staff/storage/cleanup", post(storage_admin::run_cleanup))
        .route("/staff/storage/servers/{slug}", get(storage_admin::server_detail))
        .route("/staff/storage/servers/{slug}/versions/{id}", delete(storage_admin::delete_version))
        .route("/staff/storage/files/{sha1}", post(storage_admin::block_file).delete(storage_admin::unblock_file));

    Router::new()
        .route("/health", get(health))
        .route("/convite/{slug}", get(invite::page))
        .route("/baixar", get(invite::download))
        .route("/u/{name}", get(profiles::page))
        .route("/pagamento", get(shop::return_page))
        .nest("/v1", api)
        .nest_service("/files", ServeDir::new(storage::files_dir(&state.config.storage_dir)))
        .nest_service("/images", ServeDir::new(state.config.storage_dir.join("images")))
        // Atualizações do launcher (latest.json + instaladores assinados), copiadas pelo CI
        .nest_service("/launcher", ServeDir::new(state.config.storage_dir.join("launcher")))
        .layer(middleware::from_fn_with_state(state.clone(), ratelimit::middleware))
        .layer(middleware::from_fn(access_log))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Saúde da API (healthcheck do Docker e UptimeRobot): 503 se o banco não responder
/// ou se o disco passar de `CONEXAO_DISK_ALERT_PERCENT`.
async fn health(State(state): State<AppState>) -> (StatusCode, String) {
    let db = tokio::time::timeout(std::time::Duration::from_secs(3), sqlx::query("SELECT 1").execute(&state.pool)).await;
    if !matches!(db, Ok(Ok(_))) {
        return (StatusCode::SERVICE_UNAVAILABLE, "banco indisponível".into());
    }
    match monitor::disk_used_percent(&state.config.storage_dir) {
        Some(used) if used >= state.config.disk_alert_percent => {
            (StatusCode::SERVICE_UNAVAILABLE, format!("disco quase cheio: {used:.0}% em uso"))
        }
        _ => (StatusCode::OK, "ok".into()),
    }
}

/// Log de acesso: uma linha por requisição (método, caminho, status, tempo, IP).
/// Sem parâmetros da URL nem cabeçalhos; códigos de pareamento aparecem mascarados.
async fn access_log(request: Request, next: Next) -> Response {
    let started = std::time::Instant::now();
    let method = request.method().clone();
    let path = mask_path(request.uri().path());
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or(IpAddr::from([127, 0, 0, 1]), |c| c.0.ip());
    let ip = ratelimit::client_ip(peer, request.headers());
    let response = next.run(request).await;
    if path != "/health" {
        tracing::info!(
            "{method} {path} {} {}ms {ip}",
            response.status().as_u16(),
            started.elapsed().as_millis()
        );
    }
    response
}

/// Esconde segmentos que funcionam como segredo (códigos de pareamento e de vínculo).
fn mask_path(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').collect();
    let mut out = Vec::with_capacity(segments.len());
    for (i, segment) in segments.iter().enumerate() {
        let secret = i > 0 && matches!(segments.get(i - 1), Some(&("pair" | "links"))) && !matches!(*segment, "poll" | "approve");
        out.push(if secret { "***" } else { segment });
    }
    out.join("/")
}

#[cfg(test)]
mod tests {
    use super::mask_path;

    #[test]
    fn hides_pairing_codes_in_logs() {
        assert_eq!(mask_path("/v1/auth/pair/ABCDEFGH"), "/v1/auth/pair/***");
        assert_eq!(mask_path("/v1/auth/pair/poll"), "/v1/auth/pair/poll");
        assert_eq!(mask_path("/v1/companion/links/XYZ123"), "/v1/companion/links/***");
        assert_eq!(mask_path("/v1/servers/paravexia/posts"), "/v1/servers/paravexia/posts");
    }
}
