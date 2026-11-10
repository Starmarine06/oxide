//! Desktop lyrics: a transparent, always-on-top layer over the whole screen
//! that shows the current synced line over whatever else is open. Toggled
//! from the tray menu, the title bar menu, the taskbar thumbnail button and
//! the Settings window.
//!
//! Settings (config `overlay`): what it shows besides the line (cover art,
//! title and artist, the next line, a progress bar, a visualizer and its
//! look), where it sits (a preset, or each part placed and turned by hand in
//! edit mode, anywhere on the screen) and whether it takes clicks.
//!
//! The window covers the screen's work area. Clicks pass through it except
//! over the parts when click-through is off, and everywhere while editing: a
//! small watcher follows the cursor and switches that on and off. Visualizer
//! levels are streamed from the player page (`overlay_levels`) only while
//! the overlay wants them.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::config::{ConfigStore, LayoutBox, OverlaySettings, PartColors, PartOpacity, Rect, VizSettings};
use crate::player::{Change, PlayerState, SongInfo};
use crate::plugins::{lyrics, Registry};

const LABEL: &str = "lyrics-overlay";
const DEFAULT_HEIGHT: f64 = 170.0;
const MIN_WIDTH: f64 = 280.0;
const MIN_HEIGHT: f64 = 90.0;
const MARGIN: f64 = 24.0;

/// Position presets, as (id, menu label).
pub const PRESETS: [(&str, &str); 6] = [
    ("top", "Top"),
    ("bottom", "Bottom"),
    ("top-left", "Top left"),
    ("top-right", "Top right"),
    ("bottom-left", "Bottom left"),
    ("bottom-right", "Bottom right"),
];

/// What it can show besides the current line, as (id, menu label).
pub const PARTS: [(&str, &str); 7] = [
    ("cover", "Cover art"),
    ("title", "Song title"),
    ("artist", "Artist"),
    ("lyrics", "Lyrics"),
    ("next", "Next line"),
    ("progress", "Progress bar"),
    ("visualizer", "Visualizer"),
];

/// Parts that can be placed in edit mode.
const LAYOUT_PARTS: [&str; 6] = ["cover", "title", "artist", "lyrics", "progress", "visualizer"];

static OPEN: AtomicBool = AtomicBool::new(false);
/// Edit mode: each part can be moved, resized and turned anywhere on the
/// screen. The window takes clicks everywhere while editing.
static EDITING: AtomicBool = AtomicBool::new(false);
/// The overlay is open with the visualizer on, so the player page should
/// send audio levels.
static LEVELS_WANTED: AtomicBool = AtomicBool::new(false);
/// Where the parts are (window-relative logical px), for hover detection.
static HIT_RECTS: Mutex<Vec<[f64; 4]>> = Mutex::new(Vec::new());
/// Whether the window currently lets clicks through.
static PASSING: AtomicBool = AtomicBool::new(true);
/// Mirror of the click-through setting for the cursor watcher, so it doesn't
/// re-read the config twenty times a second.
static CLICK_THROUGH: AtomicBool = AtomicBool::new(true);

/// Lyrics loaded for the overlay, as sent to the page.
static LOADED: Mutex<Option<(String, Value)>> = Mutex::new(None);

pub fn is_open() -> bool {
    OPEN.load(Ordering::SeqCst)
}

pub fn is_editing() -> bool {
    EDITING.load(Ordering::SeqCst)
}

pub fn settings(app: &AppHandle) -> OverlaySettings {
    app.state::<ConfigStore>().get().overlay
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
    menus_changed(app);
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
            menus_changed(&app);
        }
    });
}

fn close(app: &AppHandle) {
    OPEN.store(false, Ordering::SeqCst);
    EDITING.store(false, Ordering::SeqCst);
    menus_changed(app);
    if let Some(w) = window(app) {
        let _ = w.destroy();
    }
}

