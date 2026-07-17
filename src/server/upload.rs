//! Multipart `.torrent` file upload endpoint.

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::server::AppState;

const MAX_TORRENT_BYTES: usize = 25 * 1024 * 1024;

pub async fn upload_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut output_dir = String::new();
    let mut paused = false;

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
            _ => {}
        }
    }

    let Some(bytes) = file_bytes else {
        return (StatusCode::BAD_REQUEST, "missing .torrent file").into_response();
    };
    if bytes.is_empty() {
        return (StatusCode::BAD_REQUEST, "empty file").into_response();
    }
    if bytes.len() > MAX_TORRENT_BYTES {
        return (StatusCode::PAYLOAD_TOO_LARGE, ".torrent file too large").into_response();
    }

    match state.engine.add_bytes(bytes, output_dir, paused).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}
