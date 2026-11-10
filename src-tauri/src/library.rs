//! Downloaded songs and offline mode.
//!
//! Every download is recorded in `library.json` in the app data folder,
//! together with its lyrics, so the offline player (`dist/offline.html`)
//! can list and play the files, and lyrics work without a connection.
//! The main window switches to that page when YouTube Music can't be
//! reached at startup, or from the App and tray menus.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

use crate::plugins::lyrics::Lyrics;
use crate::window;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: Option<String>,
    /// Seconds.
    #[serde(default)]
    pub duration: f64,
    /// Absolute path of the audio file.
    pub file: String,
    /// Unix seconds.
    #[serde(default)]
    pub added: u64,
    /// The playlist or album it was downloaded from, if any.
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub lyrics: Option<Lyrics>,
    /// Cover art pulled out of the audio file, in the app's cache folder.
    #[serde(default)]
    pub cover: Option<String>,
    /// The cover's average colour, `#rrggbb`.
    #[serde(default)]
    pub color: Option<String>,
    /// Cover extraction was tried (it isn't retried on every listing).
    #[serde(default)]
    pub art_checked: bool,
}

/// A downloaded playlist or album, in the order the player showed it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub name: String,
    /// Video ids, in playlist order.
    pub tracks: Vec<String>,
    /// Unix seconds of the last download into it.
    #[serde(default)]
    pub updated: u64,
}

/// Serialises reads and writes of `library.json` and `playlists.json`.
static FILE_LOCK: Mutex<()> = Mutex::new(());

fn library_file(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_data_dir().ok()?.join("library.json"))
}

fn read(app: &AppHandle) -> Vec<Track> {
    let Some(path) = library_file(app) else { return Vec::new() };
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn write(app: &AppHandle, tracks: &[Track]) {
    let Some(path) = library_file(app) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("json.tmp");
    match serde_json::to_string_pretty(tracks) {
        Ok(text) => {
            if std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &path)).is_err() {
                log::warn!("can't save the library to {}", path.display());
            }
        }
        Err(e) => log::warn!("can't save the library: {e}"),
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn playlists_file(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_data_dir().ok()?.join("playlists.json"))
}

/// Saved playlists. Before playlists were saved on their own, a download
/// only remembered its playlist's name on each song; those are rebuilt
/// from the songs.
fn read_playlists(app: &AppHandle, tracks: &[Track]) -> Vec<Playlist> {
    if let Some(saved) = playlists_file(app).and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()) {
        return saved;
    }
    let mut out: Vec<Playlist> = Vec::new();
    let mut by_age: Vec<&Track> = tracks.iter().collect();
    by_age.sort_by_key(|t| t.added);
    for t in by_age {
        let Some(name) = t.collection.as_deref().filter(|n| !n.is_empty()) else { continue };
        match out.iter_mut().find(|p| p.name == name) {
            Some(p) => p.tracks.push(t.video_id.clone()),
            None => out.push(Playlist { name: name.to_string(), tracks: vec![t.video_id.clone()], updated: t.added }),
        }
    }
    out
}

/// Save a downloaded playlist (replacing one with the same name).
pub fn save_playlist(app: &AppHandle, name: &str, order: &[String]) {
    let _guard = FILE_LOCK.lock().unwrap();
    let tracks = read(app);
    let mut lists = read_playlists(app, &tracks);
    let list = Playlist { name: name.to_string(), tracks: order.to_vec(), updated: now() };
    match lists.iter_mut().find(|p| p.name == name) {
        Some(p) => *p = list,
        None => lists.push(list),
    }
    let Some(path) = playlists_file(app) else { return };
    match serde_json::to_string_pretty(&lists) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&path, text) {
                log::warn!("can't save playlists: {e}");
            }
        }
        Err(e) => log::warn!("can't save playlists: {e}"),
    }
}

