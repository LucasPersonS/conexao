//! Núcleo da monetização: catálogo, carteira de Moedas, pedidos e Mercado Pago.
//!
//! Regras de segurança (valem para tudo aqui):
//! * **Preço só no servidor.** O cliente manda um `sku`; valor, dias e moedas saem da
//!   configuração. Nada que vem do launcher/painel define quanto se paga ou se recebe.
//! * **Aviso não é prova.** O webhook só diz "olhe o pagamento X". A API sempre busca o
//!   pagamento no Mercado Pago e confere referência, valor, moeda e quem recebeu.
//! * **Uma vez só.** Cada pedido é travado (`FOR UPDATE`) e anda por estados válidos;
//!   cada movimento de moedas tem uma referência única no banco. Avisos repetidos,
//!   cliques duplos e corridas entre requisições não creditam duas vezes.
//! * **Saldo atômico.** O débito só acontece se houver saldo, na mesma instrução SQL
//!   (`... WHERE coin_balance + delta >= 0`), dentro da transação que entrega o benefício.
//! * **Circuito fechado.** Moedas não viram dinheiro, não vão para outro jogador e não
//!   pagam apoio a servidores (o apoio é só em dinheiro, com split no Mercado Pago).

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::auth::audit;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ------------------------------------------------------------------ catálogo

/// O que pode ser comprado. Vem do `sku` enviado pelo cliente e é validado contra a config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Product {
    /// Pacote de moedas (só com dinheiro).
    Coins { coins: i32, cents: i32 },
    /// Conexão+ por `days` dias (dinheiro ou moedas).
    Plus { days: i32, cents: i32 },
    /// PRO de um servidor por `days` dias (dinheiro ou moedas).
    Pro { days: i32, cents: i32 },
    /// Destaque de um servidor pago com moedas (em dinheiro continua em `featured.rs`).
    Featured { days: i32, cents: i32 },
}

impl Product {
    pub fn parse(config: &crate::config::Config, sku: &str) -> Option<Self> {
        let (kind, n) = sku.split_once('-')?;
        let n: i32 = n.parse().ok()?;
        let plan = |plans: &[(i32, i32)]| plans.iter().copied().find(|(days, _)| *days == n);
        match kind {
            "moedas" => config.coin_packs.iter().copied().find(|(c, _)| *c == n).map(|(coins, cents)| Product::Coins { coins, cents }),
            "plus" => plan(&config.plus_plans).map(|(days, cents)| Product::Plus { days, cents }),
            "pro" => plan(&config.pro_plans).map(|(days, cents)| Product::Pro { days, cents }),
            "destaque" => plan(&config.featured_plans).map(|(days, cents)| Product::Featured { days, cents }),
            _ => None,
        }
    }

    pub fn kind(self) -> &'static str {
        match self {
            Product::Coins { .. } => "coins",
            Product::Plus { .. } => "plus",
            Product::Pro { .. } => "pro",
            Product::Featured { .. } => "featured",
        }
    }

    pub fn sku(self) -> String {
        match self {
            Product::Coins { coins, .. } => format!("moedas-{coins}"),
            Product::Plus { days, .. } => format!("plus-{days}"),
            Product::Pro { days, .. } => format!("pro-{days}"),
            Product::Featured { days, .. } => format!("destaque-{days}"),
        }
    }

    pub fn cents(self) -> i32 {
        match self {
            Product::Coins { cents, .. } | Product::Plus { cents, .. } | Product::Pro { cents, .. } | Product::Featured { cents, .. } => cents,
        }
    }

    pub fn days(self) -> Option<i32> {
        match self {
            Product::Coins { .. } => None,
            Product::Plus { days, .. } | Product::Pro { days, .. } | Product::Featured { days, .. } => Some(days),
        }
    }

    /// Preço em moedas (1 moeda = 1 centavo). Pacotes de moedas não se compram com moedas.
    pub fn coin_price(self) -> Option<i64> {
        match self {
            Product::Coins { .. } => None,
            other => Some(i64::from(other.cents())),
        }
    }

    pub fn needs_server(self) -> bool {
        matches!(self, Product::Pro { .. } | Product::Featured { .. })
    }

    pub fn title(self, server_name: Option<&str>) -> String {
        let on = server_name.map(|n| format!(" — {n}")).unwrap_or_default();
        match self {
            Product::Coins { coins, .. } => format!("{coins} Moedas Conexão"),
            Product::Plus { days, .. } => format!("Conexão+ por {days} dias"),
            Product::Pro { days, .. } => format!("Conexão PRO por {days} dias{on}"),
            Product::Featured { days, .. } => format!("Destaque de {days} dias no Conexão{on}"),
        }
    }
}

