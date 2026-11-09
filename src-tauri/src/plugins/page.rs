//! Plugins that live entirely in the page. Each is a JavaScript file in
//! `scripts/plugins/` plus the metadata here (name, defaults, menu options).

use serde_json::{json, Value};
use tauri::AppHandle;

use super::{MenuAction, Plugin, PluginOption};
use crate::window;

pub struct PagePlugin {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub script: &'static str,
    pub defaults: fn() -> Value,
    pub options: fn() -> Vec<PluginOption>,
    pub actions: fn() -> Vec<MenuAction>,
}

fn no_options() -> Vec<PluginOption> {
    Vec::new()
}

fn no_actions() -> Vec<MenuAction> {
    Vec::new()
}

fn off() -> Value {
    json!({ "enabled": false })
}

fn on() -> Value {
    json!({ "enabled": true })
}

impl Plugin for PagePlugin {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn description(&self) -> &'static str {
        self.description
    }
    fn defaults(&self) -> Value {
        (self.defaults)()
    }
    fn script(&self) -> Option<&'static str> {
        Some(self.script)
    }
    fn options(&self) -> Vec<PluginOption> {
        (self.options)()
    }
    fn menu_actions(&self) -> Vec<MenuAction> {
        (self.actions)()
    }
    fn on_menu_action(&self, app: &AppHandle, action: &str, _cfg: &Value) {
        window::page_command(app, &format!("{}:{}", self.id, action), None);
    }
    // Page plugins have no Rust state to restart.
    fn on_config_change(&self, _app: &AppHandle, _cfg: &Value) {}
}

macro_rules! page_plugin {
    ($id:literal, $name:literal, $desc:literal $(, defaults = $defaults:expr)? $(, options = $options:expr)? $(, actions = $actions:expr)? , prelude = $prelude:literal $(,)?) => {{
        let mut p = page_plugin!($id, $name, $desc $(, defaults = $defaults)? $(, options = $options)? $(, actions = $actions)?);
        p.script = concat!(include_str!(concat!("../../scripts/plugins/", $prelude)), "\n", include_str!(concat!("../../scripts/plugins/", $id, ".js")));
        p
    }};
    ($id:literal, $name:literal, $desc:literal $(, defaults = $defaults:expr)? $(, options = $options:expr)? $(, actions = $actions:expr)? $(,)?) => {{
        #[allow(unused_mut, unused_assignments)]
        let mut p = PagePlugin {
            id: $id,
            name: $name,
            description: $desc,
            script: include_str!(concat!("../../scripts/plugins/", $id, ".js")),
            defaults: off,
            options: no_options,
            actions: no_actions,
        };
        $( p.defaults = $defaults; )?
        $( p.options = $options; )?
        $( p.actions = $actions; )?
        p
    }};
}

