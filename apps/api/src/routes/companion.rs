//! Conexão Companion: o mod que o parceiro coloca no servidor.
//!
//! Vínculo sem digitar nada:
//! 1. o mod gera um segredo, chama `POST /companion/hello` e mostra no console
//!    o link `painel/link/K7Q2-9XMA`;
//! 2. o dono abre o link (logado), escolhe ou cria o servidor e confirma;
//! 3. o mod, que consulta `POST /companion/poll`, recebe o token do agente e
//!    passa a se conectar como agente (whitelist, fotografia de mods, uploads).

use std::collections::{HashMap, HashSet};

use axum::Json;
use axum::extract::{Path, State};
use conexao_pack::Group;
use conexao_types::agent::{ServerInfo, SnapshotFile, ToAgent};
use conexao_types::{LoaderSpec, PackManifest};
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::agents::{issue_agent_token, sha256_hex};
use crate::auth::{AuthUser, Role, audit, require_role};
use crate::error::{ApiError, ApiResult};
use crate::modrinth;
use crate::routes::plan::{self, PlanFile, PlanInput, PlanResponse};
use crate::routes::servers::server_id_by_slug;
use crate::state::AppState;

const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

fn new_code() -> String {
    let mut rng = rand::thread_rng();
    let mut part = || (0..4).map(|_| CODE_ALPHABET[rng.gen_range(0..CODE_ALPHABET.len())] as char).collect::<String>();
    format!("{}-{}", part(), part())
}

#[derive(Deserialize)]
pub struct HelloInput {
    /// Segredo aleatório gerado pelo mod (só o hash fica no banco).
    pub secret: String,
    pub info: ServerInfo,
}

#[derive(Serialize)]
pub struct HelloResponse {
    pub code: String,
    pub link_url: String,
    pub linked: bool,
}

fn check_secret(secret: &str) -> ApiResult<String> {
    let secret = secret.trim();
    if !(32..=128).contains(&secret.len()) || !secret.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return Err(ApiError::bad("Segredo do Companion inválido."));
    }
    Ok(sha256_hex(secret))
}

pub async fn hello(State(state): State<AppState>, Json(input): Json<HelloInput>) -> ApiResult<Json<HelloResponse>> {
    let secret_hash = check_secret(&input.secret)?;
    let info = serde_json::to_value(&input.info).map_err(|e| ApiError::Internal(e.into()))?;
    if info.to_string().len() > 8_000 {
        return Err(ApiError::bad("Dados do servidor grandes demais."));
    }
    // Vínculos esquecidos expiram em 24 h.
    sqlx::query("DELETE FROM companion_links WHERE server_id IS NULL AND created_at < now() - interval '24 hours'")
        .execute(&state.pool)
        .await?;
    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM companion_links WHERE server_id IS NULL")
        .fetch_one(&state.pool)
        .await?;
    let existing: Option<(String, Option<Uuid>)> =
        sqlx::query_as("SELECT code, server_id FROM companion_links WHERE secret_sha256 = $1")
            .bind(&secret_hash)
            .fetch_optional(&state.pool)
            .await?;
    let (code, linked) = match existing {
        Some((code, server_id)) => {
            sqlx::query("UPDATE companion_links SET info = $2 WHERE code = $1").bind(&code).bind(&info).execute(&state.pool).await?;
            (code, server_id.is_some())
        }
        None => {
            if pending > 10_000 {
                return Err(ApiError::bad("Muitos vínculos pendentes; tente de novo mais tarde."));
            }
            let code = new_code();
            sqlx::query("INSERT INTO companion_links (code, secret_sha256, info) VALUES ($1, $2, $3)")
                .bind(&code)
                .bind(&secret_hash)
                .bind(&info)
                .execute(&state.pool)
                .await?;
            (code, false)
        }
    };
    Ok(Json(HelloResponse { link_url: format!("{}/link/{code}", state.config.panel_url), code, linked }))
}

#[derive(Deserialize)]
pub struct PollInput {
    pub secret: String,
}

