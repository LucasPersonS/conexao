//! Avisos do sistema: vaga aberta num servidor cheio e lembretes de eventos.
//! Tudo roda no Rust (não depende da janela estar visível) e só existe enquanto
//! há algo para vigiar.

use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use serde::Serialize;
use tauri::{Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::task::AbortHandle;

use crate::AppContext;
use crate::error::{CmdError, CmdResult};
use crate::prefs::{self, Reminder};

/// Intervalo do ping enquanto espera vaga (um único servidor, ~1 KB por ping).
const SLOT_INTERVAL: Duration = Duration::from_secs(20);
/// Desiste de esperar depois de 3 horas.
const SLOT_GIVE_UP: Duration = Duration::from_secs(3 * 60 * 60);
/// Avisa 10 minutos antes do evento.
const REMIND_BEFORE: chrono::TimeDelta = chrono::TimeDelta::minutes(10);

#[derive(Default)]
pub struct Watchers {
    slots: DashMap<String, AbortHandle>,
    reminders: DashMap<String, AbortHandle>,
}

pub async fn show(app: &tauri::AppHandle, title: &str, body: &str) {
    let enabled = match app.path().app_data_dir() {
        Ok(dir) => prefs::load(&dir).await.notifications,
        Err(_) => true,
    };
    if enabled {
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

#[derive(Clone, Serialize)]
struct SlotEvent {
    slug: String,
    open: bool,
}

#[tauri::command]
pub fn watch_slot(
    app: tauri::AppHandle,
    ctx: tauri::State<'_, Arc<AppContext>>,
    slug: String,
    name: String,
    address: String,
) -> CmdResult<()> {
    if address.trim().is_empty() {
        return Err(CmdError::new("endereço inválido"));
    }
    let key = slug.clone();
    let ctx_inner = Arc::clone(&ctx);
    let task = tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();
        while started.elapsed() < SLOT_GIVE_UP {
            tokio::time::sleep(SLOT_INTERVAL).await;
            let Ok(status) = theseus::worlds::get_server_status(&address, None).await else { continue };
            if status.players.is_some_and(|p| p.online < p.max) {
                show(&app, "Abriu vaga!", &format!("Tem lugar no {name} agora. Corre que dá tempo.")).await;
                let _ = app.emit("slot", SlotEvent { slug: slug.clone(), open: true });
                break;
            }
        }
        ctx_inner.watchers.slots.remove(&slug);
        let _ = app.emit("slot", SlotEvent { slug, open: false });
    });
    if let Some((_, old)) = ctx.watchers.slots.remove(&key) {
        old.abort();
    }
    ctx.watchers.slots.insert(key, task.inner().abort_handle());
    Ok(())
}

#[tauri::command]
pub fn unwatch_slot(ctx: tauri::State<'_, Arc<AppContext>>, slug: String) {
    if let Some((_, task)) = ctx.watchers.slots.remove(&slug) {
        task.abort();
    }
}

#[tauri::command]
pub fn watched_slots(ctx: tauri::State<'_, Arc<AppContext>>) -> Vec<String> {
    ctx.watchers.slots.iter().map(|e| e.key().clone()).collect()
}

fn arm(app: &tauri::AppHandle, ctx: &Arc<AppContext>, reminder: Reminder) {
    let at = reminder.starts_at - REMIND_BEFORE;
    let wait = (at - chrono::Utc::now()).to_std().unwrap_or_default();
    if reminder.starts_at < chrono::Utc::now() {
        return;
    }
    let app = app.clone();
    let id = reminder.id.clone();
    let task = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(wait).await;
        let minutes = (reminder.starts_at - chrono::Utc::now()).num_minutes().max(0);
        let when = if minutes <= 1 { "agora".to_string() } else { format!("em {minutes} minutos") };
        show(&app, &format!("{} começa {when}", reminder.title), &format!("Evento no {}.", reminder.server_name)).await;
    });
    if let Some(old) = ctx.watchers.reminders.insert(id, task.inner().abort_handle()) {
        old.abort();
    }
}

/// Reagenda os lembretes salvos quando o launcher abre (e descarta os vencidos).
pub async fn restore(app: tauri::AppHandle, ctx: Arc<AppContext>) {
    let Ok(dir) = app.path().app_data_dir() else { return };
    let mut saved = prefs::load(&dir).await;
    let before = saved.reminders.len();
    saved.reminders.retain(|r| r.starts_at > chrono::Utc::now());
    if saved.reminders.len() != before {
        let _ = prefs::save(&dir, &saved).await;
    }
    for reminder in saved.reminders {
        arm(&app, &ctx, reminder);
    }
}

#[tauri::command]
pub async fn set_reminder(
    app: tauri::AppHandle,
    ctx: tauri::State<'_, Arc<AppContext>>,
    reminder: Reminder,
    enabled: bool,
) -> CmdResult<Vec<String>> {
    let dir = app.path().app_data_dir()?;
    let mut saved = prefs::load(&dir).await;
    saved.reminders.retain(|r| r.id != reminder.id && r.starts_at > chrono::Utc::now());
    if let Some((_, task)) = ctx.watchers.reminders.remove(&reminder.id) {
        task.abort();
    }
    if enabled {
        saved.reminders.push(reminder.clone());
        arm(&app, &ctx, reminder);
    }
    prefs::save(&dir, &saved).await?;
    Ok(saved.reminders.into_iter().map(|r| r.id).collect())
}

#[tauri::command]
pub async fn reminders(app: tauri::AppHandle) -> CmdResult<Vec<String>> {
    let dir = app.path().app_data_dir()?;
    Ok(prefs::load(&dir).await.reminders.into_iter().filter(|r| r.starts_at > chrono::Utc::now()).map(|r| r.id).collect())
}
