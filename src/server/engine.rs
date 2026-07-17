//! librqbit integration: session/api lifecycle, actions, and stats snapshots.

use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use librqbit::api::{ApiTorrentListOpts, TorrentIdOrHash};
use librqbit::{
    AddTorrent, AddTorrentOptions, Api, Session, SessionOptions, SessionPersistenceConfig,
    TorrentStatsState,
};
use tokio::sync::watch;

use crate::server::config::AppConfig;
use crate::types::{DirEntry, DirListing, StatsSnapshot, TorrentState, TorrentView};

const MIB: f64 = 1024.0 * 1024.0;

/// Background adds get synthetic ids well above any real librqbit id (small),
/// so they never collide with managed-torrent ids. Wire ids are `u64`.
const PENDING_ID_BASE: u64 = 1u64 << 48;

/// How many 1-second samples the global graph retains (~2 minutes).
const GLOBAL_HIST_LEN: usize = 120;
/// How many 1-second samples each per-torrent sparkline retains.
const ROW_HIST_LEN: usize = 48;

/// Rolling traffic history, recorded server-side so it survives client refreshes
/// and keeps advancing even when no browser is connected.
#[derive(Default)]
struct History {
    global: VecDeque<(f64, f64)>,
    per_torrent: HashMap<usize, VecDeque<(f64, f64)>>,
}
/// Give up resolving a magnet's metadata after this long.
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(300);
/// Keep a failed pending entry visible this long before auto-expiring it.
const FAILED_TTL: Duration = Duration::from_secs(90);

#[derive(Clone)]
enum PendingStatus {
    Resolving,
    Failed(String),
}

/// An add that is still being processed in the background (e.g. a magnet whose
/// metadata is being fetched from peers). Surfaced to the UI as a placeholder row.
struct PendingAdd {
    id: u64,
    label: String,
    output_dir: String,
    status: PendingStatus,
    at: Instant,
}

pub struct Engine {
    pub api: Api,
    pub config: Arc<AppConfig>,
    pending: Mutex<Vec<PendingAdd>>,
    next_pending: AtomicU64,
    history: Mutex<History>,
    /// Latest snapshot (with embedded history); updated by the sampler, relayed by SSE.
    tx: watch::Sender<Arc<StatsSnapshot>>,
}

