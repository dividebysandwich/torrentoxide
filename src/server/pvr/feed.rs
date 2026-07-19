//! RSS feed fetching (RSS 2.0 / Torznab item shape via the shared parser).

use anyhow::Result;

use super::xmlparse::parse_items;
use crate::types::Release;

pub async fn fetch(http: &reqwest::Client, url: &str) -> Result<Vec<Release>> {
    let body = http
        .get(url.trim())
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(parse_items(&body))
}