/// Taxa do Conexão sobre um apoio, arredondada para o centavo mais próximo.
pub fn support_fee(amount_cents: i32, fee_bps: i32) -> i32 {
    let fee = (i64::from(amount_cents) * i64::from(fee_bps) + 5_000) / 10_000;
    i32::try_from(fee).unwrap_or(amount_cents).clamp(0, amount_cents)
}

// ------------------------------------------------------------------ Mercado Pago (HTTP)

pub fn platform_token(state: &AppState) -> ApiResult<&str> {
    state
        .config
        .mp_access_token
        .as_deref()
        .ok_or_else(|| ApiError::bad("Pagamentos ainda não estão ativos no Conexão. Fale com a equipe."))
}

fn http_err(error: impl Into<eyre::Report>) -> ApiError {
    ApiError::Internal(error.into())
}

/// Cria uma preferência do Checkout Pro e devolve (id, link de pagamento).
pub async fn create_preference(
    state: &AppState,
    token: &str,
    idempotency_key: Uuid,
    preference: &serde_json::Value,
) -> ApiResult<(String, String)> {
    let response = state
        .http
        .post(format!("{}/checkout/preferences", state.config.mp_api_url))
        .bearer_auth(token)
        .header("X-Idempotency-Key", idempotency_key.to_string())
        .timeout(Duration::from_secs(20))
        .json(preference)
        .send()
        .await
        .map_err(http_err)?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        tracing::error!("Mercado Pago recusou a preferência ({status}): {body}");
        return Err(ApiError::bad("Não foi possível iniciar o pagamento agora. Tente de novo em instantes."));
    }
    let created: serde_json::Value = response.json().await.map_err(http_err)?;
    let id = created["id"].as_str().unwrap_or_default().to_string();
    let url = created["init_point"].as_str().unwrap_or_default().to_string();
    if id.is_empty() || !url.starts_with("https://") {
        return Err(ApiError::Internal(eyre::eyre!("resposta inesperada do Mercado Pago ao criar preferência")));
    }
    Ok((id, url))
}

pub async fn get_payment(state: &AppState, token: &str, payment_id: &str) -> ApiResult<serde_json::Value> {
    if !valid_payment_id(payment_id) {
        return Err(ApiError::bad("pagamento inválido"));
    }
    state
        .http
        .get(format!("{}/v1/payments/{payment_id}", state.config.mp_api_url))
        .bearer_auth(token)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(http_err)?
        .error_for_status()
        .map_err(http_err)?
        .json()
        .await
        .map_err(http_err)
}

/// Ids de pagamento dos resultados de uma busca por `external_reference`.
pub async fn search_payments(state: &AppState, token: &str, reference: Uuid) -> ApiResult<Vec<String>> {
    let found: serde_json::Value = state
        .http
        .get(format!("{}/v1/payments/search", state.config.mp_api_url))
        .bearer_auth(token)
        .query(&[("external_reference", reference.to_string()), ("sort", "date_created".into()), ("criteria", "desc".into())])
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(http_err)?
        .error_for_status()
        .map_err(http_err)?
        .json()
        .await
        .map_err(http_err)?;
    Ok(found["results"].as_array().into_iter().flatten().filter_map(payment_id_of).take(10).collect())
}

pub fn valid_payment_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

pub fn payment_id_of(payment: &serde_json::Value) -> Option<String> {
    payment["id"].as_i64().map(|v| v.to_string()).or_else(|| payment["id"].as_str().map(str::to_string)).filter(|id| valid_payment_id(id))
}

