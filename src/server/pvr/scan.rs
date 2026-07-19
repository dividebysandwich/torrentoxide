//! Filename-based library scanner: walk the download tree, parse each video file
//! and group into movies and shows→episodes. Standard layouts are handled
//! (`Show/Season 01/Show.S01E04.mkv`, or `S01E04` in the filename).

use std::collections::HashMap;
use std::path::Path;

use walkdir::WalkDir;

use super::quality::parse_release;
use crate::types::{norm_title, Library, LibraryEpisode, LibraryMovie, LibraryShow};

const VIDEO_EXTS: [&str; 9] = ["mkv", "mp4", "avi", "m4v", "mov", "ts", "webm", "mpg", "wmv"];

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn is_season_folder(name: &str) -> bool {
    let l = name.to_lowercase();
    l.starts_with("season") || l.starts_with("series") || l == "specials"
}

/// Best-effort show title: the parsed title, else the nearest ancestor folder
/// that isn't a "Season N" directory.
fn show_title(path: &Path, parsed_title: &str) -> String {
    let t = parsed_title.trim();
    if !t.is_empty() {
        return t.to_string();
    }
    for anc in path.ancestors().skip(1) {
        if let Some(name) = anc.file_name().and_then(|n| n.to_str()) {
            if !name.is_empty() && !is_season_folder(name) {
                return name.to_string();
            }
        }
    }
    "Unknown".to_string()
}

pub fn scan(root: &Path, now: u64) -> Library {
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
        let parsed = parse_release(fname);
        let resolution = parsed.resolution.label().to_string();
        let path_s = path.to_string_lossy().into_owned();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

        match (parsed.season, parsed.episode) {
            (Some(season), Some(episode)) => {
                let title = show_title(path, &parsed.title);
                let show = shows
                    .entry(norm_title(&title))
                    .or_insert_with(|| LibraryShow {
                        title: title.clone(),
                        episodes: Vec::new(),
                    });
                show.episodes.push(LibraryEpisode {
                    season,
                    episode,
                    resolution,
                    path: path_s,
                });
            }
            _ => movies.push(LibraryMovie {
                title: if parsed.title.trim().is_empty() {
                    fname.to_string()
                } else {
                    parsed.title.clone()
                },
                year: parsed.year,
                resolution,
                size,
                path: path_s,
            }),
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
