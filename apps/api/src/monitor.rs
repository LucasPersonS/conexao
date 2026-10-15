//! Monitoramento: espaço em disco, idade do último backup e avisos no Discord.
//!
//! - `/health` responde 503 quando o disco passa de `CONEXAO_DISK_ALERT_PERCENT` (90%):
//!   o mesmo monitor do UptimeRobot avisa de "fora do ar" e de "disco cheio".
//! - Com `CONEXAO_ALERT_WEBHOOK` (webhook do Discord), a API avisa no canal da equipe quando
//!   o disco passa de `CONEXAO_DISK_WARN_PERCENT` (85%) e quando o backup do banco atrasa.
//!   Repete o aviso a cada 6 horas enquanto o problema continuar e avisa quando normaliza.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde_json::json;

use crate::state::AppState;

const CHECK_EVERY: Duration = Duration::from_secs(600);
const REPEAT_EVERY: Duration = Duration::from_secs(6 * 3600);
/// Backup diário: depois de 26 h sem dump novo, algo deu errado.
const BACKUP_MAX_AGE: Duration = Duration::from_secs(26 * 3600);

/// Porcentagem do disco em uso onde fica `path` (None se não der para medir).
pub fn disk_used_percent(path: &Path) -> Option<f64> {
    let total = fs4::total_space(path).ok()?;
    let available = fs4::available_space(path).ok()?;
    if total == 0 {
        return None;
    }
    Some(100.0 * (total.saturating_sub(available)) as f64 / total as f64)
}

/// Idade do dump mais novo na pasta de backups (None = nenhum dump).
pub fn newest_backup_age(dir: &Path) -> Option<Duration> {
    let newest = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".dump"))
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()?;
    SystemTime::now().duration_since(newest).ok()
}

/// Um tipo de alerta com memória de quando foi enviado (para não repetir a cada 10 min).
#[derive(Default)]
struct Alarm {
    active: bool,
    last_sent: Option<std::time::Instant>,
}

impl Alarm {
    /// O que mandar agora: `Some(true)` = avisar o problema, `Some(false)` = avisar que normalizou.
    fn update(&mut self, problem: bool) -> Option<bool> {
        let due = self.last_sent.is_none_or(|t| t.elapsed() >= REPEAT_EVERY);
        let send = match (problem, self.active) {
            (true, false) => Some(true),
            (true, true) if due => Some(true),
            (false, true) => Some(false),
            _ => None,
        };
        self.active = problem;
        if send.is_some() {
            self.last_sent = Some(std::time::Instant::now());
        }
        send
    }
}

async fn notify(state: &AppState, message: &str) {
    let Some(url) = &state.config.alert_webhook else {
        tracing::warn!("{message}");
        return;
    };
    let body = json!({ "username": "Conexão · monitor", "content": message });
    match state.http.post(url).json(&body).send().await {
        Ok(response) if response.status().is_success() => {}
        Ok(response) => tracing::warn!("webhook de alerta respondeu {}", response.status()),
        Err(error) => tracing::warn!("falha ao enviar alerta: {error}"),
    }
    tracing::warn!("{message}");
}

pub async fn run(state: AppState) {
    let mut disk = Alarm::default();
    let mut backup = Alarm::default();
    let mut interval = tokio::time::interval(CHECK_EVERY);
    loop {
        interval.tick().await;
        if let Some(used) = disk_used_percent(&state.config.storage_dir) {
            let warn = state.config.disk_warn_percent;
            match disk.update(used >= warn) {
                Some(true) => {
                    notify(&state, &format!(
                        "⚠️ Disco da VPS com {used:.0}% em uso (limite de aviso: {warn:.0}%). Libere espaço ou aumente o disco antes que uploads e backups falhem."
                    ))
                    .await;
                }
                Some(false) => notify(&state, &format!("✅ Disco da VPS voltou ao normal ({used:.0}% em uso).")).await,
                None => {}
            }
        }
        if let Some(dir) = &state.config.backup_dir {
            let age = newest_backup_age(dir);
            let late = age.is_none_or(|a| a > BACKUP_MAX_AGE);
            match backup.update(late) {
                Some(true) => {
                    let since = age.map_or("nenhum backup encontrado".to_string(), |a| {
                        format!("último backup há {} h", a.as_secs() / 3600)
                    });
                    notify(&state, &format!("⚠️ Backup do banco atrasado ({since}). Veja `docker compose logs backup`.")).await;
                }
                Some(false) => notify(&state, "✅ Backup do banco voltou a rodar.").await,
                None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_disk() {
        let used = disk_used_percent(Path::new(".")).expect("disco mensurável");
        assert!((0.0..=100.0).contains(&used));
    }

    #[test]
    fn alarm_sends_once_then_recovery() {
        let mut alarm = Alarm::default();
        assert_eq!(alarm.update(false), None);
        assert_eq!(alarm.update(true), Some(true));
        assert_eq!(alarm.update(true), None); // não repete antes de 6 h
        assert_eq!(alarm.update(false), Some(false));
        assert_eq!(alarm.update(false), None);
    }

    #[test]
    fn finds_newest_dump() {
        let dir = std::env::temp_dir().join(format!("conexao-backup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(newest_backup_age(&dir).is_none());
        std::fs::write(dir.join("conexao-20261014-0600.dump"), b"x").unwrap();
        std::fs::write(dir.join("anotacao.txt"), b"x").unwrap();
        assert!(newest_backup_age(&dir).is_some_and(|a| a < Duration::from_secs(60)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
