//! Player state reported by the page, and fan-out of changes to the
//! media controls, tray and plugins.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Manager};

use crate::{media, overlay, plugins::Registry, tray};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongInfo {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub art_url: Option<String>,
    /// Seconds.
    pub duration: f64,
    /// Seconds.
    pub elapsed: f64,
    pub paused: bool,
    pub url: String,
    /// Volume slider, 0 to 100, carried over when switching between
    /// YouTube Music and the offline player.
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub muted: Option<bool>,
}

impl SongInfo {
    fn same_metadata(&self, other: &SongInfo) -> bool {
        self.video_id == other.video_id
            && self.title == other.title
            && self.artist == other.artist
            && self.album == other.album
            && self.art_url == other.art_url
            && self.duration.round() == other.duration.round()
    }
}

/// What changed between two consecutive reports.
#[derive(Debug, Clone, Copy, Default)]
pub struct Change {
    /// A different track started.
    pub song: bool,
    /// Title, artist, album, art or duration changed (includes `song`).
    pub metadata: bool,
    /// Play/pause flipped.
    pub playback: bool,
    /// Position jumped (user seeked).
    pub seek: bool,
}

impl Change {
    pub fn any(&self) -> bool {
        self.metadata || self.playback || self.seek
    }
}

#[derive(Default)]
pub struct PlayerState {
    current: Mutex<Option<(SongInfo, Instant)>>,
}

impl PlayerState {
    pub fn current(&self) -> Option<SongInfo> {
        self.current.lock().unwrap().as_ref().map(|(s, _)| s.clone())
    }

    /// Like `current`, with `elapsed` advanced to now while playing.
    pub fn current_live(&self) -> Option<SongInfo> {
        self.current.lock().unwrap().as_ref().map(|(s, at)| {
            let mut s = s.clone();
            if !s.paused {
                s.elapsed = (s.elapsed + at.elapsed().as_secs_f64()).min(s.duration.max(s.elapsed));
            }
            s
        })
    }

    fn replace(&self, song: SongInfo) -> Change {
        let now = Instant::now();
        let mut guard = self.current.lock().unwrap();
        let change = match guard.as_ref() {
            None => Change {
                song: true,
                metadata: true,
                playback: true,
                seek: false,
            },
            Some((prev, at)) => {
                let song_changed = prev.video_id != song.video_id;
                let expected = if prev.paused {
                    prev.elapsed
                } else {
                    prev.elapsed + now.duration_since(*at).as_secs_f64()
                };
                Change {
                    song: song_changed,
                    metadata: song_changed || !prev.same_metadata(&song),
                    playback: prev.paused != song.paused,
                    seek: !song_changed && (song.elapsed - expected).abs() > 2.5,
                }
            }
        };
        *guard = Some((song, now));
        change
    }
}

/// Called by the page roughly once a second and on every play/pause/seek.
#[tauri::command]
pub fn player_update(app: AppHandle, state: SongInfo) {
    if state.video_id.is_empty() {
        return;
    }
    let change = app.state::<PlayerState>().replace(state.clone());
    // The overlay keeps its own clock; every report keeps it honest,
    // including small drift while a track buffers.
    overlay::on_player_change(&app, &state, change);
    if !change.any() {
        return;
    }
    if (change.playback || change.song) && !state.paused {
        crate::audio_session::label_soon();
    }
    if change.metadata {
        media::set_metadata(&app, &state);
        tray::set_tooltip(&app, &state);
    }
    media::set_playback(&app, &state);
    app.state::<Registry>().on_player_change(&app, &state, change);
}
