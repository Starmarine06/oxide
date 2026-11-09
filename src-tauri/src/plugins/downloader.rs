//! Song downloader built on yt-dlp. Uses yt-dlp from the config, then PATH,
//! then a copy fetched into the app data folder on first use. With ffmpeg
//! on PATH the audio is converted and tagged; without it the original
//! audio stream (usually .m4a or .webm) is saved as-is.

use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use super::{MenuAction, Plugin, PluginOption, Registry};
use crate::config::ConfigStore;
use crate::{player::PlayerState, window};

static ACTIVE: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

#[derive(Default)]
pub struct Downloader;

impl Plugin for Downloader {
    fn id(&self) -> &'static str {
        "downloader"
    }
    fn name(&self) -> &'static str {
        "Downloader"
    }
    fn defaults(&self) -> Value {
        json!({ "enabled": true, "folder": null, "format": "mp3", "ytdlpPath": null })
    }
    fn description(&self) -> &'static str {
        "Download songs with yt-dlp"
    }
    fn script(&self) -> Option<&'static str> {
        Some(include_str!("../../scripts/downloader.js"))
    }
    fn options(&self) -> Vec<PluginOption> {
        vec![PluginOption::choice(
            "format",
            "Format (needs ffmpeg)",
            vec![
                (json!("mp3"), "MP3"),
                (json!("m4a"), "M4A (AAC)"),
                (json!("opus"), "Opus"),
                (json!("flac"), "FLAC"),
                (json!("wav"), "WAV"),
            ],
        )]
    }
    fn menu_actions(&self) -> Vec<MenuAction> {
        vec![
            MenuAction {
                id: "current",
                label: "Download current song",
                accelerator: Some("CmdOrCtrl+Shift+D"),
            },
            MenuAction {
                id: "open-folder",
                label: "Open downloads folder",
                accelerator: None,
            },
        ]
    }
    fn on_menu_action(&self, app: &AppHandle, action: &str, cfg: &Value) {
        match action {
            "current" => match app.state::<PlayerState>().current() {
                Some(song) => start(app.clone(), song.video_id, format!("{} - {}", song.artist, song.title)),
                None => notify(app, "Nothing to download", "Play a song first."),
            },
            "open-folder" => {
                let folder = download_folder(cfg);
                let _ = std::fs::create_dir_all(&folder);
                crate::open_path(&folder);
            }
            _ => {}
        }
    }
}

fn valid_video_id(id: &str) -> bool {
    id.len() == 11 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn download_folder(cfg: &Value) -> PathBuf {
    cfg.get("folder")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::audio_dir()
                .or_else(dirs::download_dir)
                .unwrap_or_else(std::env::temp_dir)
                .join("Oxide")
        })
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

fn emit(app: &AppHandle, payload: Value) {
    window::page_emit(app, "download", &payload);
}

/// Called by the download button in the page.
#[tauri::command]
pub fn download(app: AppHandle, video_id: String, label: String) -> Result<(), String> {
    let store = app.state::<ConfigStore>();
    let registry = app.state::<Registry>();
    let enabled = registry.get("downloader").is_some_and(|p| registry.is_enabled(&store, p.as_ref()));
    if !enabled {
        return Err("downloader is disabled".into());
    }
    if !valid_video_id(&video_id) {
        return Err("invalid video id".into());
    }
    start(app.clone(), video_id, label);
    Ok(())
}

fn start(app: AppHandle, video_id: String, label: String) {
    if !valid_video_id(&video_id) {
        return;
    }
    if !ACTIVE.lock().unwrap().insert(video_id.clone()) {
        return; // already downloading
    }
    tauri::async_runtime::spawn(async move {
        emit(&app, json!({ "videoId": video_id, "status": "start", "label": label }));
        let result = run(&app, &video_id, &label).await;
        ACTIVE.lock().unwrap().remove(&video_id);
        match result {
            Ok(path) => {
                emit(&app, json!({ "videoId": video_id, "status": "done", "path": path }));
                notify(&app, "Download finished", &label);
            }
            Err(e) => {
                log::error!("download {video_id} failed: {e}");
                emit(&app, json!({ "videoId": video_id, "status": "error", "message": e }));
                notify(&app, "Download failed", &format!("{label}\n{e}"));
            }
        }
    });
}

async fn run(app: &AppHandle, video_id: &str, label: &str) -> Result<String, String> {
    let store = app.state::<ConfigStore>();
    let registry = app.state::<Registry>();
    let cfg = registry
        .get("downloader")
        .map(|p| registry.config(&store, p.as_ref()))
        .unwrap_or_default();

    let ytdlp = ensure_ytdlp(app, &cfg, video_id).await?;
    let folder = download_folder(&cfg);
    std::fs::create_dir_all(&folder).map_err(|e| format!("can't create {}: {e}", folder.display()))?;
    let format = cfg.get("format").and_then(Value::as_str).unwrap_or("mp3").to_string();
    let ffmpeg = command_works("ffmpeg", "-version");

    let app = app.clone();
    let video_id = video_id.to_string();
    let name = file_stem(label);
    tauri::async_runtime::spawn_blocking(move || run_ytdlp(&app, &ytdlp, &video_id, &folder, &name, ffmpeg.then_some(format.as_str())))
        .await
        .map_err(|e| e.to_string())?
}

fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn command_works(program: &str, arg: &str) -> bool {
    command(program)
        .arg(arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// "Artist - Title" as shown in YouTube Music, made safe as a file name
/// and escaped for yt-dlp's output template. Falls back to yt-dlp's own
/// naming when there is no usable label.
fn file_stem(label: &str) -> String {
    let cleaned: String = label
        .chars()
        .map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .take(150)
        .collect();
    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    if cleaned.is_empty() || cleaned == "-" {
        "%(artist,creator,uploader)s - %(title)s".to_string()
    } else {
        cleaned.replace('%', "%%")
    }
}

fn run_ytdlp(app: &AppHandle, ytdlp: &Path, video_id: &str, folder: &Path, name: &str, convert_to: Option<&str>) -> Result<String, String> {
    let template = folder.join(format!("{name}.%(ext)s"));
    let mut cmd = command(ytdlp);
    cmd.args(["--no-playlist", "--newline", "--no-colors", "--progress", "-f", "bestaudio/best"])
        .args(["--progress-template", "download:PEAR %(progress._percent_str)s"])
        .args(["--print", "after_move:PEARFILE %(filepath)s"])
        .arg("-o")
        .arg(&template);
    if let Some(fmt) = convert_to {
        cmd.args(["-x", "--audio-format", fmt, "--audio-quality", "0", "--embed-metadata", "--embed-thumbnail"]);
    }
    cmd.arg(format!("https://music.youtube.com/watch?v={video_id}"));
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("can't run yt-dlp: {e}"))?;
    let stderr = child.stderr.take().unwrap();
    let err_thread = std::thread::spawn(move || {
        let lines: Vec<String> = BufReader::new(stderr).lines().map_while(Result::ok).collect();
        lines.into_iter().rev().find(|l| l.contains("ERROR")).unwrap_or_default()
    });

    let mut file = String::new();
    let mut last_pct = -1i64;
    for line in BufReader::new(child.stdout.take().unwrap()).lines().map_while(Result::ok) {
        if let Some(rest) = line.strip_prefix("PEAR ") {
            if let Ok(pct) = rest.trim().trim_end_matches('%').trim().parse::<f64>() {
                let pct = pct.round() as i64;
                if pct != last_pct {
                    last_pct = pct;
                    emit(app, json!({ "videoId": video_id, "status": "progress", "percent": pct }));
                }
            }
        } else if let Some(path) = line.strip_prefix("PEARFILE ") {
            file = path.trim().to_string();
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let err = err_thread.join().unwrap_or_default();
    if status.success() {
        Ok(file)
    } else if err.is_empty() {
        Err(format!("yt-dlp exited with {status}"))
    } else {
        Err(err)
    }
}

fn ytdlp_asset() -> &'static str {
    if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "yt-dlp_macos"
    } else if cfg!(target_arch = "aarch64") {
        "yt-dlp_linux_aarch64"
    } else {
        "yt-dlp_linux"
    }
}

async fn ensure_ytdlp(app: &AppHandle, cfg: &Value, video_id: &str) -> Result<PathBuf, String> {
    if let Some(p) = cfg.get("ytdlpPath").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    if command_works("yt-dlp", "--version") {
        return Ok(PathBuf::from("yt-dlp"));
    }
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("bin");
    let local = dir.join(if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" });
    if local.exists() {
        return Ok(local);
    }

    emit(app, json!({ "videoId": video_id, "status": "setup", "message": "Fetching yt-dlp (first download only)" }));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let url = format!("https://github.com/yt-dlp/yt-dlp/releases/latest/download/{}", ytdlp_asset());
    let resp = crate::http().get(&url).send().await.map_err(|e| format!("can't fetch yt-dlp: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("can't fetch yt-dlp: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let tmp = local.with_extension("part");
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, &local).map_err(|e| e.to_string())?;
    Ok(local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_id_validation() {
        assert!(valid_video_id("dQw4w9WgXcQ"));
        assert!(valid_video_id("a-b_c1234XY"));
        assert!(!valid_video_id("--exec=calc"));
        assert!(!valid_video_id("short"));
        assert!(!valid_video_id("dQw4w9WgXc;"));
    }

    #[test]
    fn file_names() {
        assert_eq!(file_stem("Queen - Bohemian Rhapsody"), "Queen - Bohemian Rhapsody");
        assert_eq!(file_stem("AC/DC - What?"), "AC_DC - What_");
        assert_eq!(file_stem("100% Pure. "), "100%% Pure");
        assert_eq!(file_stem("  "), "%(artist,creator,uploader)s - %(title)s");
    }
}
