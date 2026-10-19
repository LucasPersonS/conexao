//! Testes contra um PostgreSQL de verdade (carteira, pedidos, corridas e estornos).
//!
//! Rodam só com `CONEXAO_TEST_DATABASE_URL` definido; sem ele, passam sem fazer nada.
//!   CONEXAO_TEST_DATABASE_URL=postgres://postgres@localhost:5432/conexao_test cargo test -p conexao-api

use sqlx::PgPool;
use uuid::Uuid;

use crate::billing::{self, LedgerKind, PaymentFacts, Product};
use crate::error::ApiError;
use crate::state::AppState;

async fn setup() -> Option<AppState> {
    let url = std::env::var("CONEXAO_TEST_DATABASE_URL").ok()?;
    // SAFETY: os testes só escrevem estas variáveis, sempre com os mesmos valores.
    unsafe {
        std::env::set_var("CONEXAO_JWT_SECRET", "segredo-de-teste-com-mais-de-32-caracteres");
        std::env::set_var("DATABASE_URL", &url);
    }
    let pool = PgPool::connect(&url).await.expect("conectar no banco de teste");
    sqlx::migrate!("./migrations").run(&pool).await.expect("migrações");
    Some(AppState::new(crate::config::Config::from_env().unwrap(), pool))
}

async fn user(pool: &PgPool, coins: i64) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, minecraft_uuid, minecraft_name, coin_balance) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(Uuid::new_v4())
        .bind(format!("t{}", &id.simple().to_string()[..12]))
        .bind(coins)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn server(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO servers (id, slug, name, address, minecraft_version, loader, loader_version, approval)
         VALUES ($1, $2, 'Teste', 'mc.teste', '1.21.1', 'neoforge', '21.1.1', 'approved')",
    )
    .bind(id)
    .bind(format!("s{}", &id.simple().to_string()[..12]))
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn balance(pool: &PgPool, user: Uuid) -> i64 {
    sqlx::query_scalar("SELECT coin_balance FROM users WHERE id = $1").bind(user).fetch_one(pool).await.unwrap()
}

async fn ledger_sum(pool: &PgPool, user: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COALESCE(SUM(delta), 0)::BIGINT FROM coin_ledger WHERE user_id = $1").bind(user).fetch_one(pool).await.unwrap()
}

async fn money_order(pool: &PgPool, user: Uuid, kind: &str, cents: i32, coins: i64, days: Option<i32>, server: Option<Uuid>, seller: Option<&str>) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO orders (id, user_id, kind, sku, server_id, pay_with, amount_cents, coins, days, seller_mp_user_id, fee_cents)
         VALUES ($1, $2, $3, 'teste', $4, 'money', $5, $6, $7, $8, $9)",
    )
    .bind(id)
    .bind(user)
    .bind(kind)
    .bind(server)
    .bind(cents)
    .bind(coins)
    .bind(days)
    .bind(seller)
    .bind(if kind == "support" { billing::support_fee(cents, 1500) } else { 0 })
    .execute(pool)
    .await
    .unwrap();
    id
}

/// Id de pagamento único por execução (o banco de teste guarda os anteriores).
fn pid(tag: &str) -> String {
    static RUN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    format!("{tag}{}", RUN.get_or_init(|| Uuid::new_v4().simple().to_string()))
}

fn facts(order: Uuid, status: &str, cents: i64, collector: Option<&str>) -> PaymentFacts {
    PaymentFacts { reference: Some(order), status: status.into(), paid_cents: cents, currency: "BRL".into(), collector_id: collector.map(str::to_string) }
}

#[tokio::test]
async fn concurrent_spends_never_overdraw() {
    let Some(state) = setup().await else { return };
    let user = user(&state.pool, 1000).await;
    let plus = Product::Plus { days: 30, cents: 990 };
    let mut tasks = Vec::new();
    for _ in 0..12 {
        let state = state.clone();
        tasks.push(tokio::spawn(async move { billing::spend_coins(&state, user, plus, None, Uuid::new_v4()).await.is_ok() }));
    }
    let mut ok = 0;
    for task in tasks {
        if task.await.unwrap() {
            ok += 1;
        }
    }
    assert_eq!(ok, 1, "só uma compra cabe no saldo");
    assert_eq!(balance(&state.pool, user).await, 10);
    assert_eq!(ledger_sum(&state.pool, user).await, -990);
    let days: f64 = sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM premium_until - now())::FLOAT8 / 86400 FROM users WHERE id = $1")
        .bind(user)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!((29.9..30.1).contains(&days), "Conexão+ estendido uma vez: {days}");
}

