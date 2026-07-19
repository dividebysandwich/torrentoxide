//! PVR subsystem: categories now; feeds, indexers, library, wanted and the
//! automation loops arrive in later phases. Owns the redb store and holds
//! handles to the engine + config so it can create directories and (later)
//! trigger downloads.

pub mod feed;
pub mod indexer;
pub mod meta;
pub mod quality;
pub mod scan;
pub mod store;
pub mod xmlparse;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};

use crate::server::config::AppConfig;
use crate::server::engine::Engine;
use crate::types::{
    CalendarEntry, Category, GrabHistoryEntry, Indexer, Library, MediaKind, MediaSearchResult,
    ProviderInfo, QualityProfile, Release, RssFeed, TorrentState, WantedItem, WantedKind,
};
use meta::MetadataClient;
use store::PvrStore;

const TMDB_KEY: &str = "tmdb_api_key";
const FEED_POLL_MINS_KEY: &str = "feed_poll_mins";
/// Default feed poll cadence (minutes) when unconfigured.
const DEFAULT_FEED_POLL_MINS: u64 = 15;
/// How often the download tree is re-scanned into the library.
const SCAN_INTERVAL: Duration = Duration::from_secs(3600);
/// How often finished TV downloads are imported into Show/Season folders.
const IMPORT_INTERVAL: Duration = Duration::from_secs(300);
const IMPORT_MODE_KEY: &str = "import_mode";
/// Forget in-flight grab tracking after this long — a grab resolves (success or
/// the broadcast failure) well within the engine's resolve timeout.
const GRAB_TRACK_TTL: u64 = 900;

const VIDEO_EXTS: [&str; 9] = ["mkv", "mp4", "avi", "m4v", "mov", "ts", "webm", "mpg", "wmv"];

/// Recovery context for an in-flight PVR grab, keyed by its release URL. If the
/// download fails (metadata fetch), this drives a re-search for an alternative
/// release of the same episode/movie so automation doesn't stall.
#[derive(Clone)]
struct GrabContext {
    /// Grab-history dedup key (episode/movie key for the monitor, URL otherwise).
    dedup_id: String,
    /// Canonical title to match/search alternatives against.
    title: String,
    season: Option<i32>,
    episode: Option<i32>,
    year: Option<i32>,
    category: String,
    /// Quality profile id (empty = none / seeders-ranked).
    profile_id: String,
    /// Grab source label recorded in history ("monitor", a feed name, "search").
    source: String,
    /// Release URLs already attempted for this target (never retried).
    tried: HashSet<String>,
    /// When registered (unix secs); used to prune long-resolved grabs.
    at: u64,
}

/// How a finished download is placed into the organized library.
#[derive(Clone, Copy, PartialEq)]
enum ImportMode {
    /// Relocate the file (one clean copy; stops seeding — the torrent is forgotten).
    Move,
    /// Link the file into place, keeping the download seedable (same filesystem).
    Hardlink,
    /// Duplicate the file (doubles disk, keeps seeding).
    Copy,
}

fn parse_import_mode(s: &str) -> ImportMode {
    match s.trim().to_ascii_lowercase().as_str() {
        "hardlink" => ImportMode::Hardlink,
        "copy" => ImportMode::Copy,
        _ => ImportMode::Move,
    }
}

