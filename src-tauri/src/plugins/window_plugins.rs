//! Plugins that change the window itself: picture-in-picture, transparent
//! player and the network proxy.

use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager};

use super::{MenuAction, Plugin, PluginOption, Registry};
use crate::config::ConfigStore;
use crate::window;

// ---- picture-in-picture ------------------------------------------------------

/// Position, size and maximized state from before entering PiP.
type SavedGeometry = (LogicalPosition<f64>, LogicalSize<f64>, bool);

#[derive(Default)]
pub struct PictureInPicture(Mutex<Option<SavedGeometry>>);

impl PictureInPicture {
    fn toggle(&self, app: &AppHandle, cfg: &Value) {
        let Some(win) = window::main_window(app) else { return };
        let scale = win.scale_factor().unwrap_or(1.0);
        let mut saved = self.0.lock().unwrap();
        if let Some((pos, size, maximized)) = saved.take() {
            let _ = win.set_always_on_top(false);
            let _ = win.set_size(size);
            let _ = win.set_position(pos);
            if maximized {
                let _ = win.maximize();
            }
            window::eval(app, "document.documentElement.classList.remove('oxide-pip');");
        } else {
            let maximized = win.is_maximized().unwrap_or(false);
            let pos = win.outer_position().map(|p| p.to_logical(scale)).unwrap_or(LogicalPosition::new(100.0, 100.0));
            let size = win.inner_size().map(|s| s.to_logical(scale)).unwrap_or(LogicalSize::new(1100.0, 750.0));
            *saved = Some((pos, size, maximized));
            if maximized {
                let _ = win.unmaximize();
            }
            let side = cfg.get("size").and_then(Value::as_f64).unwrap_or(420.0);
            let _ = win.set_size(LogicalSize::new(side, side + 64.0));
            if cfg.get("alwaysOnTop").and_then(Value::as_bool).unwrap_or(true) {
                let _ = win.set_always_on_top(true);
            }
            window::eval(app, "document.documentElement.classList.add('oxide-pip');");
            window::page_command(app, "pip:enter", None);
        }
    }
}

impl Plugin for PictureInPicture {
    fn id(&self) -> &'static str {
        "picture-in-picture"
    }
    fn name(&self) -> &'static str {
        "Picture-in-picture"
    }
    fn description(&self) -> &'static str {
        "Shrink Oxide to a small always-on-top player"
    }
    fn defaults(&self) -> Value {
        json!({ "enabled": false, "alwaysOnTop": true, "size": 420 })
    }
    fn script(&self) -> Option<&'static str> {
        Some(include_str!("../../scripts/plugins/picture-in-picture.js"))
    }
    fn options(&self) -> Vec<PluginOption> {
        vec![
            PluginOption::toggle("alwaysOnTop", "Keep on top"),
            PluginOption::choice("size", "Size", vec![(json!(320), "Small"), (json!(420), "Medium"), (json!(560), "Large")]),
        ]
    }
    fn menu_actions(&self) -> Vec<MenuAction> {
        vec![MenuAction { id: "toggle", label: "Enter / leave picture-in-picture", accelerator: Some("CmdOrCtrl+Shift+P") }]
    }
    fn on_menu_action(&self, app: &AppHandle, action: &str, cfg: &Value) {
        if action == "toggle" {
            self.toggle(app, cfg);
        }
    }
    fn stop(&self, app: &AppHandle) {
        if self.0.lock().unwrap().is_some() {
            self.toggle(app, &Value::Null);
        }
    }
    fn on_config_change(&self, _app: &AppHandle, _cfg: &Value) {}
}

/// The page's PiP button.
#[tauri::command]
pub fn pip_toggle(app: AppHandle) {
    let registry = app.state::<Registry>();
    let store = app.state::<ConfigStore>();
    if let Some(p) = registry.get("picture-in-picture") {
        if registry.is_enabled(&store, p.as_ref()) {
            p.on_menu_action(&app, "toggle", &registry.config(&store, p.as_ref()));
        }
    }
}

// ---- transparent player ---------------------------------------------------------

pub struct TransparentPlayer;

