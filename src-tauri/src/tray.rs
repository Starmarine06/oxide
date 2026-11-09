//! System tray icon with playback controls.

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

// Menu clicks are routed by the app-wide handler in lib.rs.
use crate::{player::SongInfo, window};

const TRAY_ID: &str = "main-tray";

/// The "Lyrics on screen" check item, kept so its tick follows the overlay.
struct LyricsItem(CheckMenuItem<Wry>);

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let lyrics = CheckMenuItem::with_id(app, "tray:lyrics", "Lyrics on screen", true, false, None::<&str>)?;
    let items = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "tray:toggle", "Play / Pause", true, None::<&str>)?,
            &MenuItem::with_id(app, "tray:next", "Next", true, None::<&str>)?,
            &MenuItem::with_id(app, "tray:previous", "Previous", true, None::<&str>)?,
            &lyrics,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "tray:show", "Show window", true, None::<&str>)?,
            &MenuItem::with_id(app, "tray:update", "Check for updates", true, None::<&str>)?,
            &MenuItem::with_id(app, "tray:quit", "Quit", true, None::<&str>)?,
        ],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .tooltip("Oxide")
        .menu(&items)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::toggle_visible(tray.app_handle());
            }
        })
        .build(app)?;
    app.manage(LyricsItem(lyrics));
    Ok(())
}

pub fn set_lyrics_checked(app: &AppHandle, checked: bool) {
    if let Some(item) = app.try_state::<LyricsItem>() {
        let _ = item.0.set_checked(checked);
    }
}

pub fn set_tooltip(app: &AppHandle, song: &SongInfo) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        // Windows caps tray tooltips at 127 characters.
        let text: String = format!("{} - {}", song.title, song.artist).chars().take(120).collect();
        let _ = tray.set_tooltip(Some(text));
    }
}
