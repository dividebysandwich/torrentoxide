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

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};

use crate::server::config::AppConfig;
use crate::server::engine::Engine;
use crate::types::{
    CalendarEntry, Category, GrabHistoryEntry, Indexer, Library, MediaSearchResult, ProviderInfo,
    QualityProfile, Release, RssFeed, WantedItem, WantedKind,
};
use meta::MetadataClient;
use store::PvrStore;

const TMDB_KEY: &str = "tmdb_api_key";
const FEED_POLL_MINS_KEY: &str = "feed_poll_mins";
/// Default feed poll cadence (minutes) when unconfigured.
const DEFAULT_FEED_POLL_MINS: u64 = 15;
/// How often the download tree is re-scanned into the library.
const SCAN_INTERVAL: Duration = Duration::from_secs(3600);
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

        Ok(pvr)
    }

    // --- library -----------------------------------------------------------

    /// Walk the download tree, rebuild the library snapshot and persist it.
    pub fn scan_library(&self) -> Library {
        let cats = self.store.list_categories().unwrap_or_default();
        let lib = scan::scan(&self.config.download_dir, now_secs(), &cats);
        let _ = self.store.set_library(&lib);
        lib
    }

    pub fn library(&self) -> Library {
        self.store.get_library().unwrap_or_default()
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
        let releases = self.search_releases(&query).await.unwrap_or_default();
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
        should
            && self
                .grab_release(dedup_id, &rel.url, &rel.title, &w.category, "monitor", sc)
                .is_ok()
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
    pub fn grab_release(
        &self,
        id: &str,
        url: &str,
        title: &str,
        category: &str,
        source: &str,
        score: i64,
    ) -> Result<()> {
        let path = self.category_path(category);
        self.engine.spawn_add_url(url.to_string(), path, false, None)?;
        self.store.record_grab(&GrabHistoryEntry {
            id: id.to_string(),
            title: title.to_string(),
            url: url.to_string(),
            category: category.to_string(),
            source: source.to_string(),
            score,
            grabbed_at: now_secs(),
        })
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
                match self.grab_release(&item.url, &item.url, &item.title, &f.category, &f.name, sc) {
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
