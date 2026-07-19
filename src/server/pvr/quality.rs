//! Release-name parsing and quality-profile scoring — the core of the
//! auto-download / upgrade decision used by the feed poller and monitor.

use std::sync::LazyLock;

use regex::Regex;
use torrent_name_parser::Metadata;

use crate::types::{HdrPref, QualityProfile, Resolution, Source};

/// Attributes extracted from a release name.
#[derive(Clone, Debug)]
pub struct ParsedRelease {
    pub title: String,
    pub year: Option<i32>,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub resolution: Resolution,
    pub source: Source,
    pub hdr: bool,
    pub dv: bool,
    pub language: Option<String>,
    pub group: Option<String>,
    pub proper: bool,
    pub repack: bool,
    pub is_show: bool,
}

// torrent-name-parser doesn't detect HDR / Dolby Vision, so match them ourselves.
static HDR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(hdr10\+|hdr10|hdr)").unwrap());
static DV_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(dolby.?vision|\bdovi\b|\bdv\b)").unwrap());

fn map_resolution(s: Option<&str>) -> Resolution {
    match s.map(|r| r.to_lowercase()) {
        Some(r) if r.contains("2160") || r.contains("4k") || r.contains("uhd") => Resolution::R2160,
        Some(r) if r.contains("1080") => Resolution::R1080,
        Some(r) if r.contains("720") => Resolution::R720,
        Some(r) if r.contains("480") || r.contains("576") => Resolution::R480,
        _ => Resolution::Unknown,
    }
}

fn map_source(s: Option<&str>) -> Source {
    match s.map(|q| q.to_lowercase()) {
        Some(q) if q.contains("remux") => Source::Remux,
        Some(q) if q.contains("blu") => Source::Bluray,
        Some(q) if q.contains("web-dl") || q.contains("webdl") || q.contains("web dl") => {
            Source::WebDl
        }
        Some(q) if q.contains("webrip") || q.contains("web") => Source::WebRip,
        Some(q) if q.contains("hdtv") || q.contains("hdrip") => Source::Hdtv,
        Some(q) if q.contains("cam") || q.contains("telesync") || q.contains(" ts") => Source::Cam,
        _ => Source::Unknown,
    }
}

pub fn parse_release(name: &str) -> ParsedRelease {
    let hdr = HDR_RE.is_match(name);
    let dv = DV_RE.is_match(name);
    match Metadata::from(name) {
        Ok(m) => ParsedRelease {
            title: m.title().to_string(),
            year: m.year(),
            season: m.season(),
            episode: m.episode(),
            resolution: map_resolution(m.resolution()),
            source: map_source(m.quality()),
            hdr,
            dv,
            language: m.language().map(|l| l.to_lowercase()),
            group: m.group().map(|g| g.to_string()),
            proper: m.proper(),
            repack: m.repack(),
            is_show: m.is_show(),
        },
        Err(_) => ParsedRelease {
            title: name.to_string(),
            year: None,
            season: None,
            episode: None,
            resolution: map_resolution(None),
            source: map_source(None),
            hdr,
            dv,
            language: None,
            group: None,
            proper: false,
            repack: false,
            is_show: false,
        },
    }
}

/// Score a release against a profile. `None` = unacceptable (fails a hard
/// constraint). Higher is better. Resolution dominates, HDR is a strong
/// secondary preference, then source, group and proper/repack.
pub fn score(p: &ParsedRelease, prof: &QualityProfile) -> Option<i64> {
    // Reject below the minimum resolution (an unknown resolution is allowed but low).
    if p.resolution != Resolution::Unknown && p.resolution.rank() < prof.min_resolution.rank() {
        return None;
    }
    // Reject blocked release groups.
    if let Some(g) = &p.group {
        if prof.blocked_groups.iter().any(|b| b.trim().eq_ignore_ascii_case(g)) {
            return None;
        }
    }
    // Language requirement — an untagged release is assumed to match.
    if !prof.languages.is_empty() {
        if let Some(lang) = &p.language {
            let ok = prof.languages.iter().any(|w| {
                let w = w.trim().to_lowercase();
                !w.is_empty() && (lang.contains(&w) || w.contains(lang.as_str()))
            });
            if !ok {
                return None;
            }
        }
    }
    // HDR requirement.
    if matches!(prof.hdr, HdrPref::Require) && !p.hdr && !p.dv {
        return None;
    }

    let mut s = p.resolution.rank() * 10_000 + p.source.rank() * 100;
    if (p.hdr || p.dv) && !matches!(prof.hdr, HdrPref::Ignore) {
        s += 2_000;
    }
    if p.dv {
        s += 50;
    }
    if let Some(g) = &p.group {
        if prof.preferred_groups.iter().any(|pg| pg.trim().eq_ignore_ascii_case(g)) {
            s += 300;
        }
    }
    if p.proper {
        s += 20;
    }
    if p.repack {
        s += 10;
    }
    Some(s)
}

/// Score at/above which a grabbed release is "good enough" — no further upgrade
/// is sought. Mirrors [`score`]'s weighting for resolution + HDR preference.
pub fn cutoff_score(prof: &QualityProfile) -> i64 {
    let mut s = prof.cutoff_resolution.rank() * 10_000;
    if !matches!(prof.hdr, HdrPref::Ignore) {
        s += 2_000;
    }
    s
}
