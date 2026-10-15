//! Convites: `conexao://servidor/<slug>` abre o launcher direto na página do servidor.
//!
//! O link vem do painel (botão "Copiar convite") ou da página web `/convite/<slug>`.
//! Com o launcher fechado, o Windows abre o Conexão com a URL; com ele aberto, o
//! single-instance repassa a URL para a janela que já existe.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};

/// Convite recebido antes de o frontend estar pronto (abertura a frio).
#[derive(Default)]
pub struct PendingInvite(pub Mutex<Option<String>>);

/// Recusa slugs estranhos vindos do frontend antes de virar URL ou nome de arquivo.
pub fn ensure_slug(slug: &str) -> crate::error::CmdResult<()> {
    if valid_slug(slug) { Ok(()) } else { Err(crate::error::CmdError::new("Servidor inválido.")) }
}

pub fn valid_slug(slug: &str) -> bool {
    (2..=48).contains(&slug.len())
        && slug.bytes().next().is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && slug.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `conexao://servidor/paravexia`, `conexao://server/paravexia/`, `conexao://paravexia`.
pub fn parse_invite(url: &str) -> Option<String> {
    let rest = url.trim().strip_prefix("conexao://")?;
    let rest = rest.split(['?', '#']).next().unwrap_or_default().trim_matches('/');
    let slug = match rest.split_once('/') {
        Some(("servidor" | "server" | "s", slug)) => slug.trim_matches('/'),
        Some(_) => return None,
        None => rest,
    };
    let slug = slug.to_lowercase();
    valid_slug(&slug).then_some(slug)
}

/// Guarda o convite e avisa a janela (se já estiver aberta).
pub fn handle(app: &AppHandle, url: &str) {
    let Some(slug) = parse_invite(url) else {
        tracing::warn!("link conexao:// inválido: {url}");
        return;
    };
    if let Some(pending) = app.try_state::<PendingInvite>()
        && let Ok(mut slot) = pending.0.lock()
    {
        *slot = Some(slug.clone());
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    let _ = app.emit("invite", slug);
}

/// O frontend pergunta ao iniciar se o launcher foi aberto por um convite.
#[tauri::command]
pub fn take_invite(pending: tauri::State<'_, PendingInvite>) -> Option<String> {
    pending.0.lock().ok().and_then(|mut slot| slot.take())
}

#[cfg(test)]
mod tests {
    use super::parse_invite;

    #[test]
    fn parses_invites() {
        assert_eq!(parse_invite("conexao://servidor/paravexia").as_deref(), Some("paravexia"));
        assert_eq!(parse_invite("conexao://server/cidade-alta/").as_deref(), Some("cidade-alta"));
        assert_eq!(parse_invite("conexao://paravexia?ref=discord").as_deref(), Some("paravexia"));
        assert_eq!(parse_invite("conexao://Paravexia").as_deref(), Some("paravexia"));
        assert_eq!(parse_invite("conexao://servidor/../../etc"), None);
        assert_eq!(parse_invite("https://evil.com/x"), None);
        assert_eq!(parse_invite("conexao://outra/coisa"), None);
    }
}
