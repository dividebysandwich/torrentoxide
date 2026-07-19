//! Feeds & indexers page: Torznab indexers, RSS feeds (auto-download), a manual
//! release search, and recent grab history. Split into sections to keep views
//! shallow.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{
    delete_feed, delete_indexer, get_feed_poll_mins, grab_release, list_feeds, list_grab_history,
    list_indexers, list_quality_profiles, poll_feeds_now, search_releases, set_feed_poll_mins,
    test_indexer, upsert_feed, upsert_indexer,
};
use crate::components::dashboard_state;
use crate::types::{fmt_bytes, Indexer, QualityProfile, Release, RssFeed};

#[component]
pub fn FeedsPage() -> impl IntoView {
    view! {
        <div class="settings-page">
            <IndexersSection/>
            <SearchSection/>
            <FeedsSection/>
            <HistorySection/>
        </div>
    }
}

#[component]
fn IndexersSection() -> impl IntoView {
    let list = RwSignal::new(Vec::<Indexer>::new());
    let name = RwSignal::new(String::new());
    let url = RwSignal::new(String::new());
    let apikey = RwSignal::new(String::new());
    let status = RwSignal::new(String::new());

    let reload = move || {
        spawn_local(async move {
            if let Ok(v) = list_indexers().await {
                list.set(v);
            }
        });
    };
    Effect::new(move |_| reload());

    let form_indexer = move || Indexer {
        id: String::new(),
        name: name.get().trim().to_string(),
        torznab_url: url.get().trim().to_string(),
        api_key: apikey.get().trim().to_string(),
        enabled: true,
    };

    let add = move |_| {
        spawn_local(async move {
            match upsert_indexer(form_indexer()).await {
                Ok(()) => {
                    name.set(String::new());
                    url.set(String::new());
                    apikey.set(String::new());
                    status.set("Saved.".into());
                    reload();
                }
                Err(e) => status.set(e.to_string()),
            }
        });
    };
    let test = move |_| {
        status.set("Testing…".into());
        spawn_local(async move {
            match test_indexer(form_indexer()).await {
                Ok(()) => status.set("✓ Indexer reachable.".into()),
                Err(e) => status.set(format!("✗ {e}")),
            }
        });
    };
    let del = move |id: String| {
        spawn_local(async move {
            let _ = delete_indexer(id).await;
            reload();
        });
    };

    view! {
        <section class="panel settings-section">
            <h2 class="page-title">"INDEXERS (TORZNAB)"</h2>
            <p class="settings-hint">
                "Point at a Jackett/Prowlarr Torznab feed, e.g. http://127.0.0.1:9117/api/v2.0/indexers/all/results/torznab/ with the Jackett API key."
            </p>
            <div class="cat-form">
                <input class="text-input" r#type="text" placeholder="name"
                    prop:value=move || name.get() on:input=move |e| name.set(event_target_value(&e))/>
                <input class="text-input grow" r#type="text" placeholder="Torznab URL"
                    prop:value=move || url.get() on:input=move |e| url.set(event_target_value(&e))/>
                <input class="text-input" r#type="password" placeholder="API key"
                    prop:value=move || apikey.get() on:input=move |e| apikey.set(event_target_value(&e))/>
                <button class="btn btn-ghost" on:click=test>"Test"</button>
                <button class="btn btn-primary" on:click=add>"+ Add"</button>
            </div>
            <p class="add-status">{move || status.get()}</p>
            <div class="cat-list">
                <For each=move || list.get() key=|i| i.id.clone() let:i>
                    <div class="cat-row">
                        <span class="cat-name">{i.name.clone()}</span>
                        <code class="cat-subdir">{i.torznab_url.clone()}</code>
                        <button class="icon-btn danger" title="Delete indexer"
                            on:click={let id = i.id.clone(); move |_| del(id.clone())}>"🗑"</button>
                    </div>
                </For>
                {move || list.get().is_empty().then(|| view! { <p class="tree-empty">"— no indexers yet —"</p> })}
            </div>
        </section>
    }
}

