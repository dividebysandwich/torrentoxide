//! Library scanner. Classifies each video file as a movie or a show episode
//! using, in priority order: the containing **category kind** (Movies vs TV),
//! the **folder structure** (`TV Shows/<Show>/…`), then filename parsing.
//! Episodes are grouped by their show folder, so absolute-numbered anime and
//! release-named folders still collapse into a single show.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use walkdir::WalkDir;

use super::quality::parse_release;
use crate::types::{norm_title, Category, Library, LibraryEpisode, LibraryMovie, LibraryShow, MediaKind};

const VIDEO_EXTS: [&str; 9] = ["mkv", "mp4", "avi", "m4v", "mov", "ts", "webm", "mpg", "wmv"];

// Supplemental season/episode extractors (torrent-name-parser misses several).
static RE_SXXEXX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)s(\d{1,3})[\s._-]*e(\d{1,4})").unwrap());
static RE_SXXMXX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)s(\d{1,3})[\s._-]*m(\d{1,4})").unwrap());
static RE_NXNN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:^|[^\dx])(\d{1,3})x(\d{1,4})").unwrap());
static RE_EXX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:^|[\s._\[\]-])e(\d{1,4})(?:[\s._\[\]-]|$)").unwrap());
// Anime absolute number, e.g. `- 22 -` or `_-_22_-_`.
static RE_ABS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|[\s._\[\]])-[\s._]*(\d{1,3})[\s._]*-").unwrap());
static RE_SEASON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)s(\d{1,3})[\s._-]*[em]\d").unwrap());

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn is_season_folder(name: &str) -> Option<i32> {
    let l = name.to_lowercase();
    if l == "specials" {
        return Some(0);
    }
    let re = Regex::new(r"(?i)^(?:season|series|staffel)\s*0*(\d{1,3})").ok()?;
    re.captures(&l)?.get(1)?.as_str().parse().ok()
}

fn cap_i32(re: &Regex, s: &str, group: usize) -> Option<i32> {
    re.captures(s)?.get(group)?.as_str().parse().ok()
}

/// Extract (season, episode) from a filename with several fallbacks.
pub(crate) fn extract_se(name: &str) -> (Option<i32>, Option<i32>) {
    if let Some(c) = RE_SXXEXX.captures(name) {
        return (
            c.get(1).and_then(|m| m.as_str().parse().ok()),
            c.get(2).and_then(|m| m.as_str().parse().ok()),
        );
    }
    if let Some(c) = RE_SXXMXX.captures(name) {
        return (
            c.get(1).and_then(|m| m.as_str().parse().ok()),
            c.get(2).and_then(|m| m.as_str().parse().ok()),
        );
    }
    if let Some(c) = RE_NXNN.captures(name) {
        return (
            c.get(1).and_then(|m| m.as_str().parse().ok()),
            c.get(2).and_then(|m| m.as_str().parse().ok()),
        );
    }
    if let Some(e) = cap_i32(&RE_EXX, name, 1) {
        return (cap_i32(&RE_SEASON, name, 1), Some(e));
    }
    if let Some(e) = cap_i32(&RE_ABS, name, 1) {
        return (None, Some(e));
    }
    (None, None)
}

/// The deepest category whose sub-folder prefixes this file's path, as
/// `(kind, prefix_len)` — `prefix_len` is how many leading dirs it consumes.
fn category_match(rel_dirs: &[String], categories: &[Category]) -> Option<(MediaKind, usize)> {
    let mut best: Option<(MediaKind, usize)> = None;
    for c in categories {
        let sub: Vec<&str> = c.subdir.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
        if sub.is_empty() || rel_dirs.len() < sub.len() {
            continue;
        }
        if rel_dirs
            .iter()
            .zip(&sub)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
            && best.map(|(_, l)| sub.len() > l).unwrap_or(true)
        {
            best = Some((c.kind, sub.len()));
        }
    }
    best
}

/// Clean a folder name into a display title (strips S01/year/resolution/etc.).
fn clean_title(folder: &str) -> String {
    let p = parse_release(folder);
    let t = p.title.trim();
    if t.is_empty() {
        folder.trim().to_string()
    } else {
        t.to_string()
    }
}

pub fn scan(root: &Path, now: u64, categories: &[Category]) -> Library {
    let mut movies = Vec::new();
    let mut shows: HashMap<String, LibraryShow> = HashMap::new();
    let mut file_count = 0usize;

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() || !is_video(entry.path()) {
            continue;
        }
        let path = entry.path();
        let fname = path.file_name().and_then(|f| f.to_str()).unwrap_or_default();
        file_count += 1;

        // Directory components relative to the download root.
        let rel_dirs: Vec<String> = path
            .parent()
            .and_then(|p| p.strip_prefix(root).ok())
            .map(|r| {
                r.components()
                    .filter_map(|c| c.as_os_str().to_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        let parsed = parse_release(fname);
        let (mut season, mut episode) = (parsed.season, parsed.episode);
        if episode.is_none() {
            let (s, e) = extract_se(fname);
            season = season.or(s);
            episode = episode.or(e);
        }
        // Season from a "Season NN" ancestor folder.
        let folder_season = rel_dirs.iter().rev().find_map(|d| is_season_folder(d));

        let cat = category_match(&rel_dirs, categories);
        let in_season_folder = folder_season.is_some();
        let is_tv = match cat.map(|(k, _)| k) {
            Some(MediaKind::Tv) => true,
            // Movie/Other categories are never grouped as shows.
            Some(MediaKind::Movie) | Some(MediaKind::Other) => false,
            None => season.is_some() || episode.is_some() || in_season_folder,
        };

        let resolution = parsed.resolution.label().to_string();
        let path_s = path.to_string_lossy().into_owned();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

        if is_tv {
            // Show folder = the dir just past the matched category (or the top-level
            // dir when uncategorized); a loose file in the category root falls back
            // to its parsed title so the category name never becomes a "show".
            let cat_prefix = cat.map(|(_, n)| n).unwrap_or(0);
            let show_folder = rel_dirs
                .get(cat_prefix)
                .filter(|d| is_season_folder(d).is_none())
                .cloned();
            let title = match show_folder {
                Some(f) => clean_title(&f),
                None if !parsed.title.trim().is_empty() => parsed.title.trim().to_string(),
                None => fname.to_string(),
            };
            let show = shows.entry(norm_title(&title)).or_insert_with(|| LibraryShow {
                title: title.clone(),
                episodes: Vec::new(),
            });
            show.episodes.push(LibraryEpisode {
                season: season.or(folder_season).unwrap_or(1),
                episode: episode.unwrap_or(0),
                resolution,
                path: path_s,
            });
        } else {
            movies.push(LibraryMovie {
                title: if parsed.title.trim().is_empty() {
                    fname.to_string()
                } else {
                    parsed.title.clone()
                },
                year: parsed.year,
                resolution,
                size,
                path: path_s,
            });
        }
    }

    let mut shows: Vec<LibraryShow> = shows.into_values().collect();
    for s in shows.iter_mut() {
        s.episodes.sort_by_key(|e| (e.season, e.episode));
    }
    shows.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    movies.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

    Library {
        movies,
        shows,
        file_count,
        scanned_at: now,
    }
}
