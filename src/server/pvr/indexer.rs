//! Torznab indexer client (Jackett / Prowlarr): capability test + search.

use anyhow::{bail, Result};

use super::xmlparse::parse_items;
use crate::types::{Indexer, Release};

fn extract_error(body: &str) -> String {
    // Torznab errors look like `<error code=".." description=".."/>`.
    if let Some(i) = body.find("description=\"") {
        let rest = &body[i + "description=\"".len()..];
        if let Some(j) = rest.find('"') {
            return rest[..j].to_string();
        }
    }
    "indexer error".to_string()
}

/// Validate an indexer via the Torznab `caps` endpoint.
pub async fn test(http: &reqwest::Client, indexer: &Indexer) -> Result<()> {
    let resp = http
        .get(indexer.torznab_url.trim())
        .query(&[("t", "caps"), ("apikey", indexer.api_key.trim())])
        .send()
        .await?;
    if !resp.status().is_success() {
        bail!("indexer returned {}", resp.status());
    }
    let body = resp.text().await?;
    if body.contains("<error") {
        bail!("{}", extract_error(&body));
    }
    if !body.contains("<caps") {
        bail!("unexpected response (not a Torznab endpoint?)");
    }
    Ok(())
}

/// Free-text search; results are tagged with the indexer name.
pub async fn search(http: &reqwest::Client, indexer: &Indexer, query: &str) -> Result<Vec<Release>> {
    let body = http
        .get(indexer.torznab_url.trim())
        .query(&[
            ("t", "search"),
            ("apikey", indexer.api_key.trim()),
            ("q", query.trim()),
        ])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    if body.contains("<error") {
        bail!("{}", extract_error(&body));
    }
    let mut items = parse_items(&body);
    for it in items.iter_mut() {
        it.indexer = indexer.name.clone();
    }
    Ok(items)
}
