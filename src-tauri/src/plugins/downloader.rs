//! Song downloader built on yt-dlp. Uses yt-dlp from the config, then PATH,
//! then a copy fetched into the app data folder on first use. With ffmpeg
//! on PATH the audio is converted and tagged; without it the original
//! audio stream (usually .m4a or .webm) is saved as-is.
//!
//! Downloads the current song, or every song on the playlist or album page
//! that's open. Each finished song goes into the offline library
//! (`crate::library`) with its lyrics.

use serde::Deserialize;
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
use crate::{library, player::PlayerState, window};

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
                id: "playlist",
                label: "Download this playlist or album",
                accelerator: Some("CmdOrCtrl+Shift+P"),
            },
            MenuAction {
                id: "choose-folder",
                label: "Choose download folder…",
                accelerator: None,
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
            "playlist" => match collection_url(app) {
                Some(url) => start_collection(app.clone(), url),
                None => notify(app, "Open a playlist or album", "Go to a playlist or album page, then try again."),
            },
            "choose-folder" => {
                use tauri_plugin_dialog::DialogExt;
                let start = download_folder(cfg);
                let app = app.clone();
                app.clone().dialog().file().set_title("Where should downloaded songs go?").set_directory(&start).pick_folder(move |picked| {
                    let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return };
                    let shown = path.display().to_string();
                    app.state::<Registry>().set_option(&app, "downloader", "folder", json!(shown));
                    crate::menu::refresh(&app);
                    notify(&app, "Download folder changed", &format!("New downloads go to {shown}. Songs already downloaded stay where they are."));
                });
            }
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

/// The playlist or album page open in the main window, if any.
fn collection_url(app: &AppHandle) -> Option<String> {
    let url = window::main_window(app)?.url().ok()?;
    if url.scheme() != "https" || url.host_str() != Some("music.youtube.com") {
        return None;
    }
    let list = url.query_pairs().find(|(k, _)| k == "list").map(|(_, v)| v.to_string());
    if let Some(id) = list.filter(|id| valid_list_id(id)) {
        return Some(format!("https://music.youtube.com/playlist?list={id}"));
    }
    // Albums open as /browse/MPREb_...
    let path = url.path();
    let id = path.strip_prefix("/browse/")?;
    valid_list_id(id).then(|| format!("https://music.youtube.com/browse/{id}"))
}

fn valid_list_id(id: &str) -> bool {
    (2..=80).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A playlist name made safe as a folder name.
fn folder_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .take(100)
        .collect();
    cleaned.trim().trim_end_matches(['.', ' ']).to_string()
}

