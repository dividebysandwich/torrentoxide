use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{pause_torrent, resume_torrent};
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

    view! {
        <div class="torrent-list">
            <Show when=move || !is_empty() fallback=EmptyState>
                <For each=ids key=|id| *id children=move |id| view! { <TorrentRow id=id/> }/>
            </Show>
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
fn TorrentRow(id: usize) -> impl IntoView {
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
