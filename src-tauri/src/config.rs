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

/// Where the on-screen lyrics sit, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Where one part of the on-screen lyrics sits on the screen, as fractions
/// (0 to 1) of the screen's work area, turned by `rot` degrees about its
/// middle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rot: f64,
    /// Visualizer bars grow from the other side; the progress bar fills
    /// from the other end.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flip: bool,
    /// Text alignment for the lyrics and title: "left", "center", "right".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
}

/// How the on-screen visualizer looks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VizSettings {
    /// "bars", "mirror", "wave", "circle" or "dots".
    pub style: String,
    /// "white", "accent", "cover" (the album cover's colour), "rainbow" or
    /// a "#rrggbb" colour.
    pub color: String,
    /// Number of bars (or points), 8 to 96.
    pub bars: u32,
    /// Level multiplier, 0.3 to 3.
    pub sensitivity: f64,
    /// 0.05 to 1.
    pub opacity: f64,
    /// How slowly bars fall back, 0 (instant) to 0.95.
    pub smoothing: f64,
}

impl Default for VizSettings {
    fn default() -> Self {
        Self { style: "bars".into(), color: "white".into(), bars: 32, sensitivity: 1.0, opacity: 0.35, smoothing: 0.6 }
    }
}

impl VizSettings {
    /// Keep values from the settings page in range.
    pub fn clean(mut self) -> Self {
        if !["bars", "mirror", "wave", "circle", "dots"].contains(&self.style.as_str()) {
            self.style = "bars".into();
        }
        if !valid_color(&self.color, true) {
            self.color = "white".into();
        }
        self.bars = self.bars.clamp(8, 96);
        self.sensitivity = if self.sensitivity.is_finite() { self.sensitivity.clamp(0.3, 3.0) } else { 1.0 };
        self.opacity = if self.opacity.is_finite() { self.opacity.clamp(0.05, 1.0) } else { 0.35 };
        self.smoothing = if self.smoothing.is_finite() { self.smoothing.clamp(0.0, 0.95) } else { 0.6 };
        self
    }
}

/// "white", "accent", "cover", a "#rrggbb" colour, or (where allowed)
/// "rainbow".
fn valid_color(c: &str, rainbow: bool) -> bool {
    let hex = c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    hex || ["white", "accent", "cover"].contains(&c) || (rainbow && c == "rainbow")
}

/// Colours of the on-screen lyrics' text and progress bar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartColors {
    pub lyrics: String,
    pub next: String,
    pub title: String,
    pub progress: String,
}

impl Default for PartColors {
    fn default() -> Self {
        Self { lyrics: "white".into(), next: "white".into(), title: "white".into(), progress: "white".into() }
    }
}

impl PartColors {
    pub fn clean(mut self) -> Self {
        for c in [&mut self.lyrics, &mut self.next, &mut self.title, &mut self.progress] {
            if !valid_color(c, false) {
                *c = "white".into();
            }
        }
        self
    }
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// The on-screen lyrics (desktop overlay).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlaySettings {
    /// Clicks go through to whatever is underneath. Off: it can be dragged,
    /// resized and has its own buttons.
    pub click_through: bool,
    pub show_cover: bool,
    pub show_title: bool,
    pub show_next: bool,
    pub show_progress: bool,
    pub show_visualizer: bool,
    /// Last preset picked ("bottom", "top-left", ...); cleared by dragging.
    pub preset: Option<String>,
    /// "left", "center" or "right", from where it sits on the screen.
    pub align: String,
    /// Where the automatic layout sits (logical screen pixels); None until
    /// it's moved or a preset is picked.
    pub rect: Option<Rect>,
    /// Parts placed by hand in edit mode ("cover", "title", "lyrics",
    /// "progress", "visualizer"); None uses the automatic layout.
    pub layout: Option<std::collections::BTreeMap<String, LayoutBox>>,
    /// The layout is in screen fractions. Layouts saved before the overlay
    /// covered the whole screen were fractions of its old, smaller window;
    /// those settings have no such field, so it reads as false for them.
    #[serde(default)]
    pub screen_layout: bool,
    pub viz: VizSettings,
    pub colors: PartColors,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            click_through: true,
            show_cover: false,
            show_title: false,
            show_next: true,
            show_progress: false,
            show_visualizer: false,
            preset: Some("bottom".into()),
            align: "center".into(),
            rect: None,
            layout: None,
            screen_layout: true,
            viz: VizSettings::default(),
            colors: PartColors::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Bumped on every save so the page can tell which copy is newer.
    pub rev: u64,
    pub options: Options,
    pub window: Option<Geometry>,
    pub overlay: OverlaySettings,
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