pub(crate) fn download_folder(cfg: &Value) -> PathBuf {
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
        let url = format!("https://music.youtube.com/watch?v={video_id}");
        let result = run(&app, &video_id, &url, Some(&label), None, None).await;
        ACTIVE.lock().unwrap().remove(&video_id);
        match result {
            Ok(done) => {
                emit(&app, json!({ "videoId": video_id, "status": "done", "path": done.files.first() }));
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

fn start_collection(app: AppHandle, url: String) {
    if !ACTIVE.lock().unwrap().insert(url.clone()) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        emit(&app, json!({ "videoId": url, "status": "start", "label": "playlist" }));
        let result = download_collection(&app, &url).await;
        ACTIVE.lock().unwrap().remove(&url);
        match result {
            Ok(done) => {
                let got = done.files.len();
                let message = match (got, done.failed) {
                    (0, 0) => "Everything here was already downloaded.".to_string(),
                    (n, 0) => format!("{n} song{} saved for offline listening.", if n == 1 { "" } else { "s" }),
                    (n, f) => format!("{n} saved, {f} couldn't be downloaded."),
                };
                emit(&app, json!({ "videoId": url, "status": "done", "message": message }));
                notify(&app, "Playlist downloaded", &message);
            }
            Err(e) => {
                log::error!("playlist download failed: {e}");
                emit(&app, json!({ "videoId": url, "status": "error", "message": e }));
                notify(&app, "Playlist download failed", &e);
            }
        }
    });
}

#[derive(Deserialize)]
struct PageList {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    items: Vec<PageItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageItem {
    video_id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    artist: Option<String>,
}

/// Download every song on the open playlist or album page. The songs are
/// read from what the player shows, so anything the listener can see
/// (Liked Music, private playlists) works without signing in anywhere, and
/// each song downloads on its own into a folder named after the playlist.
/// The playlist itself is saved for the offline player, plus an .m3u8 for
/// other players. Falls back to yt-dlp's own playlist support if the page
/// can't be read.
async fn download_collection(app: &AppHandle, url: &str) -> Result<Finished, String> {
    emit(app, json!({ "videoId": url, "status": "progress", "percent": 0, "label": "Reading the playlist" }));
    let list: Option<PageList> = crate::bridge::query_async(app, "collection-items", json!({}), std::time::Duration::from_secs(150))
        .await
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .filter(|l: &PageList| !l.items.is_empty());
    let Some(list) = list else {
        log::warn!("couldn't read the playlist from the page; asking yt-dlp for it");
        let mut done = run(app, url, url, None, None, None).await?;
        replace_unavailable(app, url, &mut done, None).await;
        return Ok(done);
    };
    let name = list.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Playlist".to_string());

    let total = list.items.len();
    let mut done = Finished { files: Vec::new(), failed: 0, unavailable: Vec::new() };
    for (n, item) in list.items.iter().enumerate() {
        if !valid_video_id(&item.video_id) || library::has(app, &item.video_id) {
            continue;
        }
        let progress = format!("Song {} of {total}", n + 1);
        let label = match (item.artist.as_deref().map(str::trim), item.title.as_deref().map(str::trim)) {
            (Some(a), Some(t)) if !a.is_empty() && !t.is_empty() => format!("{a} - {t}"),
            (_, Some(t)) => t.to_string(),
            _ => String::new(),
        };
        let watch = format!("https://music.youtube.com/watch?v={}", item.video_id);
        match run(app, &item.video_id, &watch, Some(&label), Some(&name), Some(&progress)).await {
            Ok(one) => done.files.extend(one.files),
            Err(e) => {
                done.failed += 1;
                match unavailable_id(&e) {
                    Some(id) => done.unavailable.push(id),
                    None => log::warn!("{progress} ({}) failed: {e}", item.video_id),
                }
            }
        }
    }
    let swapped = replace_unavailable(app, url, &mut done, Some(&name)).await;

    // The playlist in the page's order, with replacements in their slots.
    let order: Vec<String> = list
        .items
        .iter()
        .map(|i| swapped.iter().find(|(gone, _)| gone == &i.video_id).map(|(_, copy)| copy.clone()).unwrap_or_else(|| i.video_id.clone()))
        .filter(|id| library::has(app, id))
        .collect();
    library::save_playlist(app, &name, &order);
    write_m3u(app, &name, &order);
    Ok(done)
}

/// `<playlist folder>/<name>.m3u8`, so other players can open the playlist.
fn write_m3u(app: &AppHandle, name: &str, order: &[String]) {
    let store = app.state::<ConfigStore>();
    let registry = app.state::<Registry>();
    let cfg = registry.get("downloader").map(|p| registry.config(&store, p.as_ref())).unwrap_or_default();
    let folder = download_folder(&cfg).join(folder_name(name));
    let tracks = library::tracks(app);
    let mut body = String::from("#EXTM3U\n");
    for id in order {
        let Some(t) = tracks.iter().find(|t| &t.video_id == id) else { continue };
        body.push_str(&format!("#EXTINF:{},{} - {}\n{}\n", t.duration.round() as i64, t.artist, t.title, t.file));
    }
    if let Err(e) = std::fs::create_dir_all(&folder).and_then(|_| std::fs::write(folder.join(format!("{}.m3u8", folder_name(name))), body)) {
        log::warn!("can't write the playlist file: {e}");
    }
}

/// Songs whose upload is gone: YouTube Music plays another copy of the same
/// song in their place, so download that one instead. Returns which copy
/// replaced which song.
async fn replace_unavailable(app: &AppHandle, key: &str, done: &mut Finished, collection: Option<&str>) -> Vec<(String, String)> {
    let gone = std::mem::take(&mut done.unavailable);
    let mut swapped = Vec::new();
    for (n, id) in gone.iter().enumerate() {
        let progress = format!("Replacing unavailable songs, {} of {}", n + 1, gone.len());
        emit(app, json!({ "videoId": key, "status": "progress", "percent": 0, "label": progress }));
        let Some((copy, label)) = super::lyrics::playable_copy(id).await else { continue };
        if library::has(app, &copy) {
            done.failed = done.failed.saturating_sub(1);
            swapped.push((id.clone(), copy));
            continue;
        }
        let copy_url = format!("https://music.youtube.com/watch?v={copy}");
        match run(app, &copy, &copy_url, Some(&label), collection, Some(&progress)).await {
            Ok(one) => {
                done.files.extend(one.files);
                done.failed = done.failed.saturating_sub(1);
                swapped.push((id.clone(), copy));
            }
            Err(e) => log::warn!("replacement for {id} failed: {e}"),
        }
    }
    swapped
}

struct Finished {
    files: Vec<String>,
    /// Songs in a playlist that failed (the rest still download).
    failed: usize,
    /// Video ids that failed because their upload is gone.
    unavailable: Vec<String>,
}

/// "ERROR: [youtube] dcTvWAw52fw: Video unavailable..." -> the video id.
fn unavailable_id(line: &str) -> Option<String> {
    if !line.contains("Video unavailable") {
        return None;
    }
    let rest = &line[line.find("[youtube] ")? + "[youtube] ".len()..];
    let id = rest.get(..11)?;
    (valid_video_id(id) && rest[11..].starts_with(':')).then(|| id.to_string())
}

/// `label` is "Artist - Title" for a single song; `None` downloads every
/// song at `url`. `collection` names the playlist a song belongs to (its
/// songs go in a folder of that name); `progress` replaces the progress
/// label ("Song 3 of 14").
async fn run(
    app: &AppHandle,
    key: &str,
    url: &str,
    label: Option<&str>,
    collection: Option<&str>,
    progress: Option<&str>,
) -> Result<Finished, String> {
    let store = app.state::<ConfigStore>();
    let registry = app.state::<Registry>();
    let cfg = registry
        .get("downloader")
        .map(|p| registry.config(&store, p.as_ref()))
        .unwrap_or_default();

    let ytdlp = ensure_ytdlp(app, &cfg, key).await?;
    // Songs from a playlist go in a folder named after it.
    let folder = match collection.map(folder_name).filter(|n| !n.is_empty()) {
        Some(name) => download_folder(&cfg).join(name),
        None => download_folder(&cfg),
    };
    std::fs::create_dir_all(&folder).map_err(|e| format!("can't create {}: {e}", folder.display()))?;
    let format = cfg.get("format").and_then(Value::as_str).unwrap_or("mp3").to_string();
    let ffmpeg = command_works("ffmpeg", "-version");

    let job = Job {
        key: key.to_string(),
        url: url.to_string(),
        name: label.map(file_stem).unwrap_or_else(|| "%(artist,creator,uploader)s - %(title)s".to_string()),
        collection: label.is_none(),
        folder,
        convert_to: ffmpeg.then_some(format),
        collection_name: collection.map(str::to_string),
        progress_label: progress.map(str::to_string),
    };
    let (app, ytdlp) = (app.clone(), ytdlp.to_path_buf());
    tauri::async_runtime::spawn_blocking(move || run_ytdlp(&app, &ytdlp, &job)).await.map_err(|e| e.to_string())?
}

#[derive(Clone)]
struct Job {
    /// Identifies the download in progress events.
    key: String,
    url: String,
    /// Output file name without extension (a yt-dlp template).
    name: String,
    collection: bool,
    folder: PathBuf,
    convert_to: Option<String>,
    /// Playlist or album the song was downloaded from.
    collection_name: Option<String>,
    /// Shown instead of yt-dlp's own item counter.
    progress_label: Option<String>,
}

/// Printed by yt-dlp as each song starts (index, total, title).
const START_PRINT: &str = "before_dl:PEARSTART %(playlist_index|)s\t%(n_entries,playlist_count|)s\t%(title)s";
/// Printed by yt-dlp once per finished file, tab separated.
const ITEM_PRINT: &str = "after_move:PEARITEM %(id)s\t%(title)s\t%(artist,creator,uploader|)s\t%(album|)s\t%(duration|0)s\t%(playlist_title|)s\t%(filepath)s";

/// One finished file, from an `ITEM_PRINT` line.
fn parse_item(line: &str) -> Option<library::Track> {
    let rest = line.strip_prefix("PEARITEM ")?;
    let f: Vec<&str> = rest.splitn(7, '\t').collect();
    if f.len() != 7 || !valid_video_id(f[0]) || f[6].trim().is_empty() {
        return None;
    }
    let opt = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty() && s != "NA");
    Some(library::Track {
        video_id: f[0].to_string(),
        title: f[1].trim().to_string(),
        artist: opt(f[2]).unwrap_or_default(),
        album: opt(f[3]),
        duration: f[4].trim().parse().unwrap_or(0.0),
        collection: opt(f[5]),
        file: f[6].trim().to_string(),
        added: 0,
        lyrics: None,
        cover: None,
        color: None,
        art_checked: false,
    })
}

/// Record a finished download and fetch its lyrics for offline use.
fn register(app: &AppHandle, track: library::Track) {
    let (id, title, artist, album, duration) =
        (track.video_id.clone(), track.title.clone(), track.artist.clone(), track.album.clone(), track.duration);
    library::add(app, track);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let art_app = app.clone();
        let art_id = id.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || library::add_art(&art_app, &art_id)).await;
        if let Some(l) = super::lyrics::for_download(&app, title, artist, album, duration, id.clone()).await {
            library::set_lyrics(&app, &id, l);
        }
    });
}

