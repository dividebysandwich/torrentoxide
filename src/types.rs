//! Wire types shared between the server (native) and the browser (wasm).
//! Keep this module free of any server-only dependencies.

use serde::{Deserialize, Serialize};

/// High-level lifecycle state of a torrent, as surfaced to the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TorrentState {
    Initializing,
    Live,
    Paused,
    Finished,
    Error,
}

impl TorrentState {
    pub fn label(&self) -> &'static str {
        match self {
            TorrentState::Initializing => "initializing",
            TorrentState::Live => "downloading",
            TorrentState::Paused => "paused",
            TorrentState::Finished => "finished",
            TorrentState::Error => "error",
        }
    }
}

/// A single torrent's current view, refreshed each SSE tick.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TorrentView {
    // NOTE: `u64` (not `usize`) — the wasm client is 32-bit, so a `usize` here
    // would fail to deserialize any id above u32::MAX (e.g. pending placeholders).
    pub id: u64,
    pub name: String,
    pub state: TorrentState,
    /// 0.0 ..= 1.0
    pub progress: f32,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub uploaded_bytes: u64,
    pub down_bps: f64,
    pub up_bps: f64,
    pub eta_secs: Option<u64>,
    pub error: Option<String>,
    pub output_folder: String,
    /// A placeholder for an add still resolving in the background (metadata fetch).
    #[serde(default)]
    pub pending: bool,
    /// Server-recorded rolling (down_bps, up_bps) history for this torrent's sparkline.
    #[serde(default)]
    pub history: Vec<(f64, f64)>,
}

/// The full live snapshot pushed over `/api/events` every second.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StatsSnapshot {
    pub global_down_bps: f64,
    pub global_up_bps: f64,
    /// Server-recorded rolling (down_bps, up_bps) history for the global graph.
    #[serde(default)]
    pub global_hist: Vec<(f64, f64)>,
    pub torrents: Vec<TorrentView>,
}

/// One entry (sub-directory) inside a directory listing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub writable: bool,
}

/// A directory listing, confined to the configured browse root.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DirListing {
    pub path: String,
    /// `None` when `path` is the browse root (can't go higher).
    pub parent: Option<String>,
    pub writable: bool,
    pub entries: Vec<DirEntry>,
}

/// Default paths surfaced to the UI (from the server's `.env`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Defaults {
    pub download_dir: String,
    pub browse_root: String,
    pub auth_enabled: bool,
}

/// Request payload for adding a torrent by magnet link or http(s) URL.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddRequest {
    pub source: String,
    pub output_dir: String,
    pub paused: bool,
}

/// Human-readable byte-rate formatting (e.g. `1.4 MB/s`), shared by UI + labels.
pub fn fmt_speed(bps: f64) -> String {
    fmt_bytes(bps) + "/s"
}

/// Human-readable byte-size formatting (binary units).
pub fn fmt_bytes(bytes: f64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = bytes.max(0.0);
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{v:.0} {}", UNITS[u])
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

/// Format an ETA in seconds as a compact human string.
pub fn fmt_eta(secs: Option<u64>) -> String {
    match secs {
        None => "—".to_string(),
        Some(s) => {
            let h = s / 3600;
            let m = (s % 3600) / 60;
            let sec = s % 60;
            if h > 0 {
                format!("{h}h {m}m")
            } else if m > 0 {
                format!("{m}m {sec}s")
            } else {
                format!("{sec}s")
            }
        }
    }
}
