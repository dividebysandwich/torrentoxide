//! Wanted page: search TMDb, add monitored movies/series (with a quality profile
//! + category), and trigger the monitor on demand.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{
    add_wanted, list_indexers, list_quality_profiles, list_wanted, remove_wanted, run_monitor_now,
    tmdb_search,
};
use crate::components::dashboard_state;
use crate::types::{MediaSearchResult, QualityProfile, WantedItem, WantedKind};

/// A small movie/show poster thumbnail (TMDb), with a kind-based placeholder
/// when no poster is available.
fn poster_thumb(poster: Option<String>, is_tv: bool) -> impl IntoView {
    match poster.filter(|p| !p.trim().is_empty()) {
        Some(p) => view! {
            <img
                class="wanted-thumb"
                loading="lazy"
                src=format!("https://image.tmdb.org/t/p/w92{p}")
                alt=""
            />
        }
        .into_any(),
        None => view! {
            <span class="wanted-thumb wanted-thumb-none">
                {if is_tv { "📺" } else { "🎬" }}
            </span>
        }
        .into_any(),
    }
}

#[component]
pub fn WantedPage() -> impl IntoView {
    let state = dashboard_state();
    let wanted = RwSignal::new(Vec::<WantedItem>::new());
    let profiles = RwSignal::new(Vec::<QualityProfile>::new());
    let query = RwSignal::new(String::new());
    let results = RwSignal::new(Vec::<MediaSearchResult>::new());
    let profile = RwSignal::new(String::new());
    let category = RwSignal::new(String::new());
    let status = RwSignal::new(String::new());
    // Assume configured until we learn otherwise (avoids a warning flash).
    let has_indexers = RwSignal::new(true);

    let reload = move || {
        spawn_local(async move {
            if let Ok(w) = list_wanted().await {
                wanted.set(w);
            }
            if let Ok(p) = list_quality_profiles().await {
                profiles.set(p);
            }
            if let Ok(ix) = list_indexers().await {
                has_indexers.set(ix.iter().any(|i| i.enabled));
            }
        });
    };
    Effect::new(move |_| reload());

    let search = move |_| {
        let q = query.get().trim().to_string();
        if q.is_empty() {
            return;
        }
        status.set("Searching TMDb…".into());
        results.set(Vec::new());
        spawn_local(async move {
            match tmdb_search(q).await {
                Ok(r) => {
                    status.set(format!("{} result(s).", r.len()));
                    results.set(r);
                }
                Err(e) => status.set(e.to_string()),
            }
        });
    };

    let add = move |res: MediaSearchResult| {
        let item = WantedItem {
            id: String::new(),
            kind: if res.is_tv {
                WantedKind::Series
            } else {
                WantedKind::Movie
            },
            tmdb_id: res.tmdb_id,
            title: res.title,
            year: res.year,
            poster_path: res.poster_path,
            quality_profile: profile.get(),
            category: category.get(),
            monitored: true,
        };
        spawn_local(async move {
            match add_wanted(item).await {
                Ok(()) => reload(),
                Err(e) => status.set(e.to_string()),
            }
        });
    };
    let del = move |id: String| {
        spawn_local(async move {
            let _ = remove_wanted(id).await;
            reload();
        });
    };
    let search_now = move |_| {
        status.set("Searching indexers for wanted items…".into());
        spawn_local(async move {
            match run_monitor_now().await {
                Ok(n) => status.set(format!("Monitor run — {n} new grab(s).")),
                Err(e) => status.set(e.to_string()),
            }
        });
    };

    view! {
        <div class="settings-page">
            {move || (!has_indexers.get()).then(|| view! {
                <div class="warn-banner">
                    <strong>"No indexers configured — auto-download is off."</strong>
                    " Your Wanted list still powers the "<b>"Calendar"</b>" (upcoming episode air dates), but the monitor can't fetch anything until you add a Torznab indexer on the "<b>"Feeds"</b>" page."
                </div>
            })}
            <section class="panel settings-section">
                <h2 class="page-title">"ADD WANTED"</h2>
                <p class="settings-hint">
                    "Track movies and TV shows you want. Monitored series populate the Calendar with episode air dates (TMDb), and — when a Torznab indexer is configured — the monitor auto-downloads missing episodes and quality upgrades a few times a day. Pick a quality profile + category so grabs land in the right place."
                </p>
                <div class="cat-form">
                    <input class="text-input grow" r#type="text" placeholder="search movies & TV…"
                        prop:value=move || query.get() on:input=move |e| query.set(event_target_value(&e))/>
                    <select class="sort-select" prop:value=move || profile.get()
                        on:change=move |e| profile.set(event_target_value(&e))>
                        <option value="">"accept all"</option>
                        {move || profiles.get().iter()
                            .map(|p| view! { <option value=p.id.clone()>{p.name.clone()}</option> })
                            .collect_view()}
                    </select>
                    <select class="sort-select" prop:value=move || category.get()
                        on:change=move |e| category.set(event_target_value(&e))>
                        <option value="">"no category"</option>
                        {move || state.categories.get().iter()
                            .map(|c| view! { <option value=c.slug.clone()>{c.name.clone()}</option> })
                            .collect_view()}
                    </select>
                    <button class="btn btn-primary" on:click=search>"Search"</button>
                </div>
                <p class="add-status">{move || status.get()}</p>
                <div class="cat-list">
                    <For each=move || results.get() key=|r| r.tmdb_id let:r>
                        <div class="cat-row">
                            {poster_thumb(r.poster_path.clone(), r.is_tv)}
                            <span class=if r.is_tv { "cat-kind k-tv" } else { "cat-kind k-movie" }>
                                {if r.is_tv { "series" } else { "movie" }}
                            </span>
                            <span class="cat-name">{r.title.clone()}</span>
                            <span class="rel-meta">{r.year.map(|y| y.to_string()).unwrap_or_default()}</span>
                            <button class="btn btn-ghost btn-sm"
                                on:click={let res = r.clone(); move |_| add(res.clone())}>"+ Want"</button>
                        </div>
                    </For>
                </div>
            </section>

            <section class="panel settings-section">
                <div class="files-head">
                    <span class="detail-card-title">"MONITORED"</span>
                    <button class="btn btn-primary btn-sm" on:click=search_now>"Search now"</button>
                </div>
                <div class="cat-list">
                    <For each=move || wanted.get() key=|w| w.id.clone() let:w>
                        <div class="cat-row">
                            {poster_thumb(w.poster_path.clone(), matches!(w.kind, WantedKind::Series))}
                            <span class=if matches!(w.kind, WantedKind::Series) { "cat-kind k-tv" } else { "cat-kind k-movie" }>
                                {w.kind.label()}
                            </span>
                            <span class="cat-name">{w.title.clone()}</span>
                            <span class="rel-meta">{w.year.map(|y| y.to_string()).unwrap_or_default()}</span>
                            <button class="icon-btn danger" title="Remove"
                                on:click={let id = w.id.clone(); move |_| del(id.clone())}>"🗑"</button>
                        </div>
                    </For>
                    {move || wanted.get().is_empty().then(|| view! { <p class="tree-empty">"— nothing monitored yet —"</p> })}
                </div>
            </section>
        </div>
    }
}