pub async fn poll(State(state): State<AppState>, Json(input): Json<PollInput>) -> ApiResult<Json<serde_json::Value>> {
    let secret_hash = check_secret(&input.secret)?;
    let row: Option<(String, Option<Uuid>, Option<String>)> =
        sqlx::query_as("SELECT code, server_id, agent_token FROM companion_links WHERE secret_sha256 = $1")
            .bind(&secret_hash)
            .fetch_optional(&state.pool)
            .await?;
    let Some((code, server_id, token)) = row else {
        return Err(ApiError::NotFound);
    };
    let Some(server_id) = server_id else {
        return Ok(Json(json!({ "status": "pending", "code": code })));
    };
    let slug: String =
        sqlx::query_scalar("SELECT slug FROM servers WHERE id = $1").bind(server_id).fetch_one(&state.pool).await?;
    // O token é entregue uma única vez.
    if token.is_some() {
        sqlx::query("UPDATE companion_links SET agent_token = NULL WHERE code = $1").bind(&code).execute(&state.pool).await?;
    }
    let ws = format!("{}/v1/agent/connect", state.config.public_url.replacen("http", "ws", 1));
    Ok(Json(json!({ "status": "linked", "slug": slug, "token": token, "ws_url": ws })))
}

#[derive(Serialize)]
pub struct LinkView {
    pub code: String,
    pub info: ServerInfo,
    pub created_at: DateTime<Utc>,
    pub linked_slug: Option<String>,
}

pub async fn get_link(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(code): Path<String>,
) -> ApiResult<Json<LinkView>> {
    let row: (String, serde_json::Value, DateTime<Utc>, Option<String>) = sqlx::query_as(
        "SELECT l.code, l.info, l.created_at, s.slug FROM companion_links l
           LEFT JOIN servers s ON s.id = l.server_id WHERE l.code = $1",
    )
    .bind(code.trim().to_uppercase())
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(Json(LinkView {
        code: row.0,
        info: serde_json::from_value(row.1).unwrap_or_default(),
        created_at: row.2,
        linked_slug: row.3,
    }))
}

#[derive(Deserialize)]
pub struct ConfirmInput {
    pub slug: String,
}

pub async fn confirm_link(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(code): Path<String>,
    Json(input): Json<ConfirmInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let code = code.trim().to_uppercase();
    let server_id = server_id_by_slug(&state.pool, &input.slug).await?;
    require_role(&state.pool, server_id, &user, Role::Owner).await?;
    let linked: Option<Option<Uuid>> = sqlx::query_scalar("SELECT server_id FROM companion_links WHERE code = $1")
        .bind(&code)
        .fetch_optional(&state.pool)
        .await?;
    match linked {
        None => return Err(ApiError::NotFound),
        Some(Some(existing)) if existing != server_id => {
            return Err(ApiError::bad("Este código já foi usado para outro servidor."));
        }
        _ => {}
    }
    let token = issue_agent_token(&state.pool, server_id).await?;
    sqlx::query("UPDATE companion_links SET server_id = $2, agent_token = $3, linked_at = now() WHERE code = $1")
        .bind(&code)
        .bind(server_id)
        .bind(&token)
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(server_id), Some(user.id), "companion.linked", json!({ "code": code })).await?;
    Ok(Json(json!({ "ok": true, "slug": input.slug })))
}

#[derive(Serialize)]
pub struct SnapshotMod {
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub title: Option<String>,
    pub icon_url: Option<String>,
    /// `modrinth` ou `unknown` (não está no Modrinth).
    pub source: &'static str,
    pub server_only: bool,
    pub client_only: bool,
    /// O pacote estável dos jogadores tem exatamente este arquivo?
    pub in_pack: bool,
}

#[derive(Serialize)]
pub struct SnapshotView {
    pub connected: bool,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub agent_version: Option<String>,
    pub reported_at: Option<DateTime<Utc>>,
    pub info: Option<ServerInfo>,
    pub mods: Vec<SnapshotMod>,
    /// Mods do pacote dos jogadores que o servidor não tem mais.
    pub only_in_pack: Vec<String>,
    /// O servidor mudou em relação ao pacote publicado?
    pub out_of_sync: bool,
}

