//! OS media integration: Windows SMTC, macOS Now Playing, Linux MPRIS.
//! Media keys and the OS media overlay drive the player through here.

use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewWindow};

use crate::{player::SongInfo, window};

#[derive(Default)]
pub struct Media(Mutex<Option<MediaControls>>);

// SAFETY: MediaControls is created and used only from the main thread
// (every access below goes through `run_on_main_thread`). The Mutex
// exists so Tauri can hold it in managed state.
unsafe impl Send for Media {}
unsafe impl Sync for Media {}

/// (Re)attach media controls to `win`. Called whenever the main window is
/// created, because on Windows the controls are bound to the window handle.
pub fn attach(app: &AppHandle, win: &WebviewWindow) {
    let handle = app.clone();
    let win = win.clone();
    let _ = app.run_on_main_thread(move || {
        attach_now(&handle, &win);
        // Restore what was showing before the window was replaced.
        if let Some(song) = handle.state::<crate::player::PlayerState>().current() {
            set_metadata(&handle, &song);
            set_playback(&handle, &song);
        }
    });
}

fn attach_now(app: &AppHandle, win: &WebviewWindow) {
    #[cfg(target_os = "windows")]
    let hwnd = match win.hwnd() {
        Ok(h) => Some(h.0),
        Err(e) => {
            log::error!("media controls: no window handle: {e}");
            return;
        }
    };
    #[cfg(not(target_os = "windows"))]
    let hwnd = {
        let _ = win;
        None
    };

    let config = PlatformConfig {
        dbus_name: "oxide_music",
        display_name: "Oxide",
        hwnd,
    };
    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            log::error!("media controls unavailable: {e:?}");
            return;
        }
    };
    let handle = app.clone();
    if let Err(e) = controls.attach(move |event| handle_event(&handle, event)) {
        log::error!("media controls attach failed: {e:?}");
        return;
    }
    let state = app.state::<Media>();
    let mut slot = state.0.lock().unwrap();
    if let Some(mut old) = slot.take() {
        let _ = old.detach();
    }
    *slot = Some(controls);
}

fn handle_event(app: &AppHandle, event: MediaControlEvent) {
    use MediaControlEvent::*;
    match event {
        Play => window::page_command(app, "play", None),
        Pause => window::page_command(app, "pause", None),
        Toggle => window::page_command(app, "toggle", None),
        Next => window::page_command(app, "next", None),
        Previous => window::page_command(app, "previous", None),
        Stop => window::page_command(app, "pause", None),
        Seek(dir) => window::page_command(app, "seekBy", Some(seek_amount(dir, 10.0).into())),
        SeekBy(dir, d) => window::page_command(app, "seekBy", Some(seek_amount(dir, d.as_secs_f64()).into())),
        SetPosition(MediaPosition(d)) => window::page_command(app, "seek", Some(d.as_secs_f64().into())),
        SetVolume(v) => window::page_command(app, "volume", Some(v.clamp(0.0, 1.0).into())),
        Raise => window::show(app),
        Quit => app.exit(0),
        _ => {}
    }
}

fn seek_amount(dir: SeekDirection, secs: f64) -> f64 {
    match dir {
        SeekDirection::Forward => secs,
        SeekDirection::Backward => -secs,
    }
}

fn with_controls(app: &AppHandle, f: impl FnOnce(&mut MediaControls) + Send + 'static) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let state = handle.state::<Media>();
        let mut slot = state.0.lock().unwrap();
        if let Some(c) = slot.as_mut() {
            f(c);
        }
    });
}

pub fn set_metadata(app: &AppHandle, song: &SongInfo) {
    let song = song.clone();
    with_controls(app, move |c| {
        let res = c.set_metadata(MediaMetadata {
            title: Some(&song.title),
            artist: Some(&song.artist),
            album: song.album.as_deref(),
            cover_url: song.art_url.as_deref(),
            duration: (song.duration > 0.0).then(|| Duration::from_secs_f64(song.duration)),
        });
        if let Err(e) = res {
            log::warn!("media metadata: {e:?}");
        }
    });
}

pub fn set_playback(app: &AppHandle, song: &SongInfo) {
    let progress = Some(MediaPosition(Duration::from_secs_f64(song.elapsed.max(0.0))));
    let playback = if song.paused {
        MediaPlayback::Paused { progress }
    } else {
        MediaPlayback::Playing { progress }
    };
    with_controls(app, move |c| {
        if let Err(e) = c.set_playback(playback) {
            log::warn!("media playback: {e:?}");
        }
    });
}
