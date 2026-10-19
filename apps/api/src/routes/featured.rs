//! Destaque pago com Mercado Pago (Checkout Pro: Pix, cartão e boleto).
//!
//! 1. O dono escolhe um plano no painel → `POST /servers/{slug}/featured/checkout`
//!    cria a compra (pendente) e a preferência no Mercado Pago, e devolve o link.
//! 2. O Mercado Pago avisa no webhook (assinado com HMAC). A API **sempre** consulta
//!    o pagamento na API do Mercado Pago antes de aplicar: o corpo do aviso nunca é
//!    tratado como prova de pagamento.
//! 3. Pagamento aprovado → o destaque do servidor é estendido pelos dias do plano,
//!    uma única vez por compra (idempotente, mesmo com avisos repetidos).
//!
//! Sem webhook (ex.: teste local), a tela de retorno do painel consulta
//! `GET /servers/{slug}/featured/purchases/{id}`, que concilia pela busca de pagamentos.

use std::time::Duration;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use chrono::{DateTime, SecondsFormat, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;
use uuid::Uuid;

use crate::auth::{AuthUser, Role, audit, constant_eq, require_role};
use crate::error::{ApiError, ApiResult};
use crate::routes::servers::server_id_by_slug;
use crate::state::AppState;

#[derive(Serialize)]
pub struct Plan {
    pub days: i32,
    pub price_cents: i32,
}

pub async fn plans(State(state): State<AppState>) -> Json<serde_json::Value> {
    let plans: Vec<Plan> = state.config.featured_plans.iter().map(|(d, c)| Plan { days: *d, price_cents: *c }).collect();
    Json(json!({ "enabled": state.config.mp_access_token.is_some(), "plans": plans }))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Purchase {
    pub id: Uuid,
    pub days: i32,
    pub amount_cents: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
    pub featured_until: Option<DateTime<Utc>>,
    pub buyer_name: Option<String>,
}

const PURCHASE_SELECT: &str = "SELECT p.id, p.days, p.amount_cents, p.status, p.created_at, p.paid_at, p.featured_until,
        u.minecraft_name AS buyer_name
   FROM featured_purchases p LEFT JOIN users u ON u.id = p.user_id";

#[derive(Deserialize)]
pub struct CheckoutInput {
    pub days: i32,
    /// Opcional: nome e e-mail de quem paga. Preenchem o checkout e melhoram a
    /// aprovação de cartão (o Mercado Pago usa no antifraude).
    #[serde(default)]
    pub payer: Option<PayerInput>,
}

#[derive(Deserialize)]
pub struct PayerInput {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
}

/// Monta o bloco `payer` da preferência; campos inválidos são ignorados.
fn payer_json(payer: Option<&PayerInput>) -> Option<serde_json::Value> {
    let payer = payer?;
    let mut out = serde_json::Map::new();
    let email = payer.email.trim();
    let email_ok = email.len() <= 254
        && !email.contains(char::is_whitespace)
        && email.split_once('@').is_some_and(|(user, host)| !user.is_empty() && host.contains('.'));
    if email_ok {
        out.insert("email".into(), json!(email.to_lowercase()));
    }
    let name: String = payer.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if !name.is_empty() && name.chars().count() <= 80 {
        let (first, last) = name.split_once(' ').unwrap_or((&name, ""));
        out.insert("name".into(), json!(first));
        if !last.is_empty() {
            out.insert("surname".into(), json!(last));
        }
    }
    if out.is_empty() { None } else { Some(serde_json::Value::Object(out)) }
}

fn token(state: &AppState) -> ApiResult<&str> {
    state
        .config
        .mp_access_token
        .as_deref()
        .ok_or_else(|| ApiError::bad("Pagamentos ainda não estão ativos no Conexão. Fale com a equipe."))
}

pub async fn checkout(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Json(input): Json<CheckoutInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Admin).await?;
    let token = token(&state)?;
    let (name, approval): (String, String) = sqlx::query_as("SELECT name, approval FROM servers WHERE id = $1")
        .bind(server_id)
        .fetch_one(&state.pool)
        .await?;
    if approval != "approved" {
        return Err(ApiError::bad("O destaque pode ser comprado depois que a equipe Conexão aprovar o servidor."));
    }
    let (days, cents) = state
        .config
        .featured_plans
        .iter()
        .copied()
        .find(|(d, _)| *d == input.days)
        .ok_or_else(|| ApiError::bad("Plano inválido."))?;

    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO featured_purchases (id, server_id, user_id, days, amount_cents) VALUES ($1, $2, $3, $4, $5)")
        .bind(id)
        .bind(server_id)
        .bind(user.id)
        .bind(days)
        .bind(cents)
        .execute(&state.pool)
        .await?;

    let back = format!("{}/#/s/{slug}?tab=geral&compra={id}", state.config.panel_url);
    let mut preference = json!({
        "items": [{
            "id": format!("destaque-{days}"),
            "title": format!("Destaque de {days} dias no Conexão — {name}"),
            "quantity": 1,
            "unit_price": f64::from(cents) / 100.0,
            "currency_id": "BRL",
            "description": format!("{name} na vitrine e no topo da lista de servidores do launcher Conexão por {days} dias."),
            "category_id": "services",
        }],
        "external_reference": id.to_string(),
        "notification_url": format!("{}/v1/payments/mercadopago/webhook", state.config.public_url),
        "back_urls": { "success": back, "pending": back, "failure": back },
        "statement_descriptor": "Conexão",
        "metadata": { "purchase_id": id.to_string(), "server": slug },
        // O link de pagamento vale 24 h; depois disso o dono gera outro no painel.
        "expires": true,
        "expiration_date_from": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, false),
        "expiration_date_to": (Utc::now() + chrono::Duration::hours(24)).to_rfc3339_opts(SecondsFormat::Millis, false),
    });
    if let Some(payer) = payer_json(input.payer.as_ref()) {
        preference["payer"] = payer;
    }
    // O Mercado Pago só aceita retorno automático para URLs HTTPS.
    if state.config.panel_url.starts_with("https://") {
        preference["auto_return"] = json!("approved");
    }
    let response = state
        .http
        .post(format!("{}/checkout/preferences", state.config.mp_api_url))
        .bearer_auth(token)
        .header("X-Idempotency-Key", id.to_string())
        .timeout(Duration::from_secs(20))
        .json(&preference)
        .send()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        tracing::error!("Mercado Pago recusou a preferência ({status}): {body}");
        sqlx::query("UPDATE featured_purchases SET status = 'cancelled' WHERE id = $1").bind(id).execute(&state.pool).await?;
        return Err(ApiError::bad("Não foi possível iniciar o pagamento agora. Tente de novo em instantes."));
    }
    let created: serde_json::Value = response.json().await.map_err(|e| ApiError::Internal(e.into()))?;
    let checkout_url = created["init_point"].as_str().unwrap_or_default().to_string();
    sqlx::query("UPDATE featured_purchases SET mp_preference_id = $2 WHERE id = $1")
        .bind(id)
        .bind(created["id"].as_str())
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(server_id), Some(user.id), "featured.checkout", json!({ "purchase": id, "days": days })).await?;
    Ok(Json(json!({ "purchase_id": id, "checkout_url": checkout_url })))
}