#[component]
fn SearchSection() -> impl IntoView {
    let state = dashboard_state();
    let query = RwSignal::new(String::new());
    let results = RwSignal::new(Vec::<Release>::new());
    let grab_cat = RwSignal::new(String::new());
    let status = RwSignal::new(String::new());

    let run = move |_| {
        let q = query.get().trim().to_string();
        if q.is_empty() {
            return;
        }
        status.set("Searching…".into());
        results.set(Vec::new());
        spawn_local(async move {
            match search_releases(q).await {
                Ok(r) => {
                    status.set(format!("{} result(s).", r.len()));
                    results.set(r);
                }
                Err(e) => status.set(e.to_string()),
            }
        });
    };
    let grab = move |rel: Release| {
        let cat = grab_cat.get();
        spawn_local(async move {
            let _ = grab_release(rel.url, rel.title, cat).await;
        });
    };

    view! {
        <section class="panel settings-section">
            <h2 class="page-title">"SEARCH"</h2>
            <div class="cat-form">
                <input class="text-input grow" r#type="text" placeholder="search all indexers…"
                    prop:value=move || query.get() on:input=move |e| query.set(event_target_value(&e))/>
                <select class="sort-select" prop:value=move || grab_cat.get()
                    on:change=move |e| grab_cat.set(event_target_value(&e))>
                    <option value="">"grab to default dir"</option>
                    {move || state.categories.get().iter()
                        .map(|c| view! { <option value=c.slug.clone()>{format!("→ {}", c.name)}</option> })
                        .collect_view()}
                </select>
                <button class="btn btn-primary" on:click=run>"Search"</button>
            </div>
            <p class="add-status">{move || status.get()}</p>
            <div class="cat-list">
                <For each=move || results.get() key=|r| r.url.clone() let:r>
                    <div class="cat-row">
                        <span class="rel-title">{r.title.clone()}</span>
                        <span class="rel-meta">
                            {format!("{} · {} seed · {}",
                                fmt_bytes(r.size as f64),
                                r.seeders.map(|s| s.to_string()).unwrap_or_else(|| "?".into()),
                                r.indexer)}
                        </span>
                        <button class="btn btn-ghost btn-sm"
                            on:click={let rel = r.clone(); move |_| grab(rel.clone())}>"Grab"</button>
                    </div>
                </For>
            </div>
        </section>
    }
}

