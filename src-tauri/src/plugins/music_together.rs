//! Music Together: listen in sync with friends. The host's player state is
//! published to a private, randomly named topic on ntfy.sh; guests follow
//! it (same song, same position, play/pause together).
//!
//! Only the song id, position and play state are shared, nothing else.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use super::{MenuAction, Plugin};
use crate::player::{Change, PlayerState, SongInfo};
use crate::window;

const RELAY: &str = "https://ntfy.sh/";

enum Role {
    Host { topic: String, last_sent: Option<Instant> },
    Guest {
        #[allow(dead_code)]
        topic: String,
        run: Arc<AtomicBool>,
    },
}

#[derive(Default)]
pub struct MusicTogether(Mutex<Option<Role>>);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn new_code() -> String {
    // 12 characters from a v4 uuid: hard to guess, easy to read out.
    let raw = uuid::Uuid::new_v4().simple().to_string();
    format!("oxide-{}", &raw[..12])
}

fn topic_from_code(code: &str) -> Option<String> {
    let code = code.trim();
    let ok = code.starts_with("oxide-") && code.len() <= 40 && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    ok.then(|| code.to_string())
}

fn publish(topic: &str, song: &SongInfo) {
    let payload = json!({ "v": song.video_id, "t": song.elapsed, "p": song.paused, "at": now_ms() }).to_string();
    let url = format!("{RELAY}{topic}");
    std::thread::spawn(move || {
        let c = reqwest::blocking::Client::builder().timeout(Duration::from_secs(5)).build().unwrap();
        if let Err(e) = c.post(url).body(payload).send() {
            log::warn!("music together publish: {e}");
        }
    });
}

fn follow(app: AppHandle, topic: String, run: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        // Start with the latest state, then stream new messages.
        let mut since = "10m".to_string();
        while run.load(Ordering::SeqCst) {
            let url = format!("{RELAY}{topic}/json?since={since}");
            let client = reqwest::blocking::Client::builder().timeout(None).build().unwrap();
            let resp = match client.get(&url).send() {
                Ok(r) => r,
                Err(e) => {
                    log::warn!("music together: {e}");
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
            };
            let mut latest: Option<Value> = None;
            for line in BufReader::new(resp).lines() {
                if !run.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(line) = line else { break };
                let Ok(ev) = serde_json::from_str::<Value>(&line) else { continue };
                if let Some(id) = ev.get("id").and_then(Value::as_str) {
                    since = id.to_string();
                }
                if ev.get("event").and_then(Value::as_str) != Some("message") {
                    // "open" means backlog is done: apply the newest backlog entry.
                    if let Some(state) = latest.take() {
                        apply(&app, &state);
                    }
                    continue;
                }
                let Some(state) = ev.get("message").and_then(Value::as_str).and_then(|m| serde_json::from_str::<Value>(m).ok()) else { continue };
                latest = Some(state.clone());
                apply(&app, &state);
            }
        }
    });
}

/// Bring the local player in line with the host's state.
fn apply(app: &AppHandle, state: &Value) {
    let Some(id) = state.get("v").and_then(Value::as_str) else { return };
    if id.len() != 11 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return;
    }
    let paused = state.get("p").and_then(Value::as_bool).unwrap_or(false);
    let sent_at = state.get("at").and_then(Value::as_u64).unwrap_or_else(now_ms);
    let mut t = state.get("t").and_then(Value::as_f64).unwrap_or(0.0);
    if !paused {
        t += now_ms().saturating_sub(sent_at) as f64 / 1000.0;
    }
    let current = app.state::<PlayerState>().current_live();
    match current {
        Some(s) if s.video_id == id => {
            if (s.elapsed - t).abs() > 2.0 {
                window::page_command(app, "seek", Some(json!(t)));
            }
            if s.paused != paused {
                window::page_command(app, if paused { "pause" } else { "play" }, None);
            }
        }
        _ => {
            window::page_command(app, "navigate", Some(json!(format!("https://music.youtube.com/watch?v={id}&t={}", t as u64))));
        }
    }
}