/// Open state or settings changed: menus, tray, the Settings window and the
/// level stream.
fn menus_changed(app: &AppHandle) {
    let s = settings(app);
    CLICK_THROUGH.store(s.click_through, Ordering::SeqCst);
    let wanted = is_open() && s.show_visualizer;
    if !LEVELS_WANTED.swap(wanted, Ordering::SeqCst) && wanted {
        kick_levels(app);
    }
    crate::tray::refresh(app);
    crate::menu::refresh(app);
    crate::settings_window::refresh(app);
}

// ---- menu ---------------------------------------------------------------------

/// Handles `ovl:*` ids from the title bar menu, the tray, the Settings
/// window and the overlay's own buttons. Returns false for other ids.
pub fn handle_menu(app: &AppHandle, id: &str) -> bool {
    let Some(rest) = id.strip_prefix("ovl:") else { return false };
    match rest {
        "toggle" => toggle(app),
        "close" => close(app),
        "click-through" => change(app, |s| s.click_through = !s.click_through),
        "edit" => set_editing(app, !is_editing()),
        "reset-layout" => change(app, |s| s.layout = None),
        "settings" => crate::settings_window::open(app),
        _ => {
            if let Some(preset) = rest.strip_prefix("pos:") {
                if PRESETS.iter().any(|(p, _)| *p == preset) {
                    place_preset(app, preset);
                }
            } else if let Some(part) = rest.strip_prefix("show:") {
                change(app, |s| {
                    if let Some(flag) = part_flag(s, part) {
                        *flag = !*flag;
                    }
                });
            }
        }
    }
    true
}

pub fn part_flag<'a>(s: &'a mut OverlaySettings, part: &str) -> Option<&'a mut bool> {
    Some(match part {
        "cover" => &mut s.show_cover,
        "title" => &mut s.show_title,
        "artist" => {
            let title = s.show_title;
            s.show_artist.get_or_insert(title)
        }
        "lyrics" => &mut s.show_lyrics,
        "next" => &mut s.show_next,
        "progress" => &mut s.show_progress,
        "visualizer" => &mut s.show_visualizer,
        _ => return None,
    })
}

/// Change the settings, save them and apply them to the open overlay.
fn change(app: &AppHandle, f: impl FnOnce(&mut OverlaySettings)) {
    app.state::<ConfigStore>().update(|c| f(&mut c.overlay));
    menus_changed(app);
    apply(app);
}

/// Start or finish edit mode, opening the overlay if needed.
fn set_editing(app: &AppHandle, on: bool) {
    EDITING.store(on, Ordering::SeqCst);
    if on && !is_open() {
        open(app);
    }
    menus_changed(app);
    apply(app);
}

/// Bring the open overlay in line with the settings.
fn apply(app: &AppHandle) {
    update_passing(app);
    push_settings(app, &settings(app));
}

/// Layout from the page's edit mode: known parts only, at least a sliver in
/// size, turned no more than half a turn either way, with each part's middle
/// on the screen. (The page keeps the turned outline on screen; a turned
/// part's unturned box can stick out, so that isn't clamped here.)
fn clean_layout(layout: BTreeMap<String, LayoutBox>) -> BTreeMap<String, LayoutBox> {
    layout
        .into_iter()
        .filter(|(id, b)| LAYOUT_PARTS.contains(&id.as_str()) && [b.x, b.y, b.w, b.h, b.rot].iter().all(|v| v.is_finite()))
        .map(|(id, b)| {
            let w = b.w.clamp(0.005, 1.0);
            let h = b.h.clamp(0.005, 1.0);
            let align = b.align.filter(|a| ["left", "center", "right"].contains(&a.as_str()));
            let rot = (b.rot + 180.0).rem_euclid(360.0) - 180.0;
            let size = if b.size.is_finite() { b.size.clamp(0.3, 4.0) } else { 1.0 };
            let (cx, cy) = ((b.x + w / 2.0).clamp(0.0, 1.0), (b.y + h / 2.0).clamp(0.0, 1.0));
            (id, LayoutBox { x: cx - w / 2.0, y: cy - h / 2.0, w, h, rot, flip: b.flip, size, nowrap: b.nowrap, align })
        })
        .collect()
}

