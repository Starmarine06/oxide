//! Persistent settings stored as JSON in the platform config directory.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    /// Closing the window hides it to the tray instead of quitting.
    pub hide_to_tray: bool,
    /// Start hidden in the tray.
    pub start_minimized: bool,
    /// Present a desktop Chrome user agent (helps Google sign-in).
    pub override_user_agent: bool,
    /// Hide the menus in the title bar (press Alt to show them).
    pub hide_menu: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            hide_to_tray: true,
            start_minimized: false,
            // WebView2 (Windows) and WKWebView (macOS) are accepted by
            // YouTube Music as-is; WebKitGTK needs a Chrome user agent.
            override_user_agent: cfg!(target_os = "linux"),
            hide_menu: false,
        }
    }
}

/// Last window size and position, in logical pixels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Geometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub maximized: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Bumped on every save so the page can tell which copy is newer.
    pub rev: u64,
    pub options: Options,
    pub window: Option<Geometry>,
    /// Per-plugin settings keyed by plugin id. Every entry has an
    /// `enabled` bool plus whatever options that plugin defines.
    pub plugins: Map<String, Value>,
}

pub struct ConfigStore {
    path: PathBuf,
    inner: RwLock<Config>,
}

impl ConfigStore {
    pub fn load(path: PathBuf) -> Self {
        let cfg = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| match serde_json::from_str(&s) {
                Ok(c) => Some(c),
                Err(e) => {
                    log::warn!("config at {} is invalid ({e}), using defaults", path.display());
                    None
                }
            })
            .unwrap_or_default();
        Self {
            path,
            inner: RwLock::new(cfg),
        }
    }

    pub fn get(&self) -> Config {
        self.inner.read().unwrap().clone()
    }

    pub fn update(&self, f: impl FnOnce(&mut Config)) {
        let snapshot = {
            let mut cfg = self.inner.write().unwrap();
            f(&mut cfg);
            cfg.rev += 1;
            cfg.clone()
        };
        self.save(&snapshot);
    }

    fn save(&self, cfg: &Config) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_string_pretty(cfg) {
            Ok(s) => {
                // Write then rename so a crash never leaves a half-written file.
                let tmp = self.path.with_extension("json.tmp");
                if let Err(e) = std::fs::write(&tmp, s).and_then(|_| std::fs::rename(&tmp, &self.path)) {
                    log::error!("failed to save config: {e}");
                }
            }
            Err(e) => log::error!("failed to serialize config: {e}"),
        }
    }

    /// Returns the plugin's settings object merged over its defaults.
    pub fn plugin(&self, id: &str, defaults: &Value) -> Value {
        let mut merged = defaults.clone();
        if let (Some(target), Some(Value::Object(saved))) =
            (merged.as_object_mut(), self.inner.read().unwrap().plugins.get(id))
        {
            for (k, v) in saved {
                target.insert(k.clone(), v.clone());
            }
        }
        merged
    }

    pub fn set_plugin_value(&self, id: &str, key: &str, value: Value) {
        self.update(|cfg| {
            let entry = cfg
                .plugins
                .entry(id.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if !entry.is_object() {
                *entry = Value::Object(Map::new());
            }
            entry.as_object_mut().unwrap().insert(key.to_string(), value);
        });
    }
}