impl Plugin for MusicTogether {
    fn id(&self) -> &'static str {
        "music-together"
    }
    fn name(&self) -> &'static str {
        "Music Together"
    }
    fn description(&self) -> &'static str {
        "Listen in sync with friends using a share code"
    }
    fn menu_actions(&self) -> Vec<MenuAction> {
        vec![
            MenuAction { id: "host", label: "Host a session", accelerator: None },
            MenuAction { id: "join", label: "Join a session…", accelerator: None },
            MenuAction { id: "leave", label: "Leave session", accelerator: None },
        ]
    }
    fn on_menu_action(&self, app: &AppHandle, action: &str, _cfg: &Value) {
        match action {
            "host" => {
                self.leave();
                let topic = new_code();
                *self.0.lock().unwrap() = Some(Role::Host { topic: topic.clone(), last_sent: None });
                if let Some(s) = app.state::<PlayerState>().current_live() {
                    publish(&topic, &s);
                }
                let js = format!(
                    "navigator.clipboard && navigator.clipboard.writeText({0}).catch(()=>{{}}); window.__pear && window.__pear.toast('Session code copied: ' + {0}, 8000);",
                    serde_json::to_string(&topic).unwrap()
                );
                window::eval(app, &js);
                app.dialog()
                    .message(format!("Share this code with friends:\n\n{topic}\n\nIt's on your clipboard. Everyone who joins follows what you play."))
                    .title("Music Together")
                    .show(|_| {});
            }
            "join" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let Some(code) = crate::bridge::prompt(&app, "Join a session", "Paste the code from the host.", "", "text").await else { return };
                    let Some(topic) = topic_from_code(&code) else {
                        window::eval(&app, "window.__pear && window.__pear.toast(\"That isn't a valid session code.\", 5000);");
                        return;
                    };
                    let this = app.state::<super::Registry>();
                    if let Some(p) = this.get("music-together") {
                        p.on_menu_action(&app, &format!("join-topic:{topic}"), &Value::Null);
                    }
                });
            }
            "leave" => {
                self.leave();
                window::eval(app, "window.__pear && window.__pear.toast('Left the session.', 3000);");
            }
            other => {
                if let Some(topic) = other.strip_prefix("join-topic:") {
                    self.leave();
                    let run = Arc::new(AtomicBool::new(true));
                    *self.0.lock().unwrap() = Some(Role::Guest { topic: topic.to_string(), run: run.clone() });
                    follow(app.clone(), topic.to_string(), run);
                    window::eval(app, "window.__pear && window.__pear.toast('Joined. Following the host.', 4000);");
                }
            }
        }
    }
    fn stop(&self, _app: &AppHandle) {
        self.leave();
    }
    fn on_config_change(&self, _app: &AppHandle, _cfg: &Value) {}
    fn on_player_change(&self, _app: &AppHandle, song: &SongInfo, change: Change, _cfg: &Value) {
        let mut slot = self.0.lock().unwrap();
        if let Some(Role::Host { topic, last_sent }) = slot.as_mut() {
            // Publish real changes; ntfy.sh rate-limits bursts.
            if change.song || change.playback || change.seek || last_sent.map_or(true, |t| t.elapsed() > Duration::from_secs(30)) {
                *last_sent = Some(Instant::now());
                publish(topic, song);
            }
        }
    }
}

impl MusicTogether {
    fn leave(&self) {
        if let Some(Role::Guest { run, .. }) = self.0.lock().unwrap().take() {
            run.store(false, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_validated() {
        let c = new_code();
        assert_eq!(topic_from_code(&format!("  {c} ")), Some(c));
        assert_eq!(topic_from_code("oxide-../x"), None);
        assert_eq!(topic_from_code("other-topic"), None);
    }
}