/// Save the layout from edit mode (or a dragged one); `done` also leaves
/// edit mode.
#[tauri::command]
pub fn overlay_save_layout(app: AppHandle, layout: Option<BTreeMap<String, LayoutBox>>, done: bool) {
    let layout = layout.map(clean_layout).filter(|l| !l.is_empty());
    app.state::<ConfigStore>().update(|c| {
        c.overlay.layout = layout;
        c.overlay.screen_layout = true;
        c.overlay.monitor_layout = true;
        c.overlay.preset = None;
    });
    if done {
        set_editing(&app, false);
    } else {
        crate::tray::refresh(&app);
        crate::menu::refresh(&app);
        crate::settings_window::refresh(&app);
    }
}

/// The automatic layout was dragged to a new spot (window-relative logical
/// px, i.e. relative to the screen).
#[tauri::command]
pub fn overlay_save_group(app: AppHandle, x: f64, y: f64, width: f64, height: f64) {
    let Some(areas) = areas(&app) else { return };
    if ![x, y, width, height].iter().all(|v| v.is_finite()) {
        return;
    }
    let rect = Rect { x: areas.screen.0 + x, y: areas.screen.1 + y, width: width.max(MIN_WIDTH), height: height.max(MIN_HEIGHT) };
    let align = align_for(&rect, areas.work);
    change(&app, |s| {
        s.rect = Some(rect);
        s.preset = None;
        s.align = align.to_string();
    });
}

/// Where the parts are, from the page, for hover detection.
#[tauri::command]
pub fn overlay_hit_rects(app: AppHandle, rects: Vec<[f64; 4]>) {
    *HIT_RECTS.lock().unwrap() = rects.into_iter().take(16).filter(|r| r.iter().all(|v| v.is_finite())).collect();
    update_passing(&app);
}

// ---- clicks: through, or caught by the parts ------------------------------------

/// Whether clicks should pass through at the cursor right now.
fn should_pass(app: &AppHandle, w: &WebviewWindow) -> bool {
    if is_editing() {
        return false;
    }
    if CLICK_THROUGH.load(Ordering::SeqCst) {
        return true;
    }
    let (Ok(cursor), Ok(origin), Ok(scale)) = (app.cursor_position(), w.inner_position(), w.scale_factor()) else { return true };
    let (x, y) = ((cursor.x - origin.x as f64) / scale, (cursor.y - origin.y as f64) / scale);
    let pad = 6.0;
    !HIT_RECTS
        .lock()
        .unwrap()
        .iter()
        .any(|r| x >= r[0] - pad && x <= r[0] + r[2] + pad && y >= r[1] - pad && y <= r[1] + r[3] + pad)
}

fn update_passing(app: &AppHandle) {
    let Some(w) = window(app) else { return };
    let pass = should_pass(app, &w);
    if PASSING.swap(pass, Ordering::SeqCst) != pass {
        let _ = w.set_ignore_cursor_events(pass);
    }
}

