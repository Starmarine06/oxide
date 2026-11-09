//! Time-synced lyrics from LRCLIB, shown in a side panel in the page.
//! Fetching happens here in Rust so the page's CSP and CORS rules don't
//! get in the way.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;

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
        "Time-synced lyrics from LRCLIB in a side panel"
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    /// LRC text with `[mm:ss.xx]` timestamps, if available.
    pub synced: Option<String>,
    pub plain: Option<String>,
    pub instrumental: bool,
    pub source: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibTrack {
    track_name: Option<String>,
    artist_name: Option<String>,
    duration: Option<f64>,
    #[serde(default)]
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

impl From<LrclibTrack> for Lyrics {
    fn from(t: LrclibTrack) -> Self {
        Lyrics {
            synced: t.synced_lyrics.filter(|s| !s.trim().is_empty()),
            plain: t.plain_lyrics.filter(|s| !s.trim().is_empty()),
            instrumental: t.instrumental,
            source: "LRCLIB",
        }
    }
}

const LRCLIB: &str = "https://lrclib.net/api";

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
    artist
        .split([',', '&'])
        .next()
        .unwrap_or(artist)
        .split(" feat")
        .next()
        .unwrap_or(artist)
        .trim()
        .to_string()
}

#[tauri::command]
pub async fn lyrics_get(title: String, artist: String, album: Option<String>, duration: f64) -> Result<Option<Lyrics>, String> {
    let http = crate::http();

    // 1. Exact lookup, which LRCLIB matches on duration within ~2 seconds.
    let mut query: Vec<(&str, String)> = vec![("track_name", title.clone()), ("artist_name", artist.clone())];
    if let Some(a) = album.as_ref().filter(|a| !a.is_empty()) {
        query.push(("album_name", a.clone()));
    }
    if duration > 0.0 {
        query.push(("duration", (duration.round() as i64).to_string()));
    }
    let resp = http
        .get(format!("{LRCLIB}/get"))
        .query(&query)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.status().is_success() {
        if let Ok(track) = resp.json::<LrclibTrack>().await {
            let lyrics = Lyrics::from(track);
            if lyrics.synced.is_some() || lyrics.plain.is_some() || lyrics.instrumental {
                return Ok(Some(lyrics));
            }
        }
    }

    // 2. Fuzzy search with cleaned-up names, pick the closest duration.
    let track = clean_title(&title);
    let artist = first_artist(&artist);
    let results: Vec<LrclibTrack> = http
        .get(format!("{LRCLIB}/search"))
        .query(&[("track_name", track.as_str()), ("artist_name", artist.as_str())])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let best = results
        .into_iter()
        .filter(|t| t.synced_lyrics.is_some() || t.plain_lyrics.is_some())
        .filter(|t| duration <= 0.0 || t.duration.is_some_and(|d| (d - duration).abs() <= 8.0))
        .filter(|t| {
            // Guard against unrelated hits that only share the artist.
            let name = t.track_name.as_deref().unwrap_or("").to_lowercase();
            let want = track.to_lowercase();
            name.contains(&want) || want.contains(&name)
        })
        .min_by(|a, b| {
            let score = |t: &LrclibTrack| {
                let dd = t.duration.map(|d| (d - duration).abs()).unwrap_or(99.0);
                let synced_bonus = if t.synced_lyrics.is_some() { 0.0 } else { 5.0 };
                let artist_bonus = if t.artist_name.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(&artist)) { 0.0 } else { 2.0 };
                dd + synced_bonus + artist_bonus
            };
            score(a).total_cmp(&score(b))
        });

    Ok(best.map(Lyrics::from))
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
    }
}