/// Every downloaded track.
pub fn tracks(app: &AppHandle) -> Vec<Track> {
    let _guard = FILE_LOCK.lock().unwrap();
    read(app)
}

/// Add or replace a track (matched by video id).
pub fn add(app: &AppHandle, mut track: Track) {
    let _guard = FILE_LOCK.lock().unwrap();
    let mut tracks = read(app);
    if track.added == 0 {
        track.added = now();
    }
    match tracks.iter_mut().find(|t| t.video_id == track.video_id) {
        Some(old) => {
            // Keep lyrics found earlier if this copy has none yet.
            if track.lyrics.is_none() {
                track.lyrics = old.lyrics.take();
            }
            *old = track;
        }
        None => tracks.push(track),
    }
    write(app, &tracks);
}

/// Store lyrics for a downloaded track, and write an `.lrc` file next to the
/// audio so other players can show them too.
pub fn set_lyrics(app: &AppHandle, video_id: &str, lyrics: Lyrics) {
    let _guard = FILE_LOCK.lock().unwrap();
    let mut tracks = read(app);
    let Some(track) = tracks.iter_mut().find(|t| t.video_id == video_id) else { return };
    if let Some(lrc) = lyrics.synced.as_deref().filter(|_| !lyrics.estimated) {
        let path = Path::new(&track.file).with_extension("lrc");
        if let Err(e) = std::fs::write(&path, lrc) {
            log::warn!("can't write {}: {e}", path.display());
        }
    }
    track.lyrics = Some(lyrics);
    write(app, &tracks);
}

/// Lyrics saved with a download, for playing without a connection.
pub fn lyrics_for(app: &AppHandle, video_id: &str) -> Option<Lyrics> {
    let _guard = FILE_LOCK.lock().unwrap();
    read(app).into_iter().find(|t| t.video_id == video_id).and_then(|t| t.lyrics)
}

/// The saved cover of a downloaded song, as a URL local pages can load.
pub fn cover_url(app: &AppHandle, video_id: &str) -> Option<String> {
    let cover = {
        let _guard = FILE_LOCK.lock().unwrap();
        read(app).into_iter().find(|t| t.video_id == video_id)?.cover?
    };
    if !Path::new(&cover).is_file() {
        return None;
    }
    let _ = app.asset_protocol_scope().allow_file(&cover);
    Some(asset_url(&cover))
}

/// The average colour of a downloaded song's cover ("#rrggbb").
pub fn color_for(app: &AppHandle, video_id: &str) -> Option<String> {
    let _guard = FILE_LOCK.lock().unwrap();
    read(app).into_iter().find(|t| t.video_id == video_id)?.color
}

/// Already downloaded, with the file still on disk.
pub fn has(app: &AppHandle, video_id: &str) -> bool {
    let _guard = FILE_LOCK.lock().unwrap();
    read(app).iter().any(|t| t.video_id == video_id && Path::new(&t.file).is_file())
}

// ---- serving files to the offline page --------------------------------------------

/// A downloaded song's address on Oxide's `media` protocol.
fn media_url(video_id: &str) -> String {
    let base = if cfg!(any(windows, target_os = "android")) { "http://media.localhost/" } else { "media://localhost/" };
    format!("{base}{video_id}")
}

/// Largest slice sent per request; the player asks for the rest as it goes.
const MEDIA_CHUNK: u64 = 2 * 1024 * 1024;

/// `bytes=a-b` / `bytes=a-` / `bytes=-n` for a file of `size` bytes, as an
/// inclusive (start, end), capped at MEDIA_CHUNK. None if unsatisfiable.
fn byte_range(header: Option<&str>, size: u64) -> Option<(u64, u64)> {
    if size == 0 {
        return None;
    }
    let last = size - 1;
    let (start, end) = match header.and_then(|h| h.trim().strip_prefix("bytes=")) {
        None => (0, last),
        Some(spec) => {
            let spec = spec.split(',').next()?.trim();
            let (a, b) = spec.split_once('-')?;
            if a.is_empty() {
                let n: u64 = b.parse().ok()?;
                (size.saturating_sub(n), last)
            } else {
                let a: u64 = a.parse().ok()?;
                let b = if b.is_empty() { last } else { b.parse::<u64>().ok()?.min(last) };
                (a, b)
            }
        }
    };
    if start > end || start > last {
        return None;
    }
    Some((start, end.min(start + MEDIA_CHUNK - 1)))
}

