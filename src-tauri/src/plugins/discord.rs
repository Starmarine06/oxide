//! Discord Rich Presence ("Listening to ...") over Discord's local IPC.

use discord_rich_presence::activity::{Activity, ActivityType, Assets, Button, StatusDisplayType, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use serde_json::{json, Value};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

use super::{Plugin, PluginOption};
use crate::player::{Change, SongInfo};

/// The application id Pear Desktop registered with Discord.
const DEFAULT_CLIENT_ID: &str = "1177081335727267940";

enum Msg {
    Set(SongInfo),
    Stop,
}

#[derive(Default)]
pub struct Discord {
    tx: Mutex<Option<Sender<Msg>>>,
}

impl Plugin for Discord {
    fn id(&self) -> &'static str {
        "discord"
    }
    fn name(&self) -> &'static str {
        "Discord Rich Presence"
    }
    fn description(&self) -> &'static str {
        "Show what you are listening to on Discord"
    }
    fn defaults(&self) -> Value {
        json!({ "enabled": true, "clientId": DEFAULT_CLIENT_ID, "hideWhenPaused": false })
    }
    fn options(&self) -> Vec<PluginOption> {
        vec![PluginOption::toggle("hideWhenPaused", "Hide status while paused")]
    }

    fn start(&self, _app: &AppHandle, cfg: &Value) {
        let mut slot = self.tx.lock().unwrap();
        if slot.is_some() {
            return;
        }
        let client_id = cfg
            .get("clientId")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_CLIENT_ID)
            .to_string();
        let hide_paused = cfg.get("hideWhenPaused").and_then(Value::as_bool).unwrap_or(false);
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("discord-rpc".into())
            .spawn(move || worker(client_id, hide_paused, rx))
            .expect("spawn discord thread");
        *slot = Some(tx);
    }

    fn stop(&self, _app: &AppHandle) {
        if let Some(tx) = self.tx.lock().unwrap().take() {
            let _ = tx.send(Msg::Stop);
        }
    }

    fn on_player_change(&self, _app: &AppHandle, song: &SongInfo, _change: Change, _cfg: &Value) {
        if let Some(tx) = self.tx.lock().unwrap().as_ref() {
            let _ = tx.send(Msg::Set(song.clone()));
        }
    }
}

fn worker(client_id: String, hide_paused: bool, rx: mpsc::Receiver<Msg>) {
    let mut client: Option<DiscordIpcClient> = None;
    let mut last_attempt: Option<Instant> = None;
    let mut song: Option<SongInfo> = None;
    let mut dirty = false;

    loop {
        match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(Msg::Set(s)) => {
                song = Some(s);
                dirty = true;
            }
            Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        // Only the newest state matters; Discord rate-limits updates.
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Set(s) => {
                    song = Some(s);
                    dirty = true;
                }
                Msg::Stop => {
                    shutdown(client.take());
                    return;
                }
            }
        }
        if !dirty {
            continue;
        }

        if client.is_none() {
            // Discord may not be running; retry at most every 15 seconds.
            if last_attempt.is_some_and(|t| t.elapsed() < Duration::from_secs(15)) {
                continue;
            }
            last_attempt = Some(Instant::now());
            let mut c = DiscordIpcClient::new(&client_id);
            match c.connect() {
                Ok(()) => {
                    log::info!("connected to Discord");
                    client = Some(c);
                }
                Err(e) => {
                    log::debug!("Discord not available: {e}");
                    continue;
                }
            }
        }

        let c = client.as_mut().unwrap();
        let result = match song.as_ref() {
            Some(s) if !(hide_paused && s.paused) => c.set_activity(activity(s)),
            _ => c.clear_activity(),
        }
        // Read Discord's reply so the pipe never fills up.
        .and_then(|_| c.recv().map(|_| ()));
        match result {
            Ok(()) => dirty = false,
            Err(e) => {
                log::warn!("Discord update failed, reconnecting later: {e}");
                shutdown(client.take());
                last_attempt = None;
            }
        }
    }
    shutdown(client);
}

fn shutdown(client: Option<DiscordIpcClient>) {
    if let Some(mut c) = client {
        let _ = c.clear_activity();
        let _ = c.close();
    }
}

/// Discord rejects fields shorter than 2 or longer than 128 characters.
fn field(s: &str) -> String {
    let mut out: String = s.chars().take(128).collect();
    while out.chars().count() < 2 {
        out.push('\u{2800}');
    }
    out
}

fn activity(song: &SongInfo) -> Activity<'static> {
    let mut assets = Assets::new().large_text(field(song.album.as_deref().unwrap_or(&song.title)));
    if let Some(art) = song.art_url.as_deref().filter(|u| u.starts_with("https://")) {
        assets = assets.large_image(art.to_string());
    }
    if song.paused {
        assets = assets.small_text("Paused");
    }

    let mut act = Activity::new()
        .activity_type(ActivityType::Listening)
        .status_display_type(StatusDisplayType::Details)
        .details(field(&song.title))
        .state(field(&song.artist))
        .assets(assets)
        .buttons(vec![Button::new("Listen on YouTube Music", song.url.clone())]);

    if !song.paused && song.duration > 0.0 {
        let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64;
        let start = now_ms - (song.elapsed * 1000.0) as i64;
        let end = start + (song.duration * 1000.0) as i64;
        act = act.timestamps(Timestamps::new().start(start).end(end));
    }
    act
}
