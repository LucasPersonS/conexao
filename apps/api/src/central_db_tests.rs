//! Parceria Central SMP's e armazenamento da página do servidor, contra um PostgreSQL de verdade.
//! Rodam só com `CONEXAO_TEST_DATABASE_URL` definido (veja `billing_db_tests.rs`).

use axum::Json;
use axum::extract::{Path, Query, State};
use bytes::Bytes;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::{AuthUser, Staff, USER_COLUMNS, User};
use crate::routes::{page, profiles, servers, waitlist};
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
    let mut config = crate::config::Config::from_env().unwrap();
    config.storage_dir = std::env::temp_dir().join(format!("conexao-central-test-{}", std::process::id()));
    config.central_guild_id = Some("123456789012345678".into());
    Some(AppState::new(config, pool))
}

async fn user(pool: &PgPool, discord: Option<&str>) -> User {
    let id = Uuid::new_v4();
    let account = if discord.is_some() { "offline" } else { "microsoft" };
    sqlx::query("INSERT INTO users (id, minecraft_uuid, minecraft_name, discord_id, account_type) VALUES ($1, $2, $3, $4, $5)")
        .bind(id)
        .bind(Uuid::new_v4())
        .bind(format!("c{}", &id.simple().to_string()[..12]))
        .bind(discord)
        .bind(account)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query_as(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1")).bind(id).fetch_one(pool).await.unwrap()
}

async fn server(pool: &PgPool, owner: &User) -> (Uuid, String) {
    let id = Uuid::new_v4();
    let slug = format!("ct{}", &id.simple().to_string()[..12]);
    sqlx::query(
        "INSERT INTO servers (id, slug, name, address, minecraft_version, loader, loader_version, approval, pro_until)
         VALUES ($1, $2, 'Central', 'mc.teste', '1.21.1', 'neoforge', '21.1.1', 'approved', now() + interval '10 days')",
    )
    .bind(id)
    .bind(&slug)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO server_members (server_id, user_id, role) VALUES ($1, $2, 'owner')")
        .bind(id)
        .bind(owner.id)
        .execute(pool)
        .await
        .unwrap();
    (id, slug)
}

fn png(seed: u8) -> Bytes {
    let img = image::RgbImage::from_pixel(7, 7, image::Rgb([seed, 77, 140]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img).write_to(&mut out, image::ImageFormat::Png).unwrap();
    Bytes::from(out.into_inner())
}

fn badge_ids(profile: &profiles::PublicProfile) -> Vec<&'static str> {
    profile.badges.iter().map(|b| b.id).collect()
}

#[tokio::test]
async fn page_check_and_media_storage() {
    let Some(state) = setup().await else { return };
    let pool = state.pool.clone();
    let owner = user(&pool, None).await;
    let stranger = user(&pool, None).await;
    let (_, slug) = server(&pool, &owner).await;
    let check = |who: &User, value: serde_json::Value| {
        let state = state.clone();
        let who = who.clone();
        let slug = slug.clone();
        async move {
            page::check(State(state), AuthUser(who), Path(slug), Json(serde_json::from_value(value).unwrap())).await.is_ok()
        }
    };

    // Conferência antes do envio: `pending:N` vale, desde que haja um arquivo para cada.
    let draft = json!({ "blocks": [{ "type": "image", "url": "pending:0" }, { "type": "gallery", "urls": ["pending:1", "pending:0"] }] });
    assert!(check(&owner, json!({ "page": draft, "uploads": [{ "bytes": 1000 }, { "bytes": 2000, "gif": true }] })).await);
    assert!(!check(&owner, json!({ "page": draft, "uploads": [{ "bytes": 1000 }] })).await);
    assert!(!check(&owner, json!({ "page": draft, "uploads": [{ "bytes": 99_000_000 }, { "bytes": 1 }] })).await);
    assert!(!check(&stranger, json!({ "page": draft, "uploads": [{ "bytes": 1 }, { "bytes": 1 }] })).await);
    // Erro na página = nada sobe (a conferência já recusa).
    assert!(!check(&owner, json!({ "page": { "blocks": [{ "type": "text" }] }, "uploads": [] })).await);
    // `pending:` nunca é salvo de verdade.
    let saved = page::put(
        State(state.clone()),
        AuthUser(owner.clone()),
        Path(slug.clone()),
        Json(serde_json::from_value(json!({ "blocks": [{ "type": "image", "url": "pending:0" }] })).unwrap()),
    )
    .await;
    assert!(saved.is_err());
    // Estourar o limite diário de envios é recusado antes de enviar.
    let many: Vec<_> = (0..=page::DAILY_UPLOADS).map(|_| json!({ "bytes": 10 })).collect();
    assert!(!check(&owner, json!({ "page": {}, "uploads": many })).await);

    // Envio + página salva com uma das imagens.
    let upload = |body: Bytes| {
        let state = state.clone();
        let owner = owner.clone();
        let slug = slug.clone();
        async move { page::upload(State(state), AuthUser(owner), Path(slug), body).await.unwrap_or_else(|_| panic!("envio")).0["url"].as_str().unwrap().to_string() }
    };
    let kept = upload(png(1)).await;
    let loose = upload(png(2)).await;
    let ok = page::put(
        State(state.clone()),
        AuthUser(owner.clone()),
        Path(slug.clone()),
        Json(serde_json::from_value(json!({ "blocks": [{ "type": "image", "url": kept }] })).unwrap()),
    )
    .await;
    assert!(ok.is_ok());

    let Json(list) = page::media_list(State(state.clone()), AuthUser(owner.clone()), Path(slug.clone())).await.unwrap_or_else(|_| panic!("lista"));
    assert_eq!(list.items.len(), 2);
    let item = |url: &str| list.items.iter().find(|m| m.url == url).unwrap();
    assert!(item(&kept).in_use && !item(&loose).in_use);
    assert!(list.total_bytes > 0);
    assert!(page::media_list(State(state.clone()), AuthUser(stranger.clone()), Path(slug.clone())).await.is_err());

    let disk = |url: &str| state.config.storage_dir.join("images").join(url.rsplit('/').next().unwrap());
    let delete = |who: &User, id: Uuid| {
        let state = state.clone();
        let who = who.clone();
        let slug = slug.clone();
        async move { page::media_delete(State(state), AuthUser(who), Path((slug, id))).await.is_ok() }
    };
    // Em uso na página: não apaga. Outro servidor/estranho: não apaga.
    assert!(!delete(&owner, item(&kept).id).await);
    assert!(!delete(&stranger, item(&loose).id).await);
    assert!(disk(&loose).exists());
    assert!(delete(&owner, item(&loose).id).await);
    assert!(!disk(&loose).exists(), "sem outro uso, o arquivo sai do disco");
    assert!(disk(&kept).exists());
    assert!(!delete(&owner, item(&loose).id).await, "já apagada");

    // Mesmo arquivo usado por outro servidor: some do armazenamento deste, mas fica no disco.
    let other_owner = user(&pool, None).await;
    let (_, other_slug) = server(&pool, &other_owner).await;
    let shared = page::upload(State(state.clone()), AuthUser(other_owner.clone()), Path(other_slug.clone()), png(5)).await.unwrap_or_else(|_| panic!("envio")).0;
    let shared_url = shared["url"].as_str().unwrap().to_string();
    let mine = upload(png(5)).await;
    assert_eq!(mine, shared_url);
    let Json(list) = page::media_list(State(state.clone()), AuthUser(owner.clone()), Path(slug.clone())).await.unwrap_or_else(|_| panic!("lista"));
    let id = list.items.iter().find(|m| m.url == mine).unwrap().id;
    assert!(delete(&owner, id).await);
    assert!(disk(&shared_url).exists());
    let _ = std::fs::remove_dir_all(&state.config.storage_dir);
}

#[tokio::test]
async fn central_origin_badges_and_batch() {
    let Some(state) = setup().await else { return };
    let pool = state.pool.clone();
    let tag = Uuid::new_v4().simple().to_string();

    // Pedido na lista vindo da Central (verificado) e um só com o link (não verificado).
    let verified_discord = format!("9{}", &tag[..10].bytes().map(|b| (b % 10 + b'0') as char).collect::<String>());
    let linked_discord = format!("8{}", &tag[10..20].bytes().map(|b| (b % 10 + b'0') as char).collect::<String>());
    for (discord, central) in [(&verified_discord, true), (&linked_discord, false)] {
        sqlx::query(
            "INSERT INTO waitlist (id, discord_id, discord_name, minecraft_name, account_type, ref, central_member)
             VALUES ($1, $2, 'teste', $3, 'offline', 'central', $4)",
        )
        .bind(Uuid::new_v4())
        .bind(discord)
        .bind(format!("n{}", &discord[..8]))
        .bind(central)
        .execute(&pool)
        .await
        .unwrap();
    }

    // A conta que entra no launcher herda a origem e o selo.
    let member = user(&pool, Some(&verified_discord)).await;
    assert!(!member.central_member);
    let member = waitlist::adopt_origin(&state, member).await.unwrap();
    assert!(member.central_member);
    let signup_ref: Option<String> = sqlx::query_scalar("SELECT signup_ref FROM users WHERE id = $1").bind(member.id).fetch_one(&pool).await.unwrap();
    assert_eq!(signup_ref.as_deref(), Some("central"));
    let linked = waitlist::adopt_origin(&state, user(&pool, Some(&linked_discord)).await).await.unwrap();
    assert!(!linked.central_member, "só o link não dá o selo");

    // O Discord confirmou que saiu da Central: o selo sai. Sem resposta: fica como estava.
    let member = waitlist::mark_central(&state, member, None).await.unwrap();
    assert!(member.central_member);
    let gone = waitlist::mark_central(&state, member.clone(), Some(false)).await.unwrap();
    assert!(!gone.central_member);
    let member = waitlist::mark_central(&state, gone, Some(true)).await.unwrap();

    // Selos no perfil: Membro da Central; SMP da Central para a equipe do servidor do programa.
    let viewer = user(&pool, None).await;
    let profile = profiles::public_by_id(&state, member.id, &viewer).await.unwrap();
    assert!(badge_ids(&profile).contains(&"central"));
    let owner = user(&pool, None).await;
    let (_, slug) = server(&pool, &owner).await;
    assert!(!badge_ids(&profiles::public_by_id(&state, owner.id, &viewer).await.unwrap()).contains(&"smp-central"));
    assert!(servers::set_central(State(state.clone()), Staff(None), Path(slug.clone()), Json(servers::CentralInput { central: true })).await.is_ok());
    assert!(badge_ids(&profiles::public_by_id(&state, owner.id, &viewer).await.unwrap()).contains(&"smp-central"));
    let Json(list) = servers::list(State(state.clone())).await.unwrap_or_else(|_| panic!("list"));
    assert!(list.iter().find(|s| s.slug == slug).unwrap().central_partner);

    // Equipe: filtro e lote só da Central.
    let query = |central: bool| Query(serde_json::from_value::<waitlist::StaffListQuery>(json!({ "status": "pending", "central": central })).unwrap());
    let Json(only) = waitlist::staff_list(State(state.clone()), Staff(None), query(true)).await.unwrap_or_else(|_| panic!("lista"));
    assert!(only.central_enabled && only.central_pending >= 1);
    assert!(only.entries.iter().all(|e| e.central_member));
    assert!(only.entries.iter().any(|e| e.discord_id.as_deref() == Some(verified_discord.as_str())));
    let Json(all) = waitlist::staff_list(State(state.clone()), Staff(None), query(false)).await.unwrap_or_else(|_| panic!("lista"));
    assert!(all.entries.iter().any(|e| e.discord_id.as_deref() == Some(linked_discord.as_str()) && e.origin.as_deref() == Some("central")));

    // Lote da Central libera só membros verificados (o "via link" continua na fila).
    let pending_central = only.central_pending;
    let next: waitlist::NextInput = serde_json::from_value(json!({ "count": pending_central, "central": true })).unwrap();
    assert!(waitlist::staff_approve_next(State(state.clone()), Staff(None), Json(next)).await.is_ok());
    let status = |discord: String| {
        let pool = pool.clone();
        async move { sqlx::query_scalar::<_, String>("SELECT status FROM waitlist WHERE discord_id = $1").bind(discord).fetch_one(&pool).await.unwrap() }
    };
    assert_eq!(status(verified_discord.clone()).await, "approved");
    assert_eq!(status(linked_discord.clone()).await, "pending");
}
