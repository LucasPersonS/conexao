//! Loja do jogador: Moedas Conexão, Conexão+ e PRO (dinheiro ou moedas).
//!
//! Dinheiro → Checkout Pro na conta do Conexão; o pagamento é aplicado só pelo
//! webhook/conciliação (`billing`). Moedas → débito atômico na hora (`billing::spend_coins`).

use axum::Json;
use axum::extract::{Path, State};
use axum::response::Html;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthUser, Role, Staff, audit, require_role};
use crate::billing::{self, ORDER_VIEW_SELECT, OrderView, Product};
use crate::error::{ApiError, ApiResult};
use crate::routes::servers::server_id_by_slug;
use crate::state::AppState;

#[derive(Serialize)]
struct PlanView {
    sku: String,
    days: i32,
    price_cents: i32,
    coins: i64,
}

#[derive(Serialize)]
struct PackView {
    sku: String,
    coins: i32,
    price_cents: i32,
}

fn plans(kind: &str, list: &[(i32, i32)]) -> Vec<PlanView> {
    list.iter().map(|(days, cents)| PlanView { sku: format!("{kind}-{days}"), days: *days, price_cents: *cents, coins: i64::from(*cents) }).collect()
}

/// Catálogo público (preços vêm só daqui; o cliente nunca manda valor).
pub async fn catalog(State(state): State<AppState>) -> Json<serde_json::Value> {
    let c = &state.config;
    Json(json!({
        "money_enabled": c.mp_access_token.is_some(),
        "support_enabled": c.mp_access_token.is_some() && c.mp_client_id.is_some() && c.secrets_key.is_some(),
        "coin_packs": c.coin_packs.iter().map(|(coins, cents)| PackView { sku: format!("moedas-{coins}"), coins: *coins, price_cents: *cents }).collect::<Vec<_>>(),
        "plus": plans("plus", &c.plus_plans),
        "pro": plans("pro", &c.pro_plans),
        "featured": plans("destaque", &c.featured_plans),
        "support": {
            "min_cents": c.support_min_cents,
            "max_cents": c.support_max_cents,
            "fee_percent": f64::from(c.support_fee_bps) / 100.0,
        },
    }))
}

#[derive(Serialize, sqlx::FromRow)]
struct LedgerEntry {
    id: Uuid,
    delta: i64,
    balance_after: i64,
    kind: String,
    description: String,
    created_at: DateTime<Utc>,
}

pub async fn wallet(State(state): State<AppState>, AuthUser(user): AuthUser) -> ApiResult<Json<serde_json::Value>> {
    let (balance, premium_until): (i64, Option<DateTime<Utc>>) =
        sqlx::query_as("SELECT coin_balance, premium_until FROM users WHERE id = $1").bind(user.id).fetch_one(&state.pool).await?;
    let history = sqlx::query_as::<_, LedgerEntry>(
        "SELECT id, delta, balance_after, kind, description, created_at FROM coin_ledger
          WHERE user_id = $1 ORDER BY created_at DESC LIMIT 50",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({
        "balance": balance,
        "premium_until": premium_until.filter(|u| *u > Utc::now()),
        "history": history,
    })))
}

pub async fn my_orders(State(state): State<AppState>, AuthUser(user): AuthUser) -> ApiResult<Json<Vec<OrderView>>> {
    Ok(Json(
        sqlx::query_as::<_, OrderView>(&format!(
            "{ORDER_VIEW_SELECT} WHERE o.user_id = $1 AND o.status <> 'cancelled' ORDER BY o.created_at DESC LIMIT 30"
        ))
        .bind(user.id)
        .fetch_all(&state.pool)
        .await?,
    ))
}

/// Situação de um pedido do próprio usuário; se pendente, pergunta ao Mercado Pago.
pub async fn my_order(State(state): State<AppState>, AuthUser(user): AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<OrderView>> {
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM orders WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user.id)
        .fetch_optional(&state.pool)
        .await?;
    match status.as_deref() {
        None => return Err(ApiError::NotFound),
        Some("pending") => {
            if let Err(error) = billing::reconcile(&state, id).await {
                tracing::warn!("não foi possível conciliar o pedido {id}: {error:?}");
            }
        }
        _ => {}
    }
    Ok(Json(sqlx::query_as::<_, OrderView>(&format!("{ORDER_VIEW_SELECT} WHERE o.id = $1")).bind(id).fetch_one(&state.pool).await?))
}

#[derive(Deserialize)]
pub struct BuyInput {
    pub sku: String,
    /// Slug do servidor (PRO e destaque).
    #[serde(default)]
    pub server: Option<String>,
    /// Pagamento com moedas: chave única por clique (proteção contra clique duplo).
    #[serde(default)]
    pub key: Option<Uuid>,
}

