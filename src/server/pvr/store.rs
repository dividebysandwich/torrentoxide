//! redb-backed persistence for PVR data. Values are `serde_json` blobs keyed by
//! string id/slug, so the shared wire types in `crate::types` double as the
//! storage model. This module holds the categories table; later phases add more.

use std::path::Path;

use anyhow::{Context, Result};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

use super::DownloadTarget;
use crate::types::{
    Category, GrabHistoryEntry, Indexer, Library, QualityProfile, RssFeed, WantedItem,
};

const CATEGORIES: TableDefinition<&str, &[u8]> = TableDefinition::new("categories");
const QUALITY_PROFILES: TableDefinition<&str, &[u8]> = TableDefinition::new("quality_profiles");
const CONFIG: TableDefinition<&str, &[u8]> = TableDefinition::new("config");
const INDEXERS: TableDefinition<&str, &[u8]> = TableDefinition::new("indexers");
const FEEDS: TableDefinition<&str, &[u8]> = TableDefinition::new("feeds");
const GRAB_HISTORY: TableDefinition<&str, &[u8]> = TableDefinition::new("grab_history");
/// Release URLs that failed to download; never re-grabbed (recovery avoids them).
const GRAB_BLACKLIST: TableDefinition<&str, &[u8]> = TableDefinition::new("grab_blacklist");
const LIBRARY: TableDefinition<&str, &[u8]> = TableDefinition::new("library");
const WANTED: TableDefinition<&str, &[u8]> = TableDefinition::new("wanted");
const IMPORTED: TableDefinition<&str, &[u8]> = TableDefinition::new("imported");
/// Staging token → where the finished download should be moved.
const DOWNLOAD_TARGETS: TableDefinition<&str, &[u8]> = TableDefinition::new("download_targets");

pub struct PvrStore {
    db: Database,
}

impl PvrStore {
    /// Open (or create) the database at `path` and ensure the tables exist.
    pub fn open(path: &Path) -> Result<Self> {
        let db = Database::create(path)
            .with_context(|| format!("failed to open pvr database at {}", path.display()))?;
        let txn = db.begin_write()?;
        {
            // Opening a table in a write txn creates it if missing.
            let _ = txn.open_table(CATEGORIES)?;
            let _ = txn.open_table(QUALITY_PROFILES)?;
            let _ = txn.open_table(CONFIG)?;
            let _ = txn.open_table(INDEXERS)?;
            let _ = txn.open_table(FEEDS)?;
            let _ = txn.open_table(GRAB_HISTORY)?;
            let _ = txn.open_table(GRAB_BLACKLIST)?;
            let _ = txn.open_table(LIBRARY)?;
            let _ = txn.open_table(WANTED)?;
            let _ = txn.open_table(IMPORTED)?;
            let _ = txn.open_table(DOWNLOAD_TARGETS)?;
        }
        txn.commit()?;
        Ok(Self { db })
    }

    pub fn list_categories(&self) -> Result<Vec<Category>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(CATEGORIES)?;
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (_k, v) = entry?;
            if let Ok(c) = serde_json::from_slice::<Category>(v.value()) {
                out.push(c);
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    }

    pub fn upsert_category(&self, c: &Category) -> Result<()> {
        let bytes = serde_json::to_vec(c)?;
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(CATEGORIES)?;
            table.insert(c.slug.as_str(), bytes.as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn delete_category(&self, slug: &str) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(CATEGORIES)?;
            table.remove(slug)?;
        }
        txn.commit()?;
        Ok(())
    }

    // --- quality profiles --------------------------------------------------

    pub fn list_quality_profiles(&self) -> Result<Vec<QualityProfile>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(QUALITY_PROFILES)?;
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (_k, v) = entry?;
            if let Ok(p) = serde_json::from_slice::<QualityProfile>(v.value()) {
                out.push(p);
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    }

    pub fn upsert_quality_profile(&self, p: &QualityProfile) -> Result<()> {
        let bytes = serde_json::to_vec(p)?;
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(QUALITY_PROFILES)?;
            table.insert(p.id.as_str(), bytes.as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn delete_quality_profile(&self, id: &str) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(QUALITY_PROFILES)?;
            table.remove(id)?;
        }
        txn.commit()?;
        Ok(())
    }

