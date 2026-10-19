//! Página personalizada (PRO) e link do formulário contra um PostgreSQL de verdade.
//! Rodam só com `CONEXAO_TEST_DATABASE_URL` definido (veja `billing_db_tests.rs`).

use axum::Json;
use axum::extract::{Path, State};
use bytes::Bytes;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::{AuthUser, MaybeUser, USER_COLUMNS, User};
use crate::routes::{page, servers};
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
    config.storage_dir = std::env::temp_dir().join(format!("conexao-page-test-{}", std::process::id()));
    Some(AppState::new(config, pool))
}

async fn user(pool: &PgPool) -> User {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, minecraft_uuid, minecraft_name) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(Uuid::new_v4())
        .bind(format!("p{}", &id.simple().to_string()[..12]))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query_as(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1")).bind(id).fetch_one(pool).await.unwrap()
}

async fn server(pool: &PgPool, owner: &User) -> (Uuid, String) {
    let id = Uuid::new_v4();
    let slug = format!("pg{}", &id.simple().to_string()[..12]);
    sqlx::query(
        "INSERT INTO servers (id, slug, name, address, minecraft_version, loader, loader_version, approval, pro_until)
         VALUES ($1, $2, 'Página', 'mc.teste', '1.21.1', 'neoforge', '21.1.1', 'approved', now() + interval '10 days')",
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
    let img = image::RgbImage::from_pixel(6, 6, image::Rgb([seed, 200, 30]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img).write_to(&mut out, image::ImageFormat::Png).unwrap();
    Bytes::from(out.into_inner())
}

fn gif() -> Bytes {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, RgbaImage};
    let mut out = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut out);
        encoder.set_repeat(Repeat::Infinite).unwrap();
        for i in 0..2u8 {
            let frame = RgbaImage::from_pixel(5, 5, image::Rgba([i * 90, 10, 200, 255]));
            encoder.encode_frame(Frame::from_parts(frame, 0, 0, Delay::from_numer_denom_ms(80, 1))).unwrap();
        }
    }
    Bytes::from(out)
}

async fn upload(state: &AppState, owner: &User, slug: &str, body: Bytes) -> String {
    let Json(value) = page::upload(State(state.clone()), AuthUser(owner.clone()), Path(slug.to_string()), body)
        .await
        .unwrap_or_else(|_| panic!("envio recusado"));
    value["url"].as_str().unwrap().to_string()
}

async fn get(state: &AppState, who: Option<&User>, slug: &str) -> page::PageResponse {
    page::get(State(state.clone()), MaybeUser(who.cloned()), Path(slug.to_string())).await.unwrap_or_else(|_| panic!("get")).0
}

async fn put(state: &AppState, who: &User, slug: &str, value: serde_json::Value) -> bool {
    page::put(State(state.clone()), AuthUser(who.clone()), Path(slug.to_string()), Json(serde_json::from_value(value).unwrap()))
        .await
        .is_ok()
}

async fn summary(state: &AppState, slug: &str) -> conexao_types::ServerSummary {
    let Json(list) = servers::list(State(state.clone())).await.unwrap_or_else(|_| panic!("list"));
    list.into_iter().find(|s| s.slug == slug).unwrap()
}

