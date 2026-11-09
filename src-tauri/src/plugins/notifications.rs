//! Desktop notification when a new song starts.

use serde_json::{json, Value};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

use super::{Plugin, PluginOption};
use crate::player::{Change, SongInfo};
use crate::window;

pub struct Notifications;

impl Plugin for Notifications {
    fn id(&self) -> &'static str {
        "notifications"
    }
    fn name(&self) -> &'static str {
        "Notifications"
    }
    fn description(&self) -> &'static str {
        "Desktop notification when a song starts"
    }
    fn defaults(&self) -> Value {
        json!({ "enabled": true, "skipWhenFocused": true })
    }
    fn options(&self) -> Vec<PluginOption> {
        vec![PluginOption::toggle("skipWhenFocused", "Skip while the window is focused")]
    }

    fn on_player_change(&self, app: &AppHandle, song: &SongInfo, change: Change, cfg: &Value) {
        if !change.song || song.paused {
            return;
        }
        let skip_focused = cfg.get("skipWhenFocused").and_then(Value::as_bool).unwrap_or(true);
        if skip_focused {
            let focused = window::main_window(app)
                .map(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false))
                .unwrap_or(false);
            if focused {
                return;
            }
        }
        let body = match &song.album {
            Some(album) if !album.is_empty() => format!("{} • {}", song.artist, album),
            _ => song.artist.clone(),
        };
        if let Err(e) = app.notification().builder().title(&song.title).body(body).show() {
            log::warn!("notification failed: {e}");
        }
    }
}