#[tokio::test]
async fn same_click_is_charged_once() {
    let Some(state) = setup().await else { return };
    let user = user(&state.pool, 5000).await;
    let key = Uuid::new_v4();
    let plus = Product::Plus { days: 30, cents: 990 };
    assert!(billing::spend_coins(&state, user, plus, None, key).await.is_ok());
    assert!(matches!(billing::spend_coins(&state, user, plus, None, key).await, Err(ApiError::Conflict { .. })));
    assert_eq!(balance(&state.pool, user).await, 4010);
    let orders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE user_id = $1").bind(user).fetch_one(&state.pool).await.unwrap();
    assert_eq!(orders, 1, "o clique repetido não cria outro pedido");
}

#[tokio::test]
async fn coin_purchase_is_credited_once_and_checked() {
    let Some(state) = setup().await else { return };
    let user = user(&state.pool, 0).await;
    let order = money_order(&state.pool, user, "coins", 1000, 1050, None, None, None).await;

    // pago a menos e em outra moeda: nada acontece
    billing::apply_order_payment(&state, &pid("1"), &facts(order, "approved", 999, None), false).await.unwrap();
    let usd = PaymentFacts { currency: "USD".into(), ..facts(order, "approved", 1000, None) };
    billing::apply_order_payment(&state, &pid("2"), &usd, false).await.unwrap();
    assert_eq!(balance(&state.pool, user).await, 0);

    // canal errado (aviso de apoio apontando para pedido da plataforma): ignora
    billing::apply_order_payment(&state, &pid("3"), &facts(order, "approved", 1000, None), true).await.unwrap();
    assert_eq!(balance(&state.pool, user).await, 0);

    // pago certo; aviso repetido não credita de novo
    for _ in 0..3 {
        billing::apply_order_payment(&state, &pid("4"), &facts(order, "approved", 1000, None), false).await.unwrap();
    }
    assert_eq!(balance(&state.pool, user).await, 1050);

    // gasta e depois sofre chargeback: o Conexão+ comprado com essas moedas é desfeito
    billing::spend_coins(&state, user, Product::Plus { days: 30, cents: 990 }, None, Uuid::new_v4()).await.unwrap();
    billing::apply_order_payment(&state, &pid("4"), &facts(order, "charged_back", 1000, None), false).await.unwrap();
    assert_eq!(balance(&state.pool, user).await, 0);
    assert_eq!(ledger_sum(&state.pool, user).await, 0);
    let premium: bool = sqlx::query_scalar("SELECT COALESCE(premium_until > now() + interval '1 minute', FALSE) FROM users WHERE id = $1")
        .bind(user)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(!premium, "Conexão+ comprado com moedas contestadas foi desfeito");
    assert!(billing::spend_coins(&state, user, Product::Plus { days: 30, cents: 990 }, None, Uuid::new_v4()).await.is_err());
    // estorno repetido não desconta duas vezes
    billing::apply_order_payment(&state, &pid("4"), &facts(order, "refunded", 1000, None), false).await.unwrap();
    assert_eq!(balance(&state.pool, user).await, 0);
}

#[tokio::test]
async fn support_only_counts_when_seller_received() {
    let Some(state) = setup().await else { return };
    let user = user(&state.pool, 0).await;
    let server = server(&state.pool).await;
    let order = money_order(&state.pool, user, "support", 2000, 0, None, Some(server), Some("111")).await;

    // dinheiro foi para outra conta: não vale
    billing::apply_order_payment(&state, &pid("9"), &facts(order, "approved", 2000, Some("222")), true).await.unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM orders WHERE id = $1").bind(order).fetch_one(&state.pool).await.unwrap();
    assert_eq!(status, "pending");

    billing::apply_order_payment(&state, &pid("9"), &facts(order, "approved", 2000, Some("111")), true).await.unwrap();
    let (status, fee): (String, i32) = sqlx::query_as("SELECT status, fee_cents FROM orders WHERE id = $1").bind(order).fetch_one(&state.pool).await.unwrap();
    assert_eq!((status.as_str(), fee), ("approved", 300));
    assert!(crate::routes::support::is_recent_supporter(&state, user).await.unwrap());
    // apoio não mexe em moedas
    assert_eq!(balance(&state.pool, user).await, 0);
}

