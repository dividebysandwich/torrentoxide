//! Multipart `.torrent` endpoints: upload (add) and probe (list files only).

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::server::AppState;

const MAX_TORRENT_BYTES: usize = 25 * 1024 * 1024;

/// Parse a comma-separated list of file indices; empty → `None` (all files).
fn parse_only_files(s: &str) -> Option<Vec<usize>> {
    let v: Vec<usize> = s
        .split(',')
        .filter_map(|x| x.trim().parse::<usize>().ok())
        .collect();
    (!v.is_empty()).then_some(v)
}

pub async fn upload_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut output_dir = String::new();
    let mut paused = false;
    let mut only_files: Option<Vec<usize>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name().unwrap_or("") {
            "file" => {
                if let Ok(bytes) = field.bytes().await {
                    file_bytes = Some(bytes.to_vec());
                }
            }
            "output_dir" => {
                output_dir = field.text().await.unwrap_or_default();
            }
            "paused" => {
                paused = field.text().await.map(|t| t == "true").unwrap_or(false);
            }
            "only_files" => {
                only_files = field.text().await.ok().as_deref().and_then(parse_only_files);
            }
            _ => {}
        }
    }

    let Some(bytes) = file_bytes else {
        return (StatusCode::BAD_REQUEST, "missing .torrent file").into_response();
    };
    if let Err(resp) = validate(&bytes) {
        return resp;
    }

    // Download into the staging area, then move to `output_dir` on completion.
    match state.pvr.add_bytes_staged(bytes, output_dir, paused, only_files).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

/// List the files inside an uploaded `.torrent` without adding it, so the UI can
/// offer per-file selection before the download starts.
pub async fn probe_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut output_dir = String::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name().unwrap_or("") {
            "file" => {
                if let Ok(bytes) = field.bytes().await {
                    file_bytes = Some(bytes.to_vec());
                }
            }
            "output_dir" => {
                output_dir = field.text().await.unwrap_or_default();
            }
            _ => {}
        }
    }

    let Some(bytes) = file_bytes else {
        return (StatusCode::BAD_REQUEST, "missing .torrent file").into_response();
    };
    if let Err(resp) = validate(&bytes) {
        return resp;
    }

    match state.engine.probe_bytes(bytes, output_dir).await {
        Ok(files) => Json(files).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

/// Shared size/emptiness guard for uploaded `.torrent` bytes.
fn validate(bytes: &[u8]) -> Result<(), axum::response::Response> {
    if bytes.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty file").into_response());
    }
    if bytes.len() > MAX_TORRENT_BYTES {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, ".torrent file too large").into_response());
    }
    Ok(())
}