/// Confere o servidor de um produto de servidor: aprovado e com o usuário como admin+.
async fn product_server(state: &AppState, user: &crate::auth::User, product: Product, slug: Option<&str>) -> ApiResult<Option<(Uuid, String, String)>> {
    if !product.needs_server() {
        return Ok(None);
    }
    let slug = slug.ok_or_else(|| ApiError::bad("Escolha o servidor."))?;
    let server_id = server_id_by_slug(&state.pool, slug).await?;
    require_role(&state.pool, server_id, user, Role::Admin).await?;
    let (name, approval): (String, String) =
        sqlx::query_as("SELECT name, approval FROM servers WHERE id = $1").bind(server_id).fetch_one(&state.pool).await?;
    if approval != "approved" {
        return Err(ApiError::bad("Disponível depois que a equipe Conexão aprovar o servidor."));
    }
    Ok(Some((server_id, slug.to_string(), name)))
}

/// Compra com dinheiro (Pix, cartão ou boleto): cria o pedido e o link de pagamento.
pub async fn checkout(State(state): State<AppState>, AuthUser(user): AuthUser, Json(input): Json<BuyInput>) -> ApiResult<Json<serde_json::Value>> {
    let token = billing::platform_token(&state)?;
    let product = Product::parse(&state.config, &input.sku).ok_or_else(|| ApiError::bad("Item inválido."))?;
    if matches!(product, Product::Featured { .. }) {
        return Err(ApiError::bad("O destaque em dinheiro é comprado pelo painel do servidor."));
    }
    let server = product_server(&state, &user, product, input.server.as_deref()).await?;
    billing::ensure_pending_room(&state, user.id).await?;

    let id = Uuid::new_v4();
    let coins = match product {
        Product::Coins { coins, .. } => i64::from(coins),
        _ => 0,
    };
    sqlx::query(
        "INSERT INTO orders (id, user_id, kind, sku, server_id, pay_with, amount_cents, coins, days)
         VALUES ($1, $2, $3, $4, $5, 'money', $6, $7, $8)",
    )
    .bind(id)
    .bind(user.id)
    .bind(product.kind())
    .bind(product.sku())
    .bind(server.as_ref().map(|s| s.0))
    .bind(product.cents())
    .bind(coins)
    .bind(product.days())
    .execute(&state.pool)
    .await?;

    let back = match &server {
        Some((_, slug, _)) => format!("{}/#/s/{slug}?tab=pro&pedido={id}", state.config.panel_url),
        None => format!("{}/pagamento?pedido={id}", state.config.public_url),
    };
    let (from, to) = billing::expiration_window();
    let title = product.title(server.as_ref().map(|s| s.2.as_str()));
    let mut preference = json!({
        "items": [{
            "id": product.sku(),
            "title": title,
            "quantity": 1,
            "unit_price": f64::from(product.cents()) / 100.0,
            "currency_id": "BRL",
            "category_id": "services",
        }],
        "external_reference": id.to_string(),
        "notification_url": format!("{}/v1/payments/mercadopago/webhook", state.config.public_url),
        "back_urls": { "success": back, "pending": back, "failure": back },
        "statement_descriptor": "Conexão",
        "metadata": { "order_id": id.to_string() },
        "expires": true,
        "expiration_date_from": from,
        "expiration_date_to": to,
    });
    if back.starts_with("https://") {
        preference["auto_return"] = json!("approved");
    }
    match billing::create_preference(&state, token, id, &preference).await {
        Ok((preference_id, url)) => {
            sqlx::query("UPDATE orders SET mp_preference_id = $2 WHERE id = $1").bind(id).bind(preference_id).execute(&state.pool).await?;
            audit(&state.pool, server.as_ref().map(|s| s.0), Some(user.id), "shop.checkout", json!({ "order": id, "sku": product.sku() })).await?;
            Ok(Json(json!({ "order_id": id, "checkout_url": url })))
        }
        Err(error) => {
            sqlx::query("UPDATE orders SET status = 'cancelled' WHERE id = $1").bind(id).execute(&state.pool).await?;
            Err(error)
        }
    }
}

/// Compra com Moedas (Conexão+, PRO ou destaque). Entrega na hora.
pub async fn spend(State(state): State<AppState>, AuthUser(user): AuthUser, Json(input): Json<BuyInput>) -> ApiResult<Json<serde_json::Value>> {
    let key = input.key.ok_or_else(|| ApiError::bad("Requisição sem chave de compra."))?;
    let product = Product::parse(&state.config, &input.sku).ok_or_else(|| ApiError::bad("Item inválido."))?;
    let server = product_server(&state, &user, product, input.server.as_deref()).await?;
    let order = billing::spend_coins(&state, user.id, product, server.as_ref().map(|(id, _, name)| (*id, name.as_str())), key).await?;
    let balance: i64 = sqlx::query_scalar("SELECT coin_balance FROM users WHERE id = $1").bind(user.id).fetch_one(&state.pool).await?;
    Ok(Json(json!({ "order": order, "balance": balance })))
}

