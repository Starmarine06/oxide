//! The app menu. It's rendered by the page's title bar (scripts/titlebar.js)
//! from the JSON model built here; clicks come back as `menu_click(id)`.
//!
//! Item ids:
//! - `app:*`, `nav:*`, `play:*`, `opt:*`   built-in actions
//! - `plugin:<id>`                          toggle a plugin
//! - `set:<id>:<key>:<json value>`          set a plugin option
//! - `action:<id>:<action>`                 plugin menu action

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::config::{ConfigStore, Options};
use crate::plugins::{OptionKind, Registry};
use crate::window;

fn item(id: &str, label: &str, accel: Option<&str>) -> Value {
    json!({ "type": "item", "id": id, "label": label, "accelerator": accel })
}

fn check(id: &str, label: &str, checked: bool) -> Value {
    json!({ "type": "check", "id": id, "label": label, "checked": checked })
}

fn radio(id: &str, label: &str, checked: bool) -> Value {
    json!({ "type": "radio", "id": id, "label": label, "checked": checked })
}

fn submenu(label: &str, items: Vec<Value>) -> Value {
    json!({ "type": "submenu", "label": label, "items": items })
}

fn sep() -> Value {
    json!({ "type": "separator" })
}

pub fn model(app: &AppHandle) -> Value {
    let store = app.state::<ConfigStore>();
    let registry = app.state::<Registry>();
    let opts = store.get().options;

    let app_menu = submenu(
        "App",
        vec![
            item("app:reload", "Reload", Some("CmdOrCtrl+R")),
            item("app:devtools", "Developer tools", Some("CmdOrCtrl+Shift+I")),
            item("app:config", "Open config folder", None),
            item("app:update", "Check for updates", None),
            sep(),
            item("app:quit", "Quit", Some("CmdOrCtrl+Q")),
        ],
    );

    let nav = submenu(
        "Navigation",
        vec![
            item("nav:back", "Back", Some("Alt+Left")),
            item("nav:forward", "Forward", Some("Alt+Right")),
            item("nav:home", "Home", None),
            item("nav:search", "Search", Some("CmdOrCtrl+F")),
        ],
    );

    let playback = submenu(
        "Playback",
        vec![
            item("play:toggle", "Play / Pause", None),
            item("play:next", "Next", Some("CmdOrCtrl+Right")),
            item("play:previous", "Previous", Some("CmdOrCtrl+Left")),
            sep(),
            item("play:like", "Like", None),
            item("play:dislike", "Dislike", None),
            item("play:shuffle", "Shuffle", None),
            item("play:repeat", "Repeat", None),
            sep(),
            item("play:volup", "Volume up", Some("CmdOrCtrl+Up")),
            item("play:voldown", "Volume down", Some("CmdOrCtrl+Down")),
            item("play:mute", "Mute", Some("CmdOrCtrl+M")),
        ],
    );

    let mut plugin_items = Vec::new();
    for p in registry.iter() {
        let cfg = registry.config(&store, p.as_ref());
        let enabled = cfg.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        let options = p.options();
        let actions = p.menu_actions();
        let toggle = check(&format!("plugin:{}", p.id()), if options.is_empty() && actions.is_empty() { p.name() } else { "Enabled" }, enabled);
        if options.is_empty() && actions.is_empty() {
            let mut t = toggle;
            t["description"] = json!(p.description());
            plugin_items.push(t);
            continue;
        }
        let mut items = vec![toggle];
        if !options.is_empty() {
            items.push(sep());
        }
        for o in options {
            let current = cfg.get(o.key).cloned().unwrap_or(Value::Null);
            match o.kind {
                OptionKind::Toggle => {
                    let on = current.as_bool().unwrap_or(false);
                    items.push(check(&format!("set:{}:{}:{}", p.id(), o.key, !on), o.label, on));
                }
                OptionKind::Choice(choices) => {
                    let sub = choices
                        .into_iter()
                        .map(|(v, label)| radio(&format!("set:{}:{}:{}", p.id(), o.key, v), label, v == current))
                        .collect();
                    items.push(submenu(o.label, sub));
                }
            }
        }
        if !actions.is_empty() {
            items.push(sep());
            for a in actions {
                let mut it = item(&format!("action:{}:{}", p.id(), a.id), a.label, a.accelerator);
                if !enabled {
                    it["disabled"] = json!(true);
                }
                items.push(it);
            }
        }
        let mut sm = submenu(p.name(), items);
        sm["checked"] = json!(enabled);
        sm["description"] = json!(p.description());
        plugin_items.push(sm);
    }
    let plugins = submenu("Plugins", plugin_items);

    let options = submenu(
        "Options",
        vec![
            check("opt:hide_to_tray", "Hide to tray on close", opts.hide_to_tray),
            check("opt:start_minimized", "Start minimized to tray", opts.start_minimized),
            check("opt:hide_menu", "Hide menu (press Alt to show)", opts.hide_menu),
            check("opt:override_user_agent", "Use Chrome user agent", opts.override_user_agent),
        ],
    );

    json!({
        "menus": [app_menu, nav, playback, plugins, options],
        "hideMenu": opts.hide_menu,
        "macos": cfg!(target_os = "macos"),
    })
}