fn media_type(file: &str) -> &'static str {
    match Path::new(file).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("mp3") => "audio/mpeg",
        Some("m4a") | Some("mp4") | Some("aac") => "audio/mp4",
        Some("opus") | Some("ogg") => "audio/ogg",
        Some("webm") => "audio/webm",
        Some("flac") => "audio/flac",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

/// The `media` protocol: serves downloaded songs by video id, nothing else
/// on disk, with range requests (seeking) and CORS allowed. CORS is what
/// lets the offline player route a song through Web Audio for the
/// visualizer; a cross-origin file without it would play silent there.
pub fn serve_media(app: &AppHandle, request: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    use tauri::http::{header, Response, StatusCode};
    let reply = |status: StatusCode| {
        Response::builder()
            .status(status)
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Vec::new())
            .unwrap_or_default()
    };
    let id = request.uri().path().trim_start_matches('/');
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return reply(StatusCode::NOT_FOUND);
    }
    let file = {
        let _guard = FILE_LOCK.lock().unwrap();
        read(app).into_iter().find(|t| t.video_id == id).map(|t| t.file)
    };
    let Some(file) = file else { return reply(StatusCode::NOT_FOUND) };
    let Ok(mut f) = std::fs::File::open(&file) else { return reply(StatusCode::NOT_FOUND) };
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let range_header = request.headers().get(header::RANGE).and_then(|v| v.to_str().ok());
    let Some((start, end)) = byte_range(range_header, size) else {
        return Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::CONTENT_RANGE, format!("bytes */{size}"))
            .body(Vec::new())
            .unwrap_or_default();
    };
    let mut body = vec![0u8; (end - start + 1) as usize];
    if f.seek(SeekFrom::Start(start)).and_then(|_| f.read_exact(&mut body)).is_err() {
        return reply(StatusCode::INTERNAL_SERVER_ERROR);
    }
    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, media_type(&file))
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"))
        .header(header::CONTENT_LENGTH, body.len().to_string())
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body)
        .unwrap_or_default()
}

/// `convertFileSrc` from the JS API: a URL the webview's asset protocol serves.
fn asset_url(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len() * 3);
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    if cfg!(any(windows, target_os = "android")) {
        format!("http://asset.localhost/{encoded}")
    } else {
        format!("asset://localhost/{encoded}")
    }
}

// ---- cover art ----------------------------------------------------------------

