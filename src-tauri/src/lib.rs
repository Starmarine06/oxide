mod audio_session;
mod bridge;
mod config;
mod library;
mod media;
mod menu;
mod overlay;
mod player;
mod plugins;
mod settings_window;
mod tray;
mod updater;
mod window;

use std::path::Path;
use std::sync::OnceLock;
use tauri::{Manager, RunEvent};

use config::ConfigStore;
use player::PlayerState;
use plugins::Registry;

/// Shared HTTP client for lyrics and the yt-dlp fetch.
pub(crate) fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("oxide/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("http client")
    })
}

/// Open a folder in the OS file manager.
pub(crate) fn open_path(path: &Path) {
    let program = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(program).arg(path).spawn() {
        log::error!("can't open {}: {e}", path.display());
    }
}

/// Open a web page in the default browser.
pub(crate) fn open_url(url: &str) {
    let result = if cfg!(target_os = "windows") {
        std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    if let Err(e) = result {
        log::error!("can't open {url}: {e}");
    }
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("oxide_lib=info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| window::show(app)))
        .plugin(tauri_plugin_notification::init())
        // Downloaded songs for the offline player (library::serve_media).
        .register_asynchronous_uri_scheme_protocol("media", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || responder.respond(library::serve_media(&app, &request)));
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(plugins::shortcuts::on_shortcut)
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            player::player_update,
            plugins::lyrics::lyrics_get,
            plugins::lyrics::lyrics_lead_in,
            plugins::downloader::download,
            plugins::plugin_set_option,
            menu::menu_model,
            menu::menu_click,
            window::window_control,
            bridge::page_reply,
            bridge::http_get,
            plugins::window_plugins::pip_toggle,
            library::offline_library,
            overlay::overlay_levels,
            overlay::overlay_save_layout,
            overlay::overlay_save_group,
            overlay::overlay_hit_rects,
            settings_window::settings_get,
            settings_window::settings_set,
            library::go_online,
        ])
        // Tray menu clicks.
        .on_menu_event(|app, event| menu::handle(app, event.id().as_ref()))
        .setup(|app| {
            let handle = app.handle().clone();
            let config_path = app.path().app_config_dir()?.join("config.json");
            log::info!("config: {}", config_path.display());
            app.manage(ConfigStore::load(config_path));
            app.manage(Registry::new());
            app.manage(PlayerState::default());
            app.manage(media::Media::default());
            app.manage(updater::Declined::default());

            tray::create(&handle)?;
            let cfg = app.state::<ConfigStore>().get();
            window::create(&handle, window::START_URL, cfg.window, !cfg.options.start_minimized)?;
            app.state::<Registry>().start_enabled(&handle);
            library::check_at_startup(&handle);
            updater::start_background_checks(&handle);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the app")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                app.state::<Registry>().stop_all(app);
            }
        });
}
