//! Lyrics for the side panel and the desktop overlay. Fetching happens here
//! in Rust so the page's CSP and CORS rules don't get in the way.
//!
//! Sources, best first:
//! 1. YouTube Music's own timed lyrics for the exact video (Musixmatch, LyricFind)
//! 2. LRCLIB exact match (synced)
//! 3. LRCLIB fuzzy search over title/artist variants (synced)
//! 4. A synced LRCLIB version of a different cut, rescaled to this duration
//! 5. Plain lyrics (YouTube Music or LRCLIB) with estimated line timing
//!
//! Hits are cached in memory, and on disk per video id.

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use tauri::{AppHandle, Manager};

use super::{MenuAction, Plugin, PluginOption};
use crate::window;

pub struct SyncedLyrics;

impl Plugin for SyncedLyrics {
    fn id(&self) -> &'static str {
        "synced-lyrics"
    }
    fn name(&self) -> &'static str {
        "Synced lyrics"
    }
    fn defaults(&self) -> Value {
        json!({ "enabled": true, "offsetMs": 0, "fontSize": 26, "width": null })
    }
    fn description(&self) -> &'static str {
        "Time-synced lyrics from LRCLIB and YouTube Music in a side panel"
    }
    fn script(&self) -> Option<&'static str> {
        Some(include_str!("../../scripts/lyrics.js"))
    }
    fn options(&self) -> Vec<PluginOption> {
        vec![PluginOption::choice(
            "offsetMs",
            "Timing offset",
            vec![
                (json!(-1000), "1 s earlier"),
                (json!(-500), "0.5 s earlier"),
                (json!(0), "None"),
                (json!(500), "0.5 s later"),
                (json!(1000), "1 s later"),
            ],
        )]
    }
    fn menu_actions(&self) -> Vec<MenuAction> {
        vec![MenuAction {
            id: "toggle",
            label: "Show / hide",
            accelerator: Some("CmdOrCtrl+L"),
        }]
    }
    fn on_menu_action(&self, app: &AppHandle, action: &str, _cfg: &Value) {
        if action == "toggle" {
            window::page_command(app, "synced-lyrics:toggle", None);
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    /// LRC text with `[mm:ss.xx]` timestamps, if available.
    pub synced: Option<String>,
    pub plain: Option<String>,
    pub instrumental: bool,
    pub source: String,
    /// The timing was estimated or rescaled, not taken as-is from a source.
    #[serde(default)]
    pub estimated: bool,
    /// Length of the recording the timing was made for, when known.
    #[serde(default)]
    pub source_duration: Option<f64>,
    /// Seconds to delay the timing by for this playback; see `lyrics_lead_in`.
    #[serde(default)]
    pub shift: f64,
}

impl Lyrics {
    /// Only real timings (or a confirmed instrumental) are kept on disk.
    /// Guessed or stretched timing and plain-only lyrics are kept for this
    /// session only, so properly synced lyrics added later replace them.
    fn is_final(&self) -> bool {
        (self.synced.is_some() && !self.estimated) || self.instrumental
    }
}

// ---- LRC helpers ---------------------------------------------------------

/// `[mm:ss.xx] text` lines into `(seconds, text)`, sorted by time. Lines
/// with several stamps appear once per stamp; tags like `[ar:..]` are skipped.
pub fn parse_lrc(lrc: &str) -> Vec<(f64, String)> {
    let mut out = Vec::new();
    for raw in lrc.lines() {
        let mut rest = raw.trim();
        let mut stamps = Vec::new();
        while let Some(body) = rest.strip_prefix('[') {
            let Some(end) = body.find(']') else { break };
            let tag = &body[..end];
            rest = body[end + 1..].trim_start();
            if let Some((m, s)) = tag.split_once(':') {
                if let (Ok(m), Ok(s)) = (m.trim().parse::<f64>(), s.trim().parse::<f64>()) {
                    stamps.push(m * 60.0 + s);
                }
            }
        }
        let text = strip_word_stamps(rest);
        for t in stamps {
            out.push((t, text.clone()));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// Enhanced LRC puts `<mm:ss.xx>` before each word; keep just the words.
fn strip_word_stamps(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('>') {
            Some(j) if after[..j].chars().all(|c| c.is_ascii_digit() || c == ':' || c == '.') && after[..j].contains(':') => {
                rest = &after[j + 1..];
            }
            _ => {
                out.push('<');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn to_lrc(lines: &[(f64, String)]) -> String {
    lines
        .iter()
        .map(|(t, text)| {
            let cs = (t.max(0.0) * 100.0).round() as i64;
            format!("[{:02}:{:02}.{:02}]{text}", cs / 6000, (cs / 100) % 60, cs % 100)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn scale_lrc(lrc: &str, factor: f64) -> String {
    let lines: Vec<_> = parse_lrc(lrc).into_iter().map(|(t, s)| (t * factor, s)).collect();
    to_lrc(&lines)
}

/// Spread plain lyrics over the song. Each line gets time in proportion to
/// how much there is to sing, stanza breaks get a short instrumental gap,
/// and the intro and outro are left empty.
fn estimate_timing(plain: &str, duration: f64) -> Option<String> {
    if duration < 30.0 {
        return None;
    }
    enum Item {
        Line(String, f64),
        Break,
    }
    const BREAK: f64 = 6.0;
    let mut items: Vec<Item> = Vec::new();
    for raw in plain.lines() {
        let text = raw.trim();
        if text.is_empty() {
            if matches!(items.last(), Some(Item::Line(..))) {
                items.push(Item::Break);
            }
            continue;
        }
        let letters = text.chars().filter(|c| c.is_alphanumeric()).count() as f64;
        items.push(Item::Line(text.to_string(), 2.0 + letters / 3.0));
    }
    while matches!(items.last(), Some(Item::Break)) {
        items.pop();
    }
    let total: f64 = items.iter().map(|i| if let Item::Line(_, w) = i { *w } else { BREAK }).sum();
    if total <= 0.0 {
        return None;
    }
    let intro = (duration * 0.07).clamp(4.0, 20.0);
    let outro = (duration * 0.08).clamp(4.0, 25.0);
    let span = duration - intro - outro;
    if span < 15.0 {
        return None;
    }
    let unit = span / total;
    let mut t = intro;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match item {
            Item::Line(text, w) => {
                out.push((t, text));
                t += w * unit;
            }
            Item::Break => {
                out.push((t, String::new()));
                t += BREAK * unit;
            }
        }
    }
    Some(to_lrc(&out))
}

// ---- matching helpers ------------------------------------------------------

/// Strip decorations like "(Official Video)" or "[Remastered 2011]" that
/// stop exact matches.
fn clean_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut depth = 0i32;
    for ch in title.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    let out = out.split(" - ").next().unwrap_or(&out).trim().to_string();
    if out.is_empty() {
        title.trim().to_string()
    } else {
        out
    }
}

fn first_artist(artist: &str) -> String {
    artists(artist).into_iter().next().unwrap_or_else(|| artist.trim().to_string())
}

/// "A, B & C feat. D" -> ["A", "B", "C", "D"].
fn artists(artist: &str) -> Vec<String> {
    let mut s = artist.to_string();
    for sep in [" feat. ", " feat ", " ft. ", " ft ", " x ", " and ", " & ", " / "] {
        s = s.replace(sep, ",");
    }
    let mut out: Vec<String> = Vec::new();
    for a in s.split([',', '&']).map(str::trim).filter(|a| !a.is_empty()) {
        if !out.iter().any(|o| o.eq_ignore_ascii_case(a)) {
            out.push(a.to_string());
        }
    }
    out
}

/// Title spellings worth searching for, most specific first.
fn title_variants(title: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |s: &str| {
        let s = s.trim().trim_matches(|c: char| c == '-' || c == '|' || c == '"').trim();
        if !s.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(s)) {
            out.push(s.to_string());
        }
    };
    let cleaned = clean_title(title);
    add(&cleaned);
    // "Song feat. X" / "Song ft. X" outside brackets.
    let lower = cleaned.to_lowercase();
    for marker in [" feat.", " feat ", " ft.", " ft "] {
        if let Some(i) = lower.find(marker) {
            add(&cleaned[..i]);
        }
    }
    // "Song | Movie Name" and "Song - Remix" style suffixes.
    if let Some((head, _)) = title.split_once(" | ") {
        add(&clean_title(head));
    }
    add(title);
    out
}

/// Lowercase letters and digits only, in any script.
fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

fn names_match(a: &str, b: &str) -> bool {
    let (a, b) = (norm(a), norm(b));
    !a.is_empty() && !b.is_empty() && (a.contains(&b) || b.contains(&a))
}

// ---- LRCLIB ------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibTrack {
    id: Option<i64>,
    track_name: Option<String>,
    artist_name: Option<String>,
    duration: Option<f64>,
    #[serde(default)]
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

impl LrclibTrack {
    fn synced(&self) -> Option<&str> {
        self.synced_lyrics.as_deref().filter(|s| !s.trim().is_empty())
    }
    fn plain(&self) -> Option<&str> {
        self.plain_lyrics.as_deref().filter(|s| !s.trim().is_empty())
    }
}

const LRCLIB: &str = "https://lrclib.net/api";

async fn lrclib_get(title: &str, artist: &str, album: Option<&str>, duration: f64) -> Result<Option<LrclibTrack>, String> {
    let mut query: Vec<(&str, String)> = vec![("track_name", title.to_string()), ("artist_name", artist.to_string())];
    if let Some(a) = album.filter(|a| !a.is_empty()) {
        query.push(("album_name", a.to_string()));
    }
    if duration > 0.0 {
        query.push(("duration", (duration.round() as i64).to_string()));
    }
    let resp = crate::http().get(format!("{LRCLIB}/get")).query(&query).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    Ok(resp.json::<LrclibTrack>().await.ok())
}

async fn lrclib_search(query: &[(&str, &str)]) -> Result<Vec<LrclibTrack>, String> {
    crate::http()
        .get(format!("{LRCLIB}/search"))
        .query(query)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// Run searches for title and artist variants until one gives a synced hit
/// close to the song's duration. Returns every candidate seen.
async fn lrclib_fuzzy(title: &str, artist: &str, duration: f64) -> Result<Vec<LrclibTrack>, String> {
    let titles = title_variants(title);
    let artist_list = artists(artist);
    let mut queries: Vec<Vec<(&str, String)>> = Vec::new();
    for t in &titles {
        for a in artist_list.iter().take(3) {
            queries.push(vec![("track_name", t.clone()), ("artist_name", a.clone())]);
        }
    }
    if let Some(t) = titles.first() {
        if let Some(a) = artist_list.first() {
            queries.push(vec![("q", format!("{t} {a}"))]);
        }
        queries.push(vec![("track_name", t.clone())]);
    }

    let mut seen: HashMap<i64, ()> = HashMap::new();
    let mut all = Vec::new();
    let mut last_err = None;
    let mut ok_any = false;
    for q in queries.iter().take(8) {
        let q: Vec<(&str, &str)> = q.iter().map(|(k, v)| (*k, v.as_str())).collect();
        match lrclib_search(&q).await {
            Ok(results) => {
                ok_any = true;
                for t in results {
                    if let Some(id) = t.id {
                        if seen.insert(id, ()).is_some() {
                            continue;
                        }
                    }
                    all.push(t);
                }
            }
            Err(e) => last_err = Some(e),
        }
        let good = all.iter().any(|t| {
            t.synced().is_some() && relevant(t, &titles, &artist_list) && t.duration.is_some_and(|d| (d - duration).abs() <= 8.0)
        });
        if good {
            break;
        }
    }
    match (ok_any, last_err) {
        (false, Some(e)) => Err(e),
        _ => Ok(all),
    }
}

/// Guard against unrelated hits that only share a word or the artist.
fn relevant(t: &LrclibTrack, titles: &[String], artists: &[String]) -> bool {
    let name = t.track_name.as_deref().unwrap_or("");
    let title_ok = titles.iter().any(|w| names_match(name, w));
    let by = t.artist_name.as_deref().unwrap_or("");
    let artist_ok = artists.is_empty() || artists.iter().any(|a| names_match(by, a));
    title_ok && artist_ok
}

fn score(t: &LrclibTrack, duration: f64, artist: &str) -> f64 {
    let dd = t.duration.map(|d| (d - duration).abs()).unwrap_or(99.0);
    let synced_bonus = if t.synced().is_some() { 0.0 } else { 5.0 };
    let artist_bonus = if t.artist_name.as_deref().is_some_and(|a| names_match(a, artist)) { 0.0 } else { 2.0 };
    dd + synced_bonus + artist_bonus
}

// ---- YouTube Music -------------------------------------------------------------

const YTM: &str = "https://music.youtube.com/youtubei/v1";
const WEB_REMIX: (&str, &str) = ("WEB_REMIX", "1.20250101.01.00");
// The Android app's response carries per-line timings; the web one doesn't.
const ANDROID_MUSIC: (&str, &str) = ("ANDROID_MUSIC", "7.21.50");

async fn ytm_post(endpoint: &str, client: (&str, &str), body: Value) -> Result<Value, String> {
    let mut payload = json!({ "context": { "client": { "clientName": client.0, "clientVersion": client.1, "hl": "en" } } });
    if let (Some(p), Value::Object(extra)) = (payload.as_object_mut(), body) {
        p.extend(extra);
    }
    crate::http()
        .post(format!("{YTM}/{endpoint}?prettyPrint=false"))
        .header("Origin", "https://music.youtube.com")
        .header("Referer", "https://music.youtube.com/")
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// First value stored under `key` anywhere in `v` that passes `ok`.
fn find_key<'a>(v: &'a Value, key: &str, ok: &dyn Fn(&Value) -> bool) -> Option<&'a Value> {
    match v {
        Value::Object(map) => {
            if let Some(found) = map.get(key).filter(|x| ok(x)) {
                return Some(found);
            }
            map.values().find_map(|x| find_key(x, key, ok))
        }
        Value::Array(list) => list.iter().find_map(|x| find_key(x, key, ok)),
        _ => None,
    }
}

fn ms(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.parse().ok())
}

fn ytm_source(v: &Value) -> String {
    let src = find_key(v, "sourceMessage", &|x| x.is_string())
        .and_then(Value::as_str)
        .or_else(|| find_key(v, "footer", &|x| x.get("runs").is_some()).and_then(|f| f["runs"][0]["text"].as_str()))
        .map(|s| s.trim_start_matches("Source:").trim().to_string())
        .filter(|s| !s.is_empty());
    match src {
        Some(s) => format!("YouTube Music · {s}"),
        None => "YouTube Music".to_string(),
    }
}

async fn ytm_lyrics(video_id: String) -> Result<Option<Lyrics>, String> {
    let next = ytm_post("next", WEB_REMIX, json!({ "videoId": video_id })).await?;
    let Some(browse_id) = find_key(&next, "browseId", &|x| x.as_str().is_some_and(|s| s.starts_with("MPLY")))
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return Ok(None);
    };

    // Timed lyrics.
    let timed = ytm_post("browse", ANDROID_MUSIC, json!({ "browseId": browse_id })).await?;
    if let Some(Value::Array(cues)) = find_key(&timed, "timedLyricsData", &|x| x.is_array()) {
        let lines: Vec<(f64, String)> = cues
            .iter()
            .filter_map(|c| {
                let start = ms(&c["cueRange"]["startTimeMilliseconds"])? / 1000.0;
                let text = c["lyricLine"].as_str().unwrap_or("").trim();
                Some((start, if text == "\u{266A}" { String::new() } else { text.to_string() }))
            })
            .collect();
        // The last cue ends where the recording the timing was made for ends.
        let source_duration = cues.last().and_then(|c| ms(&c["cueRange"]["endTimeMilliseconds"])).map(|m| m / 1000.0);
        if lines.iter().any(|(_, t)| !t.is_empty()) {
            return Ok(Some(Lyrics {
                synced: Some(to_lrc(&lines)),
                source: ytm_source(&timed),
                source_duration,
                ..Default::default()
            }));
        }
    }

    // Plain lyrics.
    let plain = ytm_post("browse", WEB_REMIX, json!({ "browseId": browse_id })).await?;
    let text = find_key(&plain, "musicDescriptionShelfRenderer", &|x| x.is_object())
        .and_then(|s| s["description"]["runs"].as_array())
        .map(|runs| runs.iter().filter_map(|r| r["text"].as_str()).collect::<String>())
        .filter(|t| !t.trim().is_empty());
    Ok(text.map(|t| Lyrics {
        plain: Some(t),
        source: ytm_source(&plain),
        ..Default::default()
    }))
}

/// Every distinct video id in a response, in order.
fn video_ids(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            if let Some(Value::String(id)) = map.get("videoId") {
                if !out.contains(id) {
                    out.push(id.clone());
                }
            }
            map.values().for_each(|x| video_ids(x, out));
        }
        Value::Array(list) => list.iter().for_each(|x| video_ids(x, out)),
        _ => {}
    }
}

/// A playable copy of a song whose own upload is gone ("Video unavailable"),
/// found the way YouTube Music does it when such a song is in a playlist:
/// same title and artist, about the same length. Returns its video id and
/// "Artist - Title".
pub(crate) async fn playable_copy(video_id: &str) -> Option<(String, String)> {
    let gone = ytm_post("player", WEB_REMIX, json!({ "videoId": video_id })).await.ok()?;
    let details = &gone["videoDetails"];
    let title = details["title"].as_str()?.to_string();
    let artist = details["author"].as_str().unwrap_or("").to_string();
    let length: f64 = details["lengthSeconds"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);

    // "Songs" filter of the search page.
    let query = json!({ "query": format!("{title} {artist}"), "params": "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D" });
    let results = ytm_post("search", WEB_REMIX, query).await.ok()?;
    let mut ids = Vec::new();
    video_ids(&results, &mut ids);
    for id in ids.into_iter().filter(|id| id != video_id).take(6) {
        let Ok(p) = ytm_post("player", WEB_REMIX, json!({ "videoId": id })).await else { continue };
        if p["playabilityStatus"]["status"].as_str() != Some("OK") {
            continue;
        }
        let d = &p["videoDetails"];
        let (t, a) = (d["title"].as_str().unwrap_or(""), d["author"].as_str().unwrap_or(""));
        let len: f64 = d["lengthSeconds"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        if names_match(t, &title) && (length <= 0.0 || (len - length).abs() <= 10.0) {
            return Some((id, format!("{a} - {t}")));
        }
    }
    None
}

// ---- pipeline ------------------------------------------------------------------

fn from_lrclib(t: &LrclibTrack) -> Lyrics {
    Lyrics {
        synced: t.synced().map(str::to_string),
        plain: t.plain().map(str::to_string),
        instrumental: t.instrumental,
        source: "LRCLIB".to_string(),
        source_duration: t.duration,
        ..Default::default()
    }
}

/// How far into the song the last sung line starts, from 0 to 1. Some
/// uploads stop halfway; anything under 60% counts as incomplete.
fn coverage(l: &Lyrics, duration: f64) -> f64 {
    if duration < 60.0 {
        return 1.0;
    }
    let lines = l.synced.as_deref().map(parse_lrc).unwrap_or_default();
    let last = lines.into_iter().rev().find(|(_, t)| !t.is_empty()).map(|(t, _)| t);
    last.map(|t| t / duration).unwrap_or(0.0)
}

fn complete(l: &Lyrics, duration: f64) -> bool {
    coverage(l, duration) >= 0.6
}

async fn find(title: String, artist: String, album: Option<String>, duration: f64, video_id: Option<String>) -> Result<Option<Lyrics>, String> {
    // YouTube Music runs alongside the LRCLIB lookups.
    let ytm_task = video_id.filter(|v| !v.is_empty()).map(|id| tauri::async_runtime::spawn(ytm_lyrics(id)));
    let mut errors: Vec<String> = Vec::new();

    // LRCLIB exact match, which matches on duration within ~2 seconds.
    let exact = lrclib_get(&title, &artist, album.as_deref(), duration).await.unwrap_or_else(|e| {
        errors.push(e);
        None
    });

    let ytm = match ytm_task {
        Some(task) => match task.await {
            Ok(Ok(l)) => l,
            Ok(Err(e)) => {
                log::warn!("YouTube Music lyrics: {e}");
                errors.push(e);
                None
            }
            Err(e) => {
                errors.push(e.to_string());
                None
            }
        },
        None => None,
    };
    // 1. YouTube Music's timed lyrics are made for this exact recording.
    //    LRCLIB entries are often copied between cuts of different lengths,
    //    so their timing can be off even when the duration "matches".
    //    Synced lyrics that stop early are set aside as a last resort.
    let mut partial: Option<Lyrics> = None;
    let mut keep_partial = |l: Lyrics| {
        if !partial.as_ref().is_some_and(|p| coverage(p, duration) >= coverage(&l, duration)) {
            partial = Some(l);
        }
    };
    if let Some(l) = ytm.as_ref().filter(|l| l.synced.is_some()) {
        if complete(l, duration) {
            return Ok(Some(l.clone()));
        }
        keep_partial(l.clone());
    }

    // 2. LRCLIB exact match.
    if let Some(t) = exact.as_ref().filter(|t| t.synced().is_some()) {
        let l = from_lrclib(t);
        if complete(&l, duration) {
            return Ok(Some(l));
        }
        keep_partial(l);
    }

    // 3. LRCLIB fuzzy search.
    let titles = title_variants(&title);
    let artist_list = artists(&artist);
    let main_artist = first_artist(&artist);
    let candidates: Vec<LrclibTrack> = lrclib_fuzzy(&title, &artist, duration)
        .await
        .unwrap_or_else(|e| {
            errors.push(e);
            Vec::new()
        })
        .into_iter()
        .filter(|t| t.synced().is_some() || t.plain().is_some() || t.instrumental)
        .filter(|t| relevant(t, &titles, &artist_list))
        .collect();
    let by_score = |a: &&LrclibTrack, b: &&LrclibTrack| score(a, duration, &main_artist).total_cmp(&score(b, duration, &main_artist));

    let close: Vec<&LrclibTrack> = candidates
        .iter()
        .filter(|t| t.synced().is_some())
        .filter(|t| duration <= 0.0 || t.duration.is_some_and(|d| (d - duration).abs() <= 8.0))
        .collect();
    if let Some(t) = close.iter().copied().filter(|t| complete(&from_lrclib(t), duration)).min_by(by_score) {
        return Ok(Some(from_lrclib(t)));
    }
    if let Some(t) = close.iter().copied().min_by(by_score) {
        keep_partial(from_lrclib(t));
    }

    // 4. A synced version of a different cut (radio edit, video intro...),
    //    stretched to fit when the lengths are within 15%.
    if duration > 0.0 {
        let scalable = candidates
            .iter()
            .filter(|t| t.synced().is_some())
            .filter(|t| t.duration.is_some_and(|d| d > 0.0 && (duration / d - 1.0).abs() <= 0.15))
            .filter(|t| complete(&from_lrclib(t), t.duration.unwrap_or(duration)))
            .min_by(by_score);
        if let Some(t) = scalable {
            let mut l = from_lrclib(t);
            let factor = duration / t.duration.unwrap_or(duration);
            l.synced = l.synced.map(|s| scale_lrc(&s, factor));
            l.estimated = true;
            l.source_duration = None;
            l.source = "LRCLIB · timing adjusted".to_string();
            return Ok(Some(l));
        }
    }

    // 5. Plain lyrics with estimated timing.
    let plain = ytm
        .as_ref()
        .and_then(|l| l.plain.clone().map(|p| (p, l.source.clone())))
        .or_else(|| exact.as_ref().and_then(|t| t.plain()).map(|p| (p.to_string(), "LRCLIB".to_string())))
        .or_else(|| {
            candidates
                .iter()
                .filter(|t| t.plain().is_some())
                .filter(|t| duration <= 0.0 || t.duration.is_some_and(|d| (d - duration).abs() <= 15.0))
                .min_by(by_score)
                .and_then(|t| t.plain())
                .map(|p| (p.to_string(), "LRCLIB".to_string()))
        });
    if let Some((text, source)) = plain {
        let synced = estimate_timing(&text, duration);
        let estimated = synced.is_some();
        if synced.is_some() || partial.is_none() {
            return Ok(Some(Lyrics {
                synced,
                plain: Some(text),
                source: if estimated { format!("{source} · timing estimated") } else { source },
                estimated,
                ..Default::default()
            }));
        }
    }

    // Real timing that stops early beats nothing.
    if let Some(l) = partial {
        return Ok(Some(l));
    }

    if exact.as_ref().is_some_and(|t| t.instrumental) || candidates.iter().any(|t| t.instrumental && t.duration.is_some_and(|d| (d - duration).abs() <= 8.0)) {
        return Ok(Some(Lyrics {
            instrumental: true,
            source: "LRCLIB".to_string(),
            ..Default::default()
        }));
    }

    // Every source failed outright, as opposed to answering "nothing".
    if errors.len() >= 2 && candidates.is_empty() && exact.is_none() && ytm.is_none() {
        return Err(errors.remove(0));
    }
    Ok(None)
}

// ---- cache -----------------------------------------------------------------------

const CACHE_VERSION: &str = "v4";
static MEMORY: LazyLock<Mutex<HashMap<String, Option<Lyrics>>>> = LazyLock::new(Default::default);

fn cache_key(title: &str, artist: &str, duration: f64, video_id: Option<&str>) -> String {
    let raw = match video_id.filter(|v| !v.is_empty()) {
        Some(v) => format!("id:{v}"),
        None => format!("song:{}|{}|{}", norm(title), norm(artist), duration.round() as i64),
    };
    format!("{CACHE_VERSION}-{:x}", Md5::digest(raw.as_bytes()))
}

fn cache_file(app: &AppHandle, key: &str) -> Option<PathBuf> {
    Some(app.path().app_cache_dir().ok()?.join("lyrics").join(format!("{key}.json")))
}

fn cache_read(app: &AppHandle, key: &str) -> Option<Lyrics> {
    let text = std::fs::read_to_string(cache_file(app, key)?).ok()?;
    serde_json::from_str(&text).ok()
}

fn cache_write(app: &AppHandle, key: &str, lyrics: &Lyrics) {
    let Some(path) = cache_file(app, key) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(lyrics) {
        if let Err(e) = std::fs::write(&path, text) {
            log::warn!("can't cache lyrics: {e}");
        }
    }
}

async fn lookup(app: &AppHandle, title: String, artist: String, album: Option<String>, duration: f64, video_id: Option<String>) -> Result<Option<Lyrics>, String> {
    let key = cache_key(&title, &artist, duration, video_id.as_deref());
    if let Some(hit) = MEMORY.lock().unwrap().get(&key) {
        return Ok(hit.clone());
    }
    if let Some(hit) = cache_read(app, &key) {
        MEMORY.lock().unwrap().insert(key, Some(hit.clone()));
        return Ok(Some(hit));
    }
    // Lyrics saved with a download work without a connection. Estimated
    // ones are only used when the network has nothing better.
    let saved = video_id.as_deref().and_then(|v| crate::library::lyrics_for(app, v));
    if let Some(l) = saved.as_ref().filter(|l| l.is_final()) {
        MEMORY.lock().unwrap().insert(key, Some(l.clone()));
        return Ok(Some(l.clone()));
    }
    let found = match find(title, artist, album, duration, video_id).await {
        Ok(found) => found.or(saved),
        Err(e) => match saved {
            Some(l) => Some(l),
            None => return Err(e),
        },
    };
    match &found {
        Some(l) if l.is_final() => cache_write(app, &key, l),
        // Misses and estimated timing stay in memory only, so better
        // lyrics uploaded later are picked up after a restart.
        _ => {}
    }
    MEMORY.lock().unwrap().insert(key, found.clone());
    Ok(found)
}

/// Lyrics to save with a download: the best available now, estimated
/// timing included, so they work offline later.
pub async fn for_download(app: &AppHandle, title: String, artist: String, album: Option<String>, duration: f64, video_id: String) -> Option<Lyrics> {
    match lookup(app, title, artist, album, duration, Some(video_id)).await {
        Ok(found) => found.filter(|l| l.synced.is_some() || l.plain.is_some() || l.instrumental).map(|mut l| {
            l.shift = 0.0;
            l
        }),
        Err(e) => {
            log::warn!("no lyrics saved for a download: {e}");
            None
        }
    }
}

// ---- lead-in correction --------------------------------------------------------
//
// Lyrics are sometimes timed on another release of the same song, one that
// starts straight away while the playing one opens with a few seconds of
// silence. Every line is then off by the same amount. The page measures how
// long the playing track is silent before the first sound, and when the
// lyrics were made for a recording of a different length, the timing is
// delayed by that much.

/// Per video: how much longer the playing recording is than the one the
/// timing was made for (0 when they match).
static OTHER_CUT: LazyLock<Mutex<HashMap<String, f64>>> = LazyLock::new(Default::default);
/// Per video: seconds of silence before the first sound.
static LEAD_IN: LazyLock<Mutex<HashMap<String, f64>>> = LazyLock::new(Default::default);

/// Seconds the playing recording runs longer than the one the timing was
/// made for, or 0 when they're the same recording (within 1.5 s).
fn other_cut(l: &Lyrics, duration: f64) -> f64 {
    let extra = match l.source_duration {
        Some(s) if !l.estimated && l.synced.is_some() && duration > 0.0 && s > 0.0 => duration - s,
        _ => 0.0,
    };
    if extra > 1.5 { extra } else { 0.0 }
}

/// Seconds to delay the lyrics for `video_id` by. Extra silence at the start
/// can't be longer than the extra length, which also screens out readings
/// taken while the audio wasn't reaching the meter yet.
pub fn shift_for(video_id: &str) -> f64 {
    let extra = OTHER_CUT.lock().unwrap().get(video_id).copied().unwrap_or(0.0);
    if extra <= 0.0 {
        return 0.0;
    }
    LEAD_IN.lock().unwrap().get(video_id).copied().filter(|s| (0.3..=15.0).contains(s) && *s <= extra + 0.3).unwrap_or(0.0)
}

#[tauri::command]
pub async fn lyrics_get(
    app: AppHandle,
    title: String,
    artist: String,
    album: Option<String>,
    duration: f64,
    video_id: Option<String>,
) -> Result<Option<Lyrics>, String> {
    let mut found = lookup(&app, title, artist, album, duration, video_id.clone()).await?;
    if let (Some(v), Some(l)) = (video_id.filter(|v| !v.is_empty()), found.as_mut()) {
        let other = other_cut(l, duration);
        OTHER_CUT.lock().unwrap().insert(v.clone(), other);
        l.shift = shift_for(&v);
    }
    Ok(found)
}

/// The page reports how long the playing track is silent at the start.
/// Returns the shift to apply to that song's lyrics.
#[tauri::command]
pub fn lyrics_lead_in(app: AppHandle, video_id: String, seconds: f64) -> f64 {
    if seconds.is_finite() && seconds >= 0.0 {
        LEAD_IN.lock().unwrap().insert(video_id.clone(), seconds);
    }
    crate::overlay::resync(&app);
    shift_for(&video_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_titles() {
        assert_eq!(clean_title("Song (Official Video)"), "Song");
        assert_eq!(clean_title("Song [Remastered 2011] - Live"), "Song");
        assert_eq!(clean_title("(Intro)"), "(Intro)");
    }

    #[test]
    fn picks_first_artist() {
        assert_eq!(first_artist("A, B & C"), "A");
        assert_eq!(first_artist("A feat. B"), "A");
        assert_eq!(artists("A x B feat. C"), vec!["A", "B", "C"]);
    }

    #[test]
    fn title_variants_strip_extras() {
        let v = title_variants("Song feat. X (From \"Movie\")");
        assert_eq!(v[0], "Song feat. X");
        assert!(v.contains(&"Song".to_string()));
        let v = title_variants("Song | Movie Name");
        assert!(v.contains(&"Song".to_string()));
    }

    #[test]
    fn parses_lrc() {
        let lines = parse_lrc("[ar:Someone]\n[00:12.50]Hello\n[01:02.00][00:05.00]Again\n[00:20.00]\n");
        assert_eq!(
            lines,
            vec![
                (5.0, "Again".to_string()),
                (12.5, "Hello".to_string()),
                (20.0, String::new()),
                (62.0, "Again".to_string()),
            ]
        );
    }

    #[test]
    fn only_real_timing_is_cached_on_disk() {
        let base = Lyrics { synced: Some("[00:01.00]a".into()), source: "x".into(), ..Default::default() };
        assert!(base.is_final());
        assert!(!Lyrics { estimated: true, ..base.clone() }.is_final());
        assert!(!Lyrics { synced: None, plain: Some("a".into()), ..base.clone() }.is_final());
        assert!(Lyrics { synced: None, instrumental: true, ..base }.is_final());
    }

    #[test]
    fn spots_lyrics_timed_on_another_cut() {
        let l = Lyrics { synced: Some("[00:14.49]a".into()), source_duration: Some(147.0), ..Default::default() };
        assert_eq!(other_cut(&l, 154.0), 7.0);
        assert_eq!(other_cut(&l, 147.6), 0.0);
        assert_eq!(other_cut(&l, 140.0), 0.0); // shorter: no lead-in to add
        assert_eq!(other_cut(&Lyrics { estimated: true, ..l.clone() }, 154.0), 0.0);
        assert_eq!(other_cut(&Lyrics { source_duration: None, ..l }, 154.0), 0.0);
    }

    #[test]
    fn shift_needs_both_a_mismatch_and_a_lead_in() {
        OTHER_CUT.lock().unwrap().insert("vidA".into(), 7.0);
        assert_eq!(shift_for("vidA"), 0.0);
        LEAD_IN.lock().unwrap().insert("vidA".into(), 2.94);
        assert_eq!(shift_for("vidA"), 2.94);
        LEAD_IN.lock().unwrap().insert("vidB".into(), 2.94);
        assert_eq!(shift_for("vidB"), 0.0);
        LEAD_IN.lock().unwrap().insert("vidA".into(), 0.05);
        assert_eq!(shift_for("vidA"), 0.0);
        // More silence than extra length: the reading is wrong, not the song.
        OTHER_CUT.lock().unwrap().insert("vidC".into(), 3.0);
        LEAD_IN.lock().unwrap().insert("vidC".into(), 3.32);
        assert_eq!(shift_for("vidC"), 0.0);
    }

    #[test]
    fn coverage_flags_lyrics_that_stop_early() {
        let half = Lyrics { synced: Some("[00:10.00]a\n[01:30.00]b\n[02:00.00]".into()), ..Default::default() };
        assert!(!complete(&half, 200.0));
        assert!(complete(&half, 140.0));
    }

    #[test]
    fn strips_word_stamps() {
        let lines = parse_lrc("[00:18.89]<00:18.89>We're <00:19.25>no <00:20.33>love <3");
        assert_eq!(lines, vec![(18.89, "We're no love <3".to_string())]);
    }

    #[test]
    fn lrc_round_trips_and_scales() {
        let lrc = to_lrc(&[(5.0, "a".into()), (65.25, "b".into())]);
        assert_eq!(lrc, "[00:05.00]a\n[01:05.25]b");
        assert_eq!(parse_lrc(&scale_lrc(&lrc, 2.0)), vec![(10.0, "a".to_string()), (130.5, "b".to_string())]);
    }

    #[test]
    fn estimates_timing_inside_the_song() {
        let plain = "First line here\nSecond line\n\nChorus line one\nChorus line two\n";
        let lines = parse_lrc(&estimate_timing(plain, 200.0).unwrap());
        assert_eq!(lines.len(), 5); // four lines and one stanza gap
        assert!(lines[0].0 >= 4.0 && lines[0].0 <= 20.0);
        assert!(lines.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(lines.last().unwrap().0 < 200.0 - 16.0);
        assert_eq!(lines[2].1, "");
        assert!(estimate_timing(plain, 10.0).is_none());
    }
}

#[cfg(test)]
mod live {
    /// `SONGS=path.json cargo test coverage -- --ignored --nocapture`
    /// with `[[title, artist, duration, videoId], ...]`.
    #[test]
    #[ignore]
    fn coverage() {
        let Ok(path) = std::env::var("SONGS") else { return };
        let songs: Vec<(String, String, f64, String)> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for (t, a, d, v) in songs {
            let r = tauri::async_runtime::block_on(super::find(t.clone(), a, None, d, Some(v.clone())));
            match r {
                Ok(Some(l)) => {
                    let lines: Vec<_> = l.synced.as_deref().map(super::parse_lrc).unwrap_or_default().into_iter().filter(|x| !x.1.is_empty()).collect();
                    let (first, last) = (lines.first().map(|x| x.0).unwrap_or(-1.0), lines.last().map(|x| x.0).unwrap_or(-1.0));
                    println!("COV {v} | {t} | {} | dur {d} | lines {} | first {first:.1} | last {last:.1} | covers {:.0}%", l.source, lines.len(), last / d * 100.0);
                }
                other => println!("COV {v} | {t} | {other:?}"),
            }
        }
    }

    #[test]
    #[ignore]
    fn live_sources() {
        let cases: &[(&str, &str, f64, Option<&str>)] = &[
            ("Never Gonna Give You Up", "Rick Astley", 213.0, Some("lYBUbBu4W08")),
            ("Never Gonna Give You Up", "Rick Astley", 213.0, None),
            ("Shape of You", "Ed Sheeran", 234.0, Some("JGwWNGJdvx8")),
            ("Kesariya (From \"Brahmastra\")", "Pritam, Arijit Singh", 268.0, None),
            ("zzqq nothing", "Nobody Atall", 213.0, Some("lYBUbBu4W08")),
            ("zzqq nothing", "Nobody Atall", 234.0, Some("JGwWNGJdvx8")),
            ("zzqq nothing", "Nobody Atall", 234.0, None),
        ];
        for id in ["lYBUbBu4W08", "JGwWNGJdvx8", "80o35Bd5p-s", "cLkox4yinMo"] {
            let r = tauri::async_runtime::block_on(super::ytm_lyrics(id.to_string()));
            println!("ytm {id} / : {:?}", r.map(|o| o.map(|l| (l.source, l.synced.map(|s| s.lines().take(4).collect::<Vec<_>>().join(" | ")), l.plain.is_some()))));
        }
        for (t, a, d, v) in cases {
            let r = tauri::async_runtime::block_on(super::find(t.to_string(), a.to_string(), None, *d, v.map(str::to_string)));
            match r {
                Ok(Some(l)) => println!("{t} / {v:?}: source={} synced={} plain={} est={} first={:?}", l.source, l.synced.is_some(), l.plain.is_some(), l.estimated, l.synced.as_deref().map(|s| super::parse_lrc(s).into_iter().find(|x| !x.1.is_empty()))),
                other => println!("{t} / {v:?}: {other:?}"),
            }
        }
    }
}