/// O que interessa de um pagamento, já conferido quanto ao formato.
#[derive(Debug, Clone)]
pub struct PaymentFacts {
    pub reference: Option<Uuid>,
    pub status: String,
    pub paid_cents: i64,
    pub currency: String,
    pub collector_id: Option<String>,
}

impl PaymentFacts {
    pub fn from_json(payment: &serde_json::Value) -> Self {
        let amount = payment["transaction_amount"].as_f64().unwrap_or_default();
        PaymentFacts {
            reference: payment["external_reference"].as_str().and_then(|r| Uuid::parse_str(r).ok()),
            status: payment["status"].as_str().unwrap_or_default().to_string(),
            paid_cents: if amount.is_finite() && amount > 0.0 { (amount * 100.0).round() as i64 } else { 0 },
            currency: payment["currency_id"].as_str().unwrap_or_default().to_string(),
            collector_id: payment["collector_id"]
                .as_i64()
                .map(|v| v.to_string())
                .or_else(|| payment["collector_id"].as_str().map(str::to_string))
                .or_else(|| payment["collector"]["id"].as_i64().map(|v| v.to_string())),
        }
    }

    /// Pago no valor certo, em reais.
    pub fn covers(&self, cents: i32) -> bool {
        self.currency == "BRL" && self.paid_cents >= i64::from(cents)
    }
}

// ------------------------------------------------------------------ carteira

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerKind {
    Purchase,
    Spend,
    Refund,
    Chargeback,
    Grant,
    Revoke,
}

impl LedgerKind {
    fn as_str(self) -> &'static str {
        match self {
            LedgerKind::Purchase => "purchase",
            LedgerKind::Spend => "spend",
            LedgerKind::Refund => "refund",
            LedgerKind::Chargeback => "chargeback",
            LedgerKind::Grant => "grant",
            LedgerKind::Revoke => "revoke",
        }
    }
}

/// Teto de segurança de uma conta (evita estouro e saldos absurdos por bug).
pub const MAX_BALANCE: i64 = 100_000_000;

