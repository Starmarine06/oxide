//! Now-playing integrations for streaming tools: Tuna (OBS), Lumia Stream
//! and the Amuse widget. Same wire formats as Pear Desktop.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager};

use super::Plugin;
use crate::player::{Change, PlayerState, SongInfo};

fn blocking_client() -> &'static reqwest::blocking::Client {
    static C: std::sync::OnceLock<reqwest::blocking::Client> = std::sync::OnceLock::new();
    C.get_or_init(|| reqwest::blocking::Client::builder().timeout(Duration::from_secs(2)).build().unwrap())
}

fn ms(secs: f64) -> i64 {
    (secs * 1000.0).round() as i64
}

/// A background loop that stops when the flag is cleared.
#[derive(Default)]
struct Worker(Mutex<Option<Arc<AtomicBool>>>);

impl Worker {
    fn start(&self, f: impl FnOnce(Arc<AtomicBool>) + Send + 'static) {
        let mut slot = self.0.lock().unwrap();
        if slot.is_some() {
            return;
        }
        let flag = Arc::new(AtomicBool::new(true));
        *slot = Some(flag.clone());
        std::thread::spawn(move || f(flag));
    }
    fn stop(&self) {
        if let Some(flag) = self.0.lock().unwrap().take() {
            flag.store(false, Ordering::SeqCst);
        }
    }
}

// ---- Tuna (OBS) ----------------------------------------------------------

#[derive(Default)]
pub struct TunaObs(Worker);

impl Plugin for TunaObs {
    fn id(&self) -> &'static str {
        "tuna-obs"
    }
    fn name(&self) -> &'static str {
        "Tuna OBS"
    }
    fn description(&self) -> &'static str {
        "Send now playing to the Tuna plugin for OBS (port 1608)"
    }
    fn start(&self, app: &AppHandle, _cfg: &Value) {
        let app = app.clone();
        // Tuna shows a progress bar, so it wants a steady stream of updates.
        self.0.start(move |run| {
            while run.load(Ordering::SeqCst) {
                if let Some(s) = app.state::<PlayerState>().current_live() {
                    let data = json!({ "data": {
                        "title": s.title, "artists": [s.artist], "album": s.album,
                        "cover": s.art_url, "cover_url": s.art_url, "album_url": s.art_url,
                        "duration": ms(s.duration), "progress": ms(s.elapsed),
                        "status": if s.paused { "stopped" } else { "playing" },
                        "url": s.url, "alternativeTitle": "", "tags": [],
                    }});
                    let _ = blocking_client().post("http://127.0.0.1:1608/").json(&data).send();
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    }
    fn stop(&self, _app: &AppHandle) {
        self.0.stop();
    }
}

// ---- Lumia Stream ----------------------------------------------------------

pub struct LumiaStream;

impl Plugin for LumiaStream {
    fn id(&self) -> &'static str {
        "lumiastream"
    }
    fn name(&self) -> &'static str {
        "Lumia Stream"
    }
    fn description(&self) -> &'static str {
        "Send now playing to Lumia Stream (port 39231)"
    }
    fn on_player_change(&self, _app: &AppHandle, s: &SongInfo, change: Change, _cfg: &Value) {
        if !(change.song || change.playback) {
            return;
        }
        let body = json!({
            "token": "lsmedia_ytmsI7812",
            "data": {
                "origin": "youtubemusic",
                "eventType": if change.song { "switchSong" } else { "playPause" },
                "url": s.url, "videoId": s.video_id,
                "cover": s.art_url, "cover_url": s.art_url, "album_url": s.art_url,
                "title": s.title, "artists": [s.artist], "album": s.album,
                "status": if s.paused { "stopped" } else { "playing" }, "isPaused": s.paused,
                "progress": ms(s.elapsed), "duration": ms(s.duration),
            }
        });
        std::thread::spawn(move || {
            let _ = blocking_client().post("http://127.0.0.1:39231/api/media").json(&body).send();
        });
    }
}

// ---- Amuse -------------------------------------------------------------------

#[derive(Default)]
pub struct Amuse(Mutex<Option<Arc<tiny_http::Server>>>);

impl Plugin for Amuse {
    fn id(&self) -> &'static str {
        "amuse"
    }
    fn name(&self) -> &'static str {
        "Amuse"
    }
    fn description(&self) -> &'static str {
        "Serve now playing to the Amuse widget (port 9863)"
    }
    fn start(&self, app: &AppHandle, _cfg: &Value) {
        let mut slot = self.0.lock().unwrap();
        if slot.is_some() {
            return;
        }
        let server = match tiny_http::Server::http("127.0.0.1:9863") {
            Ok(s) => Arc::new(s),
            Err(e) => {
                log::error!("amuse: can't listen on 9863: {e}");
                return;
            }
        };
        *slot = Some(server.clone());
        let app = app.clone();
        std::thread::spawn(move || {
            for req in server.incoming_requests() {
                let body = match req.url() {
                    "/query" | "/api" => {
                        let s = app.state::<PlayerState>().current_live();
                        let v = match s {
                            Some(s) => json!({
                                "player": { "hasSong": true, "isPaused": s.paused, "seekbarCurrentPosition": s.elapsed },
                                "track": { "duration": s.duration, "title": s.title, "author": s.artist,
                                    "cover": s.art_url, "url": s.url, "id": s.video_id, "isAdvertisement": false },
                            }),
                            None => json!({ "player": { "hasSong": false, "isPaused": true, "seekbarCurrentPosition": 0 }, "track": {} }),
                        };
                        v.to_string()
                    }
                    _ => "Oxide is running. Query /query or /api for now playing.".to_string(),
                };
                let resp = tiny_http::Response::from_string(body)
                    .with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap())
                    .with_header("Access-Control-Allow-Origin: *".parse::<tiny_http::Header>().unwrap());
                let _ = req.respond(resp);
            }
        });
    }
    fn stop(&self, _app: &AppHandle) {
        if let Some(s) = self.0.lock().unwrap().take() {
            s.unblock();
        }
    }
}