pub async fn purchases(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<Vec<Purchase>>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Admin).await?;
    Ok(Json(
        sqlx::query_as::<_, Purchase>(&format!(
            "{PURCHASE_SELECT} WHERE p.server_id = $1 AND p.status <> 'cancelled' ORDER BY p.created_at DESC LIMIT 50"
        ))
        .bind(server_id)
        .fetch_all(&state.pool)
        .await?,
    ))
}

/// Situação de uma compra; se ainda pendente, pergunta ao Mercado Pago.
pub async fn purchase(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((slug, id)): Path<(String, Uuid)>,
) -> ApiResult<Json<Purchase>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Admin).await?;
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM featured_purchases WHERE id = $1 AND server_id = $2")
        .bind(id)
        .bind(server_id)
        .fetch_optional(&state.pool)
        .await?;
    match status.as_deref() {
        None => return Err(ApiError::NotFound),
        Some("pending") => {
            if let Err(error) = reconcile(&state, id).await {
                tracing::warn!("não foi possível conciliar a compra {id}: {error:?}");
            }
        }
        _ => {}
    }
    Ok(Json(
        sqlx::query_as::<_, Purchase>(&format!("{PURCHASE_SELECT} WHERE p.id = $1")).bind(id).fetch_one(&state.pool).await?,
    ))
}

