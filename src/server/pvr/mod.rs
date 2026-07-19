//! PVR subsystem: categories now; feeds, indexers, library, wanted and the
//! automation loops arrive in later phases. Owns the redb store and holds
//! handles to the engine + config so it can create directories and (later)
//! trigger downloads.

pub mod meta;
pub mod quality;
pub mod store;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Result};

use crate::server::config::AppConfig;
use crate::server::engine::Engine;
use crate::types::{Category, MediaSearchResult, ProviderInfo, QualityProfile};
use meta::MetadataClient;
use store::PvrStore;

const TMDB_KEY: &str = "tmdb_api_key";

pub struct Pvr {
    store: Arc<PvrStore>,
    engine: Arc<Engine>,
    config: Arc<AppConfig>,
    meta: MetadataClient,
}

impl Pvr {
    pub fn new(config: Arc<AppConfig>, engine: Arc<Engine>) -> Result<Arc<Self>> {
        let db_path = config.persistence_dir.join("pvr.redb");
        let store = Arc::new(PvrStore::open(&db_path)?);
        Ok(Arc::new(Self {
            store,
            engine,
            config,
            meta: MetadataClient::new(),
        }))
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
