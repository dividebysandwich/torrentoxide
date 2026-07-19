//! Server-Sent Events endpoint streaming live stats snapshots (~1/s).

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;

use crate::server::AppState;

pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.engine.subscribe();
    let stream = async_stream::stream! {
        // Send the current snapshot (with the full server-side history) right away,
        // so a freshly-loaded / refreshed page renders a populated graph immediately.
        {
            let snapshot = rx.borrow_and_update().clone();
            if let Ok(event) = Event::default().json_data(snapshot.as_ref()) {
                yield Ok(event);
            }
        }
        // Then relay each subsequent sampler update.
        while rx.changed().await.is_ok() {
            let snapshot = rx.borrow_and_update().clone();
            if let Ok(event) = Event::default().json_data(snapshot.as_ref()) {
                yield Ok(event);
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

/// Streams captured log lines: the buffered history first, then live lines.
pub async fn logs_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    use tokio::sync::broadcast::error::RecvError;

    let mut rx = state.log_buf.subscribe();
    let history = state.log_buf.history();
    let last_id = history.last().map(|l| l.id).unwrap_or(0);

    let stream = async_stream::stream! {
        for line in history {
            if let Ok(event) = Event::default().json_data(&line) {
                yield Ok(event);
            }
        }
        loop {
            match rx.recv().await {
                // Skip any line already delivered in the history batch.
                Ok(line) if line.id > last_id => {
                    if let Ok(event) = Event::default().json_data(&line) {
                        yield Ok(event);
                    }
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
