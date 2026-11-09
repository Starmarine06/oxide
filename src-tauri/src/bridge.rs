//! Request/response calls from Rust into the page (e.g. "what's the like
//! state?" for the API server, or showing an input prompt), plus a
//! host-restricted HTTP GET that page plugins use to reach APIs the page's
//! CSP would block.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, LazyLock, Mutex};
use std::time::Duration;
use tauri::AppHandle;

use crate::window;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
type Reply = mpsc::Sender<Result<Value, String>>;
static PENDING: LazyLock<Mutex<HashMap<u64, Reply>>> = LazyLock::new(Default::default);

/// Ask the page and wait for the answer. Blocking: never call this from the
/// main thread.
pub fn query(app: &AppHandle, name: &str, arg: Value, timeout: Duration) -> Result<Value, String> {
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = mpsc::channel();
    PENDING.lock().unwrap().insert(id, tx);
    let name_js = serde_json::to_string(name).unwrap();
    window::eval(app, &format!("window.__pear && window.__pear.answer({id}, {name_js}, {arg});"));
    let result = rx.recv_timeout(timeout).map_err(|_| "the page didn't answer".to_string());
    PENDING.lock().unwrap().remove(&id);
    result?
}

/// Async-friendly wrapper around `query`.
pub async fn query_async(app: &AppHandle, name: &str, arg: Value, timeout: Duration) -> Result<Value, String> {
    let app = app.clone();
    let name = name.to_string();
    tauri::async_runtime::spawn_blocking(move || query(&app, &name, arg, timeout))
        .await
        .map_err(|e| e.to_string())?
}

/// Shows an input box in the page. Returns `None` if cancelled.
pub async fn prompt(app: &AppHandle, title: &str, label: &str, value: &str, kind: &str) -> Option<String> {
    let arg = serde_json::json!({ "title": title, "label": label, "value": value, "type": kind });
    // The user may take a while to type.
    match query_async(app, "prompt", arg, Duration::from_secs(600)).await {
        Ok(Value::String(s)) => Some(s),
        _ => None,
    }
}

#[tauri::command]
pub fn page_reply(id: u64, value: Value, error: Option<String>) {
    if let Some(tx) = PENDING.lock().unwrap().remove(&id) {
        let _ = tx.send(match error {
            Some(e) => Err(e),
            None => Ok(value),
        });
    }
}

/// Hosts page plugins may fetch through Rust.
const ALLOWED_HOSTS: &[&str] = &["sponsor.ajay.app", "lrclib.net", "returnyoutubedislikeapi.com"];

#[tauri::command]
pub async fn http_get(url: String) -> Result<Value, String> {
    let parsed = reqwest::Url::parse(&url).map_err(|e| e.to_string())?;
    let host = parsed.host_str().unwrap_or_default();
    if parsed.scheme() != "https" || !ALLOWED_HOSTS.contains(&host) {
        return Err(format!("{host} is not an allowed host"));
    }
    let resp = crate::http().get(parsed).send().await.map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let json = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
    Ok(serde_json::json!({ "status": status, "json": json, "text": if json.is_null() { body } else { String::new() } }))
}