#[tokio::test]
async fn pro_refund_takes_days_back() {
    let Some(state) = setup().await else { return };
    let user = user(&state.pool, 0).await;
    let server = server(&state.pool).await;
    let order = money_order(&state.pool, user, "pro", 2990, 0, Some(30), Some(server), None).await;
    billing::apply_order_payment(&state, &pid("7"), &facts(order, "approved", 2990, None), false).await.unwrap();
    let pro: bool = sqlx::query_scalar("SELECT pro_until > now() + interval '29 days' FROM servers WHERE id = $1").bind(server).fetch_one(&state.pool).await.unwrap();
    assert!(pro);
    billing::apply_order_payment(&state, &pid("7"), &facts(order, "refunded", 2990, None), false).await.unwrap();
    let pro: bool = sqlx::query_scalar("SELECT pro_until > now() + interval '1 minute' FROM servers WHERE id = $1").bind(server).fetch_one(&state.pool).await.unwrap();
    assert!(!pro, "estorno devolve os dias");
}

#[tokio::test]
async fn ledger_rejects_overdraft_and_duplicates() {
    let Some(state) = setup().await else { return };
    let user = user(&state.pool, 100).await;
    let mut tx = state.pool.begin().await.unwrap();
    assert!(billing::ledger_move(&mut tx, user, -101, LedgerKind::Spend, "x:1", "teste", None, false).await.is_err());
    drop(tx);
    let reference = format!("x:{}", Uuid::new_v4());
    let mut tx = state.pool.begin().await.unwrap();
    billing::ledger_move(&mut tx, user, 50, LedgerKind::Grant, &reference, "teste", None, false).await.unwrap();
    tx.commit().await.unwrap();
    let mut tx = state.pool.begin().await.unwrap();
    assert!(billing::ledger_move(&mut tx, user, 50, LedgerKind::Grant, &reference, "teste", None, false).await.is_err());
    drop(tx);
    assert_eq!(balance(&state.pool, user).await, 150);
}

#[tokio::test]
async fn chargeback_undoes_what_the_coins_bought() {
    let Some(state) = setup().await else { return };
    let user = user(&state.pool, 0).await;
    let server = server(&state.pool).await;
    let order = money_order(&state.pool, user, "coins", 5000, 5750, None, None, None).await;
    billing::apply_order_payment(&state, &pid("c1"), &facts(order, "approved", 5000, None), false).await.unwrap();
    // gasta tudo em PRO para um servidor
    billing::spend_coins(&state, user, Product::Pro { days: 90, cents: 5000 }, Some((server, "Teste")), Uuid::new_v4()).await.unwrap();
    let pro: bool = sqlx::query_scalar("SELECT pro_until > now() + interval '80 days' FROM servers WHERE id = $1").bind(server).fetch_one(&state.pool).await.unwrap();
    assert!(pro);
    // contesta a compra das moedas
    billing::apply_order_payment(&state, &pid("c1"), &facts(order, "charged_back", 5000, None), false).await.unwrap();
    let pro: bool = sqlx::query_scalar("SELECT COALESCE(pro_until > now() + interval '1 minute', FALSE) FROM servers WHERE id = $1").bind(server).fetch_one(&state.pool).await.unwrap();
    assert!(!pro, "o PRO comprado com moedas contestadas é desfeito");
    assert_eq!(balance(&state.pool, user).await, 0, "as moedas contestadas saem e o gasto volta");
    assert_eq!(ledger_sum(&state.pool, user).await, 0);
}

#[tokio::test]
async fn late_or_foreign_refunds_are_safe() {
    let Some(state) = setup().await else { return };
    let user = user(&state.pool, 0).await;
    // estorno chega antes da aprovação: o "aprovado" atrasado não entrega mais nada
    let order = money_order(&state.pool, user, "coins", 500, 500, None, None, None).await;
    billing::apply_order_payment(&state, &pid("r1"), &facts(order, "refunded", 500, None), false).await.unwrap();
    billing::apply_order_payment(&state, &pid("r1"), &facts(order, "approved", 500, None), false).await.unwrap();
    assert_eq!(balance(&state.pool, user).await, 0);

    // pagou duas vezes a mesma preferência; estornar o pagamento que NÃO entregou não tira o benefício
    let order = money_order(&state.pool, user, "coins", 500, 500, None, None, None).await;
    billing::apply_order_payment(&state, &pid("p1"), &facts(order, "approved", 500, None), false).await.unwrap();
    billing::apply_order_payment(&state, &pid("p2"), &facts(order, "refunded", 500, None), false).await.unwrap();
    assert_eq!(balance(&state.pool, user).await, 500);
    billing::apply_order_payment(&state, &pid("p1"), &facts(order, "refunded", 500, None), false).await.unwrap();
    assert_eq!(balance(&state.pool, user).await, 0);
}
