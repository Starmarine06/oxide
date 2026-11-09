//! Plugin system.
//!
//! A plugin can have a Rust half (lifecycle and player hooks, menu
//! actions, IPC commands) and a page half (JavaScript injected into
//! music.youtube.com). Every page script is injected on every load and
//! registers itself with `pear.plugin(id, {...})`; the page runtime in
//! `scripts/core.js` starts and stops it live as settings change, so
//! toggling a plugin never needs a restart.
//!
//! Settings live under `plugins.<id>` in the config file.

use serde_json::{json, Map, Value};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

use crate::config::ConfigStore;
use crate::player::{Change, SongInfo};
use crate::window;

pub mod adblock;
pub mod api_server;
pub mod discord;
pub mod downloader;
pub mod integrations;
pub mod lyrics;
pub mod music_together;
pub mod notifications;
pub mod page;
pub mod scrobbler;
pub mod shortcuts;
pub mod taskbar;
pub mod window_plugins;

pub struct MenuAction {
    pub id: &'static str,
    pub label: &'static str,
    pub accelerator: Option<&'static str>,
}

/// A setting shown in the plugin's menu.
pub enum OptionKind {
    /// Checkbox for a boolean.
    Toggle,
    /// Pick one value from a list.
    Choice(Vec<(Value, &'static str)>),
}

pub struct PluginOption {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: OptionKind,
}

impl PluginOption {
    pub fn toggle(key: &'static str, label: &'static str) -> Self {
        Self { key, label, kind: OptionKind::Toggle }
    }
    pub fn choice(key: &'static str, label: &'static str, choices: Vec<(Value, &'static str)>) -> Self {
        Self { key, label, kind: OptionKind::Choice(choices) }
    }
}

pub trait Plugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;

    fn description(&self) -> &'static str {
        ""
    }

    /// Default settings. Must include `enabled`.
    fn defaults(&self) -> Value {
        json!({ "enabled": false })
    }

    /// Page JavaScript. It receives `pear` and must call `pear.plugin(id, {...})`.
    fn script(&self) -> Option<&'static str> {
        None
    }

    /// False hides the plugin on this OS.
    fn supported(&self) -> bool {
        true
    }

    fn options(&self) -> Vec<PluginOption> {
        Vec::new()
    }

    fn menu_actions(&self) -> Vec<MenuAction> {
        Vec::new()
    }

    fn on_menu_action(&self, _app: &AppHandle, _action: &str, _cfg: &Value) {}

    /// Called at startup (if enabled) and when the user enables the plugin.
    fn start(&self, _app: &AppHandle, _cfg: &Value) {}

    /// Called when the user disables the plugin and at shutdown.
    fn stop(&self, _app: &AppHandle) {}

    /// Called when an option changes while the plugin is enabled. The
    /// default restarts the Rust half so it picks up the new settings.
    fn on_config_change(&self, app: &AppHandle, cfg: &Value) {
        self.stop(app);
        self.start(app, cfg);
    }

    fn on_player_change(&self, _app: &AppHandle, _song: &SongInfo, _change: Change, _cfg: &Value) {}
}

pub struct Registry {
    plugins: Vec<Arc<dyn Plugin>>,
}

