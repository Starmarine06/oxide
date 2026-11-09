# Oxide

A YouTube Music desktop app written in Rust with [Tauri 2](https://tauri.app).

> **Inspired by [Pear Desktop](https://github.com/pear-devs/pear-desktop)** (formerly th-ch/youtube-music). Oxide is a Rust rewrite of its idea: the same plugin set, the same API-server routes and the same integrations, rebuilt on Tauri instead of Electron. Huge thanks to the Pear Desktop team and contributors. If you'd rather have the original, go use theirs.

The Windows installer is about 3 MB. It uses the browser engine your OS already ships (WebView2 on Windows, WKWebView on macOS, WebKitGTK on Linux), so the app idles at around 60 MB of RAM.

## What you get

- **Its own window.** A frameless, dark title bar holds the App, Navigation, Playback, Plugins and Options menus. Hide the menus from Options and press Alt to bring them back. Every menu item has its keyboard shortcut listed next to it.
- **Media keys and OS controls.** Windows media overlay, macOS Now Playing and Linux MPRIS, all with cover art.
- **Tray icon.** Close hides to the tray (optional), and the tray menu covers playback.
- **Stays signed in.** One persistent browser profile, kept across restarts and updates.
- **Updates itself.** Oxide checks GitHub Releases at startup and every 6 hours, then asks before updating. The update is signed, installs itself and restarts the app.

## Plugins

Turn each one on or off under **Plugins**. Changes apply instantly with no restart. Plugins with settings get a submenu.

| Plugin | What it does |
|---|---|
| Ad blocker | Strips ads out of player responses, skips any that slip through, hides Premium banners |
| Album actions | Like, dislike or clear every song on an album or playlist |
| Album colour theme | Tints the app with colours from the current album art |
| Ambient mode | Soft glow from the artwork or video around the player |
| API server | HTTP remote control on port 26538, compatible with Pear Desktop's `/api/v1` clients |
| Amuse | Now-playing feed for the Amuse widget |
| Audio compressor | Evens out loud and quiet parts (light, medium or strong) |
| Captions | Choose a captions track, with an optional auto-on |
| Clock | A clock in the top bar |
| Crossfade | Fades songs out and in instead of cutting |
| Custom output device | Send audio to a specific speaker or headset |
| Disable autoplay | Songs start paused |
| Discord Rich Presence | "Listening to" with cover, progress bar and a link |
| Do not track | Blocks telemetry and ad tracking but keeps your listening history |
| Downloader | Saves the current song as MP3, M4A, Opus, FLAC or WAV via yt-dlp |
| Equalizer | 10 bands with presets (Bass boost, Vocal, Rock, Late night and more) |
| Exponential volume | Makes low volumes easier to pick |
| Global shortcuts | System-wide hotkeys for play, next, previous, volume, like and show window |
| Lumia Stream | Now-playing feed for Lumia Stream |
| Music Together | Listen in sync with friends using a share code |
| Navigation arrows | Back and forward buttons in the top bar |
| Notifications | Desktop notification on song change |
| Performance boost | Throttles background timers to cut CPU use |
| Picture-in-picture | Shrinks Oxide to a small player that stays on top (Ctrl+Shift+P) |
| Playback speed | 0.25× to 3× with presets |
| Precise volume | Scroll on the player for 1% steps, with an on-screen readout |
| Proxy | Route traffic through an HTTP or SOCKS5 proxy |
| Scrobbler | Last.fm (connect in your browser) and ListenBrainz (token) |
| Skip disliked songs | Dislike a song and it's gone |
| Skip silences | Jumps over long silent stretches |
| SponsorBlock | Skips intros, outros and talking in music videos |
| Song / video switch | Flip between a song and its music video, or hide video completely |
| Synced lyrics | Time-synced lyrics from LRCLIB in a side panel (Ctrl+L) |
| Taskbar media controls | Previous, play/pause and next buttons in the Windows taskbar preview |
| Transparent player | See-through Acrylic or Mica window (Windows 11) |
| Tuna OBS | Now-playing feed for OBS via Tuna |
| Unobtrusive player | Starting a song doesn't take over the screen |
| Video quality | Pick the video quality for music videos |
| Visualizer | Bars, Radial or Aurora visuals in the player, coloured from the album art |

The only Pear Desktop plugin not ported is **Touch Bar**. Tauri has no API for the MacBook Touch Bar, which Apple has also discontinued. Music Together relays state through a private topic on ntfy.sh instead of WebRTC, because YouTube Music's security policy blocks WebRTC signaling inside the page.

## Releasing an update

One-time setup:

1. Create the GitHub repo `Starmarine06/oxide` and push this project. If you use a different name, change the URL in `src-tauri/tauri.conf.json` under `plugins.updater.endpoints`.
2. Add two repository secrets (**Settings > Secrets and variables > Actions**):
   - `TAURI_SIGNING_PRIVATE_KEY`: the contents of `~/.tauri/oxide.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: leave it empty (the key has no password)

   Keep `~/.tauri/oxide.key` backed up. Without it, installed copies can never be updated again.

Each release:

```sh
npm run release -- 0.2.0
```

That bumps the version, commits, tags and pushes. GitHub Actions then builds Windows, macOS and Linux installers, signs them and publishes the release. Every installed Oxide sees it within 6 hours (or right away via **App > Check for updates**), asks the user, installs and restarts.

## Build it yourself

You need Rust (stable), Node 18+ and the [Tauri system packages](https://tauri.app/start/prerequisites/).

```sh
npm install
npm run dev      # debug build
npm run build    # installers in target/release/bundle/
cargo test       # unit tests
```

## Settings

Settings are stored in `config.json` (**App > Open config folder**):

- Windows: `%APPDATA%\dev.oxide.music\`
- macOS: `~/Library/Application Support/dev.oxide.music/`
- Linux: `~/.config/dev.oxide.music/`

The browser profile, which holds your sign-in, lives in the matching local app-data folder under `profile/`.

## How it's built

```
src-tauri/src/
  lib.rs, window.rs     app setup, the frameless window, persistent profile
  menu.rs               menu model rendered by the page's title bar
  player.rs, media.rs   song state from the page, OS media controls
  updater.rs            GitHub-release self-updates
  bridge.rs             Rust <-> page requests, prompts, allow-listed fetch
  plugins/              plugin trait + Rust plugins
src-tauri/scripts/
  titlebar.js           title bar, menus, shortcuts, window buttons
  core.js               page bridge, plugin runtime, shared audio graph, UI helpers
  plugins/*.js          page plugins
```

A page plugin is a JS file that calls `pear.plugin(id, { start(ctx, config) { ... } })` plus one `page_plugin!` line in `plugins/page.rs`. Everything the plugin creates through `ctx` (styles, listeners, timers, DOM nodes, audio filters) is cleaned up when it's switched off. YouTube Music enforces Trusted Types, so build DOM with `pear.el()` and never assign HTML strings.

YouTube Music ships two player UIs: the 2026 miniplayer redesign and the older player bar. Selectors for both live in one place (`SEL` in `core.js`).

## License

MIT. Pear Desktop's license is in `LICENSE-UPSTREAM`. The Discord application ID and the Last.fm API key come from Pear Desktop.