async fn load_snapshot(state: &AppState, server_id: Uuid) -> ApiResult<Option<(ServerInfo, Vec<SnapshotFile>, DateTime<Utc>)>> {
    let row: Option<(serde_json::Value, serde_json::Value, DateTime<Utc>)> =
        sqlx::query_as("SELECT info, mods, reported_at FROM server_snapshots WHERE server_id = $1")
            .bind(server_id)
            .fetch_optional(&state.pool)
            .await?;
    Ok(row.map(|(info, mods, at)| {
        (serde_json::from_value(info).unwrap_or_default(), serde_json::from_value(mods).unwrap_or_default(), at)
    }))
}

async fn stable_manifest(state: &AppState, server_id: Uuid) -> ApiResult<Option<PackManifest>> {
    Ok(sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT pv.manifest FROM server_channels sc JOIN pack_versions pv ON pv.id = sc.version_id
          WHERE sc.server_id = $1 AND sc.channel = 'stable'",
    )
    .bind(server_id)
    .fetch_optional(&state.pool)
    .await?
    .and_then(|v| serde_json::from_value(v).ok()))
}

pub async fn snapshot(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<SnapshotView>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Moderator).await?;
    let agent: Option<(Option<DateTime<Utc>>, Option<String>)> =
        sqlx::query_as("SELECT last_seen_at, agent_version FROM agents WHERE server_id = $1")
            .bind(server_id)
            .fetch_optional(&state.pool)
            .await?;
    let (last_seen_at, agent_version) = agent.unwrap_or((None, None));
    let snapshot = load_snapshot(&state, server_id).await?;
    let pack = stable_manifest(&state, server_id).await?;
    let pack_mods: HashSet<String> = pack
        .as_ref()
        .map(|m| m.files.iter().filter(|f| f.path.starts_with("mods/")).map(|f| f.sha1.clone()).collect())
        .unwrap_or_default();
    let mut view = SnapshotView {
        connected: state.agents.is_connected(server_id),
        last_seen_at,
        agent_version,
        reported_at: None,
        info: None,
        mods: Vec::new(),
        only_in_pack: Vec::new(),
        out_of_sync: false,
    };
    if let Some((info, files, at)) = snapshot {
        let hashes: Vec<String> = files.iter().map(|f| f.sha1.clone()).collect();
        let resolved = modrinth::resolve(&state, &hashes).await?;
        let server_hashes: HashSet<&str> = files.iter().map(|f| f.sha1.as_str()).collect();
        for file in &files {
            let r = resolved.get(&file.sha1);
            let server_only = r.is_some_and(|r| r.server_only());
            view.mods.push(SnapshotMod {
                path: file.path.clone(),
                sha1: file.sha1.clone(),
                size: file.size,
                title: r.map(|r| r.title.clone()),
                icon_url: r.and_then(|r| r.icon_url.clone()),
                source: if r.is_some() { "modrinth" } else { "unknown" },
                server_only,
                client_only: r.is_some_and(|r| r.client_only()),
                in_pack: pack_mods.contains(&file.sha1),
            });
        }
        // Mods do pacote que não existem no servidor (exceto os só do cliente).
        if let Some(pack) = &pack {
            let pack_hashes: Vec<String> = pack.files.iter().map(|f| f.sha1.clone()).collect();
            let pack_resolved = modrinth::resolve(&state, &pack_hashes).await?;
            view.only_in_pack = pack
                .files
                .iter()
                .filter(|f| f.path.starts_with("mods/") && !server_hashes.contains(f.sha1.as_str()))
                .filter(|f| f.side != conexao_types::FileSide::Client && !pack_resolved.get(&f.sha1).is_some_and(|r| r.client_only()))
                .map(|f| f.path.clone())
                .collect();
        }
        view.out_of_sync = view.mods.iter().any(|m| !m.in_pack && !m.server_only) || !view.only_in_pack.is_empty();
        view.reported_at = Some(at);
        view.info = Some(info);
    }
    Ok(Json(view))
}

#[derive(Deserialize)]
pub struct FromServerInput {
    pub version: String,
    #[serde(default)]
    pub include_server_only: Vec<String>,
    #[serde(default)]
    pub excluded: Vec<String>,
}

