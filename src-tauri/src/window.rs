//! The main YouTube Music window: creation, rebuild after plugin
//! changes, geometry persistence and talking to the page.

use serde_json::Value;
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::config::{ConfigStore, Geometry};
use crate::{media, plugins::Registry};

pub const START_URL: &str = "https://music.youtube.com/";
const CORE_SCRIPT: &str = include_str!("../scripts/core.js");
const TITLEBAR_SCRIPT: &str = include_str!("../scripts/titlebar.js");

const CHROME_UA_WINDOWS: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
const CHROME_UA_MAC: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
const CHROME_UA_LINUX: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";

/// Labels are unique per build (`main-1`, `main-2`, ...) because the window
/// is recreated when page plugins change, and a new window must exist
/// before the old one is destroyed or the app would exit.
static GENERATION: AtomicU32 = AtomicU32::new(0);

/// Settings that can only be applied when the webview is created.
#[derive(PartialEq, Clone)]
struct Signature {
    user_agent: bool,
    transparent: bool,
    proxy: Option<String>,
}

static CREATED_WITH: std::sync::Mutex<Option<Signature>> = std::sync::Mutex::new(None);

fn signature(app: &AppHandle) -> Signature {
    let store = app.state::<ConfigStore>();
    let registry = app.state::<Registry>();
    let transparent = registry.get("transparent-player").is_some_and(|p| registry.is_enabled(&store, p.as_ref()));
    Signature {
        user_agent: store.get().options.override_user_agent,
        transparent,
        proxy: crate::plugins::window_plugins::proxy_url(app).map(|u| u.to_string()),
    }
}

/// Rebuild the window if a create-time setting (user agent, transparency,
/// proxy) changed since it was built.
pub fn rebuild_if_needed(app: &AppHandle) {
    let current = CREATED_WITH.lock().unwrap().clone();
    if main_window(app).is_some() && current.as_ref() != Some(&signature(app)) {
        rebuild(app);
    }
}

pub fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    let label = format!("main-{}", GENERATION.load(Ordering::SeqCst));
    app.get_webview_window(&label)
}

fn init_script(app: &AppHandle) -> String {
    let store = app.state::<ConfigStore>();
    let registry = app.state::<Registry>();
    let config = registry.page_config(&store);
    format!(
        "{TITLEBAR_SCRIPT}\nwindow.__PEAR_CONFIG__ = {config};\n{CORE_SCRIPT}\n{plugins}",
        plugins = registry.page_scripts()
    )
}

pub fn create(app: &AppHandle, url: &str, geometry: Option<Geometry>, visible: bool) -> tauri::Result<WebviewWindow> {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let label = format!("main-{generation}");
    let cfg = app.state::<ConfigStore>().get();

    // The offline player is a bundled page; everything else is the website.
    let target = if crate::library::is_offline_url(url) {
        WebviewUrl::App("offline.html".into())
    } else {
        WebviewUrl::External(url.parse().unwrap_or_else(|_| START_URL.parse().unwrap()))
    };
    let mut builder = WebviewWindowBuilder::new(app, &label, target)
        .title("Oxide")
        .min_inner_size(400.0, 300.0)
        .visible(false)
        // One fixed browser profile for every window build, so cookies (and
        // with them the Google sign-in) survive restarts and rebuilds.
        .incognito(false)
        .data_directory(profile_dir(app))
        .initialization_script(init_script(app))
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Finished {
                // The settings baked into the init script can be older than
                // the live ones (or the page's cached copy can be newer than
                // a reset config file), so resend the truth on every load.
                let app = webview.app_handle();
                let cfg = app.state::<Registry>().page_config(&app.state::<ConfigStore>());
                let _ = webview.eval(format!("window.__pear && window.__pear.setConfig({cfg});"));
                crate::overlay::on_main_page_load(app);
            }
        });

    // Frameless everywhere: the page draws its own title bar with the menus.
    // macOS keeps its traffic lights over the page.
    #[cfg(target_os = "macos")]
    {
        builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
    }
    #[cfg(not(target_os = "macos"))]
    {
        builder = builder.decorations(false);
    }

    // Forget a saved position that's no longer on any screen (e.g. a
    // monitor was unplugged) so the window opens centred instead.
    let geometry = geometry.filter(|g| on_screen(app, g));
    builder = match geometry {
        Some(g) => builder.inner_size(g.width, g.height).position(g.x, g.y).maximized(g.maximized),
        None => builder.inner_size(1100.0, 750.0).center(),
    };

    #[cfg(target_os = "windows")]
    {
        // Keep Tauri's defaults, and turn off WebView2's own media-key
        // handling so the OS overlay is driven by our media controls only.
        builder = builder.additional_browser_args(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,HardwareMediaKeyHandling,MediaSessionService \
             --autoplay-policy=no-user-gesture-required",
        );
    }

    if cfg.options.override_user_agent {
        let ua = if cfg!(target_os = "windows") {
            CHROME_UA_WINDOWS
        } else if cfg!(target_os = "macos") {
            CHROME_UA_MAC
        } else {
            CHROME_UA_LINUX
        };
        builder = builder.user_agent(ua);
    }

    let sig = signature(app);
    if sig.transparent {
        builder = builder.transparent(true);
    }
    if let Some(proxy) = crate::plugins::window_plugins::proxy_url(app) {
        builder = builder.proxy_url(proxy);
    }
    *CREATED_WITH.lock().unwrap() = Some(sig.clone());

    let win = builder.build()?;
    let handle = app.clone();
    let win_label = label.clone();
    win.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            // Ignore events from a window that is being replaced.
            if main_window(&handle).map(|w| w.label() == win_label) != Some(true) {
                return;
            }
            if handle.state::<ConfigStore>().get().options.hide_to_tray {
                api.prevent_close();
                if let Some(w) = main_window(&handle) {
                    let _ = w.hide();
                }
            } else {
                save_geometry(&handle);
                handle.exit(0);
            }
        }
    });

    media::attach(app, &win);
    // Per-window extras that plugins attach to the native window.
    crate::plugins::taskbar::reattach(app);
    if sig.transparent {
        let registry = app.state::<Registry>();
        let store = app.state::<ConfigStore>();
        if let Some(p) = registry.get("transparent-player") {
            p.start(app, &registry.config(&store, p.as_ref()));
        }
    }
    if visible {
        let _ = win.show();
        let _ = win.set_focus();
    }
    Ok(win)
}