/// Movimenta moedas e registra no histórico, na transação do chamador.
///
/// * Débito comum (`allow_debt = false`): só passa se o saldo cobrir — na mesma instrução.
/// * Estorno de compra (`allow_debt = true`): pode deixar o saldo negativo (vira dívida).
/// * A `reference` é única no banco: repetir a mesma operação falha com `Conflict`.
pub async fn ledger_move(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    delta: i64,
    kind: LedgerKind,
    reference: &str,
    description: &str,
    created_by: Option<Uuid>,
    allow_debt: bool,
) -> ApiResult<i64> {
    if delta == 0 || delta.abs() > MAX_BALANCE {
        return Err(ApiError::bad("Quantidade de moedas inválida."));
    }
    let balance: Option<i64> = if delta < 0 && !allow_debt {
        sqlx::query_scalar(
            "UPDATE users SET coin_balance = coin_balance + $2 WHERE id = $1 AND coin_balance + $2 >= 0 RETURNING coin_balance",
        )
        .bind(user_id)
        .bind(delta)
        .fetch_optional(&mut **tx)
        .await?
    } else {
        sqlx::query_scalar(
            "UPDATE users SET coin_balance = coin_balance + $2 WHERE id = $1 AND coin_balance + $2 <= $3 RETURNING coin_balance",
        )
        .bind(user_id)
        .bind(delta)
        .bind(MAX_BALANCE)
        .fetch_optional(&mut **tx)
        .await?
    };
    let Some(balance) = balance else {
        return Err(if delta < 0 {
            ApiError::bad("Saldo de Moedas insuficiente.")
        } else {
            ApiError::bad("Limite de Moedas da conta atingido. Fale com a equipe.")
        });
    };
    sqlx::query(
        "INSERT INTO coin_ledger (id, user_id, delta, balance_after, kind, ref, description, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(delta)
    .bind(balance)
    .bind(kind.as_str())
    .bind(reference)
    .bind(description)
    .bind(created_by)
    .execute(&mut **tx)
    .await?;
    Ok(balance)
}

// ------------------------------------------------------------------ benefícios

/// Estende (ou reduz, com `days` negativo) um prazo. Nunca deixa o prazo antes de agora
/// ao reduzir, e conta a partir de agora se o plano já tinha vencido.
async fn shift_until(tx: &mut Transaction<'_, Postgres>, sql_table_col: TargetCol, id: Uuid, days: i32) -> ApiResult<Option<DateTime<Utc>>> {
    let sql = match (sql_table_col, days >= 0) {
        (TargetCol::Premium, true) => "UPDATE users SET premium_until = GREATEST(COALESCE(premium_until, now()), now()) + make_interval(days => $2) WHERE id = $1 RETURNING premium_until",
        (TargetCol::Premium, false) => "UPDATE users SET premium_until = GREATEST(now(), premium_until + make_interval(days => $2)) WHERE id = $1 AND premium_until IS NOT NULL RETURNING premium_until",
        (TargetCol::Pro, true) => "UPDATE servers SET pro_until = GREATEST(COALESCE(pro_until, now()), now()) + make_interval(days => $2) WHERE id = $1 RETURNING pro_until",
        (TargetCol::Pro, false) => "UPDATE servers SET pro_until = GREATEST(now(), pro_until + make_interval(days => $2)) WHERE id = $1 AND pro_until IS NOT NULL RETURNING pro_until",
        (TargetCol::Featured, true) => "UPDATE servers SET featured_until = GREATEST(COALESCE(featured_until, now()), now()) + make_interval(days => $2) WHERE id = $1 RETURNING featured_until",
        (TargetCol::Featured, false) => "UPDATE servers SET featured_until = GREATEST(now(), featured_until + make_interval(days => $2)) WHERE id = $1 AND featured_until IS NOT NULL RETURNING featured_until",
    };
    Ok(sqlx::query_scalar(sql).bind(id).bind(days).fetch_optional(&mut **tx).await?.flatten())
}

#[derive(Clone, Copy)]
enum TargetCol {
    Premium,
    Pro,
    Featured,
}

/// Entrega (ou desfaz, com `sign = -1`) o benefício de um pedido.
async fn apply_benefit(
    tx: &mut Transaction<'_, Postgres>,
    order: &OrderRow,
    sign: i32,
    revoke_kind: LedgerKind,
) -> ApiResult<Option<DateTime<Utc>>> {
    let days = order.days.unwrap_or(0) * sign;
    match order.kind.as_str() {
        "coins" => {
            let user = order.user_id.ok_or_else(|| ApiError::Internal(eyre::eyre!("pedido de moedas sem conta")))?;
            if sign > 0 {
                ledger_move(tx, user, order.coins, LedgerKind::Purchase, &format!("order:{}", order.id), &format!("Compra de {} Moedas", order.coins), None, false)
                    .await?;
            } else {
                ledger_move(tx, user, -order.coins, revoke_kind, &format!("refund:{}", order.id), "Estorno da compra de Moedas", None, true).await?;
            }
            Ok(None)
        }
        "plus" => {
            let user = order.user_id.ok_or_else(|| ApiError::Internal(eyre::eyre!("pedido de Conexão+ sem conta")))?;
            shift_until(tx, TargetCol::Premium, user, days).await
        }
        "pro" => {
            let server = order.server_id.ok_or_else(|| ApiError::Internal(eyre::eyre!("pedido PRO sem servidor")))?;
            shift_until(tx, TargetCol::Pro, server, days).await
        }
        "featured" => {
            let server = order.server_id.ok_or_else(|| ApiError::Internal(eyre::eyre!("pedido de destaque sem servidor")))?;
            shift_until(tx, TargetCol::Featured, server, days).await
        }
        // Apoio: o dinheiro já foi direto para o dono; não há benefício para entregar.
        _ => Ok(None),
    }
}

/// Estorno/contestação de moedas que já foram gastas: desfaz os gastos com moedas feitos
/// depois daquela compra (do mais recente para o mais antigo) até o saldo voltar a zero.
/// Assim ninguém compra moedas com cartão, gasta em PRO/destaque e contesta ficando com o benefício.
async fn claw_back_spends(tx: &mut Transaction<'_, Postgres>, user: Uuid, since: Option<DateTime<Utc>>) -> ApiResult<()> {
    let spends: Vec<OrderRow> = sqlx::query_as(&format!(
        "SELECT {ORDER_COLUMNS} FROM orders
          WHERE user_id = $1 AND pay_with = 'coins' AND status = 'approved' AND paid_at >= COALESCE($2, '-infinity'::timestamptz)
          ORDER BY paid_at DESC FOR UPDATE"
    ))
    .bind(user)
    .bind(since)
    .fetch_all(&mut **tx)
    .await?;
    for spend in spends {
        let balance: i64 = sqlx::query_scalar("SELECT coin_balance FROM users WHERE id = $1").bind(user).fetch_one(&mut **tx).await?;
        if balance >= 0 {
            break;
        }
        apply_benefit(tx, &spend, -1, LedgerKind::Refund).await?;
        ledger_move(tx, user, spend.coins, LedgerKind::Refund, &format!("revert:{}", spend.id), "Compra com moedas desfeita (moedas contestadas)", None, false)
            .await?;
        sqlx::query("UPDATE orders SET status = 'refunded', refunded_at = now() WHERE id = $1").bind(spend.id).execute(&mut **tx).await?;
        tracing::warn!(%user, order = %spend.id, "compra com moedas desfeita por contestação da compra de moedas");
    }
    Ok(())
}

// ------------------------------------------------------------------ pedidos

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrderRow {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub kind: String,
    pub server_id: Option<Uuid>,
    pub pay_with: String,
    pub amount_cents: i32,
    pub coins: i64,
    pub days: Option<i32>,
    pub seller_mp_user_id: Option<String>,
    pub status: String,
    pub mp_payment_id: Option<String>,
    pub paid_at: Option<DateTime<Utc>>,
}

const ORDER_COLUMNS: &str = "id, user_id, kind, server_id, pay_with, amount_cents, coins, days, seller_mp_user_id, status, mp_payment_id, paid_at";

/// Pedido como o jogador/dono vê.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OrderView {
    pub id: Uuid,
    pub kind: String,
    pub sku: String,
    pub pay_with: String,
    pub amount_cents: i32,
    pub coins: i64,
    pub days: Option<i32>,
    pub status: String,
    pub server_slug: Option<String>,
    pub server_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
    pub granted_until: Option<DateTime<Utc>>,
}