/// Pull the cover embedded in `audio` (downloads embed it when ffmpeg is
/// around) into `covers/<id>.jpg`, and work out its average colour.
fn extract_art(app: &AppHandle, video_id: &str, audio: &str) -> (Option<String>, Option<String>) {
    let Ok(dir) = app.path().app_local_data_dir().map(|d| d.join("covers")) else { return (None, None) };
    let _ = std::fs::create_dir_all(&dir);
    let cover = dir.join(format!("{video_id}.jpg"));
    if !cover.is_file() {
        let ok = crate::plugins::downloader::command("ffmpeg")
            .args(["-y", "-v", "quiet", "-i"])
            .arg(audio)
            .args(["-map", "0:v:0", "-frames:v", "1", "-vf", "scale=600:-2"])
            .arg(&cover)
            .status()
            .is_ok_and(|s| s.success());
        if !ok || !cover.is_file() {
            return (None, None);
        }
    }
    // Shrink to one pixel: its colour is the average.
    let color = crate::plugins::downloader::command("ffmpeg")
        .args(["-v", "quiet", "-i"])
        .arg(&cover)
        .args(["-vf", "scale=1:1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .output()
        .ok()
        .filter(|o| o.stdout.len() >= 3)
        .map(|o| format!("#{:02x}{:02x}{:02x}", o.stdout[0], o.stdout[1], o.stdout[2]));
    (Some(cover.to_string_lossy().into_owned()), color)
}

/// Fill in cover art for tracks that haven't been looked at yet.
fn backfill_art(app: &AppHandle) {
    let todo: Vec<(String, String)> = {
        let _guard = FILE_LOCK.lock().unwrap();
        read(app).into_iter().filter(|t| !t.art_checked && Path::new(&t.file).is_file()).map(|t| (t.video_id, t.file)).collect()
    };
    if todo.is_empty() {
        return;
    }
    let found: Vec<(String, Option<String>, Option<String>)> =
        todo.into_iter().map(|(id, file)| { let (c, k) = extract_art(app, &id, &file); (id, c, k) }).collect();
    let _guard = FILE_LOCK.lock().unwrap();
    let mut tracks = read(app);
    for (id, cover, color) in found {
        if let Some(t) = tracks.iter_mut().find(|t| t.video_id == id) {
            t.cover = cover;
            t.color = color;
            t.art_checked = true;
        }
    }
    write(app, &tracks);
}

/// Look up the cover for a just-downloaded track.
pub fn add_art(app: &AppHandle, video_id: &str) {
    let file = {
        let _guard = FILE_LOCK.lock().unwrap();
        read(app).into_iter().find(|t| t.video_id == video_id).map(|t| t.file)
    };
    let Some(file) = file else { return };
    let (cover, color) = extract_art(app, video_id, &file);
    let _guard = FILE_LOCK.lock().unwrap();
    let mut tracks = read(app);
    if let Some(t) = tracks.iter_mut().find(|t| t.video_id == video_id) {
        t.cover = cover;
        t.color = color;
        t.art_checked = true;
    }
    write(app, &tracks);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackView {
    #[serde(flatten)]
    track: Track,
    src: String,
    cover_src: Option<String>,
}

#[derive(Serialize)]
pub struct LibraryView {
    tracks: Vec<TrackView>,
    playlists: Vec<Playlist>,
}

/// Downloaded tracks whose files still exist (newest first), and the saved
/// playlists, keeping only songs that are still there.
#[tauri::command]
pub async fn offline_library(app: AppHandle) -> LibraryView {
    // Runs ffmpeg for songs downloaded before covers were kept; off the
    // main thread so the window stays responsive.
    let app2 = app.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || backfill_art(&app2)).await;
    let tracks = {
        let _guard = FILE_LOCK.lock().unwrap();
        read(&app)
    };
    let scope = app.asset_protocol_scope();
    let mut out: Vec<TrackView> = tracks
        .into_iter()
        .filter(|t| Path::new(&t.file).is_file())
        .map(|t| {
            // Songs come from the `media` protocol (see `serve_media`).
            let src = media_url(&t.video_id);
            let cover_src = t.cover.as_deref().filter(|c| Path::new(c).is_file()).map(|c| {
                let _ = scope.allow_file(c);
                asset_url(c)
            });
            TrackView { track: t, src, cover_src }
        })
        .collect();
    out.sort_by_key(|v| std::cmp::Reverse(v.track.added));
    let all: Vec<Track> = out.iter().map(|v| v.track.clone()).collect();
    let playlists = {
        let _guard = FILE_LOCK.lock().unwrap();
        read_playlists(&app, &read(&app))
    }
    .into_iter()
    .map(|mut p| {
        p.tracks.retain(|id| all.iter().any(|t| &t.video_id == id));
        p
    })
    .filter(|p| !p.tracks.is_empty())
    .collect();
    LibraryView { tracks: out, playlists }
}

// ---- switching between online and offline ---------------------------------------

/// The offline player page, bundled with the app.
pub fn offline_url() -> tauri::Url {
    let url = if cfg!(any(windows, target_os = "android")) {
        "http://tauri.localhost/offline.html"
    } else {
        "tauri://localhost/offline.html"
    };
    url.parse().expect("valid offline url")
}

pub fn is_offline_url(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.ends_with("/offline.html") && (path.starts_with("http://tauri.localhost/") || path.starts_with("tauri://localhost/"))
}

/// The YouTube Music song that was on when Oxide went offline, and where.
static LEFT_OFF: Mutex<Option<(String, f64)>> = Mutex::new(None);

/// Switch to the offline player. If a song was on, the offline player picks
/// it up at the same point (when it's downloaded), playing if it was playing,
/// and going back online returns to it.
pub fn show_offline(app: &AppHandle) {
    let Some(w) = window::main_window(app) else { return };
    let song = app.state::<crate::player::PlayerState>().current_live().filter(|s| !s.video_id.is_empty());
    let mut url = offline_url();
    if let Some(s) = &song {
        let mut q = url.query_pairs_mut();
        q.append_pair("resume", &s.video_id)
            .append_pair("t", &format!("{:.1}", s.elapsed))
            .append_pair("play", if s.paused { "0" } else { "1" });
        if let Some(v) = s.volume.filter(|v| v.is_finite()) {
            q.append_pair("vol", &format!("{:.0}", v.clamp(0.0, 100.0)))
                .append_pair("muted", if s.muted == Some(true) { "1" } else { "0" });
        }
    }
    // Leaving the offline player for the offline player keeps the old spot.
    if !w.url().is_ok_and(|u| is_offline_url(u.as_str())) {
        *LEFT_OFF.lock().unwrap() = song.map(|s| (s.video_id, s.elapsed));
    }
    crate::overlay::page_switched(app);
    let _ = w.navigate(url);
}

/// The YouTube Music page to return to: a song opened at `time` seconds, or
/// the home page, with the volume to use. core.js applies them once the
/// player is ready.
fn online_url(song: Option<(&str, f64)>, volume: Option<f64>, muted: bool) -> tauri::Url {
    let mut url: tauri::Url = match song {
        Some(_) => "https://music.youtube.com/watch",
        None => window::START_URL,
    }
    .parse()
    .expect("valid YouTube Music url");
    {
        let mut q = url.query_pairs_mut();
        if let Some((id, time)) = song {
            q.append_pair("v", id).append_pair("oxide_t", &format!("{:.1}", time.max(0.0)));
        }
        if let Some(v) = volume.filter(|v| v.is_finite()) {
            q.append_pair("oxide_vol", &format!("{:.0}", v.clamp(0.0, 100.0)))
                .append_pair("oxide_muted", if muted { "1" } else { "0" });
        }
    }
    if url.query() == Some("") {
        url.set_query(None);
    }
    url
}

/// Back to YouTube Music at the offline player's volume, continuing the song
/// the offline player was on, or else the one that was on when the
/// connection dropped.
#[tauri::command]
pub fn go_online(app: AppHandle, video_id: Option<String>, time: Option<f64>, volume: Option<f64>, muted: Option<bool>) {
    let Some(w) = window::main_window(&app) else { return };
    let left_off = LEFT_OFF.lock().unwrap().take();
    let resume = match video_id.filter(|id| !id.is_empty()) {
        Some(id) => Some((id, time.unwrap_or(0.0))),
        None => left_off,
    };
    let url = online_url(resume.as_ref().map(|(id, t)| (id.as_str(), *t)), volume, muted.unwrap_or(false));
    crate::overlay::page_switched(&app);
    let _ = w.navigate(url);
}

/// Can YouTube Music be reached? Blocking; a few seconds at most.
fn reachable() -> bool {
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(5)).build();
    let Ok(client) = client else { return true };
    client.head(window::START_URL).send().is_ok()
}

/// How often the connection is checked during a session.
const CHECK_EVERY: Duration = Duration::from_secs(15);

/// Watch the connection for the whole session. At startup, and whenever
/// YouTube Music stops being reachable, switch to the offline player. A
/// song that's still playing from YouTube Music's buffer is left to play:
/// the switch waits until it's paused, finished or stalled. When the
/// connection comes back the offline page is told, and it decides whether
/// to go back (it won't interrupt a song).
pub fn check_at_startup(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if !reachable() {
            log::info!("YouTube Music unreachable, opening the offline player");
            show_offline(&app);
        }
        let mut failures = 0u32;
        let mut last_elapsed: Option<(String, f64)> = None;
        loop {
            std::thread::sleep(CHECK_EVERY);
            let Some(w) = window::main_window(&app) else { continue };
            let on_offline_page = w.url().is_ok_and(|u| is_offline_url(u.as_str()));
            let up = reachable();
            if on_offline_page {
                failures = 0;
                last_elapsed = None;
                if up {
                    let _ = w.eval("window.oxideBackOnline && window.oxideBackOnline()");
                }
                continue;
            }
            if up {
                failures = 0;
                continue;
            }
            failures += 1;
            // Still audibly playing: position moved since the last check.
            let song = app.state::<crate::player::PlayerState>().current();
            let playing = song.as_ref().is_some_and(|s| {
                !s.paused
                    && last_elapsed
                        .as_ref()
                        .is_none_or(|(id, at)| id != &s.video_id || (s.elapsed - at).abs() > 1.0)
            });
            last_elapsed = song.map(|s| (s.video_id, s.elapsed));
            if failures >= 2 && !playing {
                log::info!("lost the connection, opening the offline player");
                show_offline(&app);
                failures = 0;
                last_elapsed = None;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_urls_encode_paths() {
        let url = asset_url(r"C:\Users\me\Music\Oxide\A & B - Song.mp3");
        let prefix = if cfg!(windows) { "http://asset.localhost/" } else { "asset://localhost/" };
        assert_eq!(url, format!("{prefix}C%3A%5CUsers%5Cme%5CMusic%5COxide%5CA%20%26%20B%20-%20Song.mp3"));
    }

    #[test]
    fn byte_ranges() {
        assert_eq!(byte_range(None, 100), Some((0, 99)));
        assert_eq!(byte_range(Some("bytes=0-"), 100), Some((0, 99)));
        assert_eq!(byte_range(Some("bytes=10-19"), 100), Some((10, 19)));
        assert_eq!(byte_range(Some("bytes=90-500"), 100), Some((90, 99)));
        assert_eq!(byte_range(Some("bytes=-10"), 100), Some((90, 99)));
        assert_eq!(byte_range(Some("bytes=100-"), 100), None);
        assert_eq!(byte_range(Some("bytes=0-"), 0), None);
        let big = 10 * MEDIA_CHUNK;
        assert_eq!(byte_range(Some("bytes=0-"), big), Some((0, MEDIA_CHUNK - 1)));
    }

    #[test]
    fn recognises_the_offline_page() {
        assert!(is_offline_url(offline_url().as_str()));
        assert!(!is_offline_url("https://music.youtube.com/offline.html"));
        assert!(is_offline_url("http://tauri.localhost/offline.html?resume=abc&t=12.0&play=1"));
    }

    #[test]
    fn watch_urls_carry_the_position() {
        assert_eq!(
            online_url(Some(("S1jo8K4kDJc", 170.04)), Some(42.4), true).as_str(),
            "https://music.youtube.com/watch?v=S1jo8K4kDJc&oxide_t=170.0&oxide_vol=42&oxide_muted=1"
        );
        assert_eq!(online_url(None, None, false).as_str(), "https://music.youtube.com/");
        assert_eq!(online_url(None, Some(250.0), false).as_str(), "https://music.youtube.com/?oxide_vol=100&oxide_muted=0");
    }
}
