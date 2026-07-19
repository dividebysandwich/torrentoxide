use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{dismiss_pending, pause_torrent, resume_torrent};
use crate::components::bitfield::Bitfield;
use crate::components::fx::chirp;
use crate::components::scramble::Scramble;
use crate::components::sparkline::Sparkline;
use crate::components::{dashboard_state, ConfirmData};
use crate::types::{fmt_bytes, fmt_eta, fmt_speed, TorrentState, TorrentView};

/// Briefly set a boolean signal to drive a one-shot CSS effect.
fn flash(sig: RwSignal<bool>, ms: u64) {
    sig.set(true);
    set_timeout(move || sig.set(false), Duration::from_millis(ms));
}

/// Coarse status buckets for the list filter (single-select chips).
#[derive(Clone, Copy, PartialEq)]
enum StatusFilter {
    All,
    Downloading,
    Seeding,
    Paused,
    Error,
}

impl StatusFilter {
    const ALL: [StatusFilter; 5] = [
        Self::All,
        Self::Downloading,
        Self::Seeding,
        Self::Paused,
        Self::Error,
    ];

    fn matches(&self, s: TorrentState) -> bool {
        match self {
            Self::All => true,
            Self::Downloading => matches!(s, TorrentState::Live | TorrentState::Initializing),
            Self::Seeding => matches!(s, TorrentState::Finished),
            Self::Paused => matches!(s, TorrentState::Paused),
            Self::Error => matches!(s, TorrentState::Error),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Downloading => "DOWNLOADING",
            Self::Seeding => "SEEDING",
            Self::Paused => "PAUSED",
            Self::Error => "ERROR",
        }
    }
}

/// Sort keys for the list.
#[derive(Clone, Copy, PartialEq)]
enum SortKey {
    Added,
    Name,
    Size,
    Progress,
    Down,
    Up,
    Ratio,
}

impl SortKey {
    const ALL: [SortKey; 7] = [
        Self::Added,
        Self::Name,
        Self::Size,
        Self::Progress,
        Self::Down,
        Self::Up,
        Self::Ratio,
    ];

    fn label(&self) -> &'static str {
        match self {
            Self::Added => "ADDED",
            Self::Name => "NAME",
            Self::Size => "SIZE",
            Self::Progress => "PROGRESS",
            Self::Down => "DOWN",
            Self::Up => "UP",
            Self::Ratio => "RATIO",
        }
    }
}

fn ratio(t: &TorrentView) -> f64 {
    if t.downloaded_bytes > 0 {
        t.uploaded_bytes as f64 / t.downloaded_bytes as f64
    } else {
        0.0
    }
}

