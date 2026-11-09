//! System-wide keyboard shortcuts that work while Oxide is in the background.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use super::{MenuAction, Plugin, Registry};
use crate::window;

/// (config key, label, page command)
const ACTIONS: &[(&str, &str, &str)] = &[
    ("playPause", "Play / pause", "toggle"),
    ("next", "Next song", "next"),
    ("previous", "Previous song", "previous"),
    ("volumeUp", "Volume up", "volumeUp"),
    ("volumeDown", "Volume down", "volumeDown"),
    ("like", "Like", "like"),
    ("showWindow", "Show / hide window", "__window"),
];

static BOUND: LazyLock<Mutex<HashMap<u32, &'static str>>> = LazyLock::new(Default::default);

pub struct Shortcuts;

/// Called by the global-shortcut plugin for every registered shortcut.
pub fn on_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let Some(action) = BOUND.lock().unwrap().get(&shortcut.id()).copied() else { return };
    match action {
        "__window" => window::toggle_visible(app),
        "volumeUp" => window::page_command(app, "volumeBy", Some(json!(5))),
        "volumeDown" => window::page_command(app, "volumeBy", Some(json!(-5))),
        cmd => window::page_command(app, cmd, None),
    }
}

impl Plugin for Shortcuts {
    fn id(&self) -> &'static str {
        "shortcuts"
    }
    fn name(&self) -> &'static str {
        "Global shortcuts"
    }
    fn description(&self) -> &'static str {
        "Keyboard shortcuts that work even when Oxide isn't focused"
    }
    fn defaults(&self) -> Value {
        json!({
            "enabled": false,
            "playPause": "", "next": "", "previous": "",
            "volumeUp": "", "volumeDown": "", "like": "", "showWindow": "",
        })
    }
    fn menu_actions(&self) -> Vec<MenuAction> {
        // Labels are static, so spell them out.
        vec![
            MenuAction { id: "playPause", label: "Set play / pause shortcut…", accelerator: None },
            MenuAction { id: "next", label: "Set next song shortcut…", accelerator: None },
            MenuAction { id: "previous", label: "Set previous song shortcut…", accelerator: None },
            MenuAction { id: "volumeUp", label: "Set volume up shortcut…", accelerator: None },
            MenuAction { id: "volumeDown", label: "Set volume down shortcut…", accelerator: None },
            MenuAction { id: "like", label: "Set like shortcut…", accelerator: None },
            MenuAction { id: "showWindow", label: "Set show / hide window shortcut…", accelerator: None },
        ]
    }
    fn on_menu_action(&self, app: &AppHandle, action: &str, cfg: &Value) {
        let Some((key, label, _)) = ACTIONS.iter().find(|(k, _, _)| *k == action).copied() else { return };
        let current = cfg.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let hint = "For example Ctrl+Alt+P or MediaPlayPause. Leave empty to remove.";
            let Some(value) = crate::bridge::prompt(&app, &format!("{label} shortcut"), hint, &current, "text").await else { return };
            let value = value.trim().to_string();
            if !value.is_empty() && value.parse::<Shortcut>().is_err() {
                window::eval(&app, "window.__pear && window.__pear.toast(\"That shortcut isn't valid.\", 4000);");
                return;
            }
            app.state::<Registry>().set_option(&app, "shortcuts", key, json!(value));
        });
    }
    fn start(&self, app: &AppHandle, cfg: &Value) {
        let gs = app.global_shortcut();
        let mut bound = BOUND.lock().unwrap();
        for (key, _, cmd) in ACTIONS {
            let Some(text) = cfg.get(*key).and_then(Value::as_str).filter(|s| !s.trim().is_empty()) else { continue };
            match text.parse::<Shortcut>() {
                Ok(sc) => match gs.register(sc) {
                    Ok(()) => {
                        bound.insert(sc.id(), cmd);
                    }
                    Err(e) => log::warn!("shortcut {text}: {e}"),
                },
                Err(e) => log::warn!("bad shortcut {text}: {e}"),
            }
        }
    }
    fn stop(&self, app: &AppHandle) {
        let _ = app.global_shortcut().unregister_all();
        BOUND.lock().unwrap().clear();
    }
}