    // --- config key/value --------------------------------------------------

    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(CONFIG)?;
        Ok(table
            .get(key)?
            .and_then(|v| String::from_utf8(v.value().to_vec()).ok()))
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(CONFIG)?;
            table.insert(key, value.as_bytes())?;
        }
        txn.commit()?;
        Ok(())
    }

    // --- indexers ----------------------------------------------------------

    pub fn list_indexers(&self) -> Result<Vec<Indexer>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(INDEXERS)?;
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (_k, v) = entry?;
            if let Ok(i) = serde_json::from_slice::<Indexer>(v.value()) {
                out.push(i);
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    }

    pub fn upsert_indexer(&self, i: &Indexer) -> Result<()> {
        let bytes = serde_json::to_vec(i)?;
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(INDEXERS)?;
            table.insert(i.id.as_str(), bytes.as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn delete_indexer(&self, id: &str) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(INDEXERS)?;
            table.remove(id)?;
        }
        txn.commit()?;
        Ok(())
    }

    // --- rss feeds ---------------------------------------------------------

    pub fn list_feeds(&self) -> Result<Vec<RssFeed>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(FEEDS)?;
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (_k, v) = entry?;
            if let Ok(f) = serde_json::from_slice::<RssFeed>(v.value()) {
                out.push(f);
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    }

    pub fn upsert_feed(&self, f: &RssFeed) -> Result<()> {
        let bytes = serde_json::to_vec(f)?;
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(FEEDS)?;
            table.insert(f.id.as_str(), bytes.as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn delete_feed(&self, id: &str) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(FEEDS)?;
            table.remove(id)?;
        }
        txn.commit()?;
        Ok(())
    }

    // --- grab history ------------------------------------------------------

    pub fn history_contains(&self, id: &str) -> Result<bool> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(GRAB_HISTORY)?;
        Ok(table.get(id)?.is_some())
    }

    pub fn record_grab(&self, e: &GrabHistoryEntry) -> Result<()> {
        let bytes = serde_json::to_vec(e)?;
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(GRAB_HISTORY)?;
            table.insert(e.id.as_str(), bytes.as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn list_grab_history(&self) -> Result<Vec<GrabHistoryEntry>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(GRAB_HISTORY)?;
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (_k, v) = entry?;
            if let Ok(e) = serde_json::from_slice::<GrabHistoryEntry>(v.value()) {
                out.push(e);
            }
        }
        out.sort_by(|a, b| b.grabbed_at.cmp(&a.grabbed_at));
        Ok(out)
    }

    /// Remove a grab-history entry (used when a grab failed and is re-attempted).
    pub fn remove_grab(&self, id: &str) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(GRAB_HISTORY)?;
            table.remove(id)?;
        }
        txn.commit()?;
        Ok(())
    }

    // --- grab blacklist (dead release URLs) --------------------------------

    pub fn blacklist_url(&self, url: &str) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(GRAB_BLACKLIST)?;
            table.insert(url, [1u8].as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn is_blacklisted(&self, url: &str) -> Result<bool> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(GRAB_BLACKLIST)?;
        Ok(table.get(url)?.is_some())
    }

    pub fn blacklisted_urls(&self) -> Result<std::collections::HashSet<String>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(GRAB_BLACKLIST)?;
        let mut out = std::collections::HashSet::new();
        for entry in table.iter()? {
            let (k, _v) = entry?;
            out.insert(k.value().to_string());
        }
        Ok(out)
    }

    /// Best (highest) score previously grabbed under a dedup id, if any.
    pub fn history_best_score(&self, id: &str) -> Result<Option<i64>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(GRAB_HISTORY)?;
        Ok(table
            .get(id)?
            .and_then(|v| serde_json::from_slice::<GrabHistoryEntry>(v.value()).ok())
            .map(|e| e.score))
    }

    // --- library snapshot --------------------------------------------------

    pub fn get_library(&self) -> Result<Library> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(LIBRARY)?;
        Ok(table
            .get("current")?
            .and_then(|v| serde_json::from_slice::<Library>(v.value()).ok())
            .unwrap_or_default())
    }

    pub fn set_library(&self, lib: &Library) -> Result<()> {
        let bytes = serde_json::to_vec(lib)?;
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(LIBRARY)?;
            table.insert("current", bytes.as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    // --- wanted ------------------------------------------------------------

    pub fn list_wanted(&self) -> Result<Vec<WantedItem>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(WANTED)?;
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (_k, v) = entry?;
            if let Ok(w) = serde_json::from_slice::<WantedItem>(v.value()) {
                out.push(w);
            }
        }
        out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        Ok(out)
    }

    pub fn upsert_wanted(&self, w: &WantedItem) -> Result<()> {
        let bytes = serde_json::to_vec(w)?;
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(WANTED)?;
            table.insert(w.id.as_str(), bytes.as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn delete_wanted(&self, id: &str) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(WANTED)?;
            table.remove(id)?;
        }
        txn.commit()?;
        Ok(())
    }

    // --- import bookkeeping (source paths already linked into the library) --

    pub fn is_imported(&self, src: &str) -> Result<bool> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(IMPORTED)?;
        Ok(table.get(src)?.is_some())
    }

    pub fn mark_imported(&self, src: &str) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(IMPORTED)?;
            table.insert(src, [1u8].as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn imported_paths(&self) -> Result<std::collections::HashSet<String>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(IMPORTED)?;
        let mut out = std::collections::HashSet::new();
        for entry in table.iter()? {
            let (k, _v) = entry?;
            out.insert(k.value().to_string());
        }
        Ok(out)
    }

    // --- download targets (staged download → final destination) -------------

    pub fn set_download_target(&self, token: &str, t: &DownloadTarget) -> Result<()> {
        let bytes = serde_json::to_vec(t)?;
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(DOWNLOAD_TARGETS)?;
            table.insert(token, bytes.as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn get_download_target(&self, token: &str) -> Result<Option<DownloadTarget>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(DOWNLOAD_TARGETS)?;
        Ok(table
            .get(token)?
            .and_then(|v| serde_json::from_slice::<DownloadTarget>(v.value()).ok()))
    }

    pub fn remove_download_target(&self, token: &str) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(DOWNLOAD_TARGETS)?;
            table.remove(token)?;
        }
        txn.commit()?;
        Ok(())
    }
}
