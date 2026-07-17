pub mod add_panel;
pub mod confirm_modal;
pub mod dashboard;
pub mod dir_picker;
pub mod sparkline;
pub mod torrent_list;
pub mod traffic_graph;

use std::collections::{HashMap, HashSet, VecDeque};

use leptos::prelude::*;

use crate::types::{Defaults, StatsSnapshot};

/// How many samples of history the global graph keeps.
pub const GLOBAL_HIST_LEN: usize = 120;
/// How many samples each per-torrent sparkline keeps.
pub const ROW_HIST_LEN: usize = 48;

/// A pending destructive action awaiting confirmation.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmData {
    pub id: u64,
    pub name: String,
    /// `true` = cancel and delete files; `false` = cancel only.
    pub delete_files: bool,
}

/// Reactive state shared across the dashboard via context.
#[derive(Clone, Copy)]
pub struct DashboardState {
    /// Latest full snapshot from the SSE stream.
    pub snapshot: RwSignal<StatsSnapshot>,
    /// Rolling (down_bps, up_bps) history for the global graph.
    pub global_hist: RwSignal<VecDeque<(f64, f64)>>,
    /// Rolling (down_bps, up_bps) history per torrent id.
    pub torrent_hist: RwSignal<HashMap<u64, VecDeque<(f64, f64)>>>,
    /// Server defaults (download dir, browse root, auth flag).
    pub defaults: RwSignal<Defaults>,
    /// Whether the SSE stream has delivered at least one snapshot.
    pub connected: RwSignal<bool>,
    /// Destructive action pending user confirmation, if any.
    pub confirm: RwSignal<Option<ConfirmData>>,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardState {
    pub fn new() -> Self {
        Self {
            snapshot: RwSignal::new(StatsSnapshot::default()),
            global_hist: RwSignal::new(VecDeque::new()),
            torrent_hist: RwSignal::new(HashMap::new()),
            defaults: RwSignal::new(Defaults::default()),
            connected: RwSignal::new(false),
            confirm: RwSignal::new(None),
        }
    }

    /// Fold a fresh snapshot into the reactive state + history buffers.
    pub fn ingest(&self, snap: StatsSnapshot) {
        self.global_hist.update(|h| {
            h.push_back((snap.global_down_bps, snap.global_up_bps));
            while h.len() > GLOBAL_HIST_LEN {
                h.pop_front();
            }
        });

        let live_ids: HashSet<u64> = snap.torrents.iter().map(|t| t.id).collect();
        self.torrent_hist.update(|m| {
            for t in &snap.torrents {
                let dq = m.entry(t.id).or_default();
                dq.push_back((t.down_bps, t.up_bps));
                while dq.len() > ROW_HIST_LEN {
                    dq.pop_front();
                }
            }
            m.retain(|id, _| live_ids.contains(id));
        });

        self.connected.set(true);
        self.snapshot.set(snap);
    }
}

/// Convenience accessor used by child components.
pub fn dashboard_state() -> DashboardState {
    expect_context::<DashboardState>()
}
