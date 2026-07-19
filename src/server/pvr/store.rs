//! redb-backed persistence for PVR data. Values are `serde_json` blobs keyed by
//! string id/slug, so the shared wire types in `crate::types` double as the
//! storage model. This module holds the categories table; later phases add more.

use std::path::Path;

use anyhow::{Context, Result};
use redb::{Database, ReadableTable, TableDefinition};

use crate::types::{Category, QualityProfile};

const CATEGORIES: TableDefinition<&str, &[u8]> = TableDefinition::new("categories");
const QUALITY_PROFILES: TableDefinition<&str, &[u8]> = TableDefinition::new("quality_profiles");
const CONFIG: TableDefinition<&str, &[u8]> = TableDefinition::new("config");

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
}
