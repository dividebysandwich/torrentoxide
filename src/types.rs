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

/// What kind of media a category holds (drives library grouping later).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
    Movie,
    Tv,
    Other,
}

impl Default for MediaKind {
    fn default() -> Self {
        MediaKind::Other
    }
}

impl MediaKind {
    pub fn label(&self) -> &'static str {
        match self {
            MediaKind::Movie => "movie",
            MediaKind::Tv => "tv",
            MediaKind::Other => "other",
        }
    }
}

/// A user-defined category mapping to a sub-directory under the download folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Category {
    /// Stable key (server-derived from `name`).
    #[serde(default)]
    pub slug: String,
    pub name: String,
    /// Path relative to the download folder (e.g. `Movies`, `TV/Anime`).
    pub subdir: String,
    #[serde(default)]
    pub kind: MediaKind,
}

/// Video resolution tiers, ordered worst → best (discriminants used for ranking).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    Unknown = 0,
    R480 = 1,
    R720 = 2,
    R1080 = 3,
    R2160 = 4,
}

impl Resolution {
    pub const ALL: [Resolution; 5] = [
        Self::Unknown,
        Self::R480,
        Self::R720,
        Self::R1080,
        Self::R2160,
    ];
    pub fn label(&self) -> &'static str {
        match self {
            Self::Unknown => "any",
            Self::R480 => "480p",
            Self::R720 => "720p",
            Self::R1080 => "1080p",
            Self::R2160 => "2160p",
        }
    }
    pub fn rank(&self) -> i64 {
        *self as i64
    }
}

/// Release source tiers, ordered worst → best.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Unknown = 0,
    Cam = 1,
    Hdtv = 2,
    WebRip = 3,
    WebDl = 4,
    Bluray = 5,
    Remux = 6,
}

impl Source {
    pub fn rank(&self) -> i64 {
        *self as i64
    }
}

/// How a quality profile treats HDR / Dolby Vision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HdrPref {
    /// Don't care.
    Ignore,
    /// Accept SDR, but rank HDR higher and treat it as an upgrade target.
    Prefer,
    /// Only accept HDR / DV releases.
    Require,
}

impl Default for HdrPref {
    fn default() -> Self {
        HdrPref::Prefer
    }
}

impl HdrPref {
    pub const ALL: [HdrPref; 3] = [Self::Ignore, Self::Prefer, Self::Require];
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ignore => "ignore",
            Self::Prefer => "prefer",
            Self::Require => "require",
        }
    }
}

fn default_true() -> bool {
    true
}

/// A quality profile: what releases are acceptable and how upgrades are chosen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QualityProfile {
    /// Stable id (server-derived from name if empty).
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// Minimum acceptable resolution (inclusive).
    pub min_resolution: Resolution,
    /// Stop upgrading once this resolution (and HDR pref, if any) is met.
    pub cutoff_resolution: Resolution,
    #[serde(default)]
    pub hdr: HdrPref,
    /// Required languages (release must match one when non-empty; an untagged
    /// release is assumed to match). Free-form, e.g. `english`.
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub preferred_groups: Vec<String>,
    #[serde(default)]
    pub blocked_groups: Vec<String>,
    #[serde(default = "default_true")]
    pub upgrade_allowed: bool,
}

/// One TMDb search hit (movie or TV), surfaced to the wanted/library UI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaSearchResult {
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<i32>,
    #[serde(default)]
    pub overview: String,
    pub poster_path: Option<String>,
    pub is_tv: bool,
}

/// Normalize a media title for fuzzy comparison: lowercase, alphanumerics and
/// single spaces only. Shared by the scanner, monitor and matcher.
pub fn norm_title(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    cleaned.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A downloaded movie found by the library scan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryMovie {
    pub title: String,
    pub year: Option<i32>,
    pub resolution: String,
    pub size: u64,
    pub path: String,
}

/// One downloaded episode of a show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryEpisode {
    pub season: i32,
    pub episode: i32,
    pub resolution: String,
    pub path: String,
}

/// A show with its downloaded episodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryShow {
    pub title: String,
    pub episodes: Vec<LibraryEpisode>,
}

/// The scanned media library (filename-based).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Library {
    pub movies: Vec<LibraryMovie>,
    pub shows: Vec<LibraryShow>,
    pub file_count: usize,
    /// Unix seconds of the last scan (0 = never).
    pub scanned_at: u64,
}

impl Library {
    /// Is a movie with this (normalized) title already on disk?
    pub fn has_movie(&self, title: &str, year: Option<i32>) -> bool {
        let nt = norm_title(title);
        self.movies.iter().any(|m| {
            norm_title(&m.title) == nt
                && (year.is_none() || m.year.is_none() || m.year == year)
        })
    }

    /// Is this episode of this (normalized) show already on disk?
    pub fn has_episode(&self, title: &str, season: i32, episode: i32) -> bool {
        let nt = norm_title(title);
        self.shows.iter().any(|s| {
            norm_title(&s.title) == nt
                && s.episodes.iter().any(|e| e.season == season && e.episode == episode)
        })
    }
}

/// A configured Torznab indexer (e.g. a Jackett indexer endpoint).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Indexer {
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// Torznab base URL, e.g.
    /// `http://127.0.0.1:9117/api/v2.0/indexers/all/results/torznab/`.
    pub torznab_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// A configured RSS feed for passive auto-download into a category.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RssFeed {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub url: String,
    /// Category slug that grabs land in (empty = default download dir).
    #[serde(default)]
    pub category: String,
    /// Quality profile id used to filter/rank items (empty = accept everything).
    #[serde(default)]
    pub quality_profile: String,
    #[serde(default)]
    pub auto_download: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// A candidate release from an indexer search or an RSS feed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub title: String,
    /// Download link (magnet or `.torrent` URL) handed to the engine.
    pub url: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub seeders: Option<u32>,
    #[serde(default)]
    pub indexer: String,
}

/// One grab recorded in history (for dedup, audit and upgrade tracking).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrabHistoryEntry {
    /// Dedup key (release download url).
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub score: i64,
    /// Unix seconds when grabbed.
    #[serde(default)]
    pub grabbed_at: u64,
}

/// Provider status surfaced to the UI (never exposes the raw key).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub tmdb_key_set: bool,
    /// Result of the last connection test, if any (`Ok`/error message).
    pub tmdb_status: Option<String>,
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