/// Send the new menu model to the title bar.
pub fn refresh(app: &AppHandle) {
    let model = model(app);
    window::eval(app, &format!("window.__oxideTitlebar && window.__oxideTitlebar.setMenu({model});"));
}

#[tauri::command]
pub fn menu_model(app: AppHandle) -> Value {
    model(&app)
}

#[tauri::command]
pub fn menu_click(app: AppHandle, id: String) {
    handle(&app, &id);
}

/// Handles clicks from the title bar menu and the tray menu.
pub fn handle(app: &AppHandle, id: &str) {
    match id {
        "app:reload" => window::eval(app, "location.reload()"),
        "app:devtools" => {
            if let Some(w) = window::main_window(app) {
                if w.is_devtools_open() {
                    w.close_devtools();
                } else {
                    w.open_devtools();
                }
            }
        }
        "app:config" => {
            if let Ok(dir) = app.path().app_config_dir() {
                crate::open_path(&dir);
            }
        }
        "app:update" | "tray:update" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move { crate::updater::check(&app, true).await });
        }
        "app:quit" | "tray:quit" => {
            window::save_geometry(app);
            app.exit(0);
        }
        "nav:back" => window::page_command(app, "back", None),
        "nav:forward" => window::page_command(app, "forward", None),
        "nav:home" => window::page_command(app, "home", None),
        "nav:search" => window::page_command(app, "search", None),
        "play:toggle" | "tray:toggle" => window::page_command(app, "toggle", None),
        "play:next" | "tray:next" => window::page_command(app, "next", None),
        "play:previous" | "tray:previous" => window::page_command(app, "previous", None),
        "play:like" => window::page_command(app, "like", None),
        "play:dislike" => window::page_command(app, "dislike", None),
        "play:shuffle" => window::page_command(app, "shuffle", None),
        "play:repeat" => window::page_command(app, "repeat", None),
        "play:volup" => window::page_command(app, "volumeBy", Some(json!(5))),
        "play:voldown" => window::page_command(app, "volumeBy", Some(json!(-5))),
        "play:mute" => window::page_command(app, "mute", None),
        "tray:show" => window::show(app),
        "tray:lyrics" => crate::overlay::toggle(app),
        "opt:hide_to_tray" => toggle_option(app, |o| &mut o.hide_to_tray),
        "opt:start_minimized" => toggle_option(app, |o| &mut o.start_minimized),
        "opt:hide_menu" => toggle_option(app, |o| &mut o.hide_menu),
        "opt:override_user_agent" => {
            toggle_option(app, |o| &mut o.override_user_agent);
            // The user agent is fixed when the webview is created.
            window::rebuild_if_needed(app);
        }
        _ => {
            let registry = app.state::<Registry>();
            if let Some(plugin_id) = id.strip_prefix("plugin:") {
                let store = app.state::<ConfigStore>();
                let Some(p) = registry.get(plugin_id) else { return };
                let enabled = !registry.is_enabled(&store, p.as_ref());
                registry.set_enabled(app, plugin_id, enabled);
                refresh(app);
            } else if let Some(rest) = id.strip_prefix("set:") {
                let mut parts = rest.splitn(3, ':');
                if let (Some(pid), Some(key), Some(raw)) = (parts.next(), parts.next(), parts.next()) {
                    if key == "enabled" {
                        return;
                    }
                    if let Ok(value) = serde_json::from_str::<Value>(raw) {
                        registry.set_option(app, pid, key, value);
                        refresh(app);
                    }
                }
            } else if let Some(rest) = id.strip_prefix("action:") {
                if let Some((plugin_id, action)) = rest.split_once(':') {
                    registry.on_menu_action(app, plugin_id, action);
                }
            }
        }
    }
}

fn toggle_option(app: &AppHandle, field: impl Fn(&mut Options) -> &mut bool) {
    app.state::<ConfigStore>().update(|c| {
        let f = field(&mut c.options);
        *f = !*f;
    });
    refresh(app);
}
