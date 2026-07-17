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
