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
    Category, GrabHistoryEntry, Indexer, Library, MediaSearchResult, ProviderInfo, QualityProfile,
    Release, RssFeed,
};
use meta::MetadataClient;
use store::PvrStore;

const TMDB_KEY: &str = "tmdb_api_key";
/// How often enabled auto-download RSS feeds are polled.
const FEED_POLL_INTERVAL: Duration = Duration::from_secs(900);
/// How often the download tree is re-scanned into the library.
const SCAN_INTERVAL: Duration = Duration::from_secs(3600);

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
        let poller = pvr.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(FEED_POLL_INTERVAL);
            loop {
                interval.tick().await;
                if let Err(e) = poller.poll_feeds().await {
                    tracing::warn!("feed poll error: {e}");
                }
            }
        });

        // Re-scan the download tree into the library on a fixed interval
        // (blocking walk runs on a blocking thread).
        let scanner = pvr.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(SCAN_INTERVAL);
            loop {
                interval.tick().await;
                let s = scanner.clone();
                let _ = tokio::task::spawn_blocking(move || s.scan_library()).await;
            }
        });

        Ok(pvr)
    }

    // --- library -----------------------------------------------------------

    /// Walk the download tree, rebuild the library snapshot and persist it.
    pub fn scan_library(&self) -> Library {
        let lib = scan::scan(&self.config.download_dir, now_secs());
        let _ = self.store.set_library(&lib);
        lib
    }

    pub fn library(&self) -> Library {
        self.store.get_library().unwrap_or_default()
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

    /// Start a download and record it in grab history.
    pub fn grab_release(
        &self,
        url: &str,
        title: &str,
        category: &str,
        source: &str,
        score: i64,
    ) -> Result<()> {
        let path = self.category_path(category);
        self.engine.spawn_add_url(url.to_string(), path, false, None)?;
        self.store.record_grab(&GrabHistoryEntry {
            id: url.to_string(),
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
                let sc = match profile {
                    Some(prof) => match quality::score(&quality::parse_release(&item.title), prof) {
                        Some(s) => s,
                        None => continue, // fails the profile
                    },
                    None => 0,
                };
                match self.grab_release(&item.url, &item.title, &f.category, &f.name, sc) {
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
