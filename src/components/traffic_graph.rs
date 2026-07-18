use leptos::prelude::*;

use crate::components::dashboard_state;
use crate::types::fmt_speed;

const W: f64 = 1000.0;
const H: f64 = 220.0;

/// Build (down_area, up_area, down_line, up_line) path strings.
///
/// Points span from x = -dx (an off-screen cushion on the left) to x = W, so the
/// plot can be slid left by exactly one point-width without exposing a gap — that
/// is how a new sample "scrolls in" smoothly from the right each second.
fn geometry(hist: &[(f64, f64)]) -> (String, String, String, String) {
    let n = hist.len();
    if n < 3 {
        return (String::new(), String::new(), String::new(), String::new());
    }
    let max = hist.iter().fold(1.0_f64, |m, (d, u)| m.max(*d).max(*u));
    let dx = W / (n as f64 - 2.0);
    let x = |i: usize| (i as f64 - 1.0) * dx;
    let y = |v: f64| H - (v / max).clamp(0.0, 1.0) * (H - 8.0) - 4.0;

    let line = |up: bool| {
        hist.iter()
            .enumerate()
            .map(|(i, (d, u))| format!("{:.1},{:.1}", x(i), y(if up { *u } else { *d })))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let area = |up: bool| {
        let mut p = format!("M {:.1},{:.1} ", x(0), H);
        for (i, (d, u)) in hist.iter().enumerate() {
            p.push_str(&format!("L {:.1},{:.1} ", x(i), y(if up { *u } else { *d })));
        }
        p.push_str(&format!("L {:.1},{:.1} Z", x(n - 1), H));
        p
    };

    (area(false), area(true), line(false), line(true))
}

/// Global up/down traffic graph. The history has a fixed, always-full time axis;
/// new samples arrive once per second and the plot slides one step to the left.
#[component]
pub fn TrafficGraph() -> impl IntoView {
    let state = dashboard_state();

    // 1 Hz trigger — the plot is re-rendered (and its CSS slide restarts) only
    // when a new history sample lands, not on every 4 Hz snapshot.
    let hist_tick = Memo::new(move |_| state.snapshot.get().hist_tick);

    let peak = move || {
        state
            .snapshot
            .get()
            .global_hist
            .iter()
            .fold(1.0_f64, |m, (d, u)| m.max(*d).max(*u))
    };

    // Resting geometry for the pulse clip (updates reactively).
    let clip_down = move || geometry(&state.snapshot.get().global_hist).0;
    let clip_up = move || geometry(&state.snapshot.get().global_hist).1;

    let down_label = move || fmt_speed(state.snapshot.get().global_down_bps);
    let up_label = move || fmt_speed(state.snapshot.get().global_up_bps);
    let scale_top = move || fmt_speed(peak());
    let scale_mid = move || fmt_speed(peak() / 2.0);

    view! {
        <div class="graph-card">
            <div class="graph-legend">
                <span class="legend-item down">
                    <span class="legend-dot"></span>"DOWN "
                    <b>{down_label}</b>
                </span>
                <span class="legend-item up">
                    <span class="legend-dot"></span>"UP "
                    <b>{up_label}</b>
                </span>
            </div>
            <svg
                class="traffic-graph"
                viewBox=format!("0 0 {W} {H}")
                preserveAspectRatio="none"
            >
                <defs>
                    <linearGradient id="grad-down" x1="0" x2="0" y1="0" y2="1">
                        <stop offset="0%" stop-color="#24e3ff" stop-opacity="0.8"/>
                        <stop offset="100%" stop-color="#24e3ff" stop-opacity="0.02"/>
                    </linearGradient>
                    <linearGradient id="grad-up" x1="0" x2="0" y1="0" y2="1">
                        <stop offset="0%" stop-color="#ff3b7b" stop-opacity="0.6"/>
                        <stop offset="100%" stop-color="#ff3b7b" stop-opacity="0.02"/>
                    </linearGradient>
                    // pulse gradient: brightest at the leading (right) edge
                    <linearGradient id="pulse-grad" x1="0" y1="0" x2="1" y2="0">
                        <stop offset="0%" stop-color="#c9fbff" stop-opacity="0"/>
                        <stop offset="78%" stop-color="#c9fbff" stop-opacity="0.12"/>
                        <stop offset="100%" stop-color="#ffffff" stop-opacity="0.72"/>
                    </linearGradient>
                    <clipPath id="pulse-clip">
                        <path d=clip_down/>
                        <path d=clip_up/>
                    </clipPath>
                </defs>

                // The plot group is re-created each 1 Hz sample so its CSS slide
                // animation restarts in sync with the data.
                {move || {
                    let _ = hist_tick.get();
                    let hist = state.snapshot.get_untracked().global_hist;
                    let (da, ua, dl, ul) = geometry(&hist);
                    view! {
                        <g class="graph-plot">
                            <path class="area area-down" d=da fill="url(#grad-down)"/>
                            <path class="area area-up" d=ua fill="url(#grad-up)"/>
                            <polyline class="gline gline-down" points=dl/>
                            <polyline class="gline gline-up" points=ul/>
                        </g>
                    }
                }}

                // left-to-right light pulse (own animation, not re-keyed)
                <g class="graph-pulse-group" clip-path="url(#pulse-clip)">
                    <rect class="graph-pulse" x="0" y="0" width="320" height="220" fill="url(#pulse-grad)"/>
                </g>
            </svg>
            <div class="crt-scan"></div>
            <div class="graph-scale">
                <span>{scale_top}</span>
                <span>{scale_mid}</span>
                <span>"0"</span>
            </div>
        </div>
    }
}
