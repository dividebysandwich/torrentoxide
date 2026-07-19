//! redb-backed persistence for PVR data. Values are `serde_json` blobs keyed by
//! string id/slug, so the shared wire types in `crate::types` double as the
//! storage model. This module holds the categories table; later phases add more.

use std::path::Path;

use anyhow::{Context, Result};
use redb::{Database, ReadableTable, TableDefinition};

use crate::types::Category;

const CATEGORIES: TableDefinition<&str, &[u8]> = TableDefinition::new("categories");

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
}
