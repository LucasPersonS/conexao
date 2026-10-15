//! Página pública de convite (`/convite/<slug>`) e download do launcher (`/baixar`).
//!
//! O parceiro cola o link no Discord: aparece um card com banner e nome (Open Graph);
//! quem clica vê o servidor e o botão "Abrir no Conexão" (`conexao://servidor/<slug>`),
//! ou baixa o launcher se ainda não tiver.

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};

use crate::state::AppState;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Só URLs http(s) entram em atributos da página.
fn safe_url(url: Option<String>) -> Option<String> {
    url.filter(|u| u.starts_with("https://") || u.starts_with("http://")).map(|u| escape(&u))
}

type InviteRow = (String, String, String, Option<String>, Option<String>, Vec<String>, bool, i32, i32, Option<String>);

pub async fn page(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    let row: Option<InviteRow> = sqlx::query_as(
        "SELECT slug, name, description, icon_url, banner_url, tags, status_online, status_players,
                status_max_players, discord_url
           FROM servers WHERE slug = $1 AND approval = 'approved'",
    )
    .bind(slug.to_lowercase())
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    let Some((slug, name, description, icon, banner, tags, online, players, max, discord)) = row else {
        return (StatusCode::NOT_FOUND, Html(shell("Servidor não encontrado", "", &not_found()))).into_response();
    };
    let base = &state.config.public_url;
    let summary: String = description.lines().next().unwrap_or_default().chars().take(180).collect();
    let status = if online { format!("{players}/{max} jogando agora") } else { "Offline no momento".into() };
    let og_image = safe_url(banner.clone()).or_else(|| safe_url(icon.clone())).unwrap_or_default();
    let meta = format!(
        r##"<meta property="og:type" content="website">
<meta property="og:site_name" content="Conexão">
<meta property="og:title" content="{title} — jogue pelo Conexão">
<meta property="og:description" content="{desc}">
<meta property="og:image" content="{og_image}">
<meta property="og:url" content="{base}/convite/{slug}">
<meta name="twitter:card" content="summary_large_image">
<meta name="theme-color" content="#b7fe00">"##,
        title = escape(&name),
        desc = escape(&format!("{status} · {summary}")),
    );
    // Aspas e parênteses codificados: a URL não consegue sair do `url('…')` do CSS.
    let banner_style = safe_url(banner.map(|u| u.replace('\'', "%27").replace('(', "%28").replace(')', "%29")))
        .map(|u| format!(r#" style="background-image:url('{u}')""#))
        .unwrap_or_default();
    let icon_html = match safe_url(icon) {
        Some(u) => format!(r#"<img class="icon" src="{u}" alt="">"#),
        None => format!(r#"<div class="icon letter">{}</div>"#, escape(&name.chars().next().unwrap_or('?').to_uppercase().to_string())),
    };
    let tags_html: String = tags.iter().take(5).map(|t| format!(r#"<span class="tag">{}</span>"#, escape(t))).collect();
    let discord_html = safe_url(discord)
        .map(|u| format!(r#"<a class="btn ghost" href="{u}" rel="noopener">Discord do servidor</a>"#))
        .unwrap_or_default();
    let body = format!(
        r#"<main class="card">
  <div class="banner"{banner_style}><div class="shade"></div></div>
  <div class="content">
    {icon_html}
    <h1>{name}</h1>
    <p class="status"><span class="dot{dot}"></span>{status}</p>
    <div class="tags">{tags_html}</div>
    <p class="desc">{desc}</p>
    <div class="actions">
      <a class="btn primary" id="open" href="conexao://servidor/{slug}">Abrir no Conexão</a>
      {discord_html}
    </div>
    <p class="hint" id="hint">Ainda não tem o Conexão? <a href="{base}/baixar">Baixe o launcher</a> — ele instala o modpack e entra no servidor com um clique.</p>
  </div>
</main>
<script>
document.getElementById('open').addEventListener('click', function () {{
  setTimeout(function () {{
    if (!document.hidden) document.getElementById('hint').classList.add('show');
  }}, 1500);
}});
</script>"#,
        name = escape(&name),
        dot = if online { " on" } else { "" },
        desc = escape(&description),
    );
    let title = escape(&name);
    let mut response = Html(shell(&format!("{title} — Conexão"), &meta, &body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, header::HeaderValue::from_static("public, max-age=60"));
    response
}

fn not_found() -> String {
    r#"<main class="card"><div class="content"><h1>Servidor não encontrado</h1>
<p class="desc">O convite pode estar errado, ou o servidor ainda está em análise pela equipe Conexão.</p></div></main>"#
        .into()
}

fn shell(title: &str, meta: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="pt-BR">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
{meta}
<style>
:root {{ --lime:#b7fe00; --lime-hi:#cbff4a; --bg:#0a0f11; --card:#131c1f; --line:#213034; --text:#f2f6ef; --dim:#a3b1ad; }}
* {{ box-sizing:border-box; }}
body {{ margin:0; min-height:100vh; display:grid; place-items:center; padding:24px 16px;
  font-family:Inter,'Segoe UI',system-ui,sans-serif; color:var(--text); background-color:var(--bg);
  background-image:radial-gradient(900px 420px at 50% -120px,rgba(183,254,0,.14),transparent 70%),
    linear-gradient(rgba(183,254,0,.035) 1px,transparent 1px),linear-gradient(90deg,rgba(183,254,0,.035) 1px,transparent 1px);
  background-size:auto,44px 44px,44px 44px; }}
.card {{ width:100%; max-width:560px; background:var(--card); border:1px solid var(--line); border-radius:20px;
  overflow:hidden; box-shadow:0 20px 50px rgba(0,0,0,.5),0 0 60px rgba(183,254,0,.06); }}
.banner {{ position:relative; height:180px; background-color:#0b1113;
  background-image:radial-gradient(circle at 80% 15%,rgba(183,254,0,.3),transparent 55%),linear-gradient(135deg,#15241a,#0b1113);
  background-size:cover; background-position:center; }}
.shade {{ position:absolute; inset:0; background:linear-gradient(to top,rgba(19,28,31,.95),transparent 65%); }}
.content {{ position:relative; padding:0 28px 28px; }}
.icon {{ width:88px; height:88px; border-radius:20px; border:3px solid var(--lime); margin-top:-44px; object-fit:cover;
  background:var(--card); box-shadow:0 6px 18px rgba(0,0,0,.4),0 0 24px rgba(183,254,0,.3); }}
.letter {{ display:grid; place-items:center; font-size:38px; font-weight:800; color:var(--lime);
  background:linear-gradient(135deg,#1c2a1a,#0e1518); }}
h1 {{ margin:12px 0 4px; font-size:28px; }}
.status {{ margin:0; color:var(--dim); font-size:14px; display:flex; align-items:center; gap:8px; }}
.dot {{ width:9px; height:9px; border-radius:50%; background:#6a7a76; }}
.dot.on {{ background:var(--lime); box-shadow:0 0 0 3px rgba(183,254,0,.16),0 0 10px rgba(183,254,0,.6); }}
.tags {{ display:flex; flex-wrap:wrap; gap:6px; margin:12px 0; }}
.tag {{ padding:3px 10px; border-radius:999px; font-size:12px; background:#0e1518; border:1px solid var(--line); color:var(--dim); }}
.desc {{ color:var(--dim); line-height:1.55; white-space:pre-line; margin:0 0 18px; }}
.actions {{ display:flex; flex-wrap:wrap; gap:10px; }}
.btn {{ display:inline-flex; align-items:center; height:46px; padding:0 22px; border-radius:12px;
  font-weight:700; text-decoration:none; }}
.primary {{ background:var(--lime); color:var(--bg); box-shadow:0 0 0 1px rgba(183,254,0,.5),0 8px 28px rgba(183,254,0,.28); }}
.primary:hover {{ background:var(--lime-hi); }}
.ghost {{ background:#5865f2; color:#fff; }}
.hint {{ margin:16px 0 0; font-size:14px; color:var(--dim); }}
.hint.show {{ color:var(--text); font-weight:600; }}
.hint a {{ color:var(--lime); }}
</style>
</head>
<body>
{body}
</body>
</html>"#
    )
}

/// Instalador visual (apps/installer): baixa a versão mais nova sozinho.
pub const VISUAL_INSTALLER: &str = "Conexao-Instalador.exe";

#[derive(serde::Deserialize)]
pub struct DownloadQuery {
    /// `?classico` = instalador NSIS direto, sem o instalador visual.
    classico: Option<String>,
}

/// Redireciona para o instalador: o visual, se publicado; senão (ou com `?classico`) o NSIS
/// mais recente do `latest.json` publicado pelo CI.
pub async fn download(State(state): State<AppState>, Query(query): Query<DownloadQuery>) -> Response {
    if let Some(url) = &state.config.download_url {
        return Redirect::temporary(url).into_response();
    }
    let dir = state.config.storage_dir.join("launcher");
    if query.classico.is_none() && tokio::fs::try_exists(dir.join(VISUAL_INSTALLER)).await.unwrap_or(false) {
        return Redirect::temporary(&format!("{}/launcher/{VISUAL_INSTALLER}", state.config.public_url)).into_response();
    }
    let latest = dir.join("latest.json");
    let url = tokio::fs::read(&latest)
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|json| {
            ["windows-x86_64-nsis", "windows-x86_64"]
                .iter()
                .find_map(|key| json.pointer(&format!("/platforms/{key}/url")).and_then(|u| u.as_str()).map(str::to_string))
        });
    match url {
        Some(url) if url.starts_with("https://") || url.starts_with("http://") => Redirect::temporary(&url).into_response(),
        _ => (
            StatusCode::NOT_FOUND,
            Html(shell(
                "Conexão — em breve",
                "",
                r#"<main class="card"><div class="content"><h1>O Conexão chega em breve</h1>
<p class="desc">O instalador ainda não foi publicado. Fique de olho no Discord do seu servidor.</p></div></main>"#,
            )),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_html() {
        assert_eq!(escape(r#"<script>"x"</script>"#), "&lt;script&gt;&quot;x&quot;&lt;/script&gt;");
        assert_eq!(safe_url(Some("javascript:alert(1)".into())), None);
        assert_eq!(safe_url(Some("https://a.b/c'd".into())).as_deref(), Some("https://a.b/c&#39;d"));
    }
}
