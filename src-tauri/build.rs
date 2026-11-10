fn main() {
    // Commands the music.youtube.com page may call. Each one gets an
    // `allow-<name>` permission that the remote capability grants.
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "player_update",
            "lyrics_get",
            "lyrics_lead_in",
            "download",
            "plugin_set_option",
            "menu_model",
            "menu_click",
            "window_control",
            "page_reply",
            "http_get",
            "pip_toggle",
            "offline_library",
            "go_online",
        ])),
    )
    .expect("failed to run tauri-build");
}
