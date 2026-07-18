//! A torrent "piece map": a grid of cells that light up as the download
//! progresses. librqbit doesn't expose the real bitfield publicly, so cells are
//! filled in a stable per-torrent pseudo-random order — giving the scattered,
//! out-of-order look of a real bitfield from just the overall progress.

use leptos::prelude::*;

use crate::components::fx::hash2;

const CELLS: usize = 96;

/// For each cell, its rank in the fill order (0 = fills first).
fn fill_ranks(id: u64, n: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| hash2(id, i as u64));
    let mut rank = vec![0usize; n];
    for (k, &cell) in order.iter().enumerate() {
        rank[cell] = k;
    }
    rank
}

#[component]
pub fn Bitfield(#[prop(into)] progress: Signal<f32>, id: u64) -> impl IntoView {
    let rank = StoredValue::new(fill_ranks(id, CELLS));
    let filled = Memo::new(move |_| (progress.get().clamp(0.0, 1.0) * CELLS as f32).round() as usize);

    view! {
        <div class="bf-grid">
            {(0..CELLS)
                .map(|i| {
                    let on = move || rank.with_value(|r| r[i]) < filled.get();
                    view! { <div class="bf-cell" class:on=on></div> }
                })
                .collect_view()}
        </div>
    }
}