#[tokio::test]
async fn pro_page_lifecycle() {
    let Some(state) = setup().await else { return };
    let pool = state.pool.clone();
    let owner = user(&pool).await;
    let stranger = user(&pool).await;
    let (server_id, slug) = server(&pool, &owner).await;

    // Envio: PNG e GIF animado (o GIF continua .gif).
    let a = upload(&state, &owner, &slug, png(1)).await;
    let b = upload(&state, &owner, &slug, png(2)).await;
    let g = upload(&state, &owner, &slug, gif()).await;
    assert!(g.ends_with(".gif"));
    let disk = |url: &str| state.config.storage_dir.join("images").join(url.rsplit('/').next().unwrap());
    assert!(disk(&g).exists());
    // Quem não é da equipe não envia.
    assert!(page::upload(State(state.clone()), AuthUser(stranger.clone()), Path(slug.clone()), png(3)).await.is_err());

    let full = json!({
        "theme": { "colors": ["#101820", "#ff6a00", "#ffd000"], "angle": 120, "accent": "#ffd000" },
        "blocks": [
            { "type": "text", "title": "Bem-vindo", "body": "Cidade medieval." },
            { "type": "image", "url": a, "caption": "Spawn" },
            { "type": "gallery", "urls": [b, g] },
            { "type": "video", "youtube": "https://www.youtube.com/watch?v=dQw4w9WgXcQ" }
        ]
    });
    assert!(put(&state, &owner, &slug, full.clone()).await);
    assert!(!put(&state, &stranger, &slug, full.clone()).await);

    // Imagem de outro servidor não entra.
    let other_owner = user(&pool).await;
    let (_, other_slug) = server(&pool, &other_owner).await;
    let foreign = upload(&state, &other_owner, &other_slug, png(9)).await;
    assert!(!put(&state, &owner, &slug, json!({ "blocks": [{ "type": "image", "url": foreign }] })).await);

    // Jogador vê a página com o PRO ativo.
    let public = get(&state, None, &slug).await;
    assert!(public.active && !public.can_edit && public.usage.is_none());
    assert_eq!(public.page.blocks.len(), 4);
    assert!(matches!(&public.page.blocks[3], page::Block::Video { youtube, .. } if youtube == "dQw4w9WgXcQ"));
    assert!(summary(&state, &slug).await.has_page);
    let admin = get(&state, Some(&owner), &slug).await;
    assert!(admin.can_edit);
    assert_eq!(admin.usage.as_ref().unwrap().uploads_24h, 3);

    // PRO venceu: some para os jogadores, fica guardada para a equipe, não dá para editar.
    sqlx::query("UPDATE servers SET pro_until = now() - interval '1 day' WHERE id = $1").bind(server_id).execute(&pool).await.unwrap();
    let public = get(&state, None, &slug).await;
    assert!(!public.active && public.page.blocks.is_empty() && public.page.theme.is_none());
    assert_eq!(get(&state, Some(&owner), &slug).await.page.blocks.len(), 4);
    assert!(!summary(&state, &slug).await.has_page);
    assert!(!put(&state, &owner, &slug, json!({})).await);
    assert!(page::upload(State(state.clone()), AuthUser(owner.clone()), Path(slug.clone()), png(4)).await.is_err());

    // Renovou: volta como estava.
    sqlx::query("UPDATE servers SET pro_until = now() + interval '5 days' WHERE id = $1").bind(server_id).execute(&pool).await.unwrap();
    assert_eq!(get(&state, None, &slug).await.page.blocks.len(), 4);

    // Tirou a galeria: depois de 48 h, as imagens dela saem do disco (a outra continua).
    assert!(put(&state, &owner, &slug, json!({ "blocks": [{ "type": "image", "url": a }] })).await);
    sqlx::query("UPDATE server_page_media SET created_at = now() - interval '3 days' WHERE server_id = $1")
        .bind(server_id)
        .execute(&pool)
        .await
        .unwrap();
    let report = crate::cleanup::collect(&state).await.unwrap();
    assert!(report.page_media >= 2);
    assert!(disk(&a).exists());
    assert!(!disk(&b).exists() && !disk(&g).exists());

    // Página vazia volta a ser {} (sem página no launcher).
    assert!(put(&state, &owner, &slug, json!({})).await);
    assert!(!summary(&state, &slug).await.has_page);
    let _ = std::fs::remove_dir_all(&state.config.storage_dir);
}

#[tokio::test]
async fn whitelist_form_link() {
    let Some(state) = setup().await else { return };
    let owner = user(&state.pool).await;
    let (_, slug) = server(&state.pool, &owner).await;
    let patch = |value: serde_json::Value| serde_json::from_value::<servers::ServerPatch>(value).unwrap();

    let set = servers::update(
        State(state.clone()),
        AuthUser(owner.clone()),
        Path(slug.clone()),
        Json(patch(json!({ "whitelist_form_url": " https://forms.gle/AbC123 " }))),
    )
    .await;
    assert!(set.is_ok());
    assert_eq!(summary(&state, &slug).await.whitelist_form_url.as_deref(), Some("https://forms.gle/AbC123"));

    // Outros campos sem o link não apagam o link.
    assert!(servers::update(State(state.clone()), AuthUser(owner.clone()), Path(slug.clone()), Json(patch(json!({ "name": "Novo" }))))
        .await
        .is_ok());
    assert!(summary(&state, &slug).await.whitelist_form_url.is_some());

    assert!(
        servers::update(
            State(state.clone()),
            AuthUser(owner.clone()),
            Path(slug.clone()),
            Json(patch(json!({ "whitelist_form_url": "https://golpe.com/forms" })))
        )
        .await
        .is_err()
    );
    assert!(
        servers::update(State(state.clone()), AuthUser(owner.clone()), Path(slug.clone()), Json(patch(json!({ "whitelist_form_url": "" }))))
            .await
            .is_ok()
    );
    assert!(summary(&state, &slug).await.whitelist_form_url.is_none());
}