pub const ORDER_VIEW_SELECT: &str = "SELECT o.id, o.kind, o.sku, o.pay_with, o.amount_cents, o.coins, o.days, o.status,
        s.slug AS server_slug, s.name AS server_name, o.created_at, o.paid_at, o.granted_until
   FROM orders o LEFT JOIN servers s ON s.id = o.server_id";

/// Limite de pedidos em dinheiro abertos por conta (evita encher o Mercado Pago de preferências).
pub const MAX_PENDING_ORDERS: i64 = 5;

pub async fn ensure_pending_room(state: &AppState, user_id: Uuid) -> ApiResult<()> {
    // Pedidos esquecidos há mais de 24 h viram "cancelados" (o link do MP também expira).
    sqlx::query("UPDATE orders SET status = 'cancelled' WHERE user_id = $1 AND status = 'pending' AND created_at < now() - interval '24 hours'")
        .bind(user_id)
        .execute(&state.pool)
        .await?;
    let open: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE user_id = $1 AND status = 'pending'")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?;
    if open >= MAX_PENDING_ORDERS {
        return Err(ApiError::bad("Você tem muitos pagamentos em aberto. Conclua ou espere alguns minutos antes de começar outro."));
    }
    Ok(())
}

/// Janela de validade do link de pagamento (24 h), no formato do Mercado Pago.
pub fn expiration_window() -> (String, String) {
    use chrono::SecondsFormat;
    let now = Utc::now();
    (now.to_rfc3339_opts(SecondsFormat::Millis, false), (now + chrono::Duration::hours(24)).to_rfc3339_opts(SecondsFormat::Millis, false))
}

