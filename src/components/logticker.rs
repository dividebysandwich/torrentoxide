//! A terminal-style "system log" that narrates torrent events (add / state
//! change / remove) diffed from the live snapshot, plus periodic flavor lines.

use std::collections::{HashMap, VecDeque};

use leptos::prelude::*;

use crate::components::fx::{now_hms, rand_index};
use crate::components::dashboard_state;
use crate::types::TorrentState;

const MAX_LINES: usize = 40;

const FLAVOR: &[&str] = &[
    "DHT UPLINK STABLE",
    "PEER HANDSHAKE :: OK",
    "PIECE VERIFIED :: SHA1 MATCH",
    "TRACKER ANNOUNCE :: 200",
    "ROUTING TABLE REFRESHED",
    "KERNEL HEARTBEAT :: NOMINAL",
    "NAT TRAVERSAL :: HOLEPUNCH OK",
    "BLOCK REQUEST QUEUED",
    "CHOKE ALGORITHM :: RECALC",
];

fn trunc(s: &str) -> String {
    if s.chars().count() > 34 {
        let t: String = s.chars().take(33).collect();
        format!("{t}…")
    } else {
        s.to_string()
    }
}

fn state_label(s: TorrentState) -> &'static str {
    match s {
        TorrentState::Initializing => "SYNC",
        TorrentState::Live => "LIVE",
        TorrentState::Paused => "SUSPENDED",
        TorrentState::Finished => "COMPLETE",
        TorrentState::Error => "FAULT",
    }
}

#[component]
pub fn LogTicker() -> impl IntoView {
    let state = dashboard_state();
    let lines = RwSignal::new(VecDeque::<(u64, String)>::new());
    let seq = StoredValue::new(0u64);
    let tick = StoredValue::new(0u64);

    let push = move |text: String| {
        let s = seq.get_value();
        seq.set_value(s + 1);
        lines.update(|l| {
            l.push_front((s, format!("[{}]  {}", now_hms(), text)));
            while l.len() > MAX_LINES {
                l.pop_back();
            }
        });
    };

    Effect::new(move |prev: Option<HashMap<u64, (TorrentState, String)>>| {
        let snap = state.snapshot.get();
        let cur: HashMap<u64, (TorrentState, String)> = snap
            .torrents
            .iter()
            .map(|t| (t.id, (t.state, t.name.clone())))
            .collect();

        if let Some(prev) = &prev {
            for (id, (st, name)) in &cur {
                match prev.get(id) {
                    None => push(format!("ACQUIRED :: {}", trunc(name))),
                    Some((pst, _)) if pst != st => {
                        push(format!("{} :: {}", state_label(*st), trunc(name)))
                    }
                    _ => {}
                }
            }
            for (id, (_, name)) in prev {
                if !cur.contains_key(id) {
                    push(format!("PURGED :: {}", trunc(name)));
                }
            }
        }

        let tk = tick.get_value();
        tick.set_value(tk + 1);
        // ~ every 4 seconds at the 4 Hz sample rate
        if tk % 16 == 8 {
            push(FLAVOR[rand_index(FLAVOR.len())].to_string());
        }

        cur
    });

    let entries = move || lines.get().into_iter().collect::<Vec<(u64, String)>>();

    view! {
        <div class="logticker panel">
            <div class="lt-head">"// SYSTEM LOG"<span class="lt-blink">"▮"</span></div>
            <div class="lt-body">
                <For
                    each=entries
                    key=|(s, _)| *s
                    children=move |(_, text)| view! { <div class="lt-line">{text}</div> }
                />
            </div>
        </div>
    }
}
