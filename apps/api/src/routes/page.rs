//! Página personalizada do servidor (plano PRO): fundo em gradiente, cor de destaque e
//! blocos de texto, imagem/GIF, galeria e vídeo do YouTube, mostrados no launcher abaixo
//! do "Sobre o servidor".
//!
//! Como no perfil dos jogadores, é só dado estruturado: cores `#rrggbb`, ângulo, textos
//! simples, imagens enviadas ao próprio Conexão e IDs de vídeo do YouTube. Nada de HTML,
//! CSS ou link livre. Quando o PRO vence, a página fica guardada (só a equipe do servidor
//! vê no painel) e volta ao renovar.

use std::collections::HashSet;

use axum::Json;
use axum::extract::{Path, State};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthUser, MaybeUser, Role, User, audit, require_role};
use crate::error::{ApiError, ApiResult};
use crate::moderation;
use crate::routes::images::{self, store_clean};
use crate::state::AppState;

pub const MAX_BLOCKS: usize = 20;
pub const MAX_GALLERY: usize = 12;
/// Imagens somando todos os blocos.
pub const MAX_IMAGES: usize = 40;
pub const MAX_TITLE: usize = 80;
pub const MAX_BODY: usize = 3000;
pub const MAX_CAPTION: usize = 140;

/// Tamanho de cada envio.
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_GIF_BYTES: usize = 8 * 1024 * 1024;
/// Envios por servidor a cada 24 h (contagem e bytes).
pub const DAILY_UPLOADS: i64 = 40;
pub const DAILY_UPLOAD_BYTES: i64 = 120 * 1024 * 1024;

/// GIF: maior lado, quadros e pixels somados (protege contra "bombas").
const MAX_GIF_SIDE: u16 = 1600;
const MAX_GIF_FRAMES: usize = 600;
const MAX_GIF_PIXELS: u64 = 400_000_000;

// ------------------------------------------------------------------- modelo

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct PageTheme {
    /// De 2 a 4 cores `#rrggbb`.
    pub colors: Vec<String>,
    /// 0 a 360 (gradiente linear).
    #[serde(default = "default_angle")]
    pub angle: i32,
    /// `linear` ou `radial`.
    #[serde(default = "default_style")]
    pub style: String,
    /// Cor dos títulos e destaques dos blocos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
}

fn default_angle() -> i32 {
    160
}