#[component]
fn FeedsSection() -> impl IntoView {
    let state = dashboard_state();
    let list = RwSignal::new(Vec::<RssFeed>::new());
    let profiles = RwSignal::new(Vec::<QualityProfile>::new());
    let name = RwSignal::new(String::new());
    let url = RwSignal::new(String::new());
    let category = RwSignal::new(String::new());
    let profile = RwSignal::new(String::new());
    let auto = RwSignal::new(false);
    let status = RwSignal::new(String::new());
    // Some(original id) while editing an existing feed.
    let edit_id = RwSignal::new(None::<String>);
    let poll_mins = RwSignal::new(15u32);

    let reload = move || {
        spawn_local(async move {
            if let Ok(v) = list_feeds().await {
                list.set(v);
            }
            if let Ok(p) = list_quality_profiles().await {
                profiles.set(p);
            }
            if let Ok(m) = get_feed_poll_mins().await {
                poll_mins.set(m);
            }
        });
    };
    Effect::new(move |_| reload());

    let reset_form = move || {
        name.set(String::new());
        url.set(String::new());
        category.set(String::new());
        profile.set(String::new());
        auto.set(false);
        edit_id.set(None);
    };

    let save = move |_| {
        let feed = RssFeed {
            id: String::new(),
            name: name.get().trim().to_string(),
            url: url.get().trim().to_string(),
            category: category.get(),
            quality_profile: profile.get(),
            auto_download: auto.get(),
            enabled: true,
        };
        let old = edit_id.get();
        spawn_local(async move {
            // Renaming while editing changes the slug, so drop the old record.
            if let Some(old_id) = old {
                let _ = delete_feed(old_id).await;
            }
            match upsert_feed(feed).await {
                Ok(()) => {
                    reset_form();
                    status.set("Saved.".into());
                    reload();
                }
                Err(e) => status.set(e.to_string()),
            }
        });
    };
    let edit = move |f: RssFeed| {
        name.set(f.name);
        url.set(f.url);
        category.set(f.category);
        profile.set(f.quality_profile);
        auto.set(f.auto_download);
        edit_id.set(Some(f.id));
        status.set(String::new());
    };
    let del = move |id: String| {
        spawn_local(async move {
            let _ = delete_feed(id).await;
            reload();
        });
    };
    let poll = move |_| {
        status.set("Polling…".into());
        spawn_local(async move {
            match poll_feeds_now().await {
                Ok(n) => status.set(format!("Polled feeds — {n} new grab(s).")),
                Err(e) => status.set(e.to_string()),
            }
        });
    };
    let save_interval = move |e| {
        let m = event_target_value(&e).trim().parse::<u32>().unwrap_or(15).max(1);
        poll_mins.set(m);
        spawn_local(async move {
            let _ = set_feed_poll_mins(m).await;
        });
    };

    view! {
        <section class="panel settings-section">
            <h2 class="page-title">"RSS FEEDS"</h2>
            <p class="settings-hint">
                "Feeds with auto-download grab acceptable new items into their category. Items are matched against the chosen quality profile; leave it blank to accept everything."
            </p>
            <div class="cat-form">
                <input class="text-input" r#type="text" placeholder="name"
                    prop:value=move || name.get() on:input=move |e| name.set(event_target_value(&e))/>
                <input class="text-input grow" r#type="text" placeholder="feed URL"
                    prop:value=move || url.get() on:input=move |e| url.set(event_target_value(&e))/>
                <select class="sort-select" prop:value=move || category.get()
                    on:change=move |e| category.set(event_target_value(&e))>
                    <option value="">"no category"</option>
                    {move || state.categories.get().iter()
                        .map(|c| view! { <option value=c.slug.clone()>{c.name.clone()}</option> })
                        .collect_view()}
                </select>
                <select class="sort-select" prop:value=move || profile.get()
                    on:change=move |e| profile.set(event_target_value(&e))>
                    <option value="">"accept all"</option>
                    {move || profiles.get().iter()
                        .map(|p| view! { <option value=p.id.clone()>{p.name.clone()}</option> })
                        .collect_view()}
                </select>
                <label class="pause-check">
                    <input r#type="checkbox" prop:checked=move || auto.get()
                        on:change=move |e| auto.set(event_target_checked(&e))/>
                    <span>"auto"</span>
                </label>
                <button class="btn btn-primary" on:click=save>
                    {move || if edit_id.get().is_some() { "Save" } else { "+ Add" }}
                </button>
                {move || edit_id.get().is_some().then(|| view! {
                    <button class="btn btn-ghost" on:click=move |_| reset_form()>"Cancel"</button>
                })}
            </div>
            <div class="feed-toolbar">
                <button class="btn btn-ghost btn-sm" on:click=poll>"Poll now"</button>
                <label class="deck-field">
                    <span class="deck-label">"POLL EVERY"</span>
                    <input
                        class="deck-input narrow"
                        r#type="number"
                        min="1"
                        prop:value=move || poll_mins.get().to_string()
                        on:change=save_interval
                    />
                    <span class="deck-unit">"MIN"</span>
                </label>
            </div>
            <p class="add-status">{move || status.get()}</p>
            <div class="cat-list">
                <For each=move || list.get() key=|f| f.id.clone() let:f>
                    <div class="cat-row">
                        <span class=if f.auto_download { "cat-kind k-tv" } else { "cat-kind k-other" }>
                            {if f.auto_download { "auto" } else { "manual" }}
                        </span>
                        <span class="cat-name">{f.name.clone()}</span>
                        <code class="cat-subdir">{f.url.clone()}</code>
                        <button class="icon-btn" title="Edit feed"
                            on:click={let ff = f.clone(); move |_| edit(ff.clone())}>"✎"</button>
                        <button class="icon-btn danger" title="Delete feed"
                            on:click={let id = f.id.clone(); move |_| del(id.clone())}>"🗑"</button>
                    </div>
                </For>
                {move || list.get().is_empty().then(|| view! { <p class="tree-empty">"— no feeds yet —"</p> })}
            </div>
        </section>
    }
}

#[component]
fn HistorySection() -> impl IntoView {
    let list = RwSignal::new(Vec::<crate::types::GrabHistoryEntry>::new());
    let reload = move || {
        spawn_local(async move {
            if let Ok(v) = list_grab_history().await {
                list.set(v);
            }
        });
    };
    Effect::new(move |_| reload());

    view! {
        <section class="panel settings-section">
            <div class="files-head">
                <span class="detail-card-title">"RECENT GRABS"</span>
                <button class="btn btn-ghost btn-sm" on:click=move |_| reload()>"Refresh"</button>
            </div>
            <div class="cat-list">
                <For each=move || list.get() key=|h| h.id.clone() let:h>
                    <div class="cat-row">
                        <span class="rel-title">{h.title.clone()}</span>
                        <span class="rel-meta">{format!("via {}", h.source)}</span>
                    </div>
                </For>
                {move || list.get().is_empty().then(|| view! { <p class="tree-empty">"— nothing grabbed yet —"</p> })}
            </div>
        </section>
    }
}
