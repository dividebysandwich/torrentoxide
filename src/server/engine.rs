//! librqbit integration: session/api lifecycle, actions, and stats snapshots.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context};
use librqbit::api::{ApiTorrentListOpts, TorrentIdOrHash};
use librqbit::{
    AddTorrent, AddTorrentOptions, Api, Session, SessionOptions, SessionPersistenceConfig,
    TorrentStatsState,
};

use crate::server::config::AppConfig;
use crate::types::{DirEntry, DirListing, StatsSnapshot, TorrentState, TorrentView};

const MIB: f64 = 1024.0 * 1024.0;

pub struct Engine {
    pub api: Api,
    pub config: Arc<AppConfig>,
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
        Ok(Arc::new(Self { api, config }))
    }

    // --- torrent actions ---------------------------------------------------

    pub async fn add_url(
        &self,
        source: String,
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
            Duration::from_secs(90),
            self.api.api_add_torrent(AddTorrent::from_url(source), Some(opts)),
        )
        .await
        .context("timed out while adding torrent")?
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

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
            Duration::from_secs(90),
            self.api.api_add_torrent(AddTorrent::from_bytes(bytes), Some(opts)),
        )
        .await
        .context("timed out while adding torrent")?
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn pause(&self, id: usize) -> anyhow::Result<()> {
        self.api
            .api_torrent_action_pause(TorrentIdOrHash::Id(id))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn resume(&self, id: usize) -> anyhow::Result<()> {
        self.api
            .api_torrent_action_start(TorrentIdOrHash::Id(id))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    /// Cancel a torrent but keep any downloaded files.
    pub async fn cancel(&self, id: usize) -> anyhow::Result<()> {
        self.api
            .api_torrent_action_forget(TorrentIdOrHash::Id(id))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    /// Cancel a torrent AND delete its files from disk.
    pub async fn delete(&self, id: usize) -> anyhow::Result<()> {
        self.api
            .api_torrent_action_delete(TorrentIdOrHash::Id(id))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }

    // --- stats -------------------------------------------------------------

    pub fn snapshot(&self) -> StatsSnapshot {
        let list = self
            .api
            .api_torrent_list_ext(ApiTorrentListOpts { with_stats: true });

        let mut torrents = Vec::with_capacity(list.torrents.len());
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

            torrents.push(TorrentView {
                id,
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
            });
        }

        torrents.sort_by_key(|t| t.id);
        StatsSnapshot {
            global_down_bps: global_down,
            global_up_bps: global_up,
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
