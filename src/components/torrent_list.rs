use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{dismiss_pending, pause_torrent, resume_torrent};
use crate::components::sparkline::Sparkline;
use crate::components::{dashboard_state, ConfirmData};
use crate::types::{fmt_bytes, fmt_eta, fmt_speed, TorrentState, TorrentView};

#[component]
pub fn TorrentList() -> impl IntoView {
    let state = dashboard_state();

    let ids = move || {
        state
            .snapshot
            .get()
            .torrents
            .iter()
            .map(|t| t.id)
            .collect::<Vec<_>>()
    };
    let is_empty = move || state.snapshot.get().torrents.is_empty();

    let render_row = move |id: u64| {
        // A synthetic (pending) id never becomes a real id, so deciding once at
        // row-creation time is stable for the row's lifetime.
        let pending = state
            .snapshot
            .get_untracked()
            .torrents
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.pending)
            .unwrap_or(false);
        if pending {
            view! { <PendingRow id=id/> }.into_any()
        } else {
            view! { <TorrentRow id=id/> }.into_any()
        }
    };

    view! {
        <div class="torrent-list">
            <Show when=move || !is_empty() fallback=EmptyState>
                <For each=ids key=|id| *id children=render_row/>
            </Show>
        </div>
    }
}

#[component]
fn PendingRow(id: u64) -> impl IntoView {
    let state = dashboard_state();

    let torrent: Memo<Option<TorrentView>> = Memo::new(move |_| {
        state
            .snapshot
            .get()
            .torrents
            .iter()
            .find(|t| t.id == id)
            .cloned()
    });

    let label = move || torrent.get().map(|t| t.name).unwrap_or_default();
    let failed = move || matches!(torrent.get().map(|t| t.state), Some(TorrentState::Error));
    let error = move || torrent.get().and_then(|t| t.error);

    let dismiss = move |_| {
        spawn_local(async move {
            let _ = dismiss_pending(id).await;
        });
    };

    view! {
        <div class="torrent-row panel pending-row">
            <div class="tr-main">
                <div class="tr-head">
                    <span class="tr-name" title=label>{label}</span>
                    <span class=move || {
                        if failed() { "badge badge-error" } else { "badge badge-initializing" }
                    }>
                        {move || if failed() { "failed" } else { "resolving" }}
                    </span>
                </div>
                {move || {
                    if failed() {
                        view! {
                            <p class="tr-error">{error().unwrap_or_else(|| "failed to add torrent".into())}</p>
                        }
                        .into_any()
                    } else {
                        view! {
                            <div class="resolving">
                                <span class="spinner"></span>
                                "fetching metadata from peers…"
                            </div>
                        }
                        .into_any()
                    }
                }}
            </div>
            <div class="tr-side">
                {move || failed().then(|| view! {
                    <button class="btn btn-ghost btn-sm" on:click=dismiss>"Dismiss"</button>
                })}
            </div>
        </div>
    }
}

#[component]
fn EmptyState() -> impl IntoView {
    view! {
        <div class="empty-state">
            <div class="empty-glyph">"◇"</div>
            <p>"No torrents yet."</p>
            <p class="empty-sub">"Paste a magnet link, a .torrent URL, or upload a file above."</p>
        </div>
    }
}

#[component]
fn TorrentRow(id: u64) -> impl IntoView {
    let state = dashboard_state();

    let torrent: Memo<Option<TorrentView>> = Memo::new(move |_| {
        state
            .snapshot
            .get()
            .torrents
            .iter()
            .find(|t| t.id == id)
            .cloned()
    });

    let history = move || {
        state
            .torrent_hist
            .get()
            .get(&id)
            .map(|dq| dq.iter().copied().collect::<Vec<(f64, f64)>>())
            .unwrap_or_default()
    };

    let name = move || torrent.get().map(|t| t.name).unwrap_or_default();
    let st = move || torrent.get().map(|t| t.state).unwrap_or(TorrentState::Initializing);
    let progress = move || torrent.get().map(|t| t.progress).unwrap_or(0.0);
    let pct_text = move || format!("{:.1}%", progress() * 100.0);
    let down_text = move || fmt_speed(torrent.get().map(|t| t.down_bps).unwrap_or(0.0));
    let up_text = move || fmt_speed(torrent.get().map(|t| t.up_bps).unwrap_or(0.0));
    let size_text = move || {
        torrent
            .get()
            .map(|t| format!("{} / {}", fmt_bytes(t.downloaded_bytes as f64), fmt_bytes(t.total_bytes as f64)))
            .unwrap_or_default()
    };
    let eta_text = move || fmt_eta(torrent.get().and_then(|t| t.eta_secs));
    let error_text = move || torrent.get().and_then(|t| t.error);

    let is_paused = move || matches!(st(), TorrentState::Paused);

    // Actions
    let toggle_pause = move |_| {
        let paused = is_paused();
        spawn_local(async move {
            let _ = if paused {
                resume_torrent(id).await
            } else {
                pause_torrent(id).await
            };
        });
    };
    let ask_cancel = move |_| {
        state.confirm.set(Some(ConfirmData {
            id,
            name: name(),
            delete_files: false,
        }));
    };
    let ask_delete = move |_| {
        state.confirm.set(Some(ConfirmData {
            id,
            name: name(),
            delete_files: true,
        }));
    };

    view! {
        <div class="torrent-row panel">
            <div class="tr-main">
                <div class="tr-head">
                    <span class="tr-name" title=name>{name}</span>
                    <span class=move || format!("badge badge-{}", st().label())>
                        {move || st().label()}
                    </span>
                </div>

                <div class="progress">
                    <div
                        class="progress-fill"
                        class:complete=move || matches!(st(), TorrentState::Finished)
                        style=move || format!("width:{:.2}%", (progress() * 100.0).clamp(0.0, 100.0))
                    ></div>
                    <span class="progress-pct">{pct_text}</span>
                </div>

                <div class="tr-stats">
                    <span class="stat down">"▼ " {down_text}</span>
                    <span class="stat up">"▲ " {up_text}</span>
                    <span class="stat size">{size_text}</span>
                    <span class="stat eta">"ETA " {eta_text}</span>
                </div>

                {move || error_text().map(|e| view! { <p class="tr-error">{e}</p> })}
            </div>

            <div class="tr-side">
                <Sparkline points=Signal::derive(history)/>
                <div class="tr-actions">
                    <button
                        class="icon-btn"
                        title=move || if is_paused() { "Resume" } else { "Pause" }
                        on:click=toggle_pause
                    >
                        {move || if is_paused() { "▶" } else { "❚❚" }}
                    </button>
                    <button class="icon-btn" title="Cancel (keep files)" on:click=ask_cancel>"✕"</button>
                    <button class="icon-btn danger" title="Cancel & delete files" on:click=ask_delete>"🗑"</button>
                </div>
            </div>
        </div>
    }
}
