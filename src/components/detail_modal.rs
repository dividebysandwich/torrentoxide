//! Torrent inspector modal: overview, peer swarm, trackers, DHT node count and
//! a file tree. Opens when `DashboardState::detail_id` is set (row click) and
//! polls the server once a second while open.
//!
//! Split into small cards so each `view!` stays shallow — a single monolithic
//! view for the whole modal overflows Leptos's nested-type machinery.

use std::collections::HashSet;
use std::time::Duration;

use leptos::portal::Portal;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{get_torrent_detail, update_torrent_files};
use crate::components::dashboard_state;
use crate::components::file_tree::FileTree;
use crate::types::{fmt_bytes, TorrentDetail};

type Detail = RwSignal<Option<TorrentDetail>>;

#[component]
fn Overview(detail: Detail) -> impl IntoView {
    let progress = move || detail.get().map(|d| d.progress).unwrap_or(0.0);
    let pct_text = move || format!("{:.1}%", progress() * 100.0);
    let size_text = move || {
        detail
            .get()
            .map(|d| {
                format!(
                    "{} / {}",
                    fmt_bytes(d.downloaded_bytes as f64),
                    fmt_bytes(d.total_bytes as f64)
                )
            })
            .unwrap_or_default()
    };
    let up_text = move || fmt_bytes(detail.get().map(|d| d.uploaded_bytes).unwrap_or(0) as f64);
    let ratio_text = move || {
        let (up, down) = detail
            .get()
            .map(|d| (d.uploaded_bytes, d.downloaded_bytes))
            .unwrap_or((0, 0));
        let r = if down > 0 { up as f64 / down as f64 } else { 0.0 };
        format!("{r:.2}")
    };
    let output = move || detail.get().map(|d| d.output_folder).unwrap_or_default();

    view! {
        <div class="detail-path">
            <span class="dir-current-label">"PATH"</span>
            <code>{output}</code>
        </div>
        <div class="detail-overview">
            <div class="progress">
                <div
                    class="progress-fill"
                    class:complete=move || { progress() >= 0.9995 }
                    style=move || format!("width:{:.2}%", (progress() * 100.0).clamp(0.0, 100.0))
                ></div>
                <span class="progress-pct">{pct_text}</span>
            </div>
            <div class="detail-stats">
                <span class="stat down">"↓ " {size_text}</span>
                <span class="stat up">"↑ " {up_text}</span>
                <span class="stat">"RATIO " <b>{ratio_text}</b></span>
            </div>
        </div>
    }
}

#[component]
fn SwarmCard(detail: Detail) -> impl IntoView {
    let dots = move || {
        let p = detail.get().map(|d| d.peers).unwrap_or_default();
        let mut out = Vec::new();
        for _ in 0..p.live.min(64) {
            out.push(view! { <span class="swarm-dot live"></span> });
        }
        for _ in 0..p.connecting.min(32) {
            out.push(view! { <span class="swarm-dot conn"></span> });
        }
        out
    };
    let live = move || detail.get().map(|d| d.peers.live).unwrap_or(0);
    let conn = move || detail.get().map(|d| d.peers.connecting).unwrap_or(0);
    let seen = move || detail.get().map(|d| d.peers.seen).unwrap_or(0);
    let dead = move || detail.get().map(|d| d.peers.dead).unwrap_or(0);

    view! {
        <section class="detail-card swarm-card">
            <span class="detail-card-title">"SWARM"</span>
            <div class="swarm-field">{dots}</div>
            <div class="swarm-legend">
                <span class="sw live">"◉ LIVE " <b>{live}</b></span>
                <span class="sw conn">"◐ CONN " <b>{conn}</b></span>
                <span class="sw seen">"◌ SEEN " <b>{seen}</b></span>
                <span class="sw dead">"✕ DEAD " <b>{dead}</b></span>
            </div>
        </section>
    }
}

#[component]
fn DhtCard(detail: Detail) -> impl IntoView {
    let count = move || detail.get().map(|d| d.dht_nodes).unwrap_or(0).to_string();
    let on = move || detail.get().map(|d| d.dht_enabled).unwrap_or(false);
    view! {
        <section class="detail-card dht-card">
            <span class="detail-card-title">"DHT NODES"</span>
            <div class="dht-count" class:off=move || !on()>{count}</div>
            <span class="dht-status" class:on=on>
                {move || if on() { "◉ ROUTING TABLE" } else { "○ DHT DISABLED" }}
            </span>
        </section>
    }
}

