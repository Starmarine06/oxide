//! The Settings window (dist/settings.html): app options and everything about
//! the on-screen lyrics in one place. It reads with `settings_get`, changes
//! overlay settings with `settings_set`, and runs actions (open the overlay,
//! edit its layout, toggle an app option) through `menu_click`, the same
//! path as the menus. Whenever something changes elsewhere it's told to
//! re-read.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::config::{ConfigStore, PartColors, VizSettings};
use crate::overlay;

const LABEL: &str = "settings";

pub fn open(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    // Built off the main thread, like the overlay (WebView2 can deadlock
    // when a window is created from a menu handler on the main thread).
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let built = WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("settings.html".into()))
            .title("Oxide Settings")
            .inner_size(640.0, 780.0)
            .min_inner_size(480.0, 520.0)
            .center()
            .theme(Some(tauri::Theme::Dark))
            .build();
        if let Err(e) = built {
            log::error!("can't open settings: {e}");
        }
    });
}

/// Something changed elsewhere (menu, tray, overlay): have the window re-read.
pub fn refresh(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.eval("window.refreshSettings && window.refreshSettings();");
    }
}

#[tauri::command]
pub fn settings_get(app: AppHandle) -> Value {
    let options = app.state::<ConfigStore>().get().options;
    json!({
        "options": options,
        "overlay": overlay::settings_view(&app),
        "version": app.package_info().version.to_string(),
    })
}

/// Change one overlay setting: `part` (with `id` and a bool), `clickThrough`
/// (bool), `viz` (the whole visualizer look) or `colors` (text and progress).
#[tauri::command]
pub fn settings_set(app: AppHandle, key: String, id: Option<String>, value: Value) -> Result<(), String> {
    match key.as_str() {
        "part" => {
            let id = id.ok_or("missing part")?;
            overlay::set_part(&app, &id, value.as_bool().ok_or("expected true or false")?);
        }
        "clickThrough" => overlay::set_click_through(&app, value.as_bool().ok_or("expected true or false")?),
        "viz" => {
            let viz: VizSettings = serde_json::from_value(value).map_err(|e| e.to_string())?;
            overlay::set_viz(&app, viz);
        }
        "colors" => {
            let colors: PartColors = serde_json::from_value(value).map_err(|e| e.to_string())?;
            overlay::set_colors(&app, colors);
        }
        _ => return Err(format!("unknown setting {key}")),
    }
    Ok(())
}