fn default_style() -> String {
    "linear".into()
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Block {
    Text {
        #[serde(default)]
        title: String,
        #[serde(default)]
        body: String,
    },
    /// Imagem ou GIF enviado pelo painel.
    Image {
        url: String,
        #[serde(default)]
        caption: String,
    },
    Gallery {
        urls: Vec<String>,
        #[serde(default)]
        caption: String,
    },
    /// Vídeo do YouTube: aceita o link (watch, youtu.be, shorts, embed, live) e guarda só o ID.
    Video {
        youtube: String,
        #[serde(default)]
        caption: String,
    },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
pub struct Page {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<PageTheme>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<Block>,
}

// ---------------------------------------------------------------- validação

fn is_hex(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// Extrai o ID (11 caracteres) de um link do YouTube, ou aceita o próprio ID.
pub fn youtube_id(input: &str) -> Option<String> {
    let valid = |id: &str| id.len() == 11 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    let input = input.trim();
    if valid(input) {
        return Some(input.to_string());
    }
    let rest = input.strip_prefix("https://").or_else(|| input.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.to_ascii_lowercase();
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let path = path.split('#').next().unwrap_or_default();
    let id = match host.as_str() {
        "youtu.be" => path.split('/').next().map(str::to_string),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com" | "youtube-nocookie.com"
        | "www.youtube-nocookie.com" => {
            let mut parts = path.split('/');
            match parts.next() {
                Some("watch") => query
                    .split('#')
                    .next()
                    .unwrap_or_default()
                    .split('&')
                    .find_map(|kv| kv.strip_prefix("v="))
                    .map(str::to_string),
                Some("shorts" | "embed" | "live" | "v") => parts.next().map(str::to_string),
                _ => None,
            }
        }
        _ => None,
    }?;
    valid(&id).then_some(id)
}

fn check_text(label: &str, value: &str, max: usize) -> Result<String, String> {
    let value = value.trim().replace("\r\n", "\n");
    if value.chars().count() > max {
        return Err(format!("{label}: no máximo {max} caracteres."));
    }
    if value.chars().any(|c| c.is_control() && c != '\n') {
        return Err(format!("{label}: caractere inválido."));
    }
    if moderation::blocked_term(&value).is_some() {
        return Err(format!("{label}: tem uma palavra bloqueada pelo filtro do Conexão."));
    }
    Ok(value)
}

/// Imagem enviada ao próprio Conexão: `{public_url}/images/{sha1}.{png|jpg|gif}`.
pub fn is_own_image(public_url: &str, url: &str) -> bool {
    let Some(name) = url.strip_prefix(public_url).and_then(|rest| rest.strip_prefix("/images/")) else {
        return false;
    };
    let Some((hash, ext)) = name.split_once('.') else {
        return false;
    };
    hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) && matches!(ext, "png" | "jpg" | "gif")
}

/// Confere e normaliza a página (textos aparados, vídeos viram ID). Devolve também as
/// imagens usadas, para conferir se foram enviadas para este servidor.
pub fn validate(public_url: &str, input: Page) -> Result<(Page, Vec<String>), String> {
    let theme = match input.theme {
        None => None,
        Some(theme) => {
            if !(2..=4).contains(&theme.colors.len()) || !theme.colors.iter().all(|c| is_hex(c)) {
                return Err("O gradiente aceita de 2 a 4 cores no formato #rrggbb.".into());
            }
            if !(0..=360).contains(&theme.angle) {
                return Err("O ângulo do gradiente deve ficar entre 0 e 360.".into());
            }
            if !matches!(theme.style.as_str(), "linear" | "radial") {
                return Err("Tipo de gradiente inválido.".into());
            }
            if theme.accent.as_deref().is_some_and(|c| !is_hex(c)) {
                return Err("A cor de destaque deve estar no formato #rrggbb.".into());
            }
            Some(PageTheme {
                colors: theme.colors.iter().map(|c| c.to_ascii_lowercase()).collect(),
                accent: theme.accent.map(|c| c.to_ascii_lowercase()),
                ..theme
            })
        }
    };
    if input.blocks.len() > MAX_BLOCKS {
        return Err(format!("Use no máximo {MAX_BLOCKS} blocos."));
    }
    let mut images = Vec::new();
    let mut blocks = Vec::with_capacity(input.blocks.len());
    for (index, block) in input.blocks.into_iter().enumerate() {
        let n = index + 1;
        let block = match block {
            Block::Text { title, body } => {
                let title = check_text(&format!("Bloco {n}, título"), &title, MAX_TITLE)?;
                let body = check_text(&format!("Bloco {n}, texto"), &body, MAX_BODY)?;
                if title.is_empty() && body.is_empty() {
                    return Err(format!("Bloco {n}: escreva um título ou um texto (ou remova o bloco)."));
                }
                Block::Text { title, body }
            }
            Block::Image { url, caption } => {
                if !is_own_image(public_url, &url) {
                    return Err(format!("Bloco {n}: envie a imagem pelo painel."));
                }
                images.push(url.clone());
                Block::Image { url, caption: check_text(&format!("Bloco {n}, legenda"), &caption, MAX_CAPTION)? }
            }
            Block::Gallery { urls, caption } => {
                if !(2..=MAX_GALLERY).contains(&urls.len()) {
                    return Err(format!("Bloco {n}: a galeria precisa de 2 a {MAX_GALLERY} imagens."));
                }
                if urls.iter().any(|u| !is_own_image(public_url, u)) {
                    return Err(format!("Bloco {n}: envie as imagens da galeria pelo painel."));
                }
                images.extend(urls.iter().cloned());
                Block::Gallery { urls, caption: check_text(&format!("Bloco {n}, legenda"), &caption, MAX_CAPTION)? }
            }
            Block::Video { youtube, caption } => {
                let Some(id) = youtube_id(&youtube) else {
                    return Err(format!("Bloco {n}: cole um link de vídeo do YouTube."));
                };
                Block::Video { youtube: id, caption: check_text(&format!("Bloco {n}, legenda"), &caption, MAX_CAPTION)? }
            }
        };
        blocks.push(block);
    }
    if images.len() > MAX_IMAGES {
        return Err(format!("Use no máximo {MAX_IMAGES} imagens na página."));
    }
    Ok((Page { theme, blocks }, images))
}

// ---------------------------------------------------------------------- GIF

fn skip_sub_blocks(bytes: &[u8], mut pos: usize) -> Option<usize> {
    loop {
        let len = *bytes.get(pos)? as usize;
        pos += 1;
        if len == 0 {
            return Some(pos);
        }
        pos = pos.checked_add(len)?;
        if pos > bytes.len() {
            return None;
        }
    }
}

fn color_table_len(flags: u8) -> usize {
    if flags & 0x80 != 0 { 3 * (1usize << ((flags & 0x07) + 1)) } else { 0 }
}

/// Regrava o GIF bloco a bloco, sem perder a animação: mantém só os quadros, o controle
/// de tempo e o laço (NETSCAPE2.0). Comentários, XMP e qualquer coisa depois do fim
/// (arquivos poliglotas) somem. Depois, decodifica tudo para garantir que abre.
pub fn sanitize_gif(bytes: &[u8]) -> ApiResult<Vec<u8>> {
    let bad = || ApiError::bad("GIF inválido ou corrompido.");
    if bytes.len() < 13 || !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return Err(bad());
    }
    let width = u16::from_le_bytes([bytes[6], bytes[7]]);
    let height = u16::from_le_bytes([bytes[8], bytes[9]]);
    if width == 0 || height == 0 || width > MAX_GIF_SIDE || height > MAX_GIF_SIDE {
        return Err(ApiError::bad(format!("O GIF pode ter no máximo {MAX_GIF_SIDE}x{MAX_GIF_SIDE} pixels.")));
    }
    let mut pos = 13 + color_table_len(bytes[10]);
    if pos > bytes.len() {
        return Err(bad());
    }
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&bytes[..pos]);
    let mut frames = 0usize;
    loop {
        match *bytes.get(pos).ok_or_else(bad)? {
            0x3B => {
                out.push(0x3B);
                break;
            }
            0x2C => {
                let desc = bytes.get(pos..pos + 10).ok_or_else(bad)?;
                let fw = u16::from_le_bytes([desc[5], desc[6]]);
                let fh = u16::from_le_bytes([desc[7], desc[8]]);
                if fw > MAX_GIF_SIDE || fh > MAX_GIF_SIDE {
                    return Err(bad());
                }
                let lzw = pos + 10 + color_table_len(desc[9]);
                if !(1..=11).contains(bytes.get(lzw).ok_or_else(bad)?) {
                    return Err(bad());
                }
                let end = skip_sub_blocks(bytes, lzw + 1).ok_or_else(bad)?;
                out.extend_from_slice(&bytes[pos..end]);
                pos = end;
                frames += 1;
                if frames > MAX_GIF_FRAMES {
                    return Err(ApiError::bad(format!("O GIF pode ter no máximo {MAX_GIF_FRAMES} quadros.")));
                }
            }
            0x21 => {
                let label = *bytes.get(pos + 1).ok_or_else(bad)?;
                let end = skip_sub_blocks(bytes, pos + 2).ok_or_else(bad)?;
                let keep = match label {
                    0xF9 => true,
                    0xFF => bytes.get(pos + 2) == Some(&11) && bytes.get(pos + 3..pos + 14) == Some(b"NETSCAPE2.0".as_slice()),
                    _ => false,
                };
                if keep {
                    out.extend_from_slice(&bytes[pos..end]);
                }
                pos = end;
            }
            _ => return Err(bad()),
        }
    }
    if frames == 0 {
        return Err(bad());
    }
    if frames as u64 * u64::from(width) * u64::from(height) > MAX_GIF_PIXELS {
        return Err(ApiError::bad("GIF longo demais para o tamanho dele. Diminua a resolução ou corte quadros."));
    }

    use image::codecs::gif::GifDecoder;
    use image::{AnimationDecoder, ImageDecoder, Limits};
    let mut decoder = GifDecoder::new(std::io::Cursor::new(&out)).map_err(|_| bad())?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(u32::from(MAX_GIF_SIDE));
    limits.max_image_height = Some(u32::from(MAX_GIF_SIDE));
    limits.max_alloc = Some(64 * 1024 * 1024);
    decoder.set_limits(limits).map_err(|_| bad())?;
    for frame in decoder.into_frames() {
        frame.map_err(|_| bad())?;
    }
    Ok(out)
}

// ------------------------------------------------------------------- rotas

#[derive(sqlx::FromRow)]
struct PageRow {
    id: Uuid,
    approval: String,
    pro_until: Option<DateTime<Utc>>,
    page: serde_json::Value,
    page_updated_at: Option<DateTime<Utc>>,
}

async fn load(state: &AppState, slug: &str) -> ApiResult<PageRow> {
    sqlx::query_as::<_, PageRow>("SELECT id, approval, pro_until, page, page_updated_at FROM servers WHERE slug = $1")
        .bind(slug)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)
}

