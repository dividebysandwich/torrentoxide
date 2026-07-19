//! TMDb metadata client: validate the key and search movies/TV. Episode-level
//! lookups (air dates) are added with the monitor in a later phase.

use anyhow::{bail, Result};
use serde::Deserialize;

use crate::types::{CalendarEntry, MediaSearchResult};

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

    /// Fetch the poster path for a movie/series by TMDb id — used to backfill
    /// wanted items saved before posters were captured.
    pub async fn poster_by_id(&self, key: &str, tmdb_id: i64, is_tv: bool) -> Result<Option<String>> {
        let kind = if is_tv { "tv" } else { "movie" };
        let body: PosterResp = self
            .http
            .get(format!("{TMDB_BASE}/{kind}/{tmdb_id}"))
            .query(&[("api_key", key)])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(body.poster_path.filter(|p| !p.trim().is_empty()))
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

    /// All episodes of a series that have aired on/before `today` (YYYY-MM-DD).
    pub async fn series_aired_episodes(
        &self,
        key: &str,
        tmdb_id: i64,
        today: &str,
    ) -> Result<Vec<AiredEpisode>> {
        let details: TvDetails = self
            .http
            .get(format!("{TMDB_BASE}/tv/{tmdb_id}"))
            .query(&[("api_key", key)])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut out = Vec::new();
        for n in details.seasons.iter().map(|s| s.season_number).filter(|n| *n >= 1) {
            let resp = match self
                .http
                .get(format!("{TMDB_BASE}/tv/{tmdb_id}/season/{n}"))
                .query(&[("api_key", key)])
                .send()
                .await
            {
                Ok(r) => r,
                Err(_) => continue,
            };
            let season: TvSeason = match resp.json().await {
                Ok(s) => s,
                Err(_) => continue,
            };
            for ep in season.episodes {
                let aired = ep
                    .air_date
                    .as_deref()
                    .map(|d| !d.is_empty() && d <= today)
                    .unwrap_or(false);
                if aired {
                    out.push(AiredEpisode {
                        season: n,
                        episode: ep.episode_number,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Episodes (with names + air dates) of a series' most recent seasons — for
    /// the release calendar. Fetches the latest two seasons to cover a split run.
    pub async fn upcoming_episodes(
        &self,
        key: &str,
        tmdb_id: i64,
        title: &str,
    ) -> Result<Vec<CalendarEntry>> {
        let details: TvDetails = self
            .http
            .get(format!("{TMDB_BASE}/tv/{tmdb_id}"))
            .query(&[("api_key", key)])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut seasons: Vec<i32> = details
            .seasons
            .iter()
            .map(|s| s.season_number)
            .filter(|n| *n >= 1)
            .collect();
        seasons.sort_unstable();
        let recent: Vec<i32> = seasons.into_iter().rev().take(2).collect();

        let mut out = Vec::new();
        for n in recent {
            let resp = match self
                .http
                .get(format!("{TMDB_BASE}/tv/{tmdb_id}/season/{n}"))
                .query(&[("api_key", key)])
                .send()
                .await
            {
                Ok(r) => r,
                Err(_) => continue,
            };
            let season: TvSeason = match resp.json().await {
                Ok(s) => s,
                Err(_) => continue,
            };
            for ep in season.episodes {
                if let Some(date) = ep.air_date.filter(|d| !d.trim().is_empty()) {
                    out.push(CalendarEntry {
                        title: title.to_string(),
                        season: n,
                        episode: ep.episode_number,
                        name: ep.name,
                        air_date: date,
                    });
                }
            }
        }
        Ok(out)
    }
}

/// A single aired episode identified by season + episode number.
#[derive(Clone, Copy, Debug)]
pub struct AiredEpisode {
    pub season: i32,
    pub episode: i32,
}

#[derive(Deserialize)]
struct TvDetails {
    #[serde(default)]
    seasons: Vec<TvSeasonBrief>,
}

#[derive(Deserialize)]
struct TvSeasonBrief {
    season_number: i32,
}

#[derive(Deserialize)]
struct TvSeason {
    #[serde(default)]
    episodes: Vec<TvEpisode>,
}

#[derive(Deserialize)]
struct TvEpisode {
    episode_number: i32,
    air_date: Option<String>,
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct PosterResp {
    poster_path: Option<String>,
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