fn is_video_name(name: &str) -> bool {
    name.rsplit('.')
        .next()
        .map(|e| VIDEO_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Replace characters that are illegal/awkward in file names.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The show sub-folder name for an existing episode path (component just under a
/// category directory), e.g. `.../TV Shows/The Show (2026)/…` → `The Show (2026)`.
fn show_folder_of(ep_path: &str, cat_dirs: &[PathBuf]) -> Option<String> {
    let p = Path::new(ep_path);
    for d in cat_dirs {
        if let Ok(rel) = p.strip_prefix(d) {
            return rel
                .components()
                .next()
                .and_then(|c| c.as_os_str().to_str())
                .map(String::from);
        }
    }
    None
}

/// Resolve `(folder_name, display_title)` for a release: reuse an existing
/// library show's folder when the title matches, else create a new one.
fn resolve_show(
    parsed_title: &str,
    library: &Library,
    cat_dirs: &[PathBuf],
) -> (String, String) {
    for s in &library.shows {
        if title_matches(&s.title, parsed_title) {
            let folder = s
                .episodes
                .first()
                .and_then(|ep| show_folder_of(&ep.path, cat_dirs))
                .unwrap_or_else(|| s.title.clone());
            return (folder, s.title.clone());
        }
    }
    let clean = parsed_title.trim().to_string();
    (clean.clone(), clean)
}
/// How often monitored wanted items are checked (≈4×/day).
const MONITOR_INTERVAL: Duration = Duration::from_secs(6 * 3600);

/// Does a parsed release title plausibly match the wanted title?
fn title_matches(a: &str, b: &str) -> bool {
    let na = crate::types::norm_title(a);
    let nb = crate::types::norm_title(b);
    if na.is_empty() || nb.is_empty() {
        return false;
    }
    na == nb || na.contains(&nb) || nb.contains(&na) || strsim::jaro_winkler(&na, &nb) > 0.9
}

/// Pick the highest-scoring acceptable release that matches the wanted title
/// (and, for episodes, the exact season/episode).
fn best_acceptable(
    releases: &[Release],
    profile: Option<&QualityProfile>,
    title: &str,
    season: Option<i32>,
    episode: Option<i32>,
) -> Option<(Release, i64)> {
    releases
        .iter()
        .filter_map(|r| {
            let parsed = quality::parse_release(&r.title);
            if !title_matches(&parsed.title, title) {
                return None;
            }
            if let (Some(s), Some(e)) = (season, episode) {
                if parsed.season != Some(s) || parsed.episode != Some(e) {
                    return None;
                }
            }
            let sc = match profile {
                Some(p) => quality::score(&parsed, p)?,
                None => r.seeders.unwrap_or(0) as i64,
            };
            Some((r.clone(), sc))
        })
        .max_by_key(|(_, sc)| *sc)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub struct Pvr {
    store: Arc<PvrStore>,
    engine: Arc<Engine>,
    config: Arc<AppConfig>,
    meta: MetadataClient,
    http: reqwest::Client,
    /// In-flight grabs (release URL → recovery context) for failure recovery.
    grabs: Mutex<HashMap<String, GrabContext>>,
}

impl Pvr {
    pub fn new(config: Arc<AppConfig>, engine: Arc<Engine>) -> Result<Arc<Self>> {
        let db_path = config.persistence_dir.join("pvr.redb");
        let store = Arc::new(PvrStore::open(&db_path)?);
        let pvr = Arc::new(Self {
            store,
            engine,
            config,
            meta: MetadataClient::new(),
            http: reqwest::Client::new(),
            grabs: Mutex::new(HashMap::new()),
        });

        // Poll auto-download RSS feeds on a fixed interval (first tick fires soon
        // after startup; grab-history dedup prevents re-grabbing known items).
        // Populate the library once at startup so the monitor knows what's on disk.
        pvr.scan_library();

        let poller = pvr.clone();
        tokio::spawn(async move {
            loop {
                // Re-read the (runtime-configurable) cadence each cycle.
                let mins = poller.feed_poll_mins().max(1);
                tokio::time::sleep(Duration::from_secs(mins * 60)).await;
                if let Err(e) = poller.poll_feeds().await {
                    tracing::warn!("feed poll error: {e}");
                }
            }
        });

        // Re-scan the download tree into the library on a fixed interval
        // (blocking walk runs on a blocking thread; startup scan already ran).
        let scanner = pvr.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(SCAN_INTERVAL);
            interval.tick().await;
            loop {
                interval.tick().await;
                let s = scanner.clone();
                let _ = tokio::task::spawn_blocking(move || s.scan_library()).await;
            }
        });

        // Check monitored wanted items for missing/upgradeable releases.
        let monitor = pvr.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(MONITOR_INTERVAL);
            loop {
                interval.tick().await;
                if let Err(e) = monitor.run_monitor().await {
                    tracing::warn!("monitor error: {e}");
                }
            }
        });

        // Import finished TV downloads into Show/Season folders.
        let importer = pvr.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(IMPORT_INTERVAL);
            loop {
                interval.tick().await;
                importer.import_and_reap().await;
            }
        });

        // Recover stalled auto-downloads: when a grab's metadata fetch fails,
        // drop the dead release and grab an alternative for the same target.
        let recovery = pvr.clone();
        tokio::spawn(async move {
            let mut rx = recovery.engine.subscribe_add_failures();
            loop {
                match rx.recv().await {
                    Ok(f) => recovery.handle_add_failure(&f.source, &f.error).await,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Ok(pvr)
    }

    // --- library -----------------------------------------------------------

    /// Walk the download tree, rebuild the library snapshot and persist it.
    pub fn scan_library(&self) -> Library {
        let cats = self.store.list_categories().unwrap_or_default();
        let imported = self.store.imported_paths().unwrap_or_default();
        let lib = scan::scan(&self.config.download_dir, now_secs(), &cats, &imported);
        let _ = self.store.set_library(&lib);
        lib
    }

    pub fn library(&self) -> Library {
        self.store.get_library().unwrap_or_default()
    }

    pub fn import_mode(&self) -> String {
        self.store
            .get_config(IMPORT_MODE_KEY)
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "move".to_string())
    }

    pub fn set_import_mode(&self, mode: String) -> Result<()> {
        let m = match mode.trim().to_ascii_lowercase().as_str() {
            "hardlink" => "hardlink",
            "copy" => "copy",
            _ => "move",
        };
        self.store.set_config(IMPORT_MODE_KEY, m)
    }

    /// Run the import, then (for moved torrents) forget them so librqbit doesn't
    /// re-download the now-missing files. Returns the number of files imported.
    pub async fn import_and_reap(self: &Arc<Self>) -> usize {
        let me = self.clone();
        let (count, forget) = tokio::task::spawn_blocking(move || me.import_finished())
            .await
            .unwrap_or((0, Vec::new()));
        for id in forget {
            let _ = self.engine.cancel(id).await;
        }
        count
    }

    /// Place finished TV-category downloads into `<cat>/<Show>/Season NN/`, renamed
    /// `<Show> - SxxEyy.ext`, using the configured import mode. Only active torrents
    /// are considered, so the pre-existing library is untouched. Returns
    /// `(files_imported, torrent_ids_to_forget)` (the latter only for Move).
    pub fn import_finished(&self) -> (usize, Vec<u64>) {
        let mode = parse_import_mode(&self.import_mode());
        let cats = self.store.list_categories().unwrap_or_default();
        let library = self.store.get_library().unwrap_or_default();
        let snapshot = self.engine.current();

        let all_cat_dirs: Vec<PathBuf> = cats
            .iter()
            .filter_map(|c| self.category_dir(&c.subdir).ok())
            .collect();
        let tv_dirs: Vec<PathBuf> = cats
            .iter()
            .filter(|c| c.kind == MediaKind::Tv)
            .filter_map(|c| self.category_dir(&c.subdir).ok())
            .collect();

        let mut imported = 0usize;
        let mut moved: std::collections::HashSet<u64> = std::collections::HashSet::new();
        for t in &snapshot.torrents {
            if t.pending || !matches!(t.state, TorrentState::Finished) {
                continue;
            }
            let of = Path::new(&t.output_folder);
            let Some(cat_dir) = tv_dirs.iter().find(|d| of == d.as_path() || of.starts_with(d))
            else {
                continue;
            };
            let detail = match self.engine.detail(t.id) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let base = Path::new(&detail.output_folder);
            for f in &detail.files {
                let mut src = base.to_path_buf();
                for comp in &f.components {
                    src.push(comp);
                }
                let Some(fname) = src.file_name().and_then(|n| n.to_str()).map(String::from) else {
                    continue;
                };
                if !is_video_name(&fname) {
                    continue;
                }
                let src_str = src.to_string_lossy().into_owned();
                if self.store.is_imported(&src_str).unwrap_or(false) {
                    continue;
                }

                let parsed = quality::parse_release(&fname);
                let se = scan::extract_se(&fname);
                let episode = parsed.episode.or(se.1);
                let season = parsed.season.or(se.0).or(Some(1));
                let (Some(s), Some(e)) = (season, episode) else {
                    continue;
                };

                let (folder, display) = resolve_show(&parsed.title, &library, &all_cat_dirs);
                let (folder, display) = (sanitize(&folder), sanitize(&display));
                if folder.is_empty() {
                    continue;
                }
                let ext = src.extension().and_then(|x| x.to_str()).unwrap_or("mkv");
                let target_dir = cat_dir.join(&folder).join(format!("Season {s:02}"));
                let target = target_dir.join(format!("{display} - S{s:02}E{e:02}.{ext}"));

                if target.exists() {
                    // Already organized; in Move mode drop the redundant download.
                    if mode == ImportMode::Move {
                        let _ = std::fs::remove_file(&src);
                        moved.insert(t.id);
                    }
                    let _ = self.store.mark_imported(&src_str);
                    continue;
                }
                if std::fs::create_dir_all(&target_dir).is_err() {
                    continue;
                }
                let placed = match mode {
                    ImportMode::Move => {
                        std::fs::rename(&src, &target).is_ok()
                            || (std::fs::copy(&src, &target).is_ok()
                                && std::fs::remove_file(&src).is_ok())
                    }
                    ImportMode::Hardlink => {
                        std::fs::hard_link(&src, &target).is_ok()
                            || std::fs::copy(&src, &target).is_ok()
                    }
                    ImportMode::Copy => std::fs::copy(&src, &target).is_ok(),
                };
                if placed {
                    let _ = self.store.mark_imported(&src_str);
                    imported += 1;
                    if mode == ImportMode::Move {
                        moved.insert(t.id);
                    }
                    tracing::info!("imported {} -> {}", src.display(), target.display());
                }
            }
        }
        (imported, moved.into_iter().collect())
    }

    // --- wanted / monitor --------------------------------------------------

    pub fn list_wanted(&self) -> Result<Vec<WantedItem>> {
        self.store.list_wanted()
    }

    pub fn add_wanted(&self, mut w: WantedItem) -> Result<()> {
        w.title = w.title.trim().to_string();
        if w.title.is_empty() {
            bail!("title is required");
        }
        w.id = format!("{}-{}", w.kind.label(), w.tmdb_id);
        self.store.upsert_wanted(&w)
    }

    pub fn remove_wanted(&self, id: &str) -> Result<()> {
        self.store.delete_wanted(id)
    }

    /// Upcoming/recent episode air dates for all monitored series (release calendar).
    pub async fn get_calendar(&self) -> Result<Vec<CalendarEntry>> {
        let key = self
            .tmdb_key()
            .ok_or_else(|| anyhow::anyhow!("no TMDb API key set"))?;
        let wanted = self.store.list_wanted()?;
        let mut out = Vec::new();
        for w in wanted
            .iter()
            .filter(|w| w.monitored && matches!(w.kind, WantedKind::Series))
        {
            if let Ok(mut eps) = self.meta.upcoming_episodes(&key, w.tmdb_id, &w.title).await {
                out.append(&mut eps);
            }
        }
        out.sort_by(|a, b| a.air_date.cmp(&b.air_date).then_with(|| a.title.cmp(&b.title)));
        Ok(out)
    }

    /// Check every monitored wanted item, grabbing missing/upgradeable releases.
    /// Returns the number of new grabs.
    pub async fn run_monitor(&self) -> Result<usize> {
        let wanted = self.store.list_wanted()?;
        let profiles = self.store.list_quality_profiles()?;
        let library = self.store.get_library().unwrap_or_default();
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let key = self.tmdb_key();
        let mut grabbed = 0usize;

        for w in wanted.iter().filter(|w| w.monitored) {
            let profile = profiles.iter().find(|p| p.id == w.quality_profile);
            match w.kind {
                WantedKind::Movie => {
                    let id = format!("monitor:{}:movie", w.id);
                    if self.consider_grab(&id, w, &library, profile, None, None).await {
                        grabbed += 1;
                    }
                }
                WantedKind::Series => {
                    let Some(key) = key.as_deref() else { continue };
                    let episodes = self
                        .meta
                        .series_aired_episodes(key, w.tmdb_id, &today)
                        .await
                        .unwrap_or_default();
                    for ep in episodes {
                        let id = format!("monitor:{}:s{}e{}", w.id, ep.season, ep.episode);
                        if self
                            .consider_grab(&id, w, &library, profile, Some(ep.season), Some(ep.episode))
                            .await
                        {
                            grabbed += 1;
                        }
                    }
                }
            }
        }
        Ok(grabbed)
    }

    /// Decide whether to grab (or upgrade) a specific movie/episode.
    async fn consider_grab(
        &self,
        dedup_id: &str,
        w: &WantedItem,
        library: &Library,
        profile: Option<&QualityProfile>,
        season: Option<i32>,
        episode: Option<i32>,
    ) -> bool {
        let in_lib = match (season, episode) {
            (Some(s), Some(e)) => library.has_episode(&w.title, s, e),
            _ => library.has_movie(&w.title, w.year),
        };
        let cur = self.store.history_best_score(dedup_id).ok().flatten();
        // On disk already and never grabbed by us → leave it alone.
        if in_lib && cur.is_none() {
            return false;
        }

        let query = match (season, episode) {
            (Some(s), Some(e)) => format!("{} S{s:02}E{e:02}", w.title),
            _ => match w.year {
                Some(y) => format!("{} {}", w.title, y),
                None => w.title.clone(),
            },
        };
        let releases = self.filter_blacklisted(self.search_releases(&query).await.unwrap_or_default());
        let Some((rel, sc)) = best_acceptable(&releases, profile, &w.title, season, episode) else {
            return false;
        };

        let should = match cur {
            None => true,
            Some(c) => {
                profile.map(|p| p.upgrade_allowed).unwrap_or(false)
                    && c < profile.map(quality::cutoff_score).unwrap_or(i64::MAX)
                    && sc > c
            }
        };
        if !should {
            return false;
        }
        let ctx = GrabContext {
            dedup_id: dedup_id.to_string(),
            title: w.title.clone(),
            season,
            episode,
            year: w.year,
            category: w.category.clone(),
            profile_id: w.quality_profile.clone(),
            source: "monitor".to_string(),
            tried: HashSet::new(),
            at: 0,
        };
        self.grab_release_ctx(ctx, &rel.url, &rel.title, sc).is_ok()
    }

    // --- categories --------------------------------------------------------

    pub fn list_categories(&self) -> Result<Vec<Category>> {
        self.store.list_categories()
    }

    /// Create/replace a category: derive a stable slug from the name, ensure the
    /// mapped directory exists under the download folder, then persist.
    pub fn upsert_category(&self, mut c: Category) -> Result<()> {
        c.name = c.name.trim().to_string();
        if c.name.is_empty() {
            bail!("category name is required");
        }
        c.slug = slugify(&c.name);
        if c.slug.is_empty() {
            bail!("category name must contain letters or digits");
        }
        // Default the sub-directory to the name when the user leaves it blank.
        let subdir = c.subdir.trim();
        c.subdir = if subdir.is_empty() {
            c.name.clone()
        } else {
            subdir.to_string()
        };

        let dir = self.category_dir(&c.subdir)?;
        std::fs::create_dir_all(&dir).ok();
        self.store.upsert_category(&c)
    }

    pub fn delete_category(&self, slug: &str) -> Result<()> {
        self.store.delete_category(slug)
    }

    // --- quality profiles --------------------------------------------------

    pub fn list_quality_profiles(&self) -> Result<Vec<QualityProfile>> {
        self.store.list_quality_profiles()
    }

    pub fn upsert_quality_profile(&self, mut p: QualityProfile) -> Result<()> {
        p.name = p.name.trim().to_string();
        if p.name.is_empty() {
            bail!("profile name is required");
        }
        p.id = slugify(&p.name);
        if p.id.is_empty() {
            bail!("profile name must contain letters or digits");
        }
        self.store.upsert_quality_profile(&p)
    }

    pub fn delete_quality_profile(&self, id: &str) -> Result<()> {
        self.store.delete_quality_profile(id)
    }

    // --- providers (TMDb) --------------------------------------------------

    /// The effective TMDb key: a UI-configured value wins over the `.env` one.
    pub fn tmdb_key(&self) -> Option<String> {
        self.store
            .get_config(TMDB_KEY)
            .ok()
            .flatten()
            .filter(|k| !k.trim().is_empty())
            .or_else(|| self.config.tmdb_api_key.clone())
    }

    pub fn provider_info(&self) -> ProviderInfo {
        ProviderInfo {
            tmdb_key_set: self.tmdb_key().is_some(),
            tmdb_status: None,
        }
    }

    pub fn set_tmdb_key(&self, key: String) -> Result<()> {
        self.store.set_config(TMDB_KEY, key.trim())
    }

    pub async fn test_tmdb(&self) -> Result<()> {
        let key = self
            .tmdb_key()
            .ok_or_else(|| anyhow::anyhow!("no TMDb API key set"))?;
        self.meta.test_key(&key).await
    }

    pub async fn tmdb_search(&self, query: &str) -> Result<Vec<MediaSearchResult>> {
        let key = self
            .tmdb_key()
            .ok_or_else(|| anyhow::anyhow!("no TMDb API key set"))?;
        self.meta.search(&key, query).await
    }

    // --- indexers ----------------------------------------------------------

    pub fn list_indexers(&self) -> Result<Vec<Indexer>> {
        self.store.list_indexers()
    }

    pub fn upsert_indexer(&self, mut i: Indexer) -> Result<()> {
        i.name = i.name.trim().to_string();
        if i.name.is_empty() {
            bail!("indexer name is required");
        }
        if i.torznab_url.trim().is_empty() {
            bail!("Torznab URL is required");
        }
        i.id = slugify(&i.name);
        if i.id.is_empty() {
            bail!("indexer name must contain letters or digits");
        }
        self.store.upsert_indexer(&i)
    }

    pub fn delete_indexer(&self, id: &str) -> Result<()> {
        self.store.delete_indexer(id)
    }

    pub async fn test_indexer(&self, indexer: &Indexer) -> Result<()> {
        indexer::test(&self.http, indexer).await
    }

    /// Search all enabled indexers, sorted by seeders (desc).
    pub async fn search_releases(&self, query: &str) -> Result<Vec<Release>> {
        let indexers = self.store.list_indexers()?;
        let mut all = Vec::new();
        for ix in indexers.iter().filter(|i| i.enabled) {
            match indexer::search(&self.http, ix, query).await {
                Ok(mut r) => all.append(&mut r),
                Err(e) => tracing::warn!("indexer {} search failed: {e}", ix.name),
            }
        }
        all.sort_by(|a, b| b.seeders.unwrap_or(0).cmp(&a.seeders.unwrap_or(0)));
        Ok(all)
    }

    // --- rss feeds ---------------------------------------------------------

    pub fn list_feeds(&self) -> Result<Vec<RssFeed>> {
        self.store.list_feeds()
    }

    pub fn upsert_feed(&self, mut f: RssFeed) -> Result<()> {
        f.name = f.name.trim().to_string();
        if f.name.is_empty() {
            bail!("feed name is required");
        }
        if f.url.trim().is_empty() {
            bail!("feed URL is required");
        }
        f.id = slugify(&f.name);
        if f.id.is_empty() {
            bail!("feed name must contain letters or digits");
        }
        self.store.upsert_feed(&f)
    }

    pub fn delete_feed(&self, id: &str) -> Result<()> {
        self.store.delete_feed(id)
    }

    /// Configured feed poll cadence in minutes (default 15).
    pub fn feed_poll_mins(&self) -> u64 {
        self.store
            .get_config(FEED_POLL_MINS_KEY)
            .ok()
            .flatten()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|m| *m >= 1)
            .unwrap_or(DEFAULT_FEED_POLL_MINS)
    }

    pub fn set_feed_poll_mins(&self, mins: u32) -> Result<()> {
        self.store
            .set_config(FEED_POLL_MINS_KEY, &mins.max(1).to_string())
    }

    // --- grabbing / history ------------------------------------------------

    /// Resolve a category slug to an absolute download path (default dir if unset).
    pub fn category_path(&self, slug: &str) -> String {
        if slug.trim().is_empty() {
            return self.config.download_dir.to_string_lossy().into_owned();
        }
        if let Ok(cats) = self.store.list_categories() {
            if let Some(c) = cats.iter().find(|c| c.slug == slug) {
                if let Ok(dir) = self.category_dir(&c.subdir) {
                    return dir.to_string_lossy().into_owned();
                }
            }
        }
        self.config.download_dir.to_string_lossy().into_owned()
    }

    /// Start a download and record it in grab history under dedup key `id`
    /// (release URL for feeds/search; an episode/movie key for the monitor).
    /// Registers a basic recovery context derived from the release title.
    pub fn grab_release(
        &self,
        id: &str,
        url: &str,
        title: &str,
        category: &str,
        source: &str,
        score: i64,
    ) -> Result<()> {
        let parsed = quality::parse_release(title);
        let se = scan::extract_se(title);
        let ctx = GrabContext {
            dedup_id: id.to_string(),
            title: parsed.title.clone(),
            season: parsed.season.or(se.0),
            episode: parsed.episode.or(se.1),
            year: parsed.year,
            category: category.to_string(),
            profile_id: String::new(),
            source: source.to_string(),
            tried: HashSet::new(),
            at: 0,
        };
        self.grab_release_ctx(ctx, url, title, score)
    }

    /// Start a download, record it in history, and register recovery tracking so
    /// a failed metadata fetch retries with a different release (keyed by URL).
    fn grab_release_ctx(
        &self,
        mut ctx: GrabContext,
        url: &str,
        title: &str,
        score: i64,
    ) -> Result<()> {
        let path = self.category_path(&ctx.category);
        self.engine.spawn_add_url(url.to_string(), path, false, None)?;
        self.store.record_grab(&GrabHistoryEntry {
            id: ctx.dedup_id.clone(),
            title: title.to_string(),
            url: url.to_string(),
            category: ctx.category.clone(),
            source: ctx.source.clone(),
            score,
            grabbed_at: now_secs(),
        })?;
        ctx.tried.insert(url.to_string());
        ctx.at = now_secs();
        let mut grabs = self.grabs.lock().unwrap();
        // Prune grabs that have long since resolved (a success leaves no signal).
        let cutoff = now_secs().saturating_sub(GRAB_TRACK_TTL);
        grabs.retain(|_, c| c.at >= cutoff);
        grabs.insert(url.to_string(), ctx);
        Ok(())
    }

    /// Drop releases whose URL is on the failed-download blacklist.
    fn filter_blacklisted(&self, releases: Vec<Release>) -> Vec<Release> {
        let bl = self.store.blacklisted_urls().unwrap_or_default();
        releases.into_iter().filter(|r| !bl.contains(&r.url)).collect()
    }

    /// A background add failed. If it was one of our grabs, blacklist the dead
    /// URL, clear the stale history entry, and grab an alternative release.
    async fn handle_add_failure(&self, url: &str, error: &str) {
        let ctx = self.grabs.lock().unwrap().remove(url);
        let Some(ctx) = ctx else { return }; // not a PVR grab (e.g. a manual UI add)
        tracing::warn!("grab failed for \"{}\" ({error}); searching for an alternative", ctx.title);
        let _ = self.store.blacklist_url(url);
        // Drop the failed grab from history so this target can be re-attempted.
        let _ = self.store.remove_grab(&ctx.dedup_id);
        self.retry_grab(ctx).await;
    }

    /// Search for and grab the best alternative release for a failed grab's
    /// target, skipping anything already tried or blacklisted.
    async fn retry_grab(&self, ctx: GrabContext) {
        let query = match (ctx.season, ctx.episode) {
            (Some(s), Some(e)) => format!("{} S{s:02}E{e:02}", ctx.title),
            _ => match ctx.year {
                Some(y) => format!("{} {y}", ctx.title),
                None => ctx.title.clone(),
            },
        };
        let releases = self.search_releases(&query).await.unwrap_or_default();
        let blacklist = self.store.blacklisted_urls().unwrap_or_default();
        let candidates: Vec<Release> = releases
            .into_iter()
            .filter(|r| !ctx.tried.contains(&r.url) && !blacklist.contains(&r.url))
            .collect();
        let profiles = self.store.list_quality_profiles().unwrap_or_default();
        let profile = profiles.iter().find(|p| p.id == ctx.profile_id);
        match best_acceptable(&candidates, profile, &ctx.title, ctx.season, ctx.episode) {
            Some((rel, sc)) => {
                let dedup = ctx.dedup_id.clone();
                let title = rel.title.clone();
                // Carry the tried set forward so repeated failures keep advancing.
                match self.grab_release_ctx(ctx, &rel.url, &rel.title, sc) {
                    Ok(()) => tracing::info!("retrying {dedup} with alternative release: {title}"),
                    Err(e) => tracing::warn!("retry grab failed: {e}"),
                }
            }
            None => tracing::warn!("no alternative release found for \"{}\"", ctx.title),
        }
    }

    pub fn list_grab_history(&self) -> Result<Vec<GrabHistoryEntry>> {
        self.store.list_grab_history()
    }

    /// Poll every enabled auto-download feed, grabbing acceptable new items.
    /// Returns the number of new grabs.
    pub async fn poll_feeds(&self) -> Result<usize> {
        let feeds = self.store.list_feeds()?;
        let profiles = self.store.list_quality_profiles()?;
        let library = self.store.get_library().unwrap_or_default();
        let mut grabbed = 0usize;

        for f in feeds.iter().filter(|f| f.enabled && f.auto_download) {
            let items = match feed::fetch(&self.http, &f.url).await {
                Ok(i) => i,
                Err(e) => {
                    tracing::warn!("feed {} fetch failed: {e}", f.name);
                    continue;
                }
            };
            let profile = profiles.iter().find(|p| p.id == f.quality_profile);
            for item in items {
                if self.store.history_contains(&item.url).unwrap_or(false) {
                    continue;
                }
                // Never re-grab a release we've already blacklisted as dead.
                if self.store.is_blacklisted(&item.url).unwrap_or(false) {
                    continue;
                }
                // Skip episodes already present on disk (best-effort match).
                let parsed = quality::parse_release(&item.title);
                let (season, episode) = if parsed.episode.is_some() {
                    (parsed.season, parsed.episode)
                } else {
                    let (s, e) = scan::extract_se(&item.title);
                    (parsed.season.or(s), e)
                };
                if let (Some(s), Some(e)) = (season, episode) {
                    if library.has_episode(&parsed.title, s, e) {
                        continue;
                    }
                }
                let sc = match profile {
                    Some(prof) => match quality::score(&quality::parse_release(&item.title), prof) {
                        Some(s) => s,
                        None => continue, // fails the profile
                    },
                    None => 0,
                };
                let ctx = GrabContext {
                    dedup_id: item.url.clone(),
                    title: parsed.title.clone(),
                    season,
                    episode,
                    year: parsed.year,
                    category: f.category.clone(),
                    profile_id: f.quality_profile.clone(),
                    source: f.name.clone(),
                    tried: HashSet::new(),
                    at: 0,
                };
                match self.grab_release_ctx(ctx, &item.url, &item.title, sc) {
                    Ok(()) => grabbed += 1,
                    Err(e) => tracing::warn!("grab failed: {e}"),
                }
            }
        }
        Ok(grabbed)
    }

    /// Resolve a category's sub-directory to an absolute path confined to the
    /// download folder (routes through the engine's path-traversal guard).
    fn category_dir(&self, subdir: &str) -> Result<PathBuf> {
        let sub = subdir.trim().trim_start_matches(['/', '\\']);
        let joined = self.config.download_dir.join(sub);
        let confined = self.engine.confine(&joined.to_string_lossy())?;
        if !confined.starts_with(&self.config.download_dir) {
            bail!("category directory must be under the download folder");
        }
        Ok(confined)
    }
}

/// Lowercase alphanumeric slug with single hyphen separators.
fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.trim().to_lowercase().chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
