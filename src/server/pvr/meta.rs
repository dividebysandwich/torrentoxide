//! TMDb metadata client: validate the key and search movies/TV. Episode-level
//! lookups (air dates) are added with the monitor in a later phase.

use anyhow::{bail, Result};
use serde::Deserialize;

use crate::types::MediaSearchResult;

const TMDB_BASE: &str = "https://api.themoviedb.org/3";

pub struct MetadataClient {
    http: reqwest::Client,
}

impl MetadataClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Validate an API key by hitting the (auth-required) configuration endpoint.
    pub async fn test_key(&self, key: &str) -> Result<()> {
        let resp = self
            .http
            .get(format!("{TMDB_BASE}/configuration"))
            .query(&[("api_key", key)])
            .send()
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else if resp.status().as_u16() == 401 {
            bail!("invalid TMDb API key");
        } else {
            bail!("TMDb returned {}", resp.status());
        }
    }

    /// Search movies + TV shows by free-text title.
    pub async fn search(&self, key: &str, query: &str) -> Result<Vec<MediaSearchResult>> {
        let body: SearchResp = self
            .http
            .get(format!("{TMDB_BASE}/search/multi"))
            .query(&[("api_key", key), ("query", query.trim())])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(body.results.into_iter().filter_map(SearchItem::into_meta).collect())
    }
}

#[derive(Deserialize)]
struct SearchResp {
    results: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct SearchItem {
    id: i64,
    media_type: Option<String>,
    title: Option<String>,
    name: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    #[serde(default)]
    overview: String,
    poster_path: Option<String>,
}

impl SearchItem {
    fn into_meta(self) -> Option<MediaSearchResult> {
        let mt = self.media_type.as_deref().unwrap_or("");
        if mt != "movie" && mt != "tv" {
            return None;
        }
        let is_tv = mt == "tv";
        let title = self.title.or(self.name)?;
        let date = self.release_date.or(self.first_air_date).unwrap_or_default();
        let year = date.get(0..4).and_then(|y| y.parse::<i32>().ok());
        Some(MediaSearchResult {
            tmdb_id: self.id,
            title,
            year,
            overview: self.overview,
            poster_path: self.poster_path,
            is_tv,
        })
    }
}