#[derive(Serialize)]
pub struct Usage {
    pub uploads_24h: i64,
    pub bytes_24h: i64,
    pub max_uploads_24h: i64,
    pub max_bytes_24h: i64,
}

#[derive(Serialize)]
pub struct PageResponse {
    /// PRO ativo: a página aparece no launcher.
    pub active: bool,
    pub pro_until: Option<DateTime<Utc>>,
    /// Quem pode editar (admin ou dono do servidor).
    pub can_edit: bool,
    /// Para jogadores, vazia quando o PRO não está ativo. A equipe do servidor sempre vê a guardada.
    pub page: Page,
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

async fn usage(state: &AppState, server_id: Uuid) -> ApiResult<Usage> {
    let (count, bytes): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(bytes), 0)::BIGINT FROM server_page_media
          WHERE server_id = $1 AND created_at > now() - interval '24 hours'",
    )
    .bind(server_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Usage { uploads_24h: count, bytes_24h: bytes, max_uploads_24h: DAILY_UPLOADS, max_bytes_24h: DAILY_UPLOAD_BYTES })
}

async fn respond(state: &AppState, row: PageRow, user: Option<&User>) -> ApiResult<PageResponse> {
    let active = row.pro_until.is_some_and(|until| until > Utc::now());
    let can_edit = match user {
        Some(user) => require_role(&state.pool, row.id, user, Role::Admin).await.is_ok(),
        None => false,
    };
    let page = if active || can_edit { serde_json::from_value(row.page).unwrap_or_default() } else { Page::default() };
    Ok(PageResponse {
        active,
        pro_until: row.pro_until.filter(|until| *until > Utc::now()),
        can_edit,
        page,
        updated_at: row.page_updated_at,
        usage: if can_edit { Some(usage(state, row.id).await?) } else { None },
    })
}