/// Monta a próxima versão a partir dos mods do servidor: o que está no Modrinth
/// vira link; o resto o Companion envia (pedido automático, a resposta é 409
/// com `waiting` até os arquivos chegarem). Configs e outras pastas continuam
/// como na versão estável.
pub async fn plan_from_server(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Json(input): Json<FromServerInput>,
) -> ApiResult<Json<PlanResponse>> {
    let server_id = server_id_by_slug(&state.pool, &slug).await?;
    require_role(&state.pool, server_id, &user, Role::Admin).await?;
    let (info, files, _) = load_snapshot(&state, server_id)
        .await?
        .ok_or_else(|| ApiError::bad("O Companion ainda não enviou os mods deste servidor."))?;
    if files.is_empty() {
        return Err(ApiError::bad("O servidor não tem mods na pasta mods/."));
    }
    let hashes: Vec<String> = files.iter().map(|f| f.sha1.clone()).collect();
    let resolved = modrinth::resolve(&state, &hashes).await?;
    let excluded: HashSet<&str> = input.excluded.iter().map(String::as_str).collect();
    let needed: Vec<String> = files
        .iter()
        .filter(|f| !resolved.contains_key(&f.sha1) && !excluded.contains(f.path.as_str()))
        .map(|f| f.sha1.clone())
        .collect();
    let stored: Vec<String> =
        sqlx::query_scalar("UPDATE stored_files SET last_used_at = now() WHERE sha1 = ANY($1) RETURNING sha1")
        .bind(&needed)
        .fetch_all(&state.pool)
        .await?;
    let mut waiting: Vec<String> = needed.into_iter().filter(|h| !stored.contains(h)).collect();
    waiting.sort();
    waiting.dedup();
    if !waiting.is_empty() {
        let quota = crate::policy::quota_for(&state, server_id).await?;
        let sizes: HashMap<&str, u64> = files.iter().map(|f| (f.sha1.as_str(), f.size)).collect();
        let total: u64 = waiting.iter().map(|h| sizes.get(h.as_str()).copied().unwrap_or(0)).sum();
        if total > quota {
            return Err(ApiError::bad(format!(
                "Os mods que não estão no Modrinth somam {}, acima da cota de {}.",
                crate::policy::megabytes(total),
                crate::policy::megabytes(quota)
            )));
        }
        let requested = state.agents.send(server_id, ToAgent::UploadFiles { sha1: waiting.clone() });
        return Err(ApiError::Conflict {
            message: if requested {
                format!("Recebendo {} arquivo(s) do servidor…", waiting.len())
            } else {
                "O Companion está desconectado; ligue o servidor para enviar os mods.".into()
            },
            details: json!({ "waiting": waiting, "requested": requested }),
        });
    }
    let loader = match (info.loader, info.loader_version.clone()) {
        (Some(kind), Some(version)) => Some(LoaderSpec { kind, version }),
        _ => None,
    };
    // Mods só do cliente (minimapa, shaders) não existem no servidor: continuam no pacote.
    let server_paths: HashSet<String> = files.iter().map(|f| f.path.to_lowercase()).collect();
    let mut plan_files: Vec<PlanFile> = files
        .into_iter()
        .map(|f| PlanFile { path: f.path, sha1: f.sha1, url: None, sha512: None, size: Some(f.size) })
        .collect();
    if let Some(pack) = stable_manifest(&state, server_id).await? {
        for file in pack.files.iter().filter(|f| {
            f.path.starts_with("mods/") && f.side == conexao_types::FileSide::Client && !server_paths.contains(&f.path.to_lowercase())
        }) {
            plan_files.push(PlanFile {
                path: file.path.clone(),
                sha1: file.sha1.clone(),
                url: file.url.clone(),
                sha512: Some(file.sha512.clone()),
                size: Some(file.size),
            });
        }
    }
    let plan_input = PlanInput {
        version: input.version,
        minecraft: (!info.minecraft.is_empty()).then(|| info.minecraft.clone()),
        loader,
        files: plan_files,
        managed: Vec::new(),
        excluded: input.excluded,
        include_server_only: input.include_server_only,
    };
    Ok(Json(plan::build(&state, server_id, &slug, plan_input, Some(&[Group::Mods])).await?))
}
