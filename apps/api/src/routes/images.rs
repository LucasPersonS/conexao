//! Ícone e banner do servidor enviados pelo painel (sem precisar de link externo).
//!
//! As imagens ficam em `{storage}/images/{sha1}.{ext}` e são servidas em
//! `/images/...` com o tipo correto. Só PNG, JPEG e WebP, com limite de tamanho.

use axum::Json;
use axum::extract::{Path, State};
use bytes::Bytes;
use serde_json::json;

use crate::auth::{AuthUser, Role, audit, require_role};
use crate::error::{ApiError, ApiResult};
use crate::routes::servers::server_id_by_slug;
use crate::state::AppState;

pub const MAX_ICON: usize = 1024 * 1024;
pub const MAX_BANNER: usize = 5 * 1024 * 1024;

/// Reconhece o formato pelos primeiros bytes (não confia no nome nem no Content-Type).
pub fn image_kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// Maior lado aceito, em pixels (protege contra "bombas" de descompressão).
const MAX_SIDE: u32 = 4096;

/// Decodifica e grava de novo a imagem: some todo metadado (EXIF com GPS, XMP,
/// comentários) e qualquer coisa escondida depois da imagem (arquivos poliglotas).
/// PNG continua PNG, JPEG continua JPEG; WebP vira PNG (com transparência) ou JPEG.
pub fn sanitize(bytes: &[u8]) -> ApiResult<(Vec<u8>, &'static str)> {
    use image::{ImageFormat, ImageReader, Limits};
    use std::io::Cursor;

    let kind = image_kind(bytes).ok_or_else(|| ApiError::bad("Envie uma imagem PNG, JPEG ou WebP."))?;
    let format = match kind {
        "png" => ImageFormat::Png,
        "jpg" => ImageFormat::Jpeg,
        _ => ImageFormat::WebP,
    };
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(128 * 1024 * 1024);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| ApiError::bad(format!("Imagem inválida ou maior que {MAX_SIDE}x{MAX_SIDE} pixels.")))?;

    let (out_format, ext) = match format {
        ImageFormat::Png => (ImageFormat::Png, "png"),
        ImageFormat::Jpeg => (ImageFormat::Jpeg, "jpg"),
        _ if image.color().has_alpha() => (ImageFormat::Png, "png"),
        _ => (ImageFormat::Jpeg, "jpg"),
    };
    let mut out = Cursor::new(Vec::with_capacity(bytes.len()));
    let result = if out_format == ImageFormat::Jpeg {
        let rgb = image::DynamicImage::ImageRgb8(image.to_rgb8());
        rgb.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90))
    } else {
        image.write_to(&mut out, ImageFormat::Png)
    };
    result.map_err(|e| ApiError::Internal(eyre::eyre!("falha ao regravar imagem: {e}")))?;
    Ok((out.into_inner(), ext))
}

/// Limpa a imagem (sem metadados), grava e devolve a URL pública.
pub async fn store_image(state: &AppState, bytes: &[u8]) -> ApiResult<String> {
    let input = bytes.to_vec();
    let (clean, ext) = tokio::task::spawn_blocking(move || sanitize(&input))
        .await
        .map_err(|e| ApiError::Internal(e.into()))??;
    let sha1 = sha1_smol::Sha1::from(&clean).digest().to_string();
    let dir = state.config.storage_dir.join("images");
    tokio::fs::create_dir_all(&dir).await?;
    let name = format!("{sha1}.{ext}");
    let target = dir.join(&name);
    if !tokio::fs::try_exists(&target).await? {
        let tmp = dir.join(format!(".{name}.{}", uuid::Uuid::new_v4()));
        tokio::fs::write(&tmp, &clean).await?;
        tokio::fs::rename(&tmp, &target).await?;
    }
    Ok(format!("{}/images/{name}", state.config.public_url))
}

pub async fn upload(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((slug, kind)): Path<(String, String)>,
    body: Bytes,
) -> ApiResult<Json<serde_json::Value>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Admin).await?;
    let (column, max) = match kind.as_str() {
        "icon" => ("icon_url", MAX_ICON),
        "banner" => ("banner_url", MAX_BANNER),
        _ => return Err(ApiError::NotFound),
    };
    if body.len() > max {
        return Err(ApiError::bad(format!("Imagem grande demais (máximo {} MB).", max / 1024 / 1024)));
    }
    let url = store_image(&state, &body).await?;
    sqlx::query(&format!("UPDATE servers SET {column} = $2 WHERE id = $1"))
        .bind(server_id)
        .bind(&url)
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(server_id), Some(user.id), "server.image_updated", json!({ "kind": kind })).await?;
    Ok(Json(json!({ "url": url })))
}

#[cfg(test)]
mod tests {
    use super::{image_kind, sanitize};

    /// JPEG 4x4 com um segmento EXIF (APP1) contendo um texto que precisa sumir.
    fn jpeg_with_exif() -> Vec<u8> {
        let img = image::RgbImage::from_pixel(4, 4, image::Rgb([180, 250, 0]));
        let mut plain = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img).write_to(&mut plain, image::ImageFormat::Jpeg).unwrap();
        let plain = plain.into_inner();
        let payload = b"Exif\0\0GPS-SEGREDO-123";
        let len = u16::try_from(payload.len() + 2).unwrap().to_be_bytes();
        let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1, len[0], len[1]];
        out.extend_from_slice(payload);
        out.extend_from_slice(&plain[2..]);
        out
    }

    #[test]
    fn strips_metadata() {
        let dirty = jpeg_with_exif();
        assert!(dirty.windows(15).any(|w| w == b"GPS-SEGREDO-123"));
        let (clean, ext) = sanitize(&dirty).unwrap();
        assert_eq!(ext, "jpg");
        assert!(!clean.windows(15).any(|w| w == b"GPS-SEGREDO-123"));
        assert_eq!(image_kind(&clean), Some("jpg"));
    }

    #[test]
    fn rejects_garbage_and_huge_images() {
        assert!(sanitize(b"not an image").is_err());
        let mut fake_png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        fake_png.extend_from_slice(b"lixo");
        assert!(sanitize(&fake_png).is_err());
        let big = image::RgbaImage::new(5000, 10);
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(big).write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        assert!(sanitize(&bytes.into_inner()).is_err());
    }

    #[test]
    fn detects_formats() {
        assert_eq!(image_kind(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]), Some("png"));
        assert_eq!(image_kind(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(image_kind(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(image_kind(b"<svg onload=alert(1)>"), None);
        assert_eq!(image_kind(b"GIF89a"), None);
    }
}
