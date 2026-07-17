use leptos::prelude::*;

/// A tiny dual-series (download + upload) SVG sparkline.
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
        let max = pts
            .iter()
            .fold(1.0_f64, |m, (d, u)| m.max(*d).max(*u));
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

    let down_pts = move || poly(false);
    let up_pts = move || poly(true);
    let view_box = format!("0 0 {width} {height}");

    view! {
        <svg class="sparkline" viewBox=view_box preserveAspectRatio="none">
            <polyline class="spark-down" points=down_pts/>
            <polyline class="spark-up" points=up_pts/>
        </svg>
    }
}
