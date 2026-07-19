//! The `/` page body: control deck, traffic graph, add panel, torrent list and
//! log ticker. Shared state, top bar, and the SSE stream live in `Layout`.

use leptos::prelude::*;

use crate::components::add_panel::AddTorrentPanel;
use crate::components::control_deck::ControlDeck;
use crate::components::logticker::LogTicker;
use crate::components::torrent_list::TorrentList;
use crate::components::traffic_graph::TrafficGraph;

#[component]
pub fn Dashboard() -> impl IntoView {
    view! {
        <ControlDeck/>
        <TrafficGraph/>
        <AddTorrentPanel/>
        <TorrentList/>
        <LogTicker/>
    }
}
