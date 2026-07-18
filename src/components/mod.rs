pub mod add_panel;
pub mod bitfield;
pub mod boot;
pub mod confirm_modal;
pub mod control_deck;
pub mod dashboard;
pub mod dir_picker;
pub mod fx;
pub mod logticker;
pub mod scramble;
pub mod sparkline;
pub mod torrent_list;
pub mod traffic_graph;

use leptos::prelude::*;

use crate::types::{Defaults, StatsSnapshot};

/// A pending destructive action awaiting confirmation.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmData {
    pub id: u64,
    pub name: String,
    /// `true` = cancel and delete files; `false` = cancel only.
    pub delete_files: bool,
}

/// Reactive state shared across the dashboard via context.
///
/// Traffic history now lives on the server (recorded regardless of connected
/// clients), so the client simply renders whatever the latest snapshot carries.
#[derive(Clone, Copy)]
pub struct DashboardState {
    /// Latest full snapshot from the SSE stream (includes server-side history).
    pub snapshot: RwSignal<StatsSnapshot>,
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
            defaults: RwSignal::new(Defaults::default()),
            connected: RwSignal::new(false),
            confirm: RwSignal::new(None),
        }
    }

    /// Store the latest snapshot from the live stream.
    pub fn ingest(&self, snap: StatsSnapshot) {
        self.connected.set(true);
        self.snapshot.set(snap);
    }
}

/// Convenience accessor used by child components.
pub fn dashboard_state() -> DashboardState {
    expect_context::<DashboardState>()
}
