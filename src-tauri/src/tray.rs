//! System tray icon with playback controls.

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

// Menu clicks are routed by the app-wide handler in lib.rs.
use crate::{overlay, player::SongInfo, window};

const TRAY_ID: &str = "main-tray";

/// The tray menu, built from the current settings so its ticks are right.
fn menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let s = overlay::settings(app);
    let check = |id: &str, label: &str, on: bool| CheckMenuItem::with_id(app, id, label, true, on, None::<&str>);

    let positions = Submenu::with_id(app, "tray:ovl-position", "Position", true)?;
    for (id, label) in overlay::PRESETS {
        positions.append(&check(&format!("ovl:pos:{id}"), label, s.preset.as_deref() == Some(id))?)?;
    }
    let lyrics = Submenu::with_id(app, "tray:ovl", "Lyrics on screen", true)?;
    lyrics.append(&check("ovl:toggle", "Show lyrics on screen", overlay::is_open())?)?;
    lyrics.append(&check("ovl:click-through", "Click-through", s.click_through)?)?;
    lyrics.append(&positions)?;
    lyrics.append(&check("ovl:edit", "Edit layout", overlay::is_editing())?)?;
    lyrics.append(&MenuItem::with_id(app, "ovl:reset-layout", "Reset layout", true, None::<&str>)?)?;
    lyrics.append(&PredefinedMenuItem::separator(app)?)?;
    let mut shown = s.clone();
    for (id, label) in overlay::PARTS {
        let on = overlay::part_flag(&mut shown, id).map(|f| *f).unwrap_or(false);
        lyrics.append(&check(&format!("ovl:show:{id}"), label, on)?)?;
    }

    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "tray:toggle", "Play / Pause", true, None::<&str>)?,
            &MenuItem::with_id(app, "tray:next", "Next", true, None::<&str>)?,
            &MenuItem::with_id(app, "tray:previous", "Previous", true, None::<&str>)?,
            &lyrics,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "tray:show", "Show window", true, None::<&str>)?,
            &MenuItem::with_id(app, "tray:offline", "Offline library", true, None::<&str>)?,
            &MenuItem::with_id(app, "ovl:settings", "Settings…", true, None::<&str>)?,
            &MenuItem::with_id(app, "tray:update", "Check for updates", true, None::<&str>)?,
            &MenuItem::with_id(app, "tray:quit", "Quit", true, None::<&str>)?,
        ],
    )
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .tooltip("Oxide")
        .menu(&menu(app)?)
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
    Ok(())
}

/// Rebuild the menu after the lyrics overlay opened, closed or changed.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    match menu(app) {
        Ok(m) => {
            let _ = tray.set_menu(Some(m));
        }
        Err(e) => log::warn!("can't rebuild the tray menu: {e}"),
    }
}

pub fn set_tooltip(app: &AppHandle, song: &SongInfo) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        // Windows caps tray tooltips at 127 characters.
        let text: String = format!("{} - {}", song.title, song.artist).chars().take(120).collect();
        let _ = tray.set_tooltip(Some(text));
    }
}