#[cfg(target_os = "windows")]
fn apply_effect(app: &AppHandle, cfg: Option<&Value>) {
    use tauri::window::{Effect, EffectsBuilder};
    let Some(win) = window::main_window(app) else { return };
    let result = match cfg {
        Some(cfg) => {
            let effect = match cfg.get("type").and_then(Value::as_str) {
                Some("mica") => Effect::Mica,
                Some("tabbed") => Effect::Tabbed,
                _ => Effect::Acrylic,
            };
            win.set_effects(EffectsBuilder::new().effect(effect).build())
        }
        None => win.set_effects(None),
    };
    if let Err(e) = result {
        log::warn!("window effect: {e}");
    }
}

impl Plugin for TransparentPlayer {
    fn id(&self) -> &'static str {
        "transparent-player"
    }
    fn name(&self) -> &'static str {
        "Transparent player"
    }
    fn description(&self) -> &'static str {
        "See-through window with a blurred desktop behind it"
    }
    fn supported(&self) -> bool {
        cfg!(target_os = "windows")
    }
    fn defaults(&self) -> Value {
        json!({ "enabled": false, "type": "acrylic", "opacity": 0.5 })
    }
    fn script(&self) -> Option<&'static str> {
        Some(include_str!("../../scripts/plugins/transparent-player.js"))
    }
    fn options(&self) -> Vec<PluginOption> {
        vec![
            PluginOption::choice("type", "Material", vec![(json!("acrylic"), "Acrylic"), (json!("mica"), "Mica"), (json!("tabbed"), "Mica Alt")]),
            PluginOption::choice("opacity", "Tint", vec![(json!(0.2), "Light"), (json!(0.5), "Medium"), (json!(0.75), "Dark")]),
        ]
    }
    fn start(&self, app: &AppHandle, cfg: &Value) {
        // A transparent webview has to be created that way.
        window::rebuild_if_needed(app);
        #[cfg(target_os = "windows")]
        {
            let app2 = app.clone();
            let cfg = cfg.clone();
            let _ = app.run_on_main_thread(move || apply_effect(&app2, Some(&cfg)));
        }
        #[cfg(not(target_os = "windows"))]
        let _ = (app, cfg);
    }
    fn stop(&self, app: &AppHandle) {
        window::rebuild_if_needed(app);
        #[cfg(target_os = "windows")]
        {
            let app2 = app.clone();
            let _ = app.run_on_main_thread(move || apply_effect(&app2, None));
        }
        #[cfg(not(target_os = "windows"))]
        let _ = app;
    }
}

// ---- proxy ------------------------------------------------------------------------

pub struct Proxy;

impl Plugin for Proxy {
    fn id(&self) -> &'static str {
        "auth-proxy-adapter"
    }
    fn name(&self) -> &'static str {
        "Proxy"
    }
    fn description(&self) -> &'static str {
        "Route YouTube Music through an HTTP or SOCKS5 proxy"
    }
    fn defaults(&self) -> Value {
        json!({ "enabled": false, "url": "socks5://127.0.0.1:4545" })
    }
    fn menu_actions(&self) -> Vec<MenuAction> {
        vec![MenuAction { id: "url", label: "Set proxy address…", accelerator: None }]
    }
    fn on_menu_action(&self, app: &AppHandle, action: &str, cfg: &Value) {
        if action != "url" {
            return;
        }
        let current = cfg.get("url").and_then(Value::as_str).unwrap_or_default().to_string();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let hint = "http://host:port or socks5://host:port";
            if let Some(v) = crate::bridge::prompt(&app, "Proxy address", hint, &current, "text").await {
                app.state::<Registry>().set_option(&app, "auth-proxy-adapter", "url", json!(v.trim()));
            }
        });
    }
    // The proxy is fixed when the webview is created. The registry saves
    // the new state before calling these, so the check sees it.
    fn start(&self, app: &AppHandle, _cfg: &Value) {
        window::rebuild_if_needed(app);
    }
    fn stop(&self, app: &AppHandle) {
        window::rebuild_if_needed(app);
    }
    fn on_config_change(&self, app: &AppHandle, _cfg: &Value) {
        window::rebuild_if_needed(app);
    }
}

/// Proxy URL for new webviews, if the plugin is on.
pub fn proxy_url(app: &AppHandle) -> Option<tauri::Url> {
    let registry = app.state::<Registry>();
    let store = app.state::<ConfigStore>();
    let p = registry.get("auth-proxy-adapter")?;
    if !registry.is_enabled(&store, p.as_ref()) {
        return None;
    }
    let cfg = registry.config(&store, p.as_ref());
    cfg.get("url").and_then(Value::as_str).and_then(|u| u.parse().ok())
}
