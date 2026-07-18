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
    /// Increments each time a new history sample is appended (1 Hz). Lets the
    /// client drive a smooth 1-second scroll animation on the graph.
    #[serde(default)]
    pub hist_tick: u64,
    /// Free bytes available on the download folder's filesystem.
    #[serde(default)]
    pub disk_free: u64,
    /// Total bytes on the download folder's filesystem.
    #[serde(default)]
    pub disk_total: u64,
    pub torrents: Vec<TorrentView>,
}

/// Live-adjustable, persisted global settings (rate limits, seeding goals).
/// Stored server-side in `<PERSISTENCE_DIR>/torrentoxide.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Global download rate limit in KiB/s; `0` = unlimited.
    #[serde(default)]
    pub down_limit_kbps: u32,
    /// Global upload rate limit in KiB/s; `0` = unlimited.
    #[serde(default)]
    pub up_limit_kbps: u32,
    /// Auto-pause a torrent's seeding once it reaches `ratio_limit`.
    #[serde(default)]
    pub ratio_enabled: bool,
    /// Seeding ratio (uploaded / downloaded) at which to stop seeding.
    #[serde(default = "default_ratio")]
    pub ratio_limit: f32,
}

fn default_ratio() -> f32 {
    2.0
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            down_limit_kbps: 0,
            up_limit_kbps: 0,
            ratio_enabled: false,
            ratio_limit: default_ratio(),
        }
    }
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

/// Full inspector view of one torrent (files, swarm, trackers, DHT), fetched
/// on demand when the detail modal is open.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct TorrentDetail {
    pub id: u64,
    pub name: String,
    pub output_folder: String,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub uploaded_bytes: u64,
    /// 0.0 ..= 1.0
    pub progress: f32,
    pub files: Vec<FileEntry>,
    pub peers: PeerCounts,
    pub trackers: Vec<TrackerInfo>,
    /// Size of the DHT routing table (rough "known nodes" count).
    pub dht_nodes: u64,
    /// Whether DHT is enabled at all (false → the node count is meaningless).
    pub dht_enabled: bool,
}

/// A single file within a torrent, with its path components for tree building.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    pub index: usize,
    /// Path components, e.g. `["Season 1", "ep01.mkv"]`.
    pub components: Vec<String>,
    pub length: u64,
    pub have_bytes: u64,
    /// Whether this file is currently selected for download.
    pub included: bool,
}

/// Aggregate peer-swarm counts (librqbit does not expose per-peer detail publicly).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct PeerCounts {
    pub live: u64,
    pub connecting: u64,
    pub queued: u64,
    pub seen: u64,
    pub dead: u64,
}

/// A tracker configured for a torrent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackerInfo {
    pub url: String,
    /// Protocol scheme (udp / http / https / wss), upper-cased for the badge.
    pub scheme: String,
    pub host: String,
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
    /// Indices of files to download; `None` = all files.
    #[serde(default)]
    pub only_files: Option<Vec<usize>>,
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