/// Follow the cursor while the overlay is open and its parts take clicks.
fn watch_cursor(app: AppHandle) {
    std::thread::spawn(move || {
        while is_open() && window(&app).is_some() {
            if !CLICK_THROUGH.load(Ordering::SeqCst) || is_editing() {
                update_passing(&app);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
}

// ---- placement ------------------------------------------------------------------

/// A screen's whole area and its work area (without the taskbar), each as
/// logical (x, y, width, height). The overlay covers the whole screen, so
/// nothing is cut off where the taskbar is (it's hidden in full screen);
/// presets still keep to the work area.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Areas {
    screen: (f64, f64, f64, f64),
    work: (f64, f64, f64, f64),
}

fn areas_of(m: &tauri::Monitor) -> Areas {
    let scale = m.scale_factor();
    let pos = m.position().to_logical::<f64>(scale);
    let size = m.size().to_logical::<f64>(scale);
    let wpos = m.work_area().position.to_logical::<f64>(scale);
    let wsize = m.work_area().size.to_logical::<f64>(scale);
    Areas { screen: (pos.x, pos.y, size.width, size.height), work: (wpos.x, wpos.y, wsize.width, wsize.height) }
}

/// The areas of the screen the overlay is on, else the main window's, else
/// the primary screen.
fn areas(app: &AppHandle) -> Option<Areas> {
    let monitor = window(app)
        .and_then(|w| w.current_monitor().ok().flatten())
        .or_else(|| crate::window::main_window(app).and_then(|w| w.current_monitor().ok().flatten()))
        .or_else(|| app.primary_monitor().ok().flatten())?;
    Some(areas_of(&monitor))
}

/// Where a preset puts the automatic layout, and how its text lines up.
fn preset_rect(area: (f64, f64, f64, f64), preset: &str, height: f64) -> (Rect, &'static str) {
    let (ax, ay, aw, ah) = area;
    let height = height.clamp(MIN_HEIGHT, (ah - 2.0 * MARGIN).max(MIN_HEIGHT));
    let corner = preset.contains('-');
    let width = if corner { (aw * 0.45).min(560.0) } else { (aw * 0.7).min(1200.0) }.max(MIN_WIDTH.min(aw));
    let y = if preset.starts_with("top") { ay + MARGIN } else { ay + ah - height - MARGIN };
    let (x, align) = if preset.ends_with("left") {
        (ax + MARGIN, "left")
    } else if preset.ends_with("right") {
        (ax + aw - width - MARGIN, "right")
    } else {
        (ax + (aw - width) / 2.0, "center")
    };
    (Rect { x, y, width, height }, align)
}

/// Text alignment for a spot on the screen: whichever third its middle is in.
fn align_for(rect: &Rect, area: (f64, f64, f64, f64)) -> &'static str {
    let (ax, _, aw, _) = area;
    let middle = rect.x + rect.width / 2.0;
    if middle < ax + aw / 3.0 {
        "left"
    } else if middle > ax + aw * 2.0 / 3.0 {
        "right"
    } else {
        "center"
    }
}

/// Half the width and height a part covers on screen once turned, in
/// screen fractions; `aspect` is the screen's width over its height.
fn extents(b: &LayoutBox, aspect: f64) -> (f64, f64) {
    let th = b.rot.to_radians();
    let (c, s) = (th.cos().abs(), th.sin().abs());
    ((b.w * c + b.h * s / aspect) / 2.0, (b.w * s * aspect + b.h * c) / 2.0)
}

/// Bounding box (x0, y0, x1, y1) of what a hand-made layout covers on the
/// screen (turned parts by their turned outline), in screen fractions.
fn layout_bounds(layout: &BTreeMap<String, LayoutBox>, aspect: f64) -> Option<(f64, f64, f64, f64)> {
    layout.values().fold(None, |acc, b| {
        let (hx, hy) = extents(b, aspect);
        let (cx, cy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
        let (x0, y0, x1, y1) = (cx - hx, cy - hy, cx + hx, cy + hy);
        Some(match acc {
            None => (x0, y0, x1, y1),
            Some((a, c, d, e)) => (a.min(x0), c.min(y0), d.max(x1), e.max(y1)),
        })
    })
}

/// Move a hand-made layout (screen fractions), as a whole, to a preset's
/// corner or edge of the work area, so it stays clear of the taskbar.
fn shift_layout(layout: &mut BTreeMap<String, LayoutBox>, preset: &str, areas: Areas) {
    let (sx, sy, sw, sh) = areas.screen;
    let aspect = sw / sh;
    let Some((x0, y0, x1, y1)) = layout_bounds(layout, aspect) else { return };
    // The work area in screen fractions.
    let (wx, wy) = ((areas.work.0 - sx) / sw, (areas.work.1 - sy) / sh);
    let (ww, wh) = (areas.work.2 / sw, areas.work.3 / sh);
    let (mx, my) = (MARGIN / sw, MARGIN / sh);
    let (w, h) = (x1 - x0, y1 - y0);
    let tx = if preset.ends_with("left") {
        wx + mx
    } else if preset.ends_with("right") {
        wx + ww - w - mx
    } else {
        wx + (ww - w) / 2.0
    };
    let ty = if preset.starts_with("top") { wy + my } else { wy + wh - h - my };
    let (dx, dy) = (tx.max(0.0) - x0, ty.max(0.0) - y0);
    // Everything moves by the same amount, so the layout keeps its shape.
    for b in layout.values_mut() {
        b.x += dx;
        b.y += dy;
    }
}

fn place_preset(app: &AppHandle, preset: &str) {
    let Some(areas) = areas(app) else { return };
    let height = settings(app).rect.map(|r| r.height).unwrap_or(DEFAULT_HEIGHT);
    let (rect, align) = preset_rect(areas.work, preset, height);
    change(app, |s| {
        s.preset = Some(preset.to_string());
        s.align = align.to_string();
        s.rect = Some(rect);
        if let Some(layout) = s.layout.as_mut() {
            shift_layout(layout, preset, areas);
        }
    });
}

/// Turn an older layout into fractions of the whole screen, so it stays
/// where it was: layouts were once fractions of the overlay's small window
/// (`rect`), then of the screen's work area.
fn upgrade_layout(app: &AppHandle, areas: Areas) {
    let s = settings(app);
    if s.layout.is_none() || (s.screen_layout && s.monitor_layout) {
        return;
    }
    // What the old fractions were of, in logical screen pixels.
    let from = if s.screen_layout {
        areas.work
    } else {
        let r = s.rect.unwrap_or_else(|| preset_rect(areas.work, s.preset.as_deref().unwrap_or("bottom"), DEFAULT_HEIGHT).0);
        (r.x, r.y, r.width, r.height)
    };
    let (sx, sy, sw, sh) = areas.screen;
    app.state::<ConfigStore>().update(|c| {
        if let Some(layout) = c.overlay.layout.as_mut() {
            for b in layout.values_mut() {
                b.x = (from.0 - sx + b.x * from.2) / sw;
                b.y = (from.1 - sy + b.y * from.3) / sh;
                b.w *= from.2 / sw;
                b.h *= from.3 / sh;
            }
        }
        c.overlay.screen_layout = true;
        c.overlay.monitor_layout = true;
    });
}

/// The screen the overlay was on (its saved spot), else the main window's.
fn target_areas(app: &AppHandle) -> Areas {
    let s = settings(app);
    let monitors = app.available_monitors().unwrap_or_default();
    let saved = s.rect.and_then(|r| {
        monitors.into_iter().find(|m| {
            let scale = m.scale_factor();
            let pos = m.position().to_logical::<f64>(scale);
            let size = m.size().to_logical::<f64>(scale);
            let (cx, cy) = (r.x + r.width / 2.0, r.y + r.height / 2.0);
            cx >= pos.x && cx <= pos.x + size.width && cy >= pos.y && cy <= pos.y + size.height
        })
    });
    let monitor = saved
        .or_else(|| crate::window::main_window(app).and_then(|w| w.current_monitor().ok().flatten()))
        .or_else(|| app.primary_monitor().ok().flatten());
    match monitor {
        Some(m) => areas_of(&m),
        None => Areas { screen: (0.0, 0.0, 1280.0, 720.0), work: (0.0, 0.0, 1280.0, 720.0) },
    }
}

fn build(app: &AppHandle) -> tauri::Result<()> {
    let areas = target_areas(app);
    upgrade_layout(app, areas);
    // The whole screen, taskbar strip included.
    let (x, y, width, height) = areas.screen;
    CLICK_THROUGH.store(settings(app).click_through, Ordering::SeqCst);

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
        .inner_size(width, height)
        .position(x, y)
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Finished {
                let app = webview.app_handle().clone();
                push_settings(&app, &settings(&app));
                push_all(&app);
            }
        })
        .build()?;
    PASSING.store(true, Ordering::SeqCst);
    win.set_ignore_cursor_events(true)?;
    let _ = win.set_size(LogicalSize::new(width, height));
    let _ = win.set_position(LogicalPosition::new(x, y));
    win.show()?;
    update_passing(app);
    watch_cursor(app.clone());
    Ok(())
}

// ---- talking to the page ----------------------------------------------------------

fn eval(app: &AppHandle, js: &str) {
    if let Some(w) = window(app) {
        let _ = w.eval(js);
    }
}

fn push_settings(app: &AppHandle, s: &OverlaySettings) {
    let mut value = serde_json::to_value(s).unwrap_or_default();
    value["editing"] = json!(is_editing());
    // The artist follows the title until it's been set on its own.
    value["showArtist"] = json!(s.show_artist.unwrap_or(s.show_title));
    // Where the automatic layout goes, relative to the window (the screen).
    if let Some(areas) = areas(app) {
        let rect = s.rect.unwrap_or_else(|| preset_rect(areas.work, s.preset.as_deref().unwrap_or("bottom"), DEFAULT_HEIGHT).0);
        value["group"] = json!({ "x": rect.x - areas.screen.0, "y": rect.y - areas.screen.1, "w": rect.width, "h": rect.height });
    }
    eval(app, &format!("window.oxide && oxide.settings({value});"));
}

// ---- the Settings window --------------------------------------------------------

/// Everything the Settings window shows about the overlay.
pub fn settings_view(app: &AppHandle) -> Value {
    let s = settings(app);
    let mut shown = serde_json::to_value(&s).unwrap_or_default();
    shown["showArtist"] = json!(s.show_artist.unwrap_or(s.show_title));
    json!({
        "settings": shown,
        "open": is_open(),
        "editing": is_editing(),
        "presets": PRESETS.iter().map(|(id, label)| json!({ "id": id, "label": label })).collect::<Vec<_>>(),
        "parts": PARTS.iter().map(|(id, label)| json!({ "id": id, "label": label })).collect::<Vec<_>>(),
    })
}

/// Visualizer look from the Settings window.
pub fn set_viz(app: &AppHandle, viz: VizSettings) {
    change(app, |s| s.viz = viz.clean());
}

/// Text and progress colours from the Settings window.
pub fn set_colors(app: &AppHandle, colors: PartColors) {
    change(app, |s| s.colors = colors.clean());
}

/// How see-through each part is, from the Settings window.
pub fn set_opacity(app: &AppHandle, opacity: PartOpacity) {
    change(app, |s| s.opacity = opacity.clean());
}

/// A part on or off from the Settings window.
pub fn set_part(app: &AppHandle, part: &str, on: bool) {
    change(app, |s| {
        if let Some(flag) = part_flag(s, part) {
            *flag = on;
        }
    });
}

/// Click-through on or off from the Settings window.
pub fn set_click_through(app: &AppHandle, on: bool) {
    change(app, |s| s.click_through = on);
}

fn offset_secs(app: &AppHandle) -> f64 {
    let cfg = app.state::<Registry>().config_by_id(&app.state::<ConfigStore>(), "synced-lyrics");
    cfg.get("offsetMs").and_then(Value::as_f64).unwrap_or(0.0) / 1000.0
}

fn push_clock(app: &AppHandle, song: &SongInfo) {
    let shift = offset_secs(app) - lyrics::shift_for(&song.video_id);
    // Lyrics run on the shifted clock; the progress bar on the song's own.
    eval(
        app,
        &format!(
            "window.oxide && oxide.sync({}, {}, {}, {});",
            song.elapsed + shift,
            song.paused,
            song.elapsed,
            song.duration
        ),
    );
}

fn push_song(app: &AppHandle, song: &SongInfo) {
    let cover = crate::library::cover_url(app, &song.video_id).or_else(|| song.art_url.clone().filter(|u| !u.is_empty()));
    // The cover's colour, when the song is downloaded (the page measures it
    // from the image otherwise).
    let color = crate::library::color_for(app, &song.video_id);
    let data = json!({ "title": song.title, "artist": song.artist, "cover": cover, "color": color });
    eval(app, &format!("window.oxide && oxide.song({data});"));
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
    push_song(app, &song);
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

/// The main window is switching between YouTube Music and the offline
/// player. The song that was on stops with its page, so the display stops
/// following it and starts fresh with whatever plays next (before, it
/// stayed stuck on the old song's lyrics).
pub fn page_switched(app: &AppHandle) {
    app.state::<PlayerState>().stop_current();
    *LOADED.lock().unwrap() = None;
    if is_open() {
        push_lyrics(app, &json!({}));
        if let Some(song) = app.state::<PlayerState>().current_live() {
            push_clock(app, &song);
        }
    }
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
        push_song(app, song);
        load(app, song);
    } else if change.metadata {
        push_song(app, song);
    }
    push_clock(app, song);
}

// ---- visualizer levels ------------------------------------------------------------

/// Ask the player page to start sending levels (it stops by itself once
/// `overlay_levels` returns false).
fn kick_levels(app: &AppHandle) {
    crate::window::eval(app, "window.__oxideLevels && window.__oxideLevels();");
}

/// The player page loaded; restart the level stream if it's wanted.
pub fn on_main_page_load(app: &AppHandle) {
    if LEVELS_WANTED.load(Ordering::SeqCst) {
        kick_levels(app);
    }
}

/// Audio levels (0-255 per band) from the player page, passed on to the
/// overlay. Returns whether more are wanted.
#[tauri::command]
pub fn overlay_levels(app: AppHandle, levels: Vec<u8>) -> bool {
    if !LEVELS_WANTED.load(Ordering::SeqCst) {
        return false;
    }
    let levels: Vec<u8> = levels.into_iter().take(64).collect();
    eval(&app, &format!("window.oxide && oxide.levels({levels:?});"));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: (f64, f64, f64, f64) = (0.0, 0.0, 1536.0, 816.0);

    #[test]
    fn presets_sit_inside_the_screen() {
        for (preset, _) in PRESETS {
            let (r, _) = preset_rect(AREA, preset, DEFAULT_HEIGHT);
            assert!(r.x >= 0.0 && r.y >= 0.0, "{preset}");
            assert!(r.x + r.width <= AREA.2 && r.y + r.height <= AREA.3, "{preset}");
        }
        let (bottom, align) = preset_rect(AREA, "bottom", DEFAULT_HEIGHT);
        assert_eq!(align, "center");
        assert_eq!(bottom.y + bottom.height, AREA.3 - MARGIN);
        let (corner, align) = preset_rect(AREA, "top-right", DEFAULT_HEIGHT);
        assert_eq!(align, "right");
        assert_eq!(corner.x + corner.width, AREA.2 - MARGIN);
    }

    #[test]
    fn layouts_are_cleaned() {
        let mut l = BTreeMap::new();
        l.insert("cover".to_string(), LayoutBox { x: 0.95, y: -0.2, w: 0.3, h: 0.5, rot: 0.0, flip: false, size: 1.0, nowrap: false, align: None });
        l.insert("lyrics".to_string(), LayoutBox { x: 0.1, y: 0.1, w: 0.5, h: 0.5, rot: 270.0, flip: false, size: 1.0, nowrap: false, align: Some("diagonal".into()) });
        l.insert("evil".to_string(), LayoutBox { x: 0.1, y: 0.1, w: 0.5, h: 0.5, rot: 0.0, flip: false, size: 1.0, nowrap: false, align: None });
        let c = clean_layout(l);
        assert_eq!(c.len(), 2);
        // Middles are kept on the screen: x 0.95 + 0.15 -> middle 1.0; y -0.2 + 0.25 = 0.05 stays.
        assert_eq!(c["cover"], LayoutBox { x: 0.85, y: -0.2, w: 0.3, h: 0.5, rot: 0.0, flip: false, size: 1.0, nowrap: false, align: None });
        assert_eq!(c["lyrics"].align, None);
        assert_eq!(c["lyrics"].rot, -90.0);
    }

    #[test]
    fn presets_move_a_hand_made_layout_as_a_whole() {
        let mut l = BTreeMap::new();
        l.insert("cover".to_string(), LayoutBox { x: 0.4, y: 0.4, w: 0.1, h: 0.1, rot: 0.0, flip: false, size: 1.0, nowrap: false, align: None });
        l.insert("lyrics".to_string(), LayoutBox { x: 0.5, y: 0.45, w: 0.2, h: 0.1, rot: 0.0, flip: false, size: 1.0, nowrap: false, align: None });
        shift_layout(&mut l, "bottom-right", Areas { screen: AREA, work: AREA });
        let (x0, y0, x1, y1) = layout_bounds(&l, AREA.2 / AREA.3).unwrap();
        assert!((x1 - (1.0 - MARGIN / AREA.2)).abs() < 1e-9 && (y1 - (1.0 - MARGIN / AREA.3)).abs() < 1e-9);
        assert!(((x1 - x0) - 0.3).abs() < 1e-9 && ((y1 - y0) - 0.15).abs() < 1e-9);
    }

    #[test]
    fn turned_parts_are_measured_by_their_outline() {
        let aspect = AREA.2 / AREA.3;
        // A wide visualizer turned a quarter turn is as tall as it was wide.
        let b = LayoutBox { x: 0.3, y: 0.4, w: 0.4, h: 0.1, rot: 90.0, flip: false, size: 1.0, nowrap: false, align: None };
        let (hx, hy) = extents(&b, aspect);
        assert!((hx - 0.1 / aspect / 2.0).abs() < 1e-9);
        assert!((hy - 0.4 * aspect / 2.0).abs() < 1e-9);
        // So moving it to the right edge puts its outline there, not its unturned box.
        let mut l = BTreeMap::new();
        l.insert("visualizer".to_string(), b);
        shift_layout(&mut l, "top-right", Areas { screen: AREA, work: AREA });
        let (_, _, x1, _) = layout_bounds(&l, aspect).unwrap();
        assert!((x1 - (1.0 - MARGIN / AREA.2)).abs() < 1e-9);
    }

    #[test]
    fn presets_keep_clear_of_the_taskbar() {
        // 1536x864 screen with a 48px taskbar at the bottom.
        let areas = Areas { screen: (0.0, 0.0, 1536.0, 864.0), work: (0.0, 0.0, 1536.0, 816.0) };
        let mut l = BTreeMap::new();
        l.insert("lyrics".to_string(), LayoutBox { x: 0.3, y: 0.2, w: 0.4, h: 0.1, rot: 0.0, flip: false, size: 1.0, nowrap: false, align: None });
        shift_layout(&mut l, "bottom", areas);
        let bottom_px = (l["lyrics"].y + l["lyrics"].h) * 864.0;
        assert!((bottom_px - (816.0 - MARGIN)).abs() < 1e-6, "{bottom_px}");
    }

    #[test]
    fn alignment_follows_the_screen_third() {
        let at = |x: f64| Rect { x, y: 0.0, width: 200.0, height: 100.0 };
        assert_eq!(align_for(&at(0.0), AREA), "left");
        assert_eq!(align_for(&at(668.0), AREA), "center");
        assert_eq!(align_for(&at(1300.0), AREA), "right");
    }
}
