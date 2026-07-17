use leptos::prelude::*;

use crate::components::dashboard_state;
use crate::types::fmt_speed;

const W: f64 = 1000.0;
const H: f64 = 220.0;

/// Global up/down traffic graph: two filled areas with animated neon gradients.
#[component]
pub fn TrafficGraph() -> impl IntoView {
    let state = dashboard_state();

    // Returns (down_area, up_area, down_line, up_line) SVG path/points strings.
    let geometry = move || {
        let hist: Vec<(f64, f64)> = state.global_hist.get().into_iter().collect();
        let n = hist.len();
        if n < 2 {
            return (String::new(), String::new(), String::new(), String::new());
        }
        let max = hist.iter().fold(1.0_f64, |m, (d, u)| m.max(*d).max(*u));
        let dx = W / (n as f64 - 1.0);
        let y_of = |v: f64| H - (v / max).clamp(0.0, 1.0) * (H - 8.0) - 4.0;

        let line_pts = |up: bool| {
            hist.iter()
                .enumerate()
                .map(|(i, (d, u))| {
                    let v = if up { *u } else { *d };
                    format!("{:.1},{:.1}", i as f64 * dx, y_of(v))
                })
                .collect::<Vec<_>>()
                .join(" ")
        };
        let area_path = |up: bool| {
            let mut p = format!("M 0,{H:.1} ");
            for (i, (d, u)) in hist.iter().enumerate() {
                let v = if up { *u } else { *d };
                p.push_str(&format!("L {:.1},{:.1} ", i as f64 * dx, y_of(v)));
            }
            p.push_str(&format!("L {:.1},{H:.1} Z", (n as f64 - 1.0) * dx));
            p
        };

        (area_path(false), area_path(true), line_pts(false), line_pts(true))
    };

    let down_area = move || geometry().0;
    let up_area = move || geometry().1;
    let down_line = move || geometry().2;
    let up_line = move || geometry().3;

    let down_label = move || fmt_speed(state.snapshot.get().global_down_bps);
    let up_label = move || fmt_speed(state.snapshot.get().global_up_bps);

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
                        <stop offset="0%" stop-color="#00f0ff" stop-opacity="0.75"/>
                        <stop offset="100%" stop-color="#00f0ff" stop-opacity="0.02"/>
                    </linearGradient>
                    <linearGradient id="grad-up" x1="0" x2="0" y1="0" y2="1">
                        <stop offset="0%" stop-color="#ff4df0" stop-opacity="0.6"/>
                        <stop offset="100%" stop-color="#ff4df0" stop-opacity="0.02"/>
                    </linearGradient>
                </defs>
                <path class="area area-down" d=down_area fill="url(#grad-down)"/>
                <path class="area area-up" d=up_area fill="url(#grad-up)"/>
                <polyline class="gline gline-down" points=down_line/>
                <polyline class="gline gline-up" points=up_line/>
            </svg>
        </div>
    }
}