pub fn all() -> Vec<std::sync::Arc<dyn Plugin>> {
    let list: Vec<PagePlugin> = vec![
        page_plugin!("navigation", "Navigation arrows", "Back and forward buttons in the top bar", defaults = on),
        page_plugin!(
            "clock",
            "Clock",
            "A clock in the middle of the top bar",
            defaults = || json!({ "enabled": false, "displaySeconds": false, "hour12": false }),
            options = || vec![PluginOption::toggle("displaySeconds", "Show seconds"), PluginOption::toggle("hour12", "12-hour clock")],
        ),
        page_plugin!("blur-nav-bar", "Blur navigation bar", "Make the top bar transparent and blurred"),
        page_plugin!("unobtrusive-player", "Unobtrusive player", "Don't open the full player when a song starts"),
        page_plugin!("skip-disliked-songs", "Skip disliked songs", "Skip a song as soon as you dislike it"),
        page_plugin!("exponential-volume", "Exponential volume", "Make the volume slider exponential so low volumes are easier to pick"),
        page_plugin!(
            "disable-autoplay",
            "Disable autoplay",
            "Start songs paused",
            defaults = || json!({ "enabled": false, "applyOnce": false }),
            options = || vec![PluginOption::toggle("applyOnce", "Only for the first song after launch")],
        ),
        page_plugin!(
            "performance-improvement",
            "Performance boost",
            "Lower CPU use by throttling background timers",
            defaults = on,
            prelude = "vendor/performance.js",
        ),
        page_plugin!("do-not-track", "Do not track", "Block third-party ad and analytics trackers"),
        page_plugin!(
            "playback-speed",
            "Playback speed",
            "Speed up or slow down songs from the player bar",
            defaults = || json!({ "enabled": false, "speed": 1 }),
        ),
        page_plugin!(
            "precise-volume",
            "Precise volume",
            "Scroll on the player to change volume in small steps",
            defaults = || json!({ "enabled": false, "steps": 1, "arrowKeys": true }),
            options = || vec![
                PluginOption::choice("steps", "Step size", vec![(json!(1), "1%"), (json!(2), "2%"), (json!(5), "5%"), (json!(10), "10%")]),
                PluginOption::toggle("arrowKeys", "Shift+Up/Down changes volume"),
            ],
        ),
        page_plugin!("quality-changer", "Video quality", "Pick the video quality from the player bar"),
        page_plugin!(
            "captions-selector",
            "Captions",
            "Choose captions for songs that have them",
            defaults = || json!({ "enabled": false, "autoload": false, "lastLanguage": null }),
            options = || vec![PluginOption::toggle("autoload", "Turn captions on automatically")],
        ),
        page_plugin!(
            "video-toggle",
            "Song / video switch",
            "Switch between the song and its music video, or hide video entirely",
            defaults = || json!({ "enabled": false, "mode": "custom", "align": "left", "forceHide": false, "hideVideo": false }),
            options = || vec![
                PluginOption::choice("mode", "Style", vec![(json!("custom"), "Oxide switch"), (json!("native"), "YouTube Music switch"), (json!("disabled"), "No switch")]),
                PluginOption::choice("align", "Position", vec![(json!("left"), "Left"), (json!("middle"), "Middle"), (json!("right"), "Right")]),
                PluginOption::toggle("forceHide", "Hide the video area completely"),
            ],
        ),
        page_plugin!(
            "equalizer",
            "Equalizer",
            "10-band equalizer with presets",
            defaults = || json!({ "enabled": false, "preset": "Flat", "gains": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }),
        ),
        page_plugin!(
            "audio-compressor",
            "Audio compressor",
            "Even out loud and quiet parts",
            defaults = || json!({ "enabled": false, "strength": "medium" }),
            options = || vec![PluginOption::choice("strength", "Strength", vec![(json!("light"), "Light"), (json!("medium"), "Medium"), (json!("strong"), "Strong")])],
        ),
        page_plugin!(
            "custom-output-device",
            "Custom output device",
            "Play through a specific speaker or headset",
            defaults = || json!({ "enabled": false, "deviceId": null }),
            actions = || vec![MenuAction { id: "choose", label: "Choose output device…", accelerator: None }],
        ),
        page_plugin!(
            "skip-silences",
            "Skip silences",
            "Jump over long silent parts of songs",
            defaults = || json!({ "enabled": false, "onlySkipBeginning": false }),
            options = || vec![PluginOption::toggle("onlySkipBeginning", "Only at the start of songs")],
        ),
        page_plugin!(
            "crossfade",
            "Crossfade",
            "Fade between songs instead of cutting",
            defaults = || json!({ "enabled": false, "fadeInSeconds": 2, "fadeOutSeconds": 6, "skipEnding": true }),
            options = || vec![
                PluginOption::choice("fadeOutSeconds", "Fade out", vec![(json!(3), "3 seconds"), (json!(6), "6 seconds"), (json!(10), "10 seconds"), (json!(15), "15 seconds")]),
                PluginOption::choice("fadeInSeconds", "Fade in", vec![(json!(1), "1 second"), (json!(2), "2 seconds"), (json!(4), "4 seconds")]),
            ],
        ),
        page_plugin!(
            "visualizer",
            "Visualizer",
            "Animated visuals in the player, coloured from the album art",
            defaults = || json!({ "enabled": false, "mode": "bars", "placement": "art", "albumColors": true }),
            options = || vec![
                PluginOption::choice("mode", "Style", vec![(json!("bars"), "Bars"), (json!("radial"), "Radial"), (json!("aurora"), "Aurora")]),
                PluginOption::choice("placement", "Where", vec![(json!("art"), "Over the artwork"), (json!("background"), "Behind the player page")]),
                PluginOption::toggle("albumColors", "Use album art colours"),
            ],
        ),
        page_plugin!(
            "album-color-theme",
            "Album colour theme",
            "Tint the app with colours from the album art",
            defaults = || json!({ "enabled": false, "ratio": 0.5 }),
            options = || vec![PluginOption::choice("ratio", "Intensity", vec![(json!(0.3), "Subtle"), (json!(0.5), "Medium"), (json!(0.8), "Strong")])],
        ),
        page_plugin!(
            "ambient-mode",
            "Ambient mode",
            "A soft glow from the artwork or video around the player",
            defaults = || json!({ "enabled": false, "blur": 100, "opacity": 0.8, "size": 100, "interval": 80 }),
            options = || vec![
                PluginOption::choice("blur", "Blur", vec![(json!(60), "Low"), (json!(100), "Medium"), (json!(160), "High")]),
                PluginOption::choice("opacity", "Strength", vec![(json!(0.5), "Soft"), (json!(0.8), "Medium"), (json!(1.0), "Bright")]),
            ],
        ),
        page_plugin!("album-actions", "Album actions", "Like, dislike or clear every song on an album or playlist"),
        page_plugin!("sponsorblock", "SponsorBlock", "Skip intros, outros and talking in music videos"),
    ];
    list.into_iter().map(|p| std::sync::Arc::new(p) as std::sync::Arc<dyn Plugin>).collect()
}
