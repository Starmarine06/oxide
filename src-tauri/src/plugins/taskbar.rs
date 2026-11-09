//! Previous / play-pause / next buttons in the Windows taskbar thumbnail.

use serde_json::Value;
use tauri::AppHandle;

use super::Plugin;
use crate::player::{Change, SongInfo};

pub struct TaskbarMediaControl;

impl Plugin for TaskbarMediaControl {
    fn id(&self) -> &'static str {
        "taskbar-mediacontrol"
    }
    fn name(&self) -> &'static str {
        "Taskbar media controls"
    }
    fn description(&self) -> &'static str {
        "Previous, play/pause and next buttons in the taskbar preview"
    }
    fn supported(&self) -> bool {
        cfg!(target_os = "windows")
    }
    fn start(&self, app: &AppHandle, _cfg: &Value) {
        #[cfg(windows)]
        imp::install(app);
        #[cfg(not(windows))]
        let _ = app;
    }
    fn stop(&self, app: &AppHandle) {
        #[cfg(windows)]
        imp::remove(app);
        #[cfg(not(windows))]
        let _ = app;
    }
    fn on_player_change(&self, app: &AppHandle, song: &SongInfo, change: Change, _cfg: &Value) {
        #[cfg(windows)]
        if change.playback || change.song {
            imp::set_playing(app, !song.paused);
        }
        #[cfg(not(windows))]
        let _ = (app, song, change);
    }
}

/// Re-add the buttons after the main window is rebuilt.
pub fn reattach(app: &AppHandle) {
    #[cfg(windows)]
    imp::reattach(app);
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::OnceLock;
    use tauri::AppHandle;
    use windows::core::{w, Interface};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::{
        DefSubclassProc, ITaskbarList3, RemoveWindowSubclass, SetWindowSubclass, TaskbarList, THBF_ENABLED, THB_FLAGS,
        THB_ICON, THB_TOOLTIP, THUMBBUTTON, THUMBBUTTONFLAGS, THUMBBUTTONMASK,
    };
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconFromResourceEx, HICON, LR_DEFAULTCOLOR, WM_COMMAND};

    use crate::window;

    const ID_PREV: u32 = 0x4F01;
    const ID_PLAY: u32 = 0x4F02;
    const ID_NEXT: u32 = 0x4F03;
    const SUBCLASS_ID: usize = 0x0A1D;
    const THBN_CLICKED: u32 = 0x1800;

    static APP: OnceLock<AppHandle> = OnceLock::new();
    static ACTIVE: AtomicBool = AtomicBool::new(false);
    static PLAYING: AtomicBool = AtomicBool::new(false);

    fn icon(png: &[u8]) -> HICON {
        unsafe { CreateIconFromResourceEx(png, true, 0x0003_0000, 32, 32, LR_DEFAULTCOLOR).unwrap_or_default() }
    }

    fn icons() -> &'static [isize; 4] {
        // HICONs live for the whole process.
        static ICONS: OnceLock<[isize; 4]> = OnceLock::new();
        ICONS.get_or_init(|| {
            [
                icon(include_bytes!("../../icons/taskbar/previous.png")).0 as isize,
                icon(include_bytes!("../../icons/taskbar/play.png")).0 as isize,
                icon(include_bytes!("../../icons/taskbar/pause.png")).0 as isize,
                icon(include_bytes!("../../icons/taskbar/next.png")).0 as isize,
            ]
        })
    }

    fn tip(text: &str) -> [u16; 260] {
        let mut out = [0u16; 260];
        for (i, c) in text.encode_utf16().take(259).enumerate() {
            out[i] = c;
        }
        out
    }

    fn buttons(playing: bool) -> [THUMBBUTTON; 3] {
        let ic = icons();
        let mask = THUMBBUTTONMASK(THB_ICON.0 | THB_TOOLTIP.0 | THB_FLAGS.0);
        let flags: THUMBBUTTONFLAGS = THBF_ENABLED;
        let mk = |id: u32, h: isize, t: &str| THUMBBUTTON {
            dwMask: mask,
            iId: id,
            hIcon: HICON(h as *mut _),
            szTip: tip(t),
            dwFlags: flags,
            ..Default::default()
        };
        [
            mk(ID_PREV, ic[0], "Previous"),
            if playing { mk(ID_PLAY, ic[2], "Pause") } else { mk(ID_PLAY, ic[1], "Play") },
            mk(ID_NEXT, ic[3], "Next"),
        ]
    }

    fn taskbar() -> Option<ITaskbarList3> {
        unsafe {
            let tb: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok()?;
            tb.HrInit().ok()?;
            Some(tb)
        }
    }

    fn hwnd(app: &AppHandle) -> Option<HWND> {
        window::main_window(app).and_then(|w| w.hwnd().ok()).map(|h| HWND(h.0))
    }

    unsafe extern "system" fn subclass(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
        if msg == WM_COMMAND && ((wp.0 >> 16) & 0xFFFF) as u32 == THBN_CLICKED {
            if let Some(app) = APP.get() {
                match (wp.0 & 0xFFFF) as u32 {
                    ID_PREV => window::page_command(app, "previous", None),
                    ID_PLAY => window::page_command(app, "toggle", None),
                    ID_NEXT => window::page_command(app, "next", None),
                    _ => {}
                }
            }
            return LRESULT(0);
        }
        DefSubclassProc(h, msg, wp, lp)
    }

    fn add(app: &AppHandle) {
        let Some(h) = hwnd(app) else { return };
        let Some(tb) = taskbar() else { return };
        unsafe {
            let _ = SetWindowSubclass(h, Some(subclass), SUBCLASS_ID, 0);
            let btns = buttons(PLAYING.load(Ordering::SeqCst));
            // Buttons can only be added once per window; later calls update.
            if tb.ThumbBarAddButtons(h, &btns).is_err() {
                let _ = tb.ThumbBarUpdateButtons(h, &btns);
            }
        }
        let _ = w!("");
        let _ = tb.as_raw();
    }

    pub fn install(app: &AppHandle) {
        let _ = APP.set(app.clone());
        ACTIVE.store(true, Ordering::SeqCst);
        // The taskbar button must exist first, so wait for the window to settle.
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            let a = app.clone();
            let _ = app.run_on_main_thread(move || add(&a));
        });
    }

    pub fn reattach(app: &AppHandle) {
        if ACTIVE.load(Ordering::SeqCst) {
            install(app);
        }
    }

    pub fn remove(app: &AppHandle) {
        ACTIVE.store(false, Ordering::SeqCst);
        let a = app.clone();
        let _ = app.run_on_main_thread(move || {
            let Some(h) = hwnd(&a) else { return };
            unsafe {
                let _ = RemoveWindowSubclass(h, Some(subclass), SUBCLASS_ID);
                // Buttons can't be removed, only hidden.
                if let Some(tb) = taskbar() {
                    let mut btns = buttons(false);
                    for b in &mut btns {
                        b.dwFlags = windows::Win32::UI::Shell::THBF_HIDDEN;
                    }
                    let _ = tb.ThumbBarUpdateButtons(h, &btns);
                }
            }
        });
    }

    pub fn set_playing(app: &AppHandle, playing: bool) {
        PLAYING.store(playing, Ordering::SeqCst);
        if !ACTIVE.load(Ordering::SeqCst) {
            return;
        }
        let a = app.clone();
        let _ = app.run_on_main_thread(move || {
            let Some(h) = hwnd(&a) else { return };
            if let Some(tb) = taskbar() {
                unsafe {
                    let _ = tb.ThumbBarUpdateButtons(h, &buttons(playing));
                }
            }
        });
    }
}