pub async fn get(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<PageResponse>> {
    let row = load(&state, &slug).await?;
    if row.approval != "approved" {
        let user = user.as_ref().ok_or(ApiError::NotFound)?;
        require_role(&state.pool, row.id, user, Role::Moderator).await.map_err(|_| ApiError::NotFound)?;
    }
    Ok(Json(respond(&state, row, user.as_ref()).await?))
}

fn require_pro(row: &PageRow, user: &User) -> ApiResult<()> {
    if user.is_staff || row.pro_until.is_some_and(|until| until > Utc::now()) {
        Ok(())
    } else {
        Err(ApiError::bad("A página personalizada é do plano PRO. Ative o PRO na aba \"PRO e apoios\"."))
    }
}

pub async fn put(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Json(input): Json<Page>,
) -> ApiResult<Json<PageResponse>> {
    let row = load(&state, &slug).await?;
    require_role(&state.pool, row.id, &user, Role::Admin).await?;
    require_pro(&row, &user)?;
    let (page, images) = validate(&state.config.public_url, input).map_err(ApiError::bad)?;

    // Só imagens enviadas para este servidor (ou que já estavam na página).
    let unique: Vec<String> = images.into_iter().collect::<HashSet<_>>().into_iter().collect();
    let known: Vec<String> = sqlx::query_scalar(
        "SELECT url FROM server_page_media WHERE server_id = $1 AND url = ANY($2)
         UNION SELECT u FROM unnest($2::TEXT[]) u WHERE strpos($3::TEXT, u) > 0",
    )
    .bind(row.id)
    .bind(&unique)
    .bind(row.page.to_string())
    .fetch_all(&state.pool)
    .await?;
    if unique.iter().any(|u| !known.contains(u)) {
        return Err(ApiError::bad("Uma das imagens não foi enviada para este servidor. Envie de novo pelo painel."));
    }

    let value = serde_json::to_value(&page).map_err(|e| ApiError::Internal(e.into()))?;
    sqlx::query("UPDATE servers SET page = $2, page_updated_at = now() WHERE id = $1")
        .bind(row.id)
        .bind(&value)
        .execute(&state.pool)
        .await?;
    audit(
        &state.pool,
        Some(row.id),
        Some(user.id),
        "page.updated",
        json!({ "blocks": page.blocks.len(), "theme": page.theme.is_some() }),
    )
    .await?;
    let row = load(&state, &slug).await?;
    Ok(Json(respond(&state, row, Some(&user)).await?))
}

/// Envia uma imagem (PNG, JPEG, WebP) ou GIF animado para usar nos blocos.
pub async fn upload(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    body: Bytes,
) -> ApiResult<Json<serde_json::Value>> {
    let row = load(&state, &slug).await?;
    require_role(&state.pool, row.id, &user, Role::Admin).await?;
    require_pro(&row, &user)?;
    let gif = body.starts_with(b"GIF8");
    let max = if gif { MAX_GIF_BYTES } else { MAX_IMAGE_BYTES };
    if body.len() > max {
        return Err(ApiError::bad(format!(
            "{} grande demais (máximo {} MB).",
            if gif { "GIF" } else { "Imagem" },
            max / 1024 / 1024
        )));
    }
    let used = usage(&state, row.id).await?;
    if used.uploads_24h >= DAILY_UPLOADS || used.bytes_24h + body.len() as i64 > DAILY_UPLOAD_BYTES {
        return Err(ApiError::bad("Limite de envios da página nas últimas 24 horas. Tente de novo amanhã."));
    }

    let input = body.to_vec();
    let (clean, ext) = tokio::task::spawn_blocking(move || -> ApiResult<(Vec<u8>, &'static str)> {
        if gif { Ok((sanitize_gif(&input)?, "gif")) } else { images::sanitize(&input) }
    })
    .await
    .map_err(|e| ApiError::Internal(e.into()))??;
    let url = store_clean(&state, &clean, ext).await?;
    sqlx::query(
        "INSERT INTO server_page_media (id, server_id, url, kind, bytes, uploaded_by)
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (server_id, url) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(row.id)
    .bind(&url)
    .bind(if gif { "gif" } else { "image" })
    .bind(clean.len() as i64)
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    Ok(Json(json!({ "url": url, "kind": if gif { "gif" } else { "image" }, "bytes": clean.len() })))
}

#[cfg(test)]
mod tests {
    use super::*;

    const API: &str = "https://conexaosmps.com.br";

    fn img(n: u8) -> String {
        format!("{API}/images/{}.png", format!("{n:02x}").repeat(20))
    }

    #[test]
    fn youtube_links() {
        for link in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com/watch?feature=share&v=dQw4w9WgXcQ&t=10",
            "https://youtu.be/dQw4w9WgXcQ?si=abc",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ#t=3",
            "https://www.youtube.com/embed/dQw4w9WgXcQ",
            "https://www.youtube.com/live/dQw4w9WgXcQ?feature=shared",
            "dQw4w9WgXcQ",
        ] {
            assert_eq!(youtube_id(link).as_deref(), Some("dQw4w9WgXcQ"), "{link}");
        }
        for link in [
            "https://evil.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com.evil.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=curto",
            "https://www.youtube.com/@canal",
            "javascript:alert(1)",
            "dQw4w9WgXc\"",
        ] {
            assert_eq!(youtube_id(link), None, "{link}");
        }
    }

    #[test]
    fn own_images_only() {
        assert!(is_own_image(API, &img(1)));
        assert!(is_own_image(API, &format!("{API}/images/{}.gif", "ab".repeat(20))));
        assert!(!is_own_image(API, &format!("{API}/images/{}.svg", "ab".repeat(20))));
        assert!(!is_own_image(API, &format!("{API}/images/{}.png", "AB".repeat(20))));
        assert!(!is_own_image(API, &format!("{API}/images/../files/{}.png", "ab".repeat(20))));
        assert!(!is_own_image(API, "https://imgur.com/a.png"));
        assert!(!is_own_image(API, &format!("{API}.evil.com/images/{}.png", "ab".repeat(20))));
    }

    #[test]
    fn validates_page() {
        let input: Page = serde_json::from_value(json!({
            "theme": { "colors": ["#0A0F11", "#7a1fa2"], "angle": 135, "accent": "#FFCC00" },
            "blocks": [
                { "type": "text", "title": "  Nossa história ", "body": "Linha 1\r\nLinha 2" },
                { "type": "image", "url": img(1), "caption": "Spawn" },
                { "type": "gallery", "urls": [img(2), img(3)] },
                { "type": "video", "youtube": "https://youtu.be/dQw4w9WgXcQ" }
            ]
        }))
        .unwrap();
        let (page, images) = validate(API, input).unwrap();
        let theme = page.theme.unwrap();
        assert_eq!(theme.colors, vec!["#0a0f11", "#7a1fa2"]);
        assert_eq!(theme.style, "linear");
        assert_eq!(theme.accent.as_deref(), Some("#ffcc00"));
        assert_eq!(images.len(), 3);
        assert_eq!(page.blocks[0], Block::Text { title: "Nossa história".into(), body: "Linha 1\nLinha 2".into() });
        assert_eq!(page.blocks[3], Block::Video { youtube: "dQw4w9WgXcQ".into(), caption: String::new() });
    }

    #[test]
    fn rejects_bad_pages() {
        let bad = |value: serde_json::Value| validate(API, serde_json::from_value(value).unwrap()).is_err();
        assert!(bad(json!({ "theme": { "colors": ["#000000"] } })));
        assert!(bad(json!({ "theme": { "colors": ["#000000", "red"] } })));
        assert!(bad(json!({ "theme": { "colors": ["#000000", "#ffffff"], "angle": 400 } })));
        assert!(bad(json!({ "theme": { "colors": ["#000000", "#ffffff"], "style": "url(x)" } })));
        assert!(bad(json!({ "blocks": [{ "type": "text" }] })));
        assert!(bad(json!({ "blocks": [{ "type": "text", "body": "x".repeat(MAX_BODY + 1) }] })));
        assert!(bad(json!({ "blocks": [{ "type": "image", "url": "https://imgur.com/a.png" }] })));
        assert!(bad(json!({ "blocks": [{ "type": "gallery", "urls": [img(1)] }] })));
        assert!(bad(json!({ "blocks": [{ "type": "video", "youtube": "https://vimeo.com/123" }] })));
        let many: Vec<_> = (0..=MAX_BLOCKS).map(|_| json!({ "type": "text", "title": "a" })).collect();
        assert!(bad(json!({ "blocks": many })));
        assert!(serde_json::from_value::<Page>(json!({ "blocks": [{ "type": "html", "body": "<script>" }] })).is_err());
    }

    #[test]
    fn empty_page_is_empty_object() {
        assert_eq!(serde_json::to_value(Page::default()).unwrap(), json!({}));
    }

    fn sample_gif(frames: usize) -> Vec<u8> {
        use image::codecs::gif::{GifEncoder, Repeat};
        use image::{Delay, Frame, RgbaImage};
        let mut out = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut out);
            encoder.set_repeat(Repeat::Infinite).unwrap();
            for i in 0..frames {
                let img = RgbaImage::from_pixel(8, 8, image::Rgba([i as u8 * 40, 250, 0, 255]));
                encoder.encode_frame(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(100, 1))).unwrap();
            }
        }
        out
    }

    #[test]
    fn gif_keeps_animation_and_drops_extras() {
        let original = sample_gif(3);
        // Comentário no meio e lixo depois do fim (arquivo poliglota).
        let mut dirty = original[..original.len() - 1].to_vec();
        dirty.extend_from_slice(&[0x21, 0xFE, 9]);
        dirty.extend_from_slice(b"SEGREDO-1");
        dirty.extend_from_slice(&[0, 0x3B]);
        dirty.extend_from_slice(b"<html>poliglota</html>");
        let clean = sanitize_gif(&dirty).unwrap();
        assert!(!clean.windows(9).any(|w| w == b"SEGREDO-1"));
        assert!(!clean.windows(8).any(|w| w == b"poliglot"));
        assert!(clean.windows(11).any(|w| w == b"NETSCAPE2.0"));
        assert_eq!(*clean.last().unwrap(), 0x3B);
        use image::AnimationDecoder;
        let frames = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&clean)).unwrap().into_frames().count();
        assert_eq!(frames, 3);
    }

    #[test]
    fn gif_rejects_garbage() {
        assert!(sanitize_gif(b"GIF89a").is_err());
        assert!(sanitize_gif(b"not a gif at all").is_err());
        let mut truncated = sample_gif(2);
        truncated.truncate(truncated.len() / 2);
        assert!(sanitize_gif(&truncated).is_err());
        let mut huge = sample_gif(1);
        huge[6..8].copy_from_slice(&5000u16.to_le_bytes());
        assert!(sanitize_gif(&huge).is_err());
    }
}