#[component]
fn TrackerCard(detail: Detail) -> impl IntoView {
    let trackers = move || detail.get().map(|d| d.trackers).unwrap_or_default();
    let active = move || {
        detail
            .get()
            .map(|d| d.peers.live + d.peers.connecting > 0)
            .unwrap_or(false)
    };
    view! {
        <section class="detail-card tracker-card">
            <span class="detail-card-title">"TRACKERS"</span>
            <div class="tracker-list">
                <For each=trackers key=|t| t.url.clone() let:t>
                    <div class="tracker">
                        <span class=format!("tk-scheme s-{}", t.scheme.to_lowercase())>{t.scheme.clone()}</span>
                        <span class="tk-host">{t.host.clone()}</span>
                        <span class="tk-dot" class:active=active></span>
                    </div>
                </For>
                {move || trackers().is_empty().then(|| view! {
                    <p class="tree-empty">"— no trackers (DHT / PEX only) —"</p>
                })}
            </div>
        </section>
    }
}

#[component]
fn FilesCard(detail: Detail, selected: RwSignal<HashSet<usize>>) -> impl IntoView {
    let files_sig = Signal::derive(move || detail.get().map(|d| d.files).unwrap_or_default());
    let applying = RwSignal::new(false);
    let applied = RwSignal::new(false);

    let select_all = move |_| selected.set(files_sig.get().iter().map(|f| f.index).collect());
    let select_none = move |_| selected.set(HashSet::new());
    let count = move || selected.get().len();

    let apply = move |_| {
        let Some(id) = detail.get().map(|d| d.id) else {
            return;
        };
        let mut indices: Vec<usize> = selected.get().into_iter().collect();
        indices.sort_unstable();
        applying.set(true);
        applied.set(false);
        spawn_local(async move {
            let _ = update_torrent_files(id, indices).await;
            applying.set(false);
            applied.set(true);
            set_timeout(move || applied.set(false), Duration::from_millis(2500));
        });
    };

    let apply_label = move || {
        if applying.get() {
            "APPLYING…".to_string()
        } else if applied.get() {
            "✓ APPLIED".to_string()
        } else {
            format!("APPLY ({})", count())
        }
    };

    view! {
        <section class="detail-card files-card">
            <div class="files-head">
                <span class="detail-card-title">"FILES"</span>
                <div class="fsel-toolbar">
                    <button class="btn btn-ghost btn-sm" on:click=select_all>"✓ All"</button>
                    <button class="btn btn-ghost btn-sm" on:click=select_none>"✕ None"</button>
                    <button
                        class="btn btn-primary btn-sm"
                        prop:disabled=move || applying.get() || count() == 0
                        on:click=apply
                    >
                        {apply_label}
                    </button>
                </div>
            </div>
            <FileTree files=files_sig selected=selected interactive=true/>
        </section>
    }
}

#[component]
pub fn DetailModal() -> impl IntoView {
    let state = dashboard_state();
    let detail: Detail = RwSignal::new(None);
    let selected = RwSignal::new(HashSet::<usize>::new());
    let handle = StoredValue::new(None::<IntervalHandle>);
    // Seed the selection set from file inclusion only on the first fetch, so the
    // user's in-progress checkbox edits aren't clobbered by later polls.
    let sel_init = StoredValue::new(false);

    let fetch = move |id: u64| {
        spawn_local(async move {
            if let Ok(d) = get_torrent_detail(id).await {
                if !sel_init.get_value() {
                    selected.set(d.files.iter().filter(|f| f.included).map(|f| f.index).collect());
                    sel_init.set_value(true);
                }
                detail.set(Some(d));
            }
        });
    };

    // Start / stop polling as the opened torrent changes.
    Effect::new(move |_| {
        handle.update_value(|h| {
            if let Some(hh) = h.take() {
                hh.clear();
            }
        });
        match state.detail_id.get() {
            Some(id) => {
                sel_init.set_value(false);
                detail.set(None);
                fetch(id);
                let h = set_interval_with_handle(move || fetch(id), Duration::from_millis(1000)).ok();
                handle.set_value(h);
            }
            None => detail.set(None),
        }
    });

    let close = move |_| state.detail_id.set(None);
    let name = move || detail.get().map(|d| d.name).unwrap_or_default();
    let loading = move || detail.get().is_none();

    view! {
        <Portal>
        {move || state.detail_id.get().map(|_| {
            view! {
                <div class="modal-overlay" on:click=close>
                    <div class="modal detail-modal" on:click=|e| e.stop_propagation()>
                        <div class="detail-head">
                            <h3 class="modal-title">{name}</h3>
                            <button class="icon-btn" title="Close" on:click=close>"✕"</button>
                        </div>
                        <Show when=loading fallback=|| ()>
                            <div class="detail-loading"><span class="spinner"></span>"reading torrent…"</div>
                        </Show>
                        <Overview detail=detail/>
                        <div class="detail-grid">
                            <SwarmCard detail=detail/>
                            <DhtCard detail=detail/>
                        </div>
                        <TrackerCard detail=detail/>
                        <FilesCard detail=detail selected=selected/>
                    </div>
                </div>
            }
        })}
        </Portal>
    }
}
