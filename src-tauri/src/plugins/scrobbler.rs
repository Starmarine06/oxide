//! Scrobbling to Last.fm and ListenBrainz. A song is scrobbled once it has
//! played for half its length or 4 minutes, whichever comes first (the
//! Last.fm rule), and only if it's longer than 30 seconds.

use md5::{Digest, Md5};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

use super::{MenuAction, Plugin, PluginOption, Registry};
use crate::config::ConfigStore;
use crate::player::{Change, PlayerState, SongInfo};

// Pear Desktop's registered Last.fm application.
const LASTFM_API: &str = "https://ws.audioscrobbler.com/2.0/";
const LASTFM_KEY: &str = "04d76faaac8726e60988e14c105d421a";
const LASTFM_SECRET: &str = "a5d2a36fdf64819290f6982481eaffa2";
const LISTENBRAINZ_API: &str = "https://api.listenbrainz.org/1/";

#[derive(Default)]
pub struct Scrobbler {
    run: Mutex<Option<Arc<AtomicBool>>>,
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(concat!("oxide/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn cfg(app: &AppHandle) -> Value {
    let r = app.state::<Registry>();
    let store = app.state::<ConfigStore>();
    r.get("scrobbler").map(|p| r.config(&store, p.as_ref())).unwrap_or_default()
}

fn s<'a>(cfg: &'a Value, key: &str) -> Option<&'a str> {
    cfg.get(key).and_then(Value::as_str).filter(|v| !v.is_empty())
}

fn on(cfg: &Value, key: &str) -> bool {
    cfg.get(key).and_then(Value::as_bool).unwrap_or(false)
}

// ---- Last.fm -------------------------------------------------------------------

fn sign(params: &BTreeMap<&str, String>) -> String {
    let mut raw = String::new();
    for (k, v) in params {
        if *k != "format" && *k != "callback" {
            raw.push_str(k);
            raw.push_str(v);
        }
    }
    raw.push_str(LASTFM_SECRET);
    format!("{:x}", Md5::digest(raw.as_bytes()))
}

fn lastfm_call(method: &str, mut params: BTreeMap<&'static str, String>, post: bool) -> Result<Value, String> {
    params.insert("method", method.to_string());
    params.insert("api_key", LASTFM_KEY.to_string());
    let sig = sign(&params.iter().map(|(k, v)| (*k, v.clone())).collect());
    params.insert("api_sig", sig);
    params.insert("format", "json".to_string());
    let c = client();
    let resp = if post { c.post(LASTFM_API).form(&params).send() } else { c.get(LASTFM_API).query(&params).send() }
        .map_err(|e| e.to_string())?;
    let v: Value = resp.json().map_err(|e| e.to_string())?;
    if let Some(err) = v.get("message").and_then(Value::as_str) {
        return Err(err.to_string());
    }
    Ok(v)
}

fn lastfm_track(method: &str, sk: &str, song: &SongInfo, timestamp: Option<u64>) -> Result<(), String> {
    let mut p = BTreeMap::new();
    p.insert("sk", sk.to_string());
    p.insert("artist", song.artist.clone());
    p.insert("track", song.title.clone());
    if let Some(a) = song.album.as_ref().filter(|a| !a.is_empty()) {
        p.insert("album", a.clone());
    }
    if song.duration > 0.0 {
        p.insert("duration", (song.duration.round() as i64).to_string());
    }
    if let Some(ts) = timestamp {
        p.insert("timestamp", ts.to_string());
    }
    lastfm_call(method, p, true).map(|_| ())
}

// ---- ListenBrainz ------------------------------------------------------------

