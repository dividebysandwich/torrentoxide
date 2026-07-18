use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::get_defaults;
use crate::components::add_panel::AddTorrentPanel;
use crate::components::confirm_modal::ConfirmModal;
use crate::components::control_deck::ControlDeck;
use crate::components::detail_modal::DetailModal;
use crate::components::torrent_list::TorrentList;
use crate::components::traffic_graph::TrafficGraph;
use crate::components::boot::BootSequence;
use crate::components::logticker::LogTicker;
use crate::components::scramble::{Scramble, ScrambleMode};
use crate::components::DashboardState;
use crate::types::fmt_bytes;

#[component]
pub fn Dashboard() -> impl IntoView {
    let state = DashboardState::new();
    provide_context(state);

    // Fetch server defaults (download dir, browse root, auth flag) once, client-side.
    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(defaults) = get_defaults().await {
                state.defaults.set(defaults);
            }
        });
    });

    // Open the live stats stream (browser only).
    start_sse(state);

    // Total transferred across all torrents (cumulative bytes, not speed).
    let total_down = move || {
        fmt_bytes(
            state
                .snapshot
                .get()
                .torrents
                .iter()
                .map(|t| t.downloaded_bytes)
                .sum::<u64>() as f64,
        )
    };
    let total_up = move || {
        fmt_bytes(
            state
                .snapshot
                .get()
                .torrents
                .iter()
                .map(|t| t.uploaded_bytes)
                .sum::<u64>() as f64,
        )
    };
    let count = move || state.snapshot.get().torrents.len().to_string();
    let auth_on = move || state.defaults.get().auth_enabled;
    let connected = move || state.connected.get();

    view! {
        <BootSequence/>
        // decorative HUD frame + scattered technical readouts (non-interactive)
        <div class="hud-flair" aria-hidden="true">
            <div class="hud-data left">"SYS//NETRUNNER.v6\nPROTOCOL 6520-A44\nLINK: SECURE\n0x1F.4A.C2"</div>
            <div class="hud-data right">"TRAFFIC.MON\nBUF 0xA830\n1010 0110 1101\nSTATUS: LIVE"</div>
        </div>
        <div class="app-shell">
            <header class="topbar">
                <div class="brand">
                    <span class="brand-mark">"⬢"</span>
                    <span class="brand-name">"TORRENT"<b>"OXIDE"</b></span>
                    <span
                        class=move || if connected() { "status-dot online" } else { "status-dot" }
                        title=move || if connected() { "live" } else { "connecting…" }
                    ></span>
                </div>
                <div class="topbar-stats">
                    <div class="tstat">
                        <span class="tstat-label">"DOWN"</span>
                        <span class="tstat-val down">
                            <Scramble text=Signal::derive(total_down) mode=ScrambleMode::Roll/>
                        </span>
                    </div>
                    <div class="tstat">
                        <span class="tstat-label">"UP"</span>
                        <span class="tstat-val up">
                            <Scramble text=Signal::derive(total_up) mode=ScrambleMode::Roll/>
                        </span>
                    </div>
                    <div class="tstat">
                        <span class="tstat-label">"ACTIVE"</span>
                        <span class="tstat-val">
                            <Scramble text=Signal::derive(count) mode=ScrambleMode::Roll/>
                        </span>
                    </div>
                </div>
                <Show when=auth_on fallback=|| ()>
                    <form method="post" action="/logout" class="logout-form">
                        <button class="btn btn-ghost btn-sm" r#type="submit">"Logout"</button>
                    </form>
                </Show>
            </header>

            <main class="content">
                <TrafficGraph/>
                <AddTorrentPanel/>
                <ControlDeck/>
                <TorrentList/>
                <LogTicker/>
            </main>

            <ConfirmModal/>
            <DetailModal/>
        </div>
    }
}

#[cfg(feature = "hydrate")]
fn start_sse(state: DashboardState) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    use crate::types::StatsSnapshot;

    Effect::new(move |_| {
        let Ok(es) = web_sys::EventSource::new("/api/events") else {
            return;
        };
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
            move |e: web_sys::MessageEvent| {
                if let Some(text) = e.data().as_string() {
                    if let Ok(snapshot) = serde_json::from_str::<StatsSnapshot>(&text) {
                        state.ingest(snapshot);
                    }
                }
            },
        );
        es.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        // Keep the closure and connection alive for the app's lifetime.
        on_message.forget();
        std::mem::forget(es);
    });
}

#[cfg(not(feature = "hydrate"))]
fn start_sse(_state: DashboardState) {}