/// Rebuild the main window so changed page plugins take effect. Keeps the
/// current page, size, position and visibility.
pub fn rebuild(app: &AppHandle) {
    // Runs off the main thread: building a WebView2 window from inside a
    // menu handler on the main thread can deadlock on Windows.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(old) = main_window(&handle) else { return };
        let url = old.url().map(|u| u.to_string()).unwrap_or_else(|_| START_URL.to_string());
        let geometry = current_geometry(&old);
        let visible = old.is_visible().unwrap_or(true);
        match create(&handle, &url, geometry, visible) {
            Ok(_) => {
                let _ = old.destroy();
            }
            Err(e) => log::error!("failed to rebuild window: {e}"),
        }
    });
}

fn profile_dir(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_local_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("oxide"))
        .join("profile")
}

fn on_screen(app: &AppHandle, g: &Geometry) -> bool {
    let Ok(monitors) = app.available_monitors() else { return true };
    monitors.iter().any(|m| {
        let scale = m.scale_factor();
        let pos = m.position().to_logical::<f64>(scale);
        let size = m.size().to_logical::<f64>(scale);
        // The title bar's middle must be visible.
        let (x, y) = (g.x + g.width / 2.0, g.y + 16.0);
        x >= pos.x && x < pos.x + size.width && y >= pos.y && y < pos.y + size.height
    })
}

fn current_geometry(win: &WebviewWindow) -> Option<Geometry> {
    let scale = win.scale_factor().ok()?;
    let maximized = win.is_maximized().unwrap_or(false);
    let minimized = win.is_minimized().unwrap_or(false);
    if minimized {
        // Minimized windows report bogus positions on Windows.
        return win.app_handle().state::<ConfigStore>().get().window;
    }
    let pos: LogicalPosition<f64> = win.outer_position().ok()?.to_logical(scale);
    let size: LogicalSize<f64> = win.inner_size().ok()?.to_logical(scale);
    let previous = win.app_handle().state::<ConfigStore>().get().window;
    // When maximized, keep the restored size from before so un-maximizing
    // after a restart goes back to a normal window.
    if maximized {
        if let Some(mut p) = previous {
            p.maximized = true;
            return Some(p);
        }
    }
    Some(Geometry {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
        maximized,
    })
}

pub fn save_geometry(app: &AppHandle) {
    if let Some(g) = main_window(app).and_then(|w| current_geometry(&w)) {
        app.state::<ConfigStore>().update(|c| c.window = Some(g));
    }
}

pub fn show(app: &AppHandle) {
    if let Some(w) = main_window(app) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn toggle_visible(app: &AppHandle) {
    if let Some(w) = main_window(app) {
        if w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false) {
            let _ = w.hide();
        } else {
            show(app);
        }
    }
}

/// Run JavaScript in the page.
pub fn eval(app: &AppHandle, js: &str) {
    if let Some(w) = main_window(app) {
        if let Err(e) = w.eval(js) {
            log::warn!("eval failed: {e}");
        }
    }
}

/// Send a command to the page bridge in `scripts/core.js`.
pub fn page_command(app: &AppHandle, name: &str, arg: Option<Value>) {
    let name = serde_json::to_string(name).unwrap();
    let arg = serde_json::to_string(&arg.unwrap_or(Value::Null)).unwrap();
    eval(app, &format!("window.__pear && window.__pear.command({name}, {arg});"));
}

/// Fire an event that page plugins can listen for with `pear.on(name, fn)`.
pub fn page_emit(app: &AppHandle, name: &str, payload: &Value) {
    let name = serde_json::to_string(name).unwrap();
    let payload = serde_json::to_string(payload).unwrap();
    eval(app, &format!("window.__pear && window.__pear.emit({name}, {payload});"));
}

/// Window buttons and dragging for the page-drawn title bar.
#[tauri::command]
pub fn window_control(window: tauri::WebviewWindow, action: String) -> Result<Value, String> {
    let r = match action.as_str() {
        "minimize" => window.minimize(),
        "maximize" => {
            if window.is_maximized().unwrap_or(false) {
                window.unmaximize()
            } else {
                window.maximize()
            }
        }
        "close" => window.close(),
        "drag" => window.start_dragging(),
        "state" => Ok(()),
        _ => return Err(format!("unknown action {action}")),
    };
    r.map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "maximized": window.is_maximized().unwrap_or(false) }))
}