fn run_ytdlp(app: &AppHandle, ytdlp: &Path, job: &Job) -> Result<Finished, String> {
    let template = job.folder.join(format!("{}.%(ext)s", job.name));
    let mut cmd = command(ytdlp);
    // UTF-8 output, or Windows' legacy code page mangles non-ASCII file
    // names in the paths read back below.
    cmd.env("PYTHONIOENCODING", "utf-8").env("PYTHONUTF8", "1");
    cmd.args(["--encoding", "utf-8", "--newline", "--no-colors", "--progress", "-f", "bestaudio/best"])
        .args(["--progress-template", "download:PEAR %(progress._percent_str)s"])
        .args(["--print", START_PRINT, "--print", ITEM_PRINT])
        .arg("-o")
        .arg(&template);
    if job.collection {
        // Skip songs downloaded before and keep going past ones that fail.
        cmd.args(["--yes-playlist", "--ignore-errors", "--download-archive"]).arg(job.folder.join(".oxide-archive.txt"));
    } else {
        cmd.arg("--no-playlist");
    }
    if let Some(fmt) = &job.convert_to {
        cmd.args(["-x", "--audio-format", fmt, "--audio-quality", "0", "--embed-metadata", "--embed-thumbnail"]);
    }
    cmd.arg(&job.url);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("can't run yt-dlp: {e}"))?;
    let stderr = child.stderr.take().unwrap();
    let err_thread = std::thread::spawn(move || {
        let errors: Vec<String> = BufReader::new(stderr).lines().map_while(Result::ok).filter(|l| l.contains("ERROR")).collect();
        errors
    });

    let mut files = Vec::new();
    let mut last_pct = -1i64;
    let mut item = String::new();
    for line in BufReader::new(child.stdout.take().unwrap()).lines().map_while(Result::ok) {
        if let Some(rest) = line.strip_prefix("PEAR ") {
            if let Ok(pct) = rest.trim().trim_end_matches('%').trim().parse::<f64>() {
                let pct = pct.round() as i64;
                if pct != last_pct {
                    last_pct = pct;
                    let label = job.progress_label.as_deref().unwrap_or(&item);
                    emit(app, json!({ "videoId": job.key, "status": "progress", "percent": pct, "label": label }));
                }
            }
        } else if line.starts_with("PEARITEM ") {
            if let Some(mut track) = parse_item(&line) {
                if track.collection.is_none() {
                    track.collection = job.collection_name.clone();
                }
                files.push(track.file.clone());
                register(app, track);
            }
        } else if let Some(rest) = line.strip_prefix("PEARSTART ") {
            let f: Vec<&str> = rest.splitn(3, '\t').collect();
            item = match (f.first().copied().unwrap_or(""), f.get(1).copied().unwrap_or("")) {
                (i, n) if job.collection && !i.is_empty() && !n.is_empty() => {
                    format!("Song {} of {n}", i.trim_start_matches('0'))
                }
                _ => String::new(),
            };
            last_pct = -1;
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let errors = err_thread.join().unwrap_or_default();
    for e in &errors {
        log::warn!("yt-dlp: {e}");
    }
    let unavailable: Vec<String> = errors.iter().filter_map(|e| unavailable_id(e)).collect();
    // A playlist counts as done when some songs came through, some were gone
    // (they get replaced), or there was nothing new to fetch.
    if status.success() || (job.collection && (!files.is_empty() || !unavailable.is_empty() || errors.is_empty())) {
        Ok(Finished { files, failed: errors.len(), unavailable })
    } else {
        Err(errors.last().cloned().unwrap_or_else(|| format!("yt-dlp exited with {status}")))
    }
}

pub(crate) fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
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
    fn parses_finished_items() {
        let line = "PEARITEM dQw4w9WgXcQ\tNever Gonna Give You Up\tRick Astley\tWhenever You Need Somebody\t213\tMy Mix\tC:\\Music\\Rick Astley - Never Gonna Give You Up.mp3";
        let t = parse_item(line).unwrap();
        assert_eq!(t.video_id, "dQw4w9WgXcQ");
        assert_eq!(t.artist, "Rick Astley");
        assert_eq!(t.album.as_deref(), Some("Whenever You Need Somebody"));
        assert_eq!(t.duration, 213.0);
        assert_eq!(t.collection.as_deref(), Some("My Mix"));
        assert!(t.file.ends_with("Never Gonna Give You Up.mp3"));
        let bare = parse_item("PEARITEM dQw4w9WgXcQ\tSong\t\tNA\tNA\t\tC:\\a.mp3").unwrap();
        assert_eq!(bare.album, None);
        assert_eq!(bare.duration, 0.0);
        assert!(parse_item("PEARITEM --exec=x\ta\tb\tc\t1\t\tf").is_none());
    }

    #[test]
    fn spots_unavailable_songs() {
        let line = "ERROR: [youtube] dcTvWAw52fw: Video unavailable. This video is not available";
        assert_eq!(unavailable_id(line).as_deref(), Some("dcTvWAw52fw"));
        assert_eq!(unavailable_id("ERROR: [youtube] dcTvWAw52fw: Sign in to confirm your age"), None);
        assert_eq!(unavailable_id("Video unavailable"), None);
    }

    #[test]
    fn folder_names() {
        assert_eq!(folder_name("Malayalam cool"), "Malayalam cool");
        assert_eq!(folder_name("Mix: 90s / 00s?"), "Mix_ 90s _ 00s_");
        assert_eq!(folder_name("  dots... "), "dots");
    }

    #[test]
    fn list_ids() {
        assert!(valid_list_id("PLx0sYbCqOb8TBPRdmBHs5Iftvv9TPboYG"));
        assert!(valid_list_id("MPREb_abc123"));
        assert!(!valid_list_id("PL;rm"));
        assert!(!valid_list_id(""));
    }

    #[test]
    fn file_names() {
        assert_eq!(file_stem("Queen - Bohemian Rhapsody"), "Queen - Bohemian Rhapsody");
        assert_eq!(file_stem("AC/DC - What?"), "AC_DC - What_");
        assert_eq!(file_stem("100% Pure. "), "100%% Pure");
        assert_eq!(file_stem("  "), "%(artist,creator,uploader)s - %(title)s");
    }
}