impl Engine {
    pub async fn new(config: Arc<AppConfig>) -> anyhow::Result<Arc<Self>> {
        let opts = SessionOptions {
            persistence: Some(SessionPersistenceConfig::Json {
                folder: Some(config.persistence_dir.clone()),
            }),
            fastresume: true,
            ..Default::default()
        };
        let session = Session::new_with_opts(config.download_dir.clone(), opts)
            .await
            .context("failed to start librqbit session")?;
        let api = Api::new(session, None);

        let (tx, _rx) = watch::channel(Arc::new(StatsSnapshot::default()));
        let engine = Arc::new(Self {
            api,
            config,
            pending: Mutex::new(Vec::new()),
            next_pending: AtomicU64::new(0),
            history: Mutex::new(History::default()),
            tx,
        });

        // Sample stats + record history once per second, independent of any
        // connected client, and broadcast the result to SSE subscribers.
        let sampler = engine.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let snapshot = sampler.sample_and_record();
                // `send_replace` always stores the value (even with zero subscribers),
                // so a client connecting later immediately gets the full history.
                let _ = sampler.tx.send_replace(Arc::new(snapshot));
            }
        });

        Ok(engine)
    }

    /// Subscribe to the live snapshot stream (used by the SSE endpoint).
    pub fn subscribe(&self) -> watch::Receiver<Arc<StatsSnapshot>> {
        self.tx.subscribe()
    }

    /// The most recent snapshot recorded by the sampler.
    pub fn current(&self) -> Arc<StatsSnapshot> {
        self.tx.borrow().clone()
    }

    // --- adding torrents ---------------------------------------------------

    /// Add a magnet link or http(s) URL WITHOUT blocking: validates the target
    /// directory synchronously (fast failure for bad input), then resolves
    /// metadata in the background. A placeholder row appears immediately.
    pub fn spawn_add_url(
        self: &Arc<Self>,
        source: String,
        output_dir: String,
        paused: bool,
    ) -> anyhow::Result<()> {
        let source = source.trim().to_string();
        if source.is_empty() {
            bail!("no magnet link or URL provided");
        }
        let dir = self.confine(&output_dir)?;
        std::fs::create_dir_all(&dir).ok();
        let dir = dir.to_string_lossy().into_owned();

        let id = self.push_pending(source_label(&source), dir.clone());
        let engine = self.clone();
        tokio::spawn(async move {
            let opts = AddTorrentOptions {
                output_folder: Some(dir),
                paused,
                overwrite: true,
                ..Default::default()
            };
            let outcome = match tokio::time::timeout(
                RESOLVE_TIMEOUT,
                engine.api.api_add_torrent(AddTorrent::from_url(source), Some(opts)),
            )
            .await
            {
                Err(_) => Err("timed out fetching metadata from peers".to_string()),
                Ok(Err(e)) => Err(e.to_string()),
                Ok(Ok(_)) => Ok(()),
            };
            engine.finish_pending(id, outcome);
        });
        Ok(())
    }

    /// Add from raw `.torrent` bytes (metadata is embedded, so this is fast and
    /// stays synchronous to give the uploader immediate success/error feedback).
    pub async fn add_bytes(
        &self,
        bytes: Vec<u8>,
        output_dir: String,
        paused: bool,
    ) -> anyhow::Result<()> {
        let dir = self.confine(&output_dir)?;
        std::fs::create_dir_all(&dir).ok();
        let opts = AddTorrentOptions {
            output_folder: Some(dir.to_string_lossy().into_owned()),
            paused,
            overwrite: true,
            ..Default::default()
        };
        tokio::time::timeout(
            Duration::from_secs(60),
            self.api.api_add_torrent(AddTorrent::from_bytes(bytes), Some(opts)),
        )
        .await
        .context("timed out while adding torrent")?
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    fn push_pending(&self, label: String, output_dir: String) -> u64 {
        let id = PENDING_ID_BASE + self.next_pending.fetch_add(1, Ordering::Relaxed);
        self.pending.lock().unwrap().push(PendingAdd {
            id,
            label,
            output_dir,
            status: PendingStatus::Resolving,
            at: Instant::now(),
        });
        id
    }

    fn finish_pending(&self, id: u64, outcome: Result<(), String>) {
        let mut pending = self.pending.lock().unwrap();
        match outcome {
            // Success: the real torrent now shows up in the managed list; drop the placeholder.
            Ok(()) => pending.retain(|p| p.id != id),
            Err(msg) => {
                if let Some(p) = pending.iter_mut().find(|p| p.id == id) {
                    p.status = PendingStatus::Failed(msg);
                    p.at = Instant::now();
                }
            }
        }
    }

    /// Dismiss a failed pending placeholder row.
    pub fn dismiss_pending(&self, id: u64) {
        self.pending.lock().unwrap().retain(|p| p.id != id);
    }

    // --- torrent actions ---------------------------------------------------

    pub async fn pause(&self, id: u64) -> anyhow::Result<()> {
        self.api
            .api_torrent_action_pause(TorrentIdOrHash::Id(id as usize))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn resume(&self, id: u64) -> anyhow::Result<()> {
        self.api
            .api_torrent_action_start(TorrentIdOrHash::Id(id as usize))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    /// Cancel a torrent but keep any downloaded files.
    pub async fn cancel(&self, id: u64) -> anyhow::Result<()> {
        self.api
            .api_torrent_action_forget(TorrentIdOrHash::Id(id as usize))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    /// Cancel a torrent AND delete its files from disk.
    pub async fn delete(&self, id: u64) -> anyhow::Result<()> {
        self.api
            .api_torrent_action_delete(TorrentIdOrHash::Id(id as usize))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    // --- stats -------------------------------------------------------------

    /// Sample current stats, fold them into the rolling history, and produce the
    /// snapshot (with history embedded) that gets broadcast to clients.
    fn sample_and_record(&self) -> StatsSnapshot {
        let list = self
            .api
            .api_torrent_list_ext(ApiTorrentListOpts { with_stats: true });

        // id (usize) kept alongside each view so we can key its history.
        let mut reals: Vec<(usize, TorrentView)> = Vec::with_capacity(list.torrents.len());
        let mut global_down = 0.0f64;
        let mut global_up = 0.0f64;

        for t in list.torrents {
            let Some(id) = t.id else { continue };
            let Some(stats) = t.stats else { continue };

            let (down_bps, up_bps) = stats
                .live
                .as_ref()
                .map(|l| (l.download_speed.mbps * MIB, l.upload_speed.mbps * MIB))
                .unwrap_or((0.0, 0.0));
            global_down += down_bps;
            global_up += up_bps;

            let progress = if stats.total_bytes > 0 {
                (stats.progress_bytes as f64 / stats.total_bytes as f64) as f32
            } else {
                0.0
            };

            let state = if stats.finished {
                TorrentState::Finished
            } else {
                match stats.state {
                    TorrentStatsState::Initializing => TorrentState::Initializing,
                    TorrentStatsState::Live => TorrentState::Live,
                    TorrentStatsState::Paused => TorrentState::Paused,
                    TorrentStatsState::Error => TorrentState::Error,
                }
            };

            let eta_secs = if down_bps > 1.0 && !stats.finished && stats.total_bytes >= stats.progress_bytes {
                Some(((stats.total_bytes - stats.progress_bytes) as f64 / down_bps) as u64)
            } else {
                None
            };

            reals.push((
                id,
                TorrentView {
                    id: id as u64,
                    name: t.name.unwrap_or_else(|| t.info_hash.clone()),
                    state,
                    progress,
                    total_bytes: stats.total_bytes,
                    downloaded_bytes: stats.progress_bytes,
                    uploaded_bytes: stats.uploaded_bytes,
                    down_bps,
                    up_bps,
                    eta_secs,
                    error: stats.error,
                    output_folder: t.output_folder,
                    pending: false,
                    history: Vec::new(),
                },
            ));
        }

        // Record the sample into the rolling buffers and read the history back
        // into each view / the global series.
        let global_hist = {
            let mut hist = self.history.lock().unwrap();
            hist.global.push_back((global_down, global_up));
            while hist.global.len() > GLOBAL_HIST_LEN {
                hist.global.pop_front();
            }

            let live: HashSet<usize> = reals.iter().map(|(id, _)| *id).collect();
            for (id, view) in reals.iter_mut() {
                let dq = hist.per_torrent.entry(*id).or_default();
                dq.push_back((view.down_bps, view.up_bps));
                while dq.len() > ROW_HIST_LEN {
                    dq.pop_front();
                }
                view.history = dq.iter().copied().collect();
            }
            hist.per_torrent.retain(|id, _| live.contains(id));

            hist.global.iter().copied().collect::<Vec<_>>()
        };

        let mut torrents: Vec<TorrentView> = reals.into_iter().map(|(_, v)| v).collect();
        torrents.sort_by_key(|t| t.id);

        // Append background-add placeholders (and expire stale failed ones).
        {
            let mut pending = self.pending.lock().unwrap();
            pending.retain(|p| !(matches!(p.status, PendingStatus::Failed(_)) && p.at.elapsed() > FAILED_TTL));
            for p in pending.iter() {
                let (state, error) = match &p.status {
                    PendingStatus::Resolving => (TorrentState::Initializing, None),
                    PendingStatus::Failed(msg) => (TorrentState::Error, Some(msg.clone())),
                };
                torrents.push(TorrentView {
                    id: p.id,
                    name: p.label.clone(),
                    state,
                    progress: 0.0,
                    total_bytes: 0,
                    downloaded_bytes: 0,
                    uploaded_bytes: 0,
                    down_bps: 0.0,
                    up_bps: 0.0,
                    eta_secs: None,
                    error,
                    output_folder: p.output_dir.clone(),
                    pending: true,
                    history: Vec::new(),
                });
            }
        }

        StatsSnapshot {
            global_down_bps: global_down,
            global_up_bps: global_up,
            global_hist,
            torrents,
        }
    }

    // --- directory browser -------------------------------------------------

    pub fn browse(&self, path: Option<String>) -> anyhow::Result<DirListing> {
        let dir = match path {
            Some(p) if !p.trim().is_empty() => self.confine(&p)?,
            _ => self.config.browse_root.clone(),
        };
        if !dir.is_dir() {
            bail!("{} is not a directory", dir.display());
        }

        let mut entries = Vec::new();
        if let Ok(read) = std::fs::read_dir(&dir) {
            for e in read.flatten() {
                let p = e.path();
                if p.is_dir() {
                    entries.push(DirEntry {
                        name: e.file_name().to_string_lossy().into_owned(),
                        path: p.to_string_lossy().into_owned(),
                        writable: is_writable(&p),
                    });
                }
            }
        }
        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        let parent = if dir == self.config.browse_root {
            None
        } else {
            dir.parent().map(|p| p.to_string_lossy().into_owned())
        };

        Ok(DirListing {
            path: dir.to_string_lossy().into_owned(),
            parent,
            writable: is_writable(&dir),
            entries,
        })
    }

    pub fn make_dir(&self, parent: String, name: String) -> anyhow::Result<DirListing> {
        let name = name.trim();
        if name.is_empty() || name.contains('/') || name.contains('\\') || name == ".." || name == "." {
            bail!("invalid folder name");
        }
        let base = self.confine(&parent)?;
        let target = base.join(name);
        // Re-confine the joined path as a defense-in-depth check.
        let target = self.confine(&target.to_string_lossy())?;
        std::fs::create_dir_all(&target)
            .with_context(|| format!("failed to create {}", target.display()))?;
        self.browse(Some(base.to_string_lossy().into_owned()))
    }

    /// Resolve a requested path and assert it lives inside `browse_root`.
    /// Handles paths that don't exist yet (new folders) and resolves symlinks
    /// on the existing prefix to prevent escapes.
    pub fn confine(&self, requested: &str) -> anyhow::Result<PathBuf> {
        let req = Path::new(requested);
        let abs = if req.is_absolute() {
            req.to_path_buf()
        } else {
            self.config.browse_root.join(req)
        };
        let normalized = normalize_lexical(&abs);

        // Walk up to the deepest existing ancestor, remembering the missing tail.
        let mut existing = normalized.clone();
        let mut tail: Vec<OsString> = Vec::new();
        while !existing.exists() {
            match existing.file_name() {
                Some(name) => {
                    tail.push(name.to_os_string());
                    match existing.parent() {
                        Some(p) => existing = p.to_path_buf(),
                        None => break,
                    }
                }
                None => break,
            }
        }
        let canon = std::fs::canonicalize(&existing).unwrap_or(existing);
        let mut full = canon;
        for name in tail.iter().rev() {
            full.push(name);
        }

        if !full.starts_with(&self.config.browse_root) {
            bail!(
                "path {} is outside the allowed root {}",
                full.display(),
                self.config.browse_root.display()
            );
        }
        Ok(full)
    }
}

/// Derive a friendly label for a pending add from a magnet link or URL.
fn source_label(source: &str) -> String {
    if let Some(rest) = source.strip_prefix("magnet:?") {
        for kv in rest.split('&') {
            if let Some(v) = kv.strip_prefix("dn=") {
                let name = percent_decode(v);
                if !name.trim().is_empty() {
                    return name;
                }
            }
        }
        for kv in rest.split('&') {
            if let Some(v) = kv.strip_prefix("xt=urn:btih:") {
                let short = &v[..v.len().min(16)];
                return format!("magnet {short}…");
            }
        }
        return "magnet link".to_string();
    }
    source
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(source)
        .to_string()
}

/// Minimal application/x-www-form-urlencoded percent-decoder for display labels.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                (Some(a), Some(b)) => {
                    out.push(a * 16 + b);
                    i += 3;
                }
                _ => {
                    out.push(bytes[i]);
                    i += 1;
                }
            },
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Lexically resolve `.` and `..` without touching the filesystem.
fn normalize_lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Best-effort writability hint for the UI (not a security boundary).
fn is_writable(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(m) => !m.permissions().readonly(),
        Err(_) => false,
    }
}
