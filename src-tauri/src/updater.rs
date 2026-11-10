//! Self-update from GitHub releases.
//!
//! Every release built by CI publishes signed installers plus a
//! `latest.json` manifest. Oxide reads that manifest at startup and every
//! six hours; when it lists a newer version, the user is asked whether to
//! update, and on yes the installer is downloaded, its signature checked
//! against the public key in tauri.conf.json, installed, and the app restarts.

use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

use crate::window;

/// The version the user said "Later" to, so we don't nag every 6 hours.
#[derive(Default)]
pub struct Declined(Mutex<Option<String>>);

pub fn start_background_checks(app: &AppHandle) {
    // Dev builds aren't installed, so there's nothing to update.
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio_sleep(Duration::from_secs(15)).await;
        loop {
            check(&app, false).await;
            tokio_sleep(Duration::from_secs(6 * 60 * 60)).await;
        }
    });
}

async fn tokio_sleep(d: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}

/// `manual` = the user picked "Check for updates", so also report
/// "up to date" and errors instead of staying quiet.
pub async fn check(app: &AppHandle, manual: bool) {
    let result = async {
        let updater = app.updater().map_err(|e| e.to_string())?;
        updater.check().await.map_err(|e| e.to_string())
    }
    .await;

    let update = match result {
        Ok(Some(u)) => u,
        Ok(None) => {
            if manual {
                app.dialog()
                    .message(format!("You're on the latest version ({}).", app.package_info().version))
                    .title("Oxide")
                    .show(|_| {});
            }
            return;
        }
        Err(e) => {
            log::warn!("update check failed: {e}");
            if manual {
                app.dialog()
                    .message(format!("Couldn't check for updates.\n\n{e}"))
                    .title("Oxide")
                    .kind(MessageDialogKind::Warning)
                    .show(|_| {});
            }
            return;
        }
    };

    let declined = app.state::<Declined>();
    if !manual && declined.0.lock().unwrap().as_deref() == Some(update.version.as_str()) {
        return;
    }

    let mut message = format!(
        "Oxide {} is available. You have {}.\n\nUpdate now? Oxide will restart when it's done.",
        update.version, update.current_version
    );
    if let Some(notes) = update.body.as_deref().map(plain_text).filter(|n| !n.is_empty()) {
        let mut notes: String = notes.chars().take(600).collect();
        if notes.len() < plain_text(update.body.as_deref().unwrap_or_default()).len() {
            notes.push('…');
        }
        message.push_str(&format!("\n\nWhat's new:\n{notes}"));
    }

    let handle = app.clone();
    app.dialog()
        .message(message)
        .title("Update available")
        .buttons(MessageDialogButtons::OkCancelCustom("Update now".into(), "Later".into()))
        .show(move |yes| {
            if !yes {
                *handle.state::<Declined>().0.lock().unwrap() = Some(update.version.clone());
                return;
            }
            let app = handle.clone();
            tauri::async_runtime::spawn(async move {
                let mut downloaded: u64 = 0;
                let mut last_pct = u64::MAX;
                let progress_app = app.clone();
                let result = update
                    .download_and_install(
                        move |chunk, total| {
                            downloaded += chunk as u64;
                            if let Some(total) = total.filter(|t| *t > 0) {
                                let pct = downloaded * 100 / total;
                                if pct != last_pct {
                                    last_pct = pct;
                                    window::eval(&progress_app, &format!("window.__pear && window.__pear.toast('Downloading update… {pct}%', 0);"));
                                }
                            }
                        },
                        || {},
                    )
                    .await;
                match result {
                    Ok(()) => {
                        window::save_geometry(&app);
                        app.restart();
                    }
                    Err(e) => {
                        log::error!("update failed: {e}");
                        window::eval(&app, "window.__pear && window.__pear.toast('Update failed. Try again later.', 6000);");
                        app.dialog()
                            .message(format!("The update couldn't be installed.\n\n{e}"))
                            .title("Update failed")
                            .kind(MessageDialogKind::Error)
                            .show(|_| {});
                    }
                }
            });
        });
}

/// Release notes are Markdown; the update prompt is plain text. Drops
/// heading marks, bold/italic marks and backticks, and turns list bullets
/// into "•".
fn plain_text(markdown: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in markdown.lines() {
        let line = line.trim_end();
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];
        let body = if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            format!("{indent}• {rest}")
        } else {
            trimmed.trim_start_matches('#').trim_start().to_string()
        };
        let body = body.replace("**", "").replace("__", "").replace('`', "");
        // No more than one blank line in a row.
        if body.is_empty() && out.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        out.push(body);
    }
    out.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::plain_text;

    #[test]
    fn notes_become_plain_text() {
        let md = "Intro line.\n\n## Downloads\n\n- **Bold** item with `code`\n- Second\n\n\n## Fixes\n* One";
        assert_eq!(plain_text(md), "Intro line.\n\nDownloads\n\n• Bold item with code\n• Second\n\nFixes\n• One");
    }
}