/// Busca pagamentos da compra no Mercado Pago (quando o webhook não chegou).
async fn reconcile(state: &AppState, id: Uuid) -> ApiResult<()> {
    let token = token(state)?;
    let found: serde_json::Value = state
        .http
        .get(format!("{}/v1/payments/search", state.config.mp_api_url))
        .bearer_auth(token)
        .query(&[("external_reference", id.to_string()), ("sort", "date_created".into()), ("criteria", "desc".into())])
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .json()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    for payment in found["results"].as_array().into_iter().flatten() {
        if let Some(payment_id) = payment["id"].as_i64().map(|v| v.to_string()).or_else(|| payment["id"].as_str().map(str::to_string)) {
            process_payment(state, &payment_id).await?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ webhook

/// `x-signature: ts=…,v1=…`, onde v1 = HMAC-SHA256("id:{data.id};request-id:{x-request-id};ts:{ts};").
pub fn signature_ok(secret: &str, header: &str, request_id: &str, data_id: &str) -> bool {
    let mut ts = None;
    let mut v1 = None;
    for part in header.split(',') {
        if let Some((key, value)) = part.split_once('=') {
            match key.trim() {
                "ts" => ts = Some(value.trim()),
                "v1" => v1 = Some(value.trim()),
                _ => {}
            }
        }
    }
    let (Some(ts), Some(v1)) = (ts, v1) else { return false };
    // Partes ausentes saem do manifesto (regra do Mercado Pago).
    let id_part = if data_id.is_empty() { String::new() } else { format!("id:{data_id};") };
    let request_part = if request_id.is_empty() { String::new() } else { format!("request-id:{request_id};") };
    let manifest = format!("{id_part}{request_part}ts:{ts};");
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else { return false };
    mac.update(manifest.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    constant_eq(expected.as_bytes(), v1.to_lowercase().as_bytes())
}

#[derive(Deserialize, Default)]
pub struct WebhookQuery {
    #[serde(rename = "data.id")]
    pub data_id: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

pub async fn webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<WebhookQuery>,
    body: bytes::Bytes,
) -> StatusCode {
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    let kind = query.kind.clone().or_else(|| json["type"].as_str().map(str::to_string)).unwrap_or_default();
    let data_id = query
        .data_id
        .or_else(|| json["data"]["id"].as_str().map(str::to_string))
        .or_else(|| json["data"]["id"].as_i64().map(|v| v.to_string()));
    let Some(data_id) = data_id.filter(|id| !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())) else {
        return StatusCode::BAD_REQUEST;
    };
    if let Some(secret) = &state.config.mp_webhook_secret {
        let header = headers.get("x-signature").and_then(|v| v.to_str().ok()).unwrap_or_default();
        let request_id = headers.get("x-request-id").and_then(|v| v.to_str().ok()).unwrap_or_default();
        if !signature_ok(secret, header, request_id, &data_id.to_lowercase()) {
            tracing::warn!("webhook do Mercado Pago com assinatura inválida (data.id {data_id})");
            return StatusCode::UNAUTHORIZED;
        }
    }
    if kind == "payment" {
        // Responde 200 na hora e processa depois (o Mercado Pago reenvia se demorarmos).
        tokio::spawn(async move {
            if let Err(error) = process_payment(&state, &data_id).await {
                tracing::error!("falha ao processar o pagamento {data_id}: {error:?}");
            }
        });
    }
    StatusCode::OK
}

/// Consulta o pagamento no Mercado Pago e aplica o que ele pagou (destaque ou pedido).
pub async fn process_payment(state: &AppState, payment_id: &str) -> ApiResult<()> {
    crate::billing::process_platform_payment(state, payment_id).await
}

/// Aplica/reverte o destaque se o pagamento for de uma compra de destaque.
/// Devolve `false` quando o pagamento não é de destaque (aí `billing` cuida).
pub async fn apply_payment(state: &AppState, payment_id: &str, payment: &serde_json::Value) -> ApiResult<bool> {
    let Some(purchase_id) = payment["external_reference"].as_str().and_then(|r| Uuid::parse_str(r).ok()) else {
        return Ok(true); // pagamento que não é do Conexão
    };
    let status = payment["status"].as_str().unwrap_or_default();
    let amount = payment["transaction_amount"].as_f64().unwrap_or_default();
    let currency = payment["currency_id"].as_str().unwrap_or_default();

    let mut tx = state.pool.begin().await?;
    let row: Option<(Uuid, i32, i32, String, Option<String>)> =
        sqlx::query_as("SELECT server_id, days, amount_cents, status, mp_payment_id FROM featured_purchases WHERE id = $1 FOR UPDATE")
            .bind(purchase_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((server_id, days, cents, current, paid_with)) = row else { return Ok(false) };
    let paid_cents = (amount * 100.0).round() as i64;

    match status {
        "approved" if current == "pending" || current == "rejected" || current == "cancelled" => {
            if currency != "BRL" || paid_cents < i64::from(cents) {
                tracing::error!(%purchase_id, amount, currency, "valor do pagamento não confere; destaque não aplicado");
                return Ok(true);
            }
            let until: DateTime<Utc> = sqlx::query_scalar(
                "UPDATE servers SET featured_until = GREATEST(COALESCE(featured_until, now()), now()) + make_interval(days => $2)
                  WHERE id = $1 RETURNING featured_until",
            )
            .bind(server_id)
            .bind(days)
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE featured_purchases SET status = 'approved', paid_at = now(), mp_payment_id = $2, featured_until = $3
                  WHERE id = $1",
            )
            .bind(purchase_id)
            .bind(payment_id)
            .bind(until)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            audit(&state.pool, Some(server_id), None, "featured.paid", json!({ "purchase": purchase_id, "days": days, "until": until })).await?;
            tracing::info!(%purchase_id, %server_id, days, "destaque aplicado até {until}");
        }
        "rejected" | "cancelled" if current == "pending" => {
            sqlx::query("UPDATE featured_purchases SET status = $2, mp_payment_id = $3 WHERE id = $1")
                .bind(purchase_id)
                .bind(status)
                .bind(payment_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        // Estorno que chega antes da aprovação: fecha a compra para um "aprovado" atrasado não ligar o destaque.
        "refunded" | "charged_back" if current == "pending" => {
            sqlx::query("UPDATE featured_purchases SET status = 'refunded', mp_payment_id = $2 WHERE id = $1")
                .bind(purchase_id)
                .bind(payment_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        // Só o pagamento que ligou o destaque o desliga (um pagamento duplicado estornado não tira os dias).
        "refunded" | "charged_back" if current == "approved" && paid_with.as_deref() == Some(payment_id) => {
            // Estorno: devolve os dias (o destaque nunca fica antes de agora).
            sqlx::query(
                "UPDATE servers SET featured_until = GREATEST(now(), featured_until - make_interval(days => $2))
                  WHERE id = $1 AND featured_until IS NOT NULL",
            )
            .bind(server_id)
            .bind(days)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE featured_purchases SET status = 'refunded' WHERE id = $1").bind(purchase_id).execute(&mut *tx).await?;
            tx.commit().await?;
            audit(&state.pool, Some(server_id), None, "featured.refunded", json!({ "purchase": purchase_id, "days": days })).await?;
        }
        _ => {} // pendente/em processo, ou aviso repetido: nada a fazer
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_mercadopago_signature() {
        let secret = "segredo";
        let manifest = "id:123456;request-id:req-1;ts:1700000000;";
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(manifest.as_bytes());
        let v1 = hex::encode(mac.finalize().into_bytes());
        let header = format!("ts=1700000000,v1={v1}");
        assert!(signature_ok(secret, &header, "req-1", "123456"));
        assert!(!signature_ok(secret, &header, "req-2", "123456"));
        assert!(!signature_ok(secret, &header, "req-1", "999"));
        assert!(!signature_ok("outro", &header, "req-1", "123456"));
        assert!(!signature_ok(secret, "lixo", "req-1", "123456"));

        // Sem x-request-id, o manifesto não leva essa parte.
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(b"id:123456;ts:1700000000;");
        let header = format!("ts=1700000000,v1={}", hex::encode(mac.finalize().into_bytes()));
        assert!(signature_ok(secret, &header, "", "123456"));
    }

    #[test]
    fn builds_payer() {
        let p = |name: &str, email: &str| payer_json(Some(&PayerInput { name: name.into(), email: email.into() }));
        assert_eq!(
            p("  Lucas  Servollo Silva ", "Lucas@Exemplo.com"),
            Some(json!({"name": "Lucas", "surname": "Servollo Silva", "email": "lucas@exemplo.com"}))
        );
        assert_eq!(p("Lucas", "sem-arroba"), Some(json!({"name": "Lucas"})));
        assert_eq!(p("", "a b@x.com"), None);
        assert_eq!(payer_json(None), None);
    }

    #[test]
    fn parses_plans() {
        assert_eq!(crate::config::parse_plans("7:29.90, 30:89.90").unwrap(), vec![(7, 2990), (30, 8990)]);
        assert!(crate::config::parse_plans("0:10").is_err());
    }
}
