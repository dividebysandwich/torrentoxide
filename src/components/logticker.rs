//! A terminal-style "system log" that streams the real app + librqbit `tracing`
//! output over `/api/logs` — buffered history on connect, then live — capped to
//! the most recent lines so the browser stays responsive.

use std::collections::VecDeque;

use leptos::prelude::*;

use crate::types::LogLine;

#[component]
pub fn LogTicker() -> impl IntoView {
    let lines = RwSignal::new(VecDeque::<LogLine>::new());
    start_log_stream(lines);

    // Newest first (push_front on arrival), capped to MAX_LINES.
    let entries = move || lines.get().into_iter().collect::<Vec<LogLine>>();

    view! {
        <div class="logticker panel">
            <div class="lt-head">"// SYSTEM LOG"<span class="lt-blink">"▮"</span></div>
            <div class="lt-body">
                <For each=entries key=|l| l.id let:l>
                    <div class=format!("lt-line lt-{}", l.level.to_lowercase())>
                        <span class="lt-time">{l.time.clone()}</span>
                        <span class="lt-level">{l.level.clone()}</span>
                        <span class="lt-msg">{l.message.clone()}</span>
                    </div>
                </For>
            </div>
        </div>
    }
}

#[cfg(feature = "hydrate")]
fn start_log_stream(lines: RwSignal<VecDeque<LogLine>>) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    const MAX_LINES: usize = 100;

    Effect::new(move |_| {
        let Ok(es) = web_sys::EventSource::new("/api/logs") else {
            return;
        };
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
            move |e: web_sys::MessageEvent| {
                if let Some(text) = e.data().as_string() {
                    if let Ok(line) = serde_json::from_str::<LogLine>(&text) {
                        lines.update(|l| {
                            l.push_front(line);
                            while l.len() > MAX_LINES {
                                l.pop_back();
                            }
                        });
                    }
                }
            },
        );
        es.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();
        // Close the stream when this component unmounts (e.g. navigating away),
        // so EventSource connections don't accumulate.
        let es_close = es.clone();
        on_cleanup(move || {
            es_close.close();
        });
    });
}

#[cfg(not(feature = "hydrate"))]
fn start_log_stream(_lines: RwSignal<VecDeque<LogLine>>) {}
