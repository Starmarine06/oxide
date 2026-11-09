//! Desktop lyrics: a transparent, always-on-top, click-through window
//! near the bottom of the screen that shows the current synced line over
//! whatever else is open. Toggled from the tray menu and the taskbar
//! thumbnail button.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::config::ConfigStore;
use crate::player::{Change, PlayerState, SongInfo};
use crate::plugins::{lyrics, Registry};

const LABEL: &str = "lyrics-overlay";
const HEIGHT: f64 = 150.0;

static OPEN: AtomicBool = AtomicBool::new(false);

/// Lyrics loaded for the overlay, as sent to the page.
static LOADED: Mutex<Option<(String, Value)>> = Mutex::new(None);

pub fn is_open() -> bool {
    OPEN.load(Ordering::SeqCst)
}

pub fn toggle(app: &AppHandle) {
    if is_open() {
        close(app);
    } else {
        open(app);
    }
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

fn open(app: &AppHandle) {
    OPEN.store(true, Ordering::SeqCst);
    crate::tray::set_lyrics_checked(app, true);
    // Built off the main thread: creating a WebView2 window from a menu or
    // taskbar handler on the main thread can deadlock on Windows.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if window(&app).is_some() {
            return;
        }
        if let Err(e) = build(&app) {
            log::error!("can't open the lyrics overlay: {e}");
            OPEN.store(false, Ordering::SeqCst);
            crate::tray::set_lyrics_checked(&app, false);
        }
    });
}

fn close(app: &AppHandle) {
    OPEN.store(false, Ordering::SeqCst);
    crate::tray::set_lyrics_checked(app, false);
    if let Some(w) = window(app) {
        let _ = w.destroy();
    }
}

fn build(app: &AppHandle) -> tauri::Result<()> {
    // Bottom centre of the screen the main window is on, above the taskbar.
    let monitor = crate::window::main_window(app)
        .and_then(|w| w.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let (x, y, width) = match &monitor {
        Some(m) => {
            let scale = m.scale_factor();
            let area = m.work_area();
            let pos = area.position.to_logical::<f64>(scale);
            let size = area.size.to_logical::<f64>(scale);
            let width = (size.width * 0.7).min(1200.0);
            (pos.x + (size.width - width) / 2.0, pos.y + size.height - HEIGHT - 24.0, width)
        }
        None => (100.0, 600.0, 1000.0),
    };

    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Lyrics")
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .inner_size(width, HEIGHT)
        .position(x, y)
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Finished {
                let app = webview.app_handle().clone();
                push_all(&app);
            }
        })
        .build()?;
    // Clicks go straight through to whatever is underneath.
    win.set_ignore_cursor_events(true)?;
    let _ = win.set_size(LogicalSize::new(width, HEIGHT));
    let _ = win.set_position(LogicalPosition::new(x, y));
    win.show()?;
    Ok(())
}

fn eval(app: &AppHandle, js: &str) {
    if let Some(w) = window(app) {
        let _ = w.eval(js);
    }
}

fn offset_secs(app: &AppHandle) -> f64 {
    let cfg = app.state::<Registry>().config_by_id(&app.state::<ConfigStore>(), "synced-lyrics");
    cfg.get("offsetMs").and_then(Value::as_f64).unwrap_or(0.0) / 1000.0
}

fn push_clock(app: &AppHandle, song: &SongInfo) {
    let t = song.elapsed + offset_secs(app) - lyrics::shift_for(&song.video_id);
    eval(app, &format!("window.oxide && oxide.sync({t}, {});", song.paused));
}

fn push_lyrics(app: &AppHandle, data: &Value) {
    eval(app, &format!("window.oxide && oxide.set({data});"));
}

/// Send everything to a freshly loaded overlay page.
fn push_all(app: &AppHandle) {
    let Some(song) = app.state::<PlayerState>().current_live() else {
        push_lyrics(app, &json!({ "message": "Nothing playing" }));
        return;
    };
    let loaded = LOADED.lock().unwrap().clone();
    match loaded {
        Some((id, data)) if id == song.video_id => push_lyrics(app, &data),
        _ => load(app, &song),
    }
    push_clock(app, &song);
}

/// Fetch lyrics for `song` and send them to the overlay.
fn load(app: &AppHandle, song: &SongInfo) {
    let placeholder = json!({ "message": format!("{} \u{2013} {}", song.title, song.artist) });
    // Claim the track now so the once-a-second reports don't start
    // another fetch while this one is running.
    *LOADED.lock().unwrap() = Some((song.video_id.clone(), placeholder.clone()));
    push_lyrics(app, &placeholder);
    let app = app.clone();
    let song = song.clone();
    tauri::async_runtime::spawn(async move {
        let result = lyrics::lyrics_get(
            app.clone(),
            song.title.clone(),
            song.artist.clone(),
            song.album.clone(),
            song.duration,
            Some(song.video_id.clone()),
        )
        .await;
        // Messages are shown once, briefly, then the overlay goes quiet.
        let data = match result {
            Ok(Some(l)) if l.synced.is_some() => json!({ "lines": lyrics::parse_lrc(l.synced.as_deref().unwrap_or_default()) }),
            Ok(Some(l)) if l.instrumental => json!({ "message": "\u{266A} Instrumental", "transient": true }),
            Ok(_) => json!({ "message": "No lyrics for this song", "transient": true }),
            Err(e) => {
                log::warn!("overlay lyrics: {e}");
                json!({ "message": "Couldn't load lyrics", "transient": true })
            }
        };
        // The track may have changed while this was loading.
        let current = app.state::<PlayerState>().current().map(|s| s.video_id);
        if current.as_deref() != Some(song.video_id.as_str()) {
            return;
        }
        // A message has had its moment; reopening the overlay stays blank.
        let keep = if data.get("transient").is_some() { json!({}) } else { data.clone() };
        *LOADED.lock().unwrap() = Some((song.video_id.clone(), keep));
        push_lyrics(&app, &data);
        if let Some(live) = app.state::<PlayerState>().current_live() {
            push_clock(&app, &live);
        }
    });
}

/// Re-send the clock, e.g. after the lyrics shift for the song changed.
pub fn resync(app: &AppHandle) {
    if !is_open() {
        return;
    }
    if let Some(song) = app.state::<PlayerState>().current_live() {
        push_clock(app, &song);
    }
}

/// Called from `player_update` on every reported change.
pub fn on_player_change(app: &AppHandle, song: &SongInfo, change: Change) {
    if !is_open() || window(app).is_none() {
        return;
    }
    let stale = LOADED.lock().unwrap().as_ref().map(|(id, _)| id != &song.video_id).unwrap_or(true);
    if change.song || stale {
        load(app, song);
    }
    push_clock(app, song);
}
