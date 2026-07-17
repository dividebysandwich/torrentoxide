use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::get_defaults;
use crate::components::add_panel::AddTorrentPanel;
use crate::components::confirm_modal::ConfirmModal;
use crate::components::torrent_list::TorrentList;
use crate::components::traffic_graph::TrafficGraph;
use crate::components::DashboardState;
use crate::types::fmt_speed;

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

    let total_down = move || fmt_speed(state.snapshot.get().global_down_bps);
    let total_up = move || fmt_speed(state.snapshot.get().global_up_bps);
    let count = move || state.snapshot.get().torrents.len().to_string();
    let auth_on = move || state.defaults.get().auth_enabled;
    let connected = move || state.connected.get();

    view! {
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
                        <span class="tstat-val down">{total_down}</span>
                    </div>
                    <div class="tstat">
                        <span class="tstat-label">"UP"</span>
                        <span class="tstat-val up">{total_up}</span>
                    </div>
                    <div class="tstat">
                        <span class="tstat-label">"ACTIVE"</span>
                        <span class="tstat-val">{count}</span>
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
                <TorrentList/>
            </main>

            <ConfirmModal/>
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
