//! In-memory capture of `tracing` output (the app + librqbit) for the system-log
//! UI: a capped ring buffer for history plus a broadcast channel for live lines.

use std::collections::VecDeque;
use std::fmt::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

use crate::types::LogLine;

pub struct LogBuffer {
    ring: Mutex<VecDeque<LogLine>>,
    tx: broadcast::Sender<LogLine>,
    seq: AtomicU64,
    cap: usize,
}

impl LogBuffer {
    pub fn new(cap: usize) -> Arc<Self> {
        let (tx, _rx) = broadcast::channel(512);
        Arc::new(Self {
            ring: Mutex::new(VecDeque::with_capacity(cap)),
            tx,
            seq: AtomicU64::new(1),
            cap,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogLine> {
        self.tx.subscribe()
    }

    /// Buffered history, oldest first.
    pub fn history(&self) -> Vec<LogLine> {
        self.ring
            .lock()
            .map(|r| r.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn push(&self, level: &str, message: String) {
        let line = LogLine {
            id: self.seq.fetch_add(1, Ordering::Relaxed),
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            level: level.to_string(),
            message,
        };
        if let Ok(mut ring) = self.ring.lock() {
            ring.push_back(line.clone());
            while ring.len() > self.cap {
                ring.pop_front();
            }
        }
        // Best-effort broadcast; slow/absent receivers just miss live lines.
        let _ = self.tx.send(line);
    }
}

/// Collects an event's `message` field (plus any structured fields) into a string.
#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.message, " {}={value:?}", field.name());
        }
    }
}

/// A `tracing` layer that mirrors every (filtered) event into a [`LogBuffer`].
pub struct LogLayer {
    buf: Arc<LogBuffer>,
}

impl LogLayer {
    pub fn new(buf: Arc<LogBuffer>) -> Self {
        Self { buf }
    }
}

impl<S: Subscriber> Layer<S> for LogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let msg = visitor.message.trim().to_string();
        // Prefix with the crate name (e.g. `librqbit`, `torrentoxide`).
        let src = meta.target().split("::").next().unwrap_or(meta.target());
        let message = if msg.is_empty() {
            src.to_string()
        } else {
            format!("{src}: {msg}")
        };
        self.buf.push(&meta.level().to_string(), message);
    }
}
