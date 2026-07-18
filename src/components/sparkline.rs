use leptos::prelude::*;

/// A tiny dual-series (download + upload) SVG sparkline with an oscilloscope
/// look: faint baseline grid + a glowing leading-edge dot on each trace.
/// `points` is a list of `(down_bps, up_bps)` samples, oldest first.
#[component]
pub fn Sparkline(
    #[prop(into)] points: Signal<Vec<(f64, f64)>>,
    #[prop(default = 150.0)] width: f64,
    #[prop(default = 36.0)] height: f64,
) -> impl IntoView {
    let poly = move |up: bool| {
        let pts = points.get();
        let n = pts.len();
        if n < 2 {
            return String::new();
        }
        let max = pts.iter().fold(1.0_f64, |m, (d, u)| m.max(*d).max(*u));
        let dx = width / (n as f64 - 1.0);
        pts.iter()
            .enumerate()
            .map(|(i, (d, u))| {
                let v = if up { *u } else { *d };
                let x = i as f64 * dx;
                let y = height - (v / max).clamp(0.0, 1.0) * (height - 3.0) - 1.5;
                format!("{x:.1},{y:.1}")
            })
            .collect::<Vec<_>>()
            .join(" ")
    };

    // The leading-edge point (right side) for the glowing dot.
    let last_y = move |up: bool| {
        let pts = points.get();
        let n = pts.len();
        if n < 2 {
            return -10.0;
        }
        let max = pts.iter().fold(1.0_f64, |m, (d, u)| m.max(*d).max(*u));
        let (d, u) = pts[n - 1];
        let v = if up { u } else { d };
        height - (v / max).clamp(0.0, 1.0) * (height - 3.0) - 1.5
    };

    let down_pts = move || poly(false);
    let up_pts = move || poly(true);
    let down_dot = move || last_y(false);
    let up_dot = move || last_y(true);
    let mid = height / 2.0;
    let view_box = format!("0 0 {width} {height}");

    view! {
        <svg class="sparkline" viewBox=view_box preserveAspectRatio="none">
            <line class="spark-grid" x1=0.0 y1=mid x2=width y2=mid/>
            <polyline class="spark-down" points=down_pts/>
            <polyline class="spark-up" points=up_pts/>
            <circle class="spark-dot down" cx=width cy=down_dot r=1.7/>
            <circle class="spark-dot up" cx=width cy=up_dot r=1.7/>
        </svg>
    }
}