fn listenbrainz(token: &str, kind: &str, song: &SongInfo, listened_at: Option<u64>) -> Result<(), String> {
    let mut entry = json!({
        "track_metadata": {
            "artist_name": song.artist,
            "track_name": song.title,
            "additional_info": {
                "media_player": "Oxide",
                "submission_client": "Oxide",
                "origin_url": song.url,
                "duration": song.duration.round() as i64,
            }
        }
    });
    if let Some(a) = song.album.as_ref().filter(|a| !a.is_empty()) {
        entry["track_metadata"]["release_name"] = json!(a);
    }
    if let Some(ts) = listened_at {
        entry["listened_at"] = json!(ts);
    }
    let resp = client()
        .post(format!("{LISTENBRAINZ_API}submit-listens"))
        .header("Authorization", format!("Token {token}"))
        .json(&json!({ "listen_type": kind, "payload": [entry] }))
        .send()
        .map_err(|e| e.to_string())?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

// ---- plugin ----------------------------------------------------------------------

fn now_playing(app: &AppHandle, song: &SongInfo) {
    let c = cfg(app);
    if on(&c, "lastfmEnabled") {
        if let Some(sk) = s(&c, "lastfmSessionKey") {
            if let Err(e) = lastfm_track("track.updateNowPlaying", sk, song, None) {
                log::warn!("last.fm now playing: {e}");
            }
        }
    }
    if on(&c, "listenbrainzEnabled") {
        if let Some(token) = s(&c, "listenbrainzToken") {
            if let Err(e) = listenbrainz(token, "playing_now", song, None) {
                log::warn!("listenbrainz now playing: {e}");
            }
        }
    }
}

fn scrobble(app: &AppHandle, song: &SongInfo, started: u64) {
    let c = cfg(app);
    if on(&c, "lastfmEnabled") {
        if let Some(sk) = s(&c, "lastfmSessionKey") {
            match lastfm_track("track.scrobble", sk, song, Some(started)) {
                Ok(()) => log::info!("scrobbled to last.fm: {} - {}", song.artist, song.title),
                Err(e) => log::warn!("last.fm scrobble: {e}"),
            }
        }
    }
    if on(&c, "listenbrainzEnabled") {
        if let Some(token) = s(&c, "listenbrainzToken") {
            if let Err(e) = listenbrainz(token, "single", song, Some(started)) {
                log::warn!("listenbrainz scrobble: {e}");
            }
        }
    }
}

impl Plugin for Scrobbler {
    fn id(&self) -> &'static str {
        "scrobbler"
    }
    fn name(&self) -> &'static str {
        "Scrobbler"
    }
    fn description(&self) -> &'static str {
        "Scrobble to Last.fm and ListenBrainz"
    }
    fn defaults(&self) -> Value {
        json!({ "enabled": false, "lastfmEnabled": true, "lastfmSessionKey": null, "lastfmUser": null,
                "listenbrainzEnabled": false, "listenbrainzToken": null })
    }
    fn options(&self) -> Vec<PluginOption> {
        vec![
            PluginOption::toggle("lastfmEnabled", "Last.fm"),
            PluginOption::toggle("listenbrainzEnabled", "ListenBrainz"),
        ]
    }
    fn menu_actions(&self) -> Vec<MenuAction> {
        vec![
            MenuAction { id: "lastfm-connect", label: "Connect Last.fm account…", accelerator: None },
            MenuAction { id: "lastfm-disconnect", label: "Disconnect Last.fm", accelerator: None },
            MenuAction { id: "listenbrainz-token", label: "Set ListenBrainz token…", accelerator: None },
        ]
    }

    fn on_menu_action(&self, app: &AppHandle, action: &str, cfg: &Value) {
        let app = app.clone();
        match action {
            "lastfm-connect" => {
                std::thread::spawn(move || {
                    let token = match lastfm_call("auth.gettoken", BTreeMap::new(), false) {
                        Ok(v) => v.get("token").and_then(Value::as_str).unwrap_or_default().to_string(),
                        Err(e) => {
                            app.dialog().message(format!("Couldn't reach Last.fm: {e}")).title("Last.fm").blocking_show();
                            return;
                        }
                    };
                    crate::open_url(&format!("https://www.last.fm/api/auth/?api_key={LASTFM_KEY}&token={token}"));
                    let ok = app
                        .dialog()
                        .message("Your browser opened Last.fm. Click \"Yes, allow access\" there, then come back and press Done.")
                        .title("Connect Last.fm")
                        .buttons(MessageDialogButtons::OkCancelCustom("Done".into(), "Cancel".into()))
                        .blocking_show();
                    if !ok {
                        return;
                    }
                    let mut p = BTreeMap::new();
                    p.insert("token", token);
                    match lastfm_call("auth.getsession", p, false) {
                        Ok(v) => {
                            let key = v["session"]["key"].as_str().unwrap_or_default().to_string();
                            let name = v["session"]["name"].as_str().unwrap_or_default().to_string();
                            let reg = app.state::<Registry>();
                            reg.set_option(&app, "scrobbler", "lastfmSessionKey", json!(key));
                            reg.set_option(&app, "scrobbler", "lastfmUser", json!(name));
                            reg.set_option(&app, "scrobbler", "lastfmEnabled", json!(true));
                            crate::menu::refresh(&app);
                            app.dialog().message(format!("Connected as {name}.")).title("Last.fm").blocking_show();
                        }
                        Err(e) => {
                            app.dialog().message(format!("Last.fm didn't confirm the connection: {e}")).title("Last.fm").blocking_show();
                        }
                    }
                });
            }
            "lastfm-disconnect" => {
                let reg = app.state::<Registry>();
                reg.set_option(&app, "scrobbler", "lastfmSessionKey", Value::Null);
                reg.set_option(&app, "scrobbler", "lastfmUser", Value::Null);
            }
            "listenbrainz-token" => {
                let current = s(cfg, "listenbrainzToken").unwrap_or_default().to_string();
                tauri::async_runtime::spawn(async move {
                    let label = "Paste your user token from listenbrainz.org/settings.";
                    if let Some(token) = crate::bridge::prompt(&app, "ListenBrainz token", label, &current, "password").await {
                        let reg = app.state::<Registry>();
                        reg.set_option(&app, "scrobbler", "listenbrainzToken", json!(token.trim()));
                        reg.set_option(&app, "scrobbler", "listenbrainzEnabled", json!(!token.trim().is_empty()));
                        crate::menu::refresh(&app);
                    }
                });
            }
            _ => {}
        }
    }

    fn start(&self, app: &AppHandle, _cfg: &Value) {
        let mut slot = self.run.lock().unwrap();
        if slot.is_some() {
            return;
        }
        let flag = Arc::new(AtomicBool::new(true));
        *slot = Some(flag.clone());
        let app = app.clone();
        // Count real listening time (pauses and seeks don't count).
        std::thread::spawn(move || {
            let mut current = String::new();
            let mut listened = 0.0f64;
            let mut started = 0u64;
            let mut done = false;
            let tick = Duration::from_secs(2);
            while flag.load(Ordering::SeqCst) {
                std::thread::sleep(tick);
                let Some(song) = app.state::<PlayerState>().current_live() else { continue };
                if song.video_id != current {
                    current = song.video_id.clone();
                    listened = 0.0;
                    started = now();
                    done = false;
                    continue;
                }
                if song.paused || done {
                    continue;
                }
                listened += tick.as_secs_f64();
                let needed = (song.duration / 2.0).min(240.0);
                if song.duration > 30.0 && listened >= needed {
                    done = true;
                    let app = app.clone();
                    std::thread::spawn(move || scrobble(&app, &song, started));
                }
            }
        });
    }

    fn stop(&self, _app: &AppHandle) {
        if let Some(f) = self.run.lock().unwrap().take() {
            f.store(false, Ordering::SeqCst);
        }
    }

    fn on_config_change(&self, _app: &AppHandle, _cfg: &Value) {}

    fn on_player_change(&self, app: &AppHandle, song: &SongInfo, change: Change, _cfg: &Value) {
        if (change.song || (change.playback && !song.paused)) && !song.paused {
            let app = app.clone();
            let song = song.clone();
            std::thread::spawn(move || now_playing(&app, &song));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lastfm_signature_sorts_and_skips_format() {
        let mut p = BTreeMap::new();
        p.insert("method", "auth.gettoken".to_string());
        p.insert("api_key", "k".to_string());
        p.insert("format", "json".to_string());
        let expected = format!("{:x}", Md5::digest(format!("api_keykmethodauth.gettoken{LASTFM_SECRET}").as_bytes()));
        assert_eq!(sign(&p), expected);
    }
}