/// Tela de volta do Mercado Pago para compras feitas pelo launcher.
pub async fn return_page() -> Html<&'static str> {
    Html(
        r#"<!doctype html><html lang="pt-BR"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Conexão — pagamento</title><style>body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a0f11;color:#fff;font-family:system-ui,sans-serif;text-align:center;padding:16px}
.c{max-width:420px}h1{color:#b7fe00;font-size:24px}p{color:#c9d1d3;line-height:1.5}</style></head>
<body><div class="c"><h1>Obrigado!</h1><p>Assim que o Mercado Pago confirmar o pagamento, a sua compra aparece no launcher Conexão.</p>
<p>Pix costuma levar segundos; boleto pode levar até 3 dias úteis. Pode fechar esta aba e voltar ao launcher.</p></div></body></html>"#,
    )
}

// ------------------------------------------------------------------ equipe

#[derive(Deserialize)]
pub struct GrantInput {
    /// Nick Minecraft de quem recebe.
    pub name: String,
    /// Positivo dá, negativo tira (sem deixar negativo).
    pub delta: i64,
    pub reason: String,
}

/// Equipe dá ou tira moedas (ex.: crédito de booster). Sempre registrado e auditado.
pub async fn staff_grant(State(state): State<AppState>, Staff(staff): Staff, Json(input): Json<GrantInput>) -> ApiResult<Json<serde_json::Value>> {
    let staff = staff.ok_or_else(|| ApiError::bad("Use a sua conta da equipe (não o token) para movimentar moedas."))?;
    let reason = input.reason.trim();
    if !(3..=200).contains(&reason.chars().count()) {
        return Err(ApiError::bad("Explique o motivo (3 a 200 caracteres)."));
    }
    if input.delta == 0 || input.delta.abs() > 100_000 {
        return Err(ApiError::bad("Quantidade inválida (até 100.000 moedas por vez)."));
    }
    let target: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE lower(minecraft_name) = lower($1)")
        .bind(input.name.trim())
        .fetch_optional(&state.pool)
        .await?;
    let target = target.ok_or(ApiError::NotFound)?;
    let kind = if input.delta > 0 { billing::LedgerKind::Grant } else { billing::LedgerKind::Revoke };
    let mut tx = state.pool.begin().await?;
    let balance = billing::ledger_move(&mut tx, target, input.delta, kind, &format!("staff:{}", Uuid::new_v4()), reason, Some(staff.id), false).await?;
    tx.commit().await?;
    audit(&state.pool, None, Some(staff.id), "coins.staff", json!({ "target": target, "delta": input.delta, "reason": reason })).await?;
    Ok(Json(json!({ "balance": balance })))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct StaffOrder {
    id: Uuid,
    kind: String,
    sku: String,
    pay_with: String,
    amount_cents: i32,
    fee_cents: i32,
    coins: i64,
    status: String,
    buyer: Option<String>,
    server: Option<String>,
    created_at: DateTime<Utc>,
    paid_at: Option<DateTime<Utc>>,
}

/// Últimos pedidos e receita dos últimos 30 dias (dinheiro que ficou com o Conexão).
pub async fn staff_orders(State(state): State<AppState>, Staff(_): Staff) -> ApiResult<Json<serde_json::Value>> {
    let orders = sqlx::query_as::<_, StaffOrder>(
        "SELECT o.id, o.kind, o.sku, o.pay_with, o.amount_cents, o.fee_cents, o.coins, o.status,
                u.minecraft_name AS buyer, s.name AS server, o.created_at, o.paid_at
           FROM orders o LEFT JOIN users u ON u.id = o.user_id LEFT JOIN servers s ON s.id = o.server_id
          WHERE o.status <> 'cancelled' ORDER BY o.created_at DESC LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await?;
    let revenue: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CASE WHEN kind = 'support' THEN fee_cents ELSE amount_cents END), 0)::BIGINT
           FROM orders WHERE status = 'approved' AND pay_with = 'money' AND paid_at > now() - interval '30 days'",
    )
    .fetch_one(&state.pool)
    .await?;
    let featured: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_cents), 0)::BIGINT FROM featured_purchases WHERE status = 'approved' AND paid_at > now() - interval '30 days'",
    )
    .fetch_one(&state.pool)
    .await?;
    let coins_outstanding: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(coin_balance), 0)::BIGINT FROM users").fetch_one(&state.pool).await?;
    Ok(Json(json!({
        "revenue_30d_cents": revenue + featured,
        "coins_outstanding": coins_outstanding,
        "orders": orders,
    })))
}
