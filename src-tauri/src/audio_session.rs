//! Names Oxide in the Windows volume mixer. Sound is played by WebView2's
//! audio process, so Windows would otherwise list it as "Microsoft Edge
//! WebView2". Whenever playback starts, Oxide finds the audio sessions that
//! belong to its own WebView2 processes and gives them its name and icon.

/// Label Oxide's audio sessions, shortly after playback starts (the session
/// only exists once sound is playing).
pub fn label_soon() {
    #[cfg(windows)]
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        if let Err(e) = unsafe { windows_impl::label() } {
            log::debug!("couldn't name the audio session: {e}");
        }
    });
}

#[cfg(windows)]
mod windows_impl {
    use std::collections::HashSet;
    use windows::core::{Interface, HSTRING, PCWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Media::Audio::{
        eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::GetCurrentProcessId;

    /// Oxide and every process it started (WebView2's processes included).
    unsafe fn own_processes() -> HashSet<u32> {
        let mut parents: Vec<(u32, u32)> = Vec::new();
        if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            if Process32FirstW(snap, &mut entry).is_ok() {
                loop {
                    parents.push((entry.th32ProcessID, entry.th32ParentProcessID));
                    if Process32NextW(snap, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snap);
        }
        let mut ours = HashSet::from([GetCurrentProcessId()]);
        loop {
            let before = ours.len();
            for &(pid, parent) in &parents {
                if ours.contains(&parent) {
                    ours.insert(pid);
                }
            }
            if ours.len() == before {
                return ours;
            }
        }
    }

    pub unsafe fn label() -> windows::core::Result<()> {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let ours = own_processes();
        let name = HSTRING::from("Oxide");
        let icon = HSTRING::from(match std::env::current_exe() {
            Ok(exe) => format!("{},0", exe.display()),
            Err(_) => String::new(),
        });
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let endpoints = devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        for i in 0..endpoints.GetCount()? {
            let Ok(device) = endpoints.Item(i) else { continue };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
            let Ok(sessions) = manager.GetSessionEnumerator() else { continue };
            for j in 0..sessions.GetCount().unwrap_or(0) {
                let Ok(session) = sessions.GetSession(j) else { continue };
                let Ok(session) = session.cast::<IAudioSessionControl2>() else { continue };
                let Ok(pid) = session.GetProcessId() else { continue };
                if pid == 0 || !ours.contains(&pid) {
                    continue;
                }
                session.SetDisplayName(PCWSTR(name.as_ptr()), std::ptr::null())?;
                if !icon.is_empty() {
                    session.SetIconPath(PCWSTR(icon.as_ptr()), std::ptr::null())?;
                }
            }
        }
        Ok(())
    }
}
