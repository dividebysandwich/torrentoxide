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
    let engine = state.engine.clone();
    let stream = async_stream::stream! {
        // `interval` fires immediately on the first `tick`, so the client gets
        // a snapshot right away and then one per second.
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let snapshot = engine.snapshot();
            if let Ok(event) = Event::default().json_data(&snapshot) {
                yield Ok(event);
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