impl Registry {
    pub fn new() -> Self {
        let mut plugins: Vec<Arc<dyn Plugin>> = vec![
            Arc::new(adblock::AdBlocker),
            Arc::new(discord::Discord::default()),
            Arc::new(notifications::Notifications),
            Arc::new(lyrics::SyncedLyrics),
            Arc::new(downloader::Downloader),
            Arc::new(scrobbler::Scrobbler::default()),
            Arc::new(api_server::ApiServer::default()),
            Arc::new(integrations::TunaObs::default()),
            Arc::new(integrations::LumiaStream),
            Arc::new(integrations::Amuse::default()),
            Arc::new(shortcuts::Shortcuts),
            Arc::new(taskbar::TaskbarMediaControl),
            Arc::new(window_plugins::PictureInPicture::default()),
            Arc::new(window_plugins::TransparentPlayer),
            Arc::new(window_plugins::Proxy),
            Arc::new(music_together::MusicTogether::default()),
        ];
        plugins.extend(page::all());
        plugins.retain(|p| p.supported());
        plugins.sort_by_key(|p| p.name().to_lowercase());
        Self { plugins }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn Plugin>> {
        self.plugins.iter()
    }

    pub fn get(&self, id: &str) -> Option<&Arc<dyn Plugin>> {
        self.plugins.iter().find(|p| p.id() == id)
    }

    pub fn config(&self, store: &ConfigStore, plugin: &dyn Plugin) -> Value {
        store.plugin(plugin.id(), &plugin.defaults())
    }

    pub fn config_by_id(&self, store: &ConfigStore, id: &str) -> Value {
        self.get(id).map(|p| self.config(store, p.as_ref())).unwrap_or(Value::Null)
    }

    pub fn is_enabled(&self, store: &ConfigStore, plugin: &dyn Plugin) -> bool {
        self.config(store, plugin)
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// `{ rev, plugins: { id: settings } }`, given to the page.
    pub fn page_config(&self, store: &ConfigStore) -> Value {
        let mut map = Map::new();
        for p in &self.plugins {
            map.insert(p.id().to_string(), self.config(store, p.as_ref()));
        }
        json!({ "rev": store.get().rev, "plugins": Value::Object(map) })
    }

    /// Every plugin's page script, each isolated so one failing plugin
    /// can't take the others down.
    pub fn page_scripts(&self) -> String {
        let mut out = String::new();
        for p in &self.plugins {
            let Some(src) = p.script() else { continue };
            let id = serde_json::to_string(p.id()).unwrap();
            out.push_str(&format!(
                "if (window.__pear) {{ try {{ (function (pear) {{\n{src}\n}})(window.__pear); }} catch (e) {{ console.error('[oxide] plugin ' + {id} + ' failed to load', e); }} }}\n"
            ));
        }
        out.push_str("if (window.__pear) window.__pear.ready();\n");
        out
    }

    pub fn start_enabled(&self, app: &AppHandle) {
        let store = app.state::<ConfigStore>();
        for p in &self.plugins {
            if self.is_enabled(&store, p.as_ref()) {
                p.start(app, &self.config(&store, p.as_ref()));
            }
        }
    }

    pub fn stop_all(&self, app: &AppHandle) {
        for p in &self.plugins {
            p.stop(app);
        }
    }

    /// Send the current settings to the page so it can apply them live.
    fn push(&self, app: &AppHandle) {
        let store = app.state::<ConfigStore>();
        let cfg = self.page_config(&store);
        window::eval(app, &format!("window.__pear && window.__pear.setConfig({cfg});"));
    }

    pub fn set_enabled(&self, app: &AppHandle, id: &str, enabled: bool) {
        let Some(p) = self.get(id) else { return };
        let store = app.state::<ConfigStore>();
        store.set_plugin_value(id, "enabled", Value::Bool(enabled));
        if enabled {
            p.start(app, &self.config(&store, p.as_ref()));
        } else {
            p.stop(app);
        }
        self.push(app);
    }

    pub fn set_option(&self, app: &AppHandle, id: &str, key: &str, value: Value) {
        let Some(p) = self.get(id) else { return };
        let store = app.state::<ConfigStore>();
        store.set_plugin_value(id, key, value);
        if self.is_enabled(&store, p.as_ref()) {
            p.on_config_change(app, &self.config(&store, p.as_ref()));
        }
        self.push(app);
    }

    pub fn on_player_change(&self, app: &AppHandle, song: &SongInfo, change: Change) {
        let store = app.state::<ConfigStore>();
        for p in &self.plugins {
            let cfg = self.config(&store, p.as_ref());
            if cfg.get("enabled").and_then(Value::as_bool).unwrap_or(false) {
                p.on_player_change(app, song, change, &cfg);
            }
        }
    }

    pub fn on_menu_action(&self, app: &AppHandle, id: &str, action: &str) {
        let store = app.state::<ConfigStore>();
        if let Some(p) = self.get(id) {
            p.on_menu_action(app, action, &self.config(&store, p.as_ref()));
        }
    }
}

/// Lets page plugins save their own settings (e.g. a slider position).
#[tauri::command]
pub fn plugin_set_option(app: AppHandle, id: String, key: String, value: Value) -> Result<(), String> {
    if key == "enabled" {
        return Err("use the Plugins menu to enable or disable plugins".into());
    }
    let registry = app.state::<Registry>();
    if registry.get(&id).is_none() {
        return Err(format!("unknown plugin {id}"));
    }
    registry.set_option(&app, &id, &key, value);
    Ok(())
}
