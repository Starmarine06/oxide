//! HTTP API for remote control (stream decks, phone apps, scripts). Routes
//! follow Pear Desktop's `/api/v1/*` so existing companion apps work.
//!
//! Auth: a client calls `POST /auth/{name}` once; Oxide asks the user to
//! allow it and returns `{ "accessToken": ... }`, which the client sends as
//! `Authorization: Bearer <token>` from then on.

use serde_json::{json, Value};
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

use super::{MenuAction, Plugin, PluginOption, Registry};
use crate::config::ConfigStore;
use crate::player::PlayerState;
use crate::{bridge, window};

#[derive(Default)]
pub struct ApiServer(Mutex<Option<Arc<tiny_http::Server>>>);

fn cfg(app: &AppHandle) -> Value {
    let r = app.state::<Registry>();
    r.config_by_id(&app.state::<ConfigStore>(), "api-server")
}

fn header(name: &str, value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()).unwrap()
}

fn respond(req: tiny_http::Request, status: u16, body: Option<Value>) {
    let text = body.map(|b| b.to_string()).unwrap_or_default();
    let resp = tiny_http::Response::from_string(text)
        .with_status_code(status)
        .with_header(header("Content-Type", "application/json"))
        .with_header(header("Access-Control-Allow-Origin", "*"))
        .with_header(header("Access-Control-Allow-Headers", "Authorization, Content-Type"))
        .with_header(header("Access-Control-Allow-Methods", "GET, POST, PATCH, DELETE, OPTIONS"));
    let _ = req.respond(resp);
}

fn authorized(app: &AppHandle, req: &tiny_http::Request) -> bool {
    let c = cfg(app);
    if c.get("authStrategy").and_then(Value::as_str) == Some("NONE") {
        return true;
    }
    let Some(token) = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("Authorization"))
        .and_then(|h| h.value.as_str().strip_prefix("Bearer "))
    else {
        return false;
    };
    c.get("authorizedClients")
        .and_then(Value::as_array)
        .is_some_and(|list| list.iter().any(|cl| cl.get("token").and_then(Value::as_str) == Some(token)))
}

fn song_json(app: &AppHandle) -> Option<Value> {
    app.state::<PlayerState>().current_live().map(|s| {
        json!({
            "title": s.title, "artist": s.artist, "album": s.album,
            "imageSrc": s.art_url, "isPaused": s.paused,
            "songDuration": s.duration.round() as i64, "elapsedSeconds": s.elapsed.floor() as i64,
            "url": s.url, "videoId": s.video_id, "mediaType": "AUDIO",
        })
    })
}

fn num(body: &Value, key: &str) -> Option<f64> {
    body.get(key).and_then(Value::as_f64)
}

fn handle(app: &AppHandle, mut req: tiny_http::Request) {
    let method = req.method().as_str().to_uppercase();
    let path = req.url().split('?').next().unwrap_or("").trim_end_matches('/').to_string();
    if method == "OPTIONS" {
        return respond(req, 204, None);
    }

    if method == "POST" {
        if let Some(id) = path.strip_prefix("/auth/") {
            return auth(app, req, id.to_string());
        }
    }

    let Some(route) = path.strip_prefix("/api/v1/") else {
        return respond(req, 404, Some(json!({ "error": "not found" })));
    };
    if !authorized(app, &req) {
        return respond(req, 401, Some(json!({ "error": "unauthorized" })));
    }
    let mut raw = String::new();
    let _ = req.as_reader().take(64 * 1024).read_to_string(&mut raw);
    let body: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
    let cmd = |name: &str, arg: Option<Value>| window::page_command(app, name, arg);
    let query = |name: &str| bridge::query(app, name, Value::Null, Duration::from_secs(3));

    match (method.as_str(), route) {
        ("POST", "previous") => cmd("previous", None),
        ("POST", "next") => cmd("next", None),
        ("POST", "play") => cmd("play", None),
        ("POST", "pause") => cmd("pause", None),
        ("POST", "toggle-play") => cmd("toggle", None),
        ("POST", "like") => cmd("like", None),
        ("POST", "dislike") => cmd("dislike", None),
        ("POST", "toggle-mute") => cmd("mute", None),
        ("POST", "shuffle") => cmd("shuffle", None),
        ("POST", "switch-repeat") => {
            let n = num(&body, "iteration").unwrap_or(1.0).max(1.0) as usize;
            for _ in 0..n.min(3) {
                cmd("repeat", None);
            }
        }
        ("POST", "seek-to") => match num(&body, "seconds") {
            Some(s) => cmd("seek", Some(json!(s))),
            None => return respond(req, 400, Some(json!({ "error": "seconds required" }))),
        },
        ("POST", "go-back") => cmd("seekBy", Some(json!(-num(&body, "seconds").unwrap_or(10.0)))),
        ("POST", "go-forward") => cmd("seekBy", Some(json!(num(&body, "seconds").unwrap_or(10.0)))),
        ("POST", "volume") => match num(&body, "volume") {
            Some(v) => cmd("volume", Some(json!((v / 100.0).clamp(0.0, 1.0)))),
            None => return respond(req, 400, Some(json!({ "error": "volume required" }))),
        },
        ("POST", "fullscreen") => cmd("playerPage", None),
        ("GET", "song-info") | ("GET", "song") => {
            return match song_json(app) {
                Some(s) => respond(req, 200, Some(s)),
                None => respond(req, 204, None),
            }
        }
        ("GET", "like-state") => {
            return match query("like-state") {
                Ok(v) => respond(req, 200, Some(json!({ "state": v }))),
                Err(e) => respond(req, 503, Some(json!({ "error": e }))),
            }
        }
        ("GET", name @ ("volume" | "shuffle" | "repeat-mode" | "fullscreen" | "queue" | "queue-info")) => {
            let name = if name == "queue-info" { "queue" } else { name };
            return match query(name) {
                Ok(v) => respond(req, 200, Some(v)),
                Err(e) => respond(req, 503, Some(json!({ "error": e }))),
            };
        }
        _ => return respond(req, 404, Some(json!({ "error": "not supported" }))),
    }
    respond(req, 204, None)
}