#[component]
pub fn TorrentList() -> impl IntoView {
    let state = dashboard_state();

    // Client-side list controls (low-friction filter + sort).
    let query = RwSignal::new(String::new());
    let status = RwSignal::new(StatusFilter::All);
    let sort_key = RwSignal::new(SortKey::Added);
    let sort_desc = RwSignal::new(false);

    // Filter + sort once per change; the keyed <For> below preserves rows on reorder.
    let filtered: Memo<Vec<TorrentView>> = Memo::new(move |_| {
        let snap = state.snapshot.get();
        let q = query.get().trim().to_lowercase();
        let sf = status.get();
        let key = sort_key.get();
        let desc = sort_desc.get();

        let mut v: Vec<TorrentView> = snap
            .torrents
            .into_iter()
            .filter(|t| (q.is_empty() || t.name.to_lowercase().contains(&q)) && sf.matches(t.state))
            .collect();

        v.sort_by(|a, b| {
            use std::cmp::Ordering::Equal;
            let ord = match key {
                SortKey::Added => a.id.cmp(&b.id),
                SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortKey::Size => a.total_bytes.cmp(&b.total_bytes),
                SortKey::Progress => a.progress.partial_cmp(&b.progress).unwrap_or(Equal),
                SortKey::Down => a.down_bps.partial_cmp(&b.down_bps).unwrap_or(Equal),
                SortKey::Up => a.up_bps.partial_cmp(&b.up_bps).unwrap_or(Equal),
                SortKey::Ratio => ratio(a).partial_cmp(&ratio(b)).unwrap_or(Equal),
            };
            if desc {
                ord.reverse()
            } else {
                ord
            }
        });
        v
    });

    let ids = move || filtered.get().iter().map(|t| t.id).collect::<Vec<_>>();
    let is_empty = move || state.snapshot.get().torrents.is_empty();
    let no_matches = move || !is_empty() && filtered.get().is_empty();

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

    let sort_index = move || SortKey::ALL.iter().position(|k| *k == sort_key.get()).unwrap_or(0);

    view! {
        <div class="torrent-list">
            <Show when=move || !is_empty() fallback=EmptyState>
                <div class="list-toolbar">
                    <input
                        class="text-input grow"
                        r#type="text"
                        placeholder="filter by name…"
                        prop:value=move || query.get()
                        on:input=move |e| query.set(event_target_value(&e))
                    />
                    <div class="filter-chips">
                        {StatusFilter::ALL
                            .iter()
                            .map(|&f| {
                                view! {
                                    <button
                                        class="filter-chip"
                                        class:active=move || status.get() == f
                                        on:click=move |_| status.set(f)
                                    >
                                        {f.label()}
                                    </button>
                                }
                            })
                            .collect_view()}
                    </div>
                    <div class="sort-controls">
                        <span class="sort-label">"SORT"</span>
                        <select
                            class="sort-select"
                            prop:value=move || sort_index().to_string()
                            on:change=move |e| {
                                let i = event_target_value(&e).parse::<usize>().unwrap_or(0);
                                sort_key.set(SortKey::ALL[i.min(SortKey::ALL.len() - 1)]);
                            }
                        >
                            {SortKey::ALL
                                .iter()
                                .enumerate()
                                .map(|(i, &k)| {
                                    view! { <option value=i.to_string()>{k.label()}</option> }
                                })
                                .collect_view()}
                        </select>
                        <button
                            class="sort-dir"
                            title=move || if sort_desc.get() { "descending" } else { "ascending" }
                            on:click=move |_| sort_desc.update(|d| *d = !*d)
                        >
                            {move || if sort_desc.get() { "▼" } else { "▲" }}
                        </button>
                    </div>
                </div>
                <Show
                    when=move || !no_matches()
                    fallback=|| view! { <p class="list-nomatch">"— no torrents match the filter —"</p> }
                >
                    <For each=ids key=|id| *id children=render_row/>
                </Show>
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

    let history = move || torrent.get().map(|t| t.history).unwrap_or_default();

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
    let uploaded_text = move || fmt_bytes(torrent.get().map(|t| t.uploaded_bytes).unwrap_or(0) as f64);
    let eta_text = move || fmt_eta(torrent.get().and_then(|t| t.eta_secs));
    let error_text = move || torrent.get().and_then(|t| t.error);

    let is_paused = move || matches!(st(), TorrentState::Paused);

    // One-shot effects: "materialize" glitch on first appearance, RGB-glitch on
    // error, and a completion burst when the torrent finishes.
    let burst = RwSignal::new(false);
    let glitch = RwSignal::new(false);
    Effect::new(move |prev: Option<TorrentState>| {
        let cur = st();
        match prev {
            None => flash(glitch, 520),
            Some(p) if p != cur => match cur {
                // only celebrate a genuine completion, not a resume from paused
                TorrentState::Finished
                    if matches!(p, TorrentState::Live | TorrentState::Initializing) =>
                {
                    flash(burst, 1500)
                }
                TorrentState::Error => flash(glitch, 520),
                _ => {}
            },
            _ => {}
        }
        cur
    });

    // Actions
    let toggle_pause = move |_| {
        let paused = is_paused();
        chirp(if paused { 720.0 } else { 520.0 });
        spawn_local(async move {
            let _ = if paused {
                resume_torrent(id).await
            } else {
                pause_torrent(id).await
            };
        });
    };
    let ask_cancel = move |_| {
        chirp(440.0);
        state.confirm.set(Some(ConfirmData {
            id,
            name: name(),
            delete_files: false,
        }));
    };
    let ask_delete = move |_| {
        chirp(300.0);
        state.confirm.set(Some(ConfirmData {
            id,
            name: name(),
            delete_files: true,
        }));
    };
    // Clicking anywhere on the row (outside the action buttons) opens the inspector.
    let open_detail = move |_| {
        chirp(640.0);
        state.detail_id.set(Some(id));
    };

    view! {
        <div
            class="torrent-row panel row-clickable"
            class:completing=move || burst.get()
            class:glitch=move || glitch.get()
            on:click=open_detail
        >
            {move || burst.get().then(|| view! {
                <div class="completion-burst">"◤ COMPLETE ◥"</div>
            })}
            <div class="tr-main">
                <div class="tr-head">
                    <span class="tr-name" title=name>
                        <Scramble text=Signal::derive(name)/>
                    </span>
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

                <Bitfield progress=Signal::derive(progress) id=id/>

                <div class="tr-stats">
                    <span class="stat down">"▼ " {down_text}</span>
                    <span class="stat up">"▲ " {up_text}</span>
                    <span class="stat down">"↓ " {size_text}</span>
                    <span class="stat up">"↑ " {uploaded_text}</span>
                    <span class="stat eta">"ETA " {eta_text}</span>
                </div>

                {move || error_text().map(|e| view! { <p class="tr-error">{e}</p> })}
            </div>

            <div class="tr-side">
                <Sparkline points=Signal::derive(history)/>
                <div class="tr-actions" on:click=|e| e.stop_propagation()>
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
