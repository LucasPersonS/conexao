//! Presença no Discord: "Jogando no <servidor>" com o ícone do servidor e o botão
//! "Jogar também", que leva ao convite (`/convite/<slug>`) e abre o Conexão.
//!
//! Usa o app do Conexão no Discord (`CONEXAO_DISCORD_APP_ID`, definido no build).
//! Sem ele, a presença fica desligada. A conexão com o Discord é feita numa thread
//! própria, só enquanto há jogo aberto: com o launcher parado, nada roda.

use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use discord_rich_presence::activity::{Activity, Assets, Button, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

const APP_ID: Option<&str> = option_env!("CONEXAO_DISCORD_APP_ID");

pub fn app_id() -> Option<String> {
    APP_ID.map(str::to_string).or_else(|| std::env::var("CONEXAO_DISCORD_APP_ID").ok()).filter(|id| !id.is_empty())
}

pub fn available() -> bool {
    app_id().is_some()
}

#[derive(Debug, Clone)]
pub struct Playing {
    pub server_name: String,
    pub icon_url: Option<String>,
    pub invite_url: Option<String>,
    pub started_at: i64,
}

enum Command {
    Set(Playing),
    Clear,
}

#[derive(Default)]
pub struct Presence {
    sender: Mutex<Option<Sender<Command>>>,
}

impl Presence {
    fn send(&self, command: Command) {
        let Some(id) = app_id() else { return };
        let Ok(mut guard) = self.sender.lock() else { return };
        if guard.is_none() {
            if matches!(command, Command::Clear) {
                return; // nada aberto, nada a limpar
            }
            let (tx, rx) = mpsc::channel::<Command>();
            std::thread::Builder::new()
                .name("conexao-discord".into())
                .spawn(move || run(&id, &rx))
                .ok();
            *guard = Some(tx);
        }
        let failed = guard.as_ref().is_some_and(|tx| tx.send(command).is_err());
        if failed {
            *guard = None;
        }
    }

    pub fn playing(&self, playing: Playing) {
        self.send(Command::Set(playing));
    }

    pub fn clear(&self) {
        self.send(Command::Clear);
    }
}

/// Thread do Discord: conecta sob demanda, tenta de novo a cada 30 s se o Discord
/// estiver fechado e termina quando a presença é limpa.
fn run(app_id: &str, rx: &mpsc::Receiver<Command>) {
    let mut client = DiscordIpcClient::new(app_id);
    let mut connected = false;
    let mut current: Option<Playing> = None;
    loop {
        let command = if current.is_some() && !connected {
            match rx.recv_timeout(Duration::from_secs(30)) {
                Ok(command) => Some(command),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(command) => Some(command),
                Err(_) => break,
            }
        };
        match command {
            Some(Command::Set(playing)) => current = Some(playing),
            Some(Command::Clear) => {
                if connected {
                    let _ = client.clear_activity();
                    let _ = client.close();
                }
                break;
            }
            None => {}
        }
        let Some(playing) = &current else { continue };
        if !connected {
            connected = client.connect().is_ok();
        }
        if connected && client.set_activity(activity(playing)).is_err() {
            connected = client.reconnect().is_ok() && client.set_activity(activity(playing)).is_ok();
        }
    }
}

fn activity(playing: &Playing) -> Activity<'_> {
    let image = playing.icon_url.as_deref().filter(|u| u.starts_with("https://")).unwrap_or("conexao");
    let mut activity = Activity::new()
        .details(&playing.server_name)
        .state("Roleplay no Conexão")
        .timestamps(Timestamps::new().start(playing.started_at))
        .assets(
            Assets::new()
                .large_image(image)
                .large_text(&playing.server_name)
                .small_image("conexao")
                .small_text("Conexão"),
        );
    if let Some(url) = playing.invite_url.as_deref().filter(|u| u.starts_with("https://")) {
        activity = activity.buttons(vec![Button::new("Jogar também", url)]);
    }
    activity
}