/// Aplica um pagamento a um pedido (Conexão+, PRO, moedas ou apoio).
///
/// `expected_collector`: para apoios, a conta do dono que tem que ter recebido.
pub async fn apply_order_payment(state: &AppState, payment_id: &str, facts: &PaymentFacts, only_support: bool) -> ApiResult<()> {
    let Some(order_id) = facts.reference else { return Ok(()) };
    let mut tx = state.pool.begin().await?;
    let row: Option<OrderRow> = sqlx::query_as(&format!("SELECT {ORDER_COLUMNS} FROM orders WHERE id = $1 FOR UPDATE"))
        .bind(order_id)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(order) = row else { return Ok(()) };
    if order.pay_with != "money" || (order.kind == "support") != only_support {
        // Pagamento apontando para um pedido de outro tipo/canal: ignora.
        tracing::warn!(%order_id, payment_id, "pagamento não corresponde ao canal do pedido; ignorado");
        return Ok(());
    }
    if order.kind == "support" && facts.collector_id.as_deref() != order.seller_mp_user_id.as_deref() {
        tracing::error!(%order_id, payment_id, "apoio pago para outra conta Mercado Pago; ignorado");
        return Ok(());
    }

    match facts.status.as_str() {
        "approved" if matches!(order.status.as_str(), "pending" | "rejected" | "cancelled") => {
            if !facts.covers(order.amount_cents) {
                tracing::error!(%order_id, payment_id, paid = facts.paid_cents, currency = %facts.currency, "valor do pagamento não confere; pedido não entregue");
                return Ok(());
            }
            let until = apply_benefit(&mut tx, &order, 1, LedgerKind::Refund).await?;
            sqlx::query("UPDATE orders SET status = 'approved', paid_at = now(), mp_payment_id = $2, granted_until = $3 WHERE id = $1")
                .bind(order_id)
                .bind(payment_id)
                .bind(until)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            audit(&state.pool, order.server_id, order.user_id, &format!("{}.paid", order.kind), json!({ "order": order_id, "cents": order.amount_cents })).await?;
            tracing::info!(%order_id, kind = %order.kind, "pedido pago e entregue");
        }
        "rejected" | "cancelled" if order.status == "pending" => {
            sqlx::query("UPDATE orders SET status = $2, mp_payment_id = $3 WHERE id = $1")
                .bind(order_id)
                .bind(&facts.status)
                .bind(payment_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        // Estorno antes da aprovação (avisos fora de ordem): fecha o pedido para um "aprovado"
        // atrasado não entregar nada depois.
        "refunded" | "charged_back" if order.status == "pending" => {
            sqlx::query("UPDATE orders SET status = 'refunded', refunded_at = now(), mp_payment_id = $2 WHERE id = $1")
                .bind(order_id)
                .bind(payment_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        // Só o pagamento que entregou o pedido desfaz o pedido (estornar um pagamento duplicado não tira o benefício).
        "refunded" | "charged_back" if order.status == "approved" && order.mp_payment_id.as_deref() == Some(payment_id) => {
            let kind = if facts.status == "charged_back" { LedgerKind::Chargeback } else { LedgerKind::Refund };
            apply_benefit(&mut tx, &order, -1, kind).await?;
            sqlx::query("UPDATE orders SET status = 'refunded', refunded_at = now() WHERE id = $1").bind(order_id).execute(&mut *tx).await?;
            if order.kind == "coins"
                && let Some(user) = order.user_id
            {
                claw_back_spends(&mut tx, user, order.paid_at).await?;
            }
            tx.commit().await?;
            audit(&state.pool, order.server_id, order.user_id, &format!("{}.refunded", order.kind), json!({ "order": order_id, "reason": facts.status })).await?;
        }
        _ => {} // em processamento ou aviso repetido
    }
    Ok(())
}

/// Pagamento recebido na conta do Conexão: destaque (fluxo antigo) ou pedido novo.
pub async fn process_platform_payment(state: &AppState, payment_id: &str) -> ApiResult<()> {
    let token = platform_token(state)?;
    let payment = get_payment(state, token, payment_id).await?;
    if crate::routes::featured::apply_payment(state, payment_id, &payment).await? {
        return Ok(());
    }
    apply_order_payment(state, payment_id, &PaymentFacts::from_json(&payment), false).await
}

/// Pagamento de um apoio: consultado com o token do dono que recebeu.
pub async fn process_support_payment(state: &AppState, order_id: Uuid, payment_id: &str) -> ApiResult<()> {
    let server_id: Option<Option<Uuid>> =
        sqlx::query_scalar("SELECT server_id FROM orders WHERE id = $1 AND kind = 'support' AND status IN ('pending', 'approved')")
        .bind(order_id)
        .fetch_optional(&state.pool)
        .await?;
    let Some(Some(server_id)) = server_id else { return Ok(()) };
    let token = crate::routes::support::seller_token(state, server_id).await?;
    let payment = get_payment(state, &token, payment_id).await?;
    let facts = PaymentFacts::from_json(&payment);
    if facts.reference != Some(order_id) {
        tracing::warn!(%order_id, payment_id, "aviso de apoio com pagamento de outro pedido; ignorado");
        return Ok(());
    }
    apply_order_payment(state, payment_id, &facts, true).await
}

/// Concilia um pedido pendente buscando os pagamentos dele (quando o webhook não chegou).
pub async fn reconcile(state: &AppState, order_id: Uuid) -> ApiResult<()> {
    // No máximo uma consulta ao Mercado Pago a cada 20 s por pedido, por mais que perguntem.
    let row: Option<(String, Option<Uuid>)> = sqlx::query_as(
        "UPDATE orders SET last_checked_at = now()
          WHERE id = $1 AND status = 'pending' AND pay_with = 'money'
            AND (last_checked_at IS NULL OR last_checked_at < now() - interval '20 seconds')
          RETURNING kind, server_id",
    )
    .bind(order_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((kind, server_id)) = row else { return Ok(()) };
    if kind == "support" {
        let Some(server_id) = server_id else { return Ok(()) };
        let token = crate::routes::support::seller_token(state, server_id).await?;
        for payment_id in search_payments(state, &token, order_id).await? {
            process_support_payment(state, order_id, &payment_id).await?;
        }
    } else {
        let token = platform_token(state)?;
        for payment_id in search_payments(state, token, order_id).await? {
            process_platform_payment(state, &payment_id).await?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ gasto com moedas

/// Paga um produto com moedas: debita, entrega e registra, tudo numa transação.
/// `key` vem do cliente (UUID por clique): repetir o mesmo clique não cobra duas vezes.
pub async fn spend_coins(
    state: &AppState,
    user_id: Uuid,
    product: Product,
    server: Option<(Uuid, &str)>,
    key: Uuid,
) -> ApiResult<OrderView> {
    let price = product.coin_price().ok_or_else(|| ApiError::bad("Este item não pode ser pago com Moedas."))?;
    let order_id = Uuid::new_v4();
    let reference = format!("spend:{user_id}:{key}");
    let duplicate = || ApiError::Conflict { message: "Esta compra já foi feita.".into(), details: json!({ "code": "duplicate" }) };
    // Clique repetido: responde "já feita" antes de olhar o saldo (a trava real é o índice único).
    let seen: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM coin_ledger WHERE ref = $1)").bind(&reference).fetch_one(&state.pool).await?;
    if seen {
        return Err(duplicate());
    }
    let mut tx = state.pool.begin().await?;
    let description = product.title(server.map(|(_, name)| name));
    match ledger_move(&mut tx, user_id, -price, LedgerKind::Spend, &reference, &description, None, false).await {
        Ok(_) => {}
        Err(ApiError::Conflict { .. }) => return Err(duplicate()),
        Err(other) => return Err(other),
    }
    let order = OrderRow {
        id: order_id,
        user_id: Some(user_id),
        kind: product.kind().into(),
        server_id: server.map(|(id, _)| id),
        pay_with: "coins".into(),
        amount_cents: 0,
        coins: price,
        days: product.days(),
        seller_mp_user_id: None,
        status: "approved".into(),
        mp_payment_id: None,
        paid_at: None,
    };
    let until = apply_benefit(&mut tx, &order, 1, LedgerKind::Refund).await?;
    sqlx::query(
        "INSERT INTO orders (id, user_id, kind, sku, server_id, pay_with, amount_cents, coins, days, status, paid_at, granted_until)
         VALUES ($1, $2, $3, $4, $5, 'coins', 0, $6, $7, 'approved', now(), $8)",
    )
    .bind(order_id)
    .bind(user_id)
    .bind(product.kind())
    .bind(product.sku())
    .bind(order.server_id)
    .bind(price)
    .bind(product.days())
    .bind(until)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    audit(&state.pool, order.server_id, Some(user_id), &format!("{}.coins", product.kind()), json!({ "order": order_id, "coins": price })).await?;
    Ok(sqlx::query_as::<_, OrderView>(&format!("{ORDER_VIEW_SELECT} WHERE o.id = $1")).bind(order_id).fetch_one(&state.pool).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> crate::config::Config {
        // SAFETY dos testes: só variáveis próprias; os testes deste módulo não rodam em paralelo com outros que leem estas.
        unsafe {
            std::env::set_var("CONEXAO_JWT_SECRET", "x".repeat(40));
            std::env::set_var("DATABASE_URL", "postgres://localhost/teste");
        }
        crate::config::Config::from_env().unwrap()
    }

    #[test]
    fn parses_skus_only_from_catalog() {
        let config = config();
        assert_eq!(Product::parse(&config, "moedas-500"), Some(Product::Coins { coins: 500, cents: 500 }));
        assert_eq!(Product::parse(&config, "plus-30"), Some(Product::Plus { days: 30, cents: 990 }));
        assert_eq!(Product::parse(&config, "pro-90"), Some(Product::Pro { days: 90, cents: 7990 }));
        assert_eq!(Product::parse(&config, "destaque-7"), Some(Product::Featured { days: 7, cents: 2990 }));
        // fora do catálogo, negativo, lixo
        for sku in ["moedas-999999", "plus-31", "pro--30", "plus-", "admin-1", "plus-30-1", "", "moedas-500.0"] {
            assert_eq!(Product::parse(&config, sku), None, "{sku}");
        }
        assert_eq!(Product::Coins { coins: 500, cents: 500 }.coin_price(), None);
        assert_eq!(Product::Plus { days: 30, cents: 990 }.coin_price(), Some(990));
    }

    #[test]
    fn support_fee_rounds_and_clamps() {
        assert_eq!(support_fee(1000, 1500), 150);
        assert_eq!(support_fee(999, 1500), 150); // 149,85 → 150
        assert_eq!(support_fee(500, 1500), 75);
        assert_eq!(support_fee(100, 0), 0);
        assert_eq!(support_fee(100, 10_000), 100);
    }

    #[test]
    fn payment_facts() {
        let p = json!({ "external_reference": "6f1c2b9e-1111-4222-8333-444455556666", "status": "approved",
            "transaction_amount": 9.9, "currency_id": "BRL", "collector_id": 12345 });
        let f = PaymentFacts::from_json(&p);
        assert_eq!(f.paid_cents, 990);
        assert!(f.covers(990));
        assert!(!f.covers(991));
        assert_eq!(f.collector_id.as_deref(), Some("12345"));
        let usd = PaymentFacts { currency: "USD".into(), ..f };
        assert!(!usd.covers(990));
        let weird = PaymentFacts::from_json(&json!({ "transaction_amount": -5, "external_reference": "nao-uuid" }));
        assert_eq!(weird.paid_cents, 0);
        assert_eq!(weird.reference, None);
        assert!(!valid_payment_id("1/../2"));
        assert!(valid_payment_id("123456789"));
    }
}