fn auth(app: &AppHandle, req: tiny_http::Request, id: String) {
    let c = cfg(app);
    let id: String = id.chars().filter(|c| c.is_alphanumeric() || " -_.".contains(*c)).take(64).collect();
    let none = c.get("authStrategy").and_then(Value::as_str) == Some("NONE");
    if !none {
        let allowed = app
            .dialog()
            .message(format!("\"{id}\" wants to control Oxide through the API server. Allow it?"))
            .title("API server")
            .buttons(MessageDialogButtons::OkCancelCustom("Allow".into(), "Deny".into()))
            .blocking_show();
        if !allowed {
            return respond(req, 403, Some(json!({ "error": "denied" })));
        }
    }
    let token = uuid::Uuid::new_v4().simple().to_string();
    let mut clients = c.get("authorizedClients").and_then(Value::as_array).cloned().unwrap_or_default();
    clients.retain(|cl| cl.get("id").and_then(Value::as_str) != Some(id.as_str()));
    clients.push(json!({ "id": id, "token": token }));
    app.state::<Registry>().set_option(app, "api-server", "authorizedClients", Value::Array(clients));
    respond(req, 200, Some(json!({ "accessToken": token })));
}

impl Plugin for ApiServer {
    fn id(&self) -> &'static str {
        "api-server"
    }
    fn name(&self) -> &'static str {
        "API server"
    }
    fn description(&self) -> &'static str {
        "HTTP API to control playback from other apps"
    }
    fn defaults(&self) -> Value {
        json!({ "enabled": false, "port": 26538, "lan": false, "authStrategy": "AUTH_AT_FIRST", "authorizedClients": [] })
    }
    fn options(&self) -> Vec<PluginOption> {
        vec![
            PluginOption::toggle("lan", "Allow other devices on my network"),
            PluginOption::choice(
                "authStrategy",
                "Authorization",
                vec![(json!("AUTH_AT_FIRST"), "Ask before a new app connects"), (json!("NONE"), "No authorization")],
            ),
        ]
    }
    fn menu_actions(&self) -> Vec<MenuAction> {
        vec![
            MenuAction { id: "port", label: "Change port…", accelerator: None },
            MenuAction { id: "forget", label: "Forget all connected apps", accelerator: None },
        ]
    }
    fn on_menu_action(&self, app: &AppHandle, action: &str, cfg: &Value) {
        let app = app.clone();
        match action {
            "port" => {
                let current = cfg.get("port").and_then(Value::as_u64).unwrap_or(26538).to_string();
                tauri::async_runtime::spawn(async move {
                    if let Some(p) = bridge::prompt(&app, "API server port", "1024 to 65535", &current, "number").await {
                        if let Ok(port) = p.trim().parse::<u16>() {
                            if port >= 1024 {
                                app.state::<Registry>().set_option(&app, "api-server", "port", json!(port));
                            }
                        }
                    }
                });
            }
            "forget" => app.state::<Registry>().set_option(&app, "api-server", "authorizedClients", json!([])),
            _ => {}
        }
    }
    fn start(&self, app: &AppHandle, cfg: &Value) {
        let mut slot = self.0.lock().unwrap();
        if slot.is_some() {
            return;
        }
        let port = cfg.get("port").and_then(Value::as_u64).unwrap_or(26538);
        let host = if cfg.get("lan").and_then(Value::as_bool).unwrap_or(false) { "0.0.0.0" } else { "127.0.0.1" };
        let server = match tiny_http::Server::http(format!("{host}:{port}")) {
            Ok(s) => Arc::new(s),
            Err(e) => {
                log::error!("api server: can't listen on {host}:{port}: {e}");
                return;
            }
        };
        log::info!("api server listening on {host}:{port}");
        *slot = Some(server.clone());
        let app = app.clone();
        std::thread::spawn(move || {
            for req in server.incoming_requests() {
                let app = app.clone();
                std::thread::spawn(move || handle(&app, req));
            }
        });
    }
    fn stop(&self, _app: &AppHandle) {
        if let Some(s) = self.0.lock().unwrap().take() {
            s.unblock();
        }
    }
    fn on_config_change(&self, app: &AppHandle, cfg: &Value) {
        // Only restart when the address changes, not when a client is added.
        let restart = {
            let slot = self.0.lock().unwrap();
            slot.as_ref().is_some_and(|s| {
                let port = cfg.get("port").and_then(Value::as_u64).unwrap_or(26538);
                let lan = cfg.get("lan").and_then(Value::as_bool).unwrap_or(false);
                let addr = s.server_addr().to_ip().map(|a| (a.port() as u64, a.ip().is_unspecified()));
                addr != Some((port, lan))
            })
        };
        if restart {
            self.stop(app);
            self.start(app, cfg);
        }
    }
}
