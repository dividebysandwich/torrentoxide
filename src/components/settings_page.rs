//! Settings page. Currently: category management (name → sub-folder under the
//! download directory). Quality profiles and providers arrive in later phases.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{delete_category, list_categories, upsert_category};
use crate::components::dashboard_state;
use crate::types::{Category, MediaKind};

const KINDS: [MediaKind; 3] = [MediaKind::Movie, MediaKind::Tv, MediaKind::Other];

fn kind_class(k: MediaKind) -> &'static str {
    match k {
        MediaKind::Movie => "k-movie",
        MediaKind::Tv => "k-tv",
        MediaKind::Other => "k-other",
    }
}

#[component]
pub fn SettingsPage() -> impl IntoView {
    let state = dashboard_state();
    let name = RwSignal::new(String::new());
    let subdir = RwSignal::new(String::new());
    let kind = RwSignal::new(MediaKind::Other);
    let error = RwSignal::new(String::new());

    let reload = move || {
        spawn_local(async move {
            if let Ok(cats) = list_categories().await {
                state.categories.set(cats);
            }
        });
    };

    let add = move |_| {
        let n = name.get().trim().to_string();
        if n.is_empty() {
            error.set("Category name is required.".into());
            return;
        }
        let cat = Category {
            slug: String::new(),
            name: n,
            subdir: subdir.get().trim().to_string(),
            kind: kind.get(),
        };
        error.set(String::new());
        spawn_local(async move {
            match upsert_category(cat).await {
                Ok(()) => {
                    name.set(String::new());
                    subdir.set(String::new());
                    kind.set(MediaKind::Other);
                    reload();
                }
                Err(e) => error.set(e.to_string()),
            }
        });
    };

    let del = move |slug: String| {
        spawn_local(async move {
            let _ = delete_category(slug).await;
            reload();
        });
    };

    let kind_index = move || KINDS.iter().position(|k| *k == kind.get()).unwrap_or(2);

    view! {
        <div class="settings-page">
            <section class="panel settings-section">
                <h2 class="page-title">"CATEGORIES"</h2>
                <p class="settings-hint">
                    "Map a category to a sub-folder under the download directory. New downloads can target a category, and the torrent list can be filtered by it."
                </p>

                <div class="cat-form">
                    <input
                        class="text-input grow"
                        r#type="text"
                        placeholder="name (e.g. Movies)"
                        prop:value=move || name.get()
                        on:input=move |e| name.set(event_target_value(&e))
                    />
                    <input
                        class="text-input grow"
                        r#type="text"
                        placeholder="sub-folder (defaults to name)"
                        prop:value=move || subdir.get()
                        on:input=move |e| subdir.set(event_target_value(&e))
                    />
                    <select
                        class="sort-select"
                        prop:value=move || kind_index().to_string()
                        on:change=move |e| {
                            let i = event_target_value(&e).parse::<usize>().unwrap_or(2);
                            kind.set(KINDS[i.min(KINDS.len() - 1)]);
                        }
                    >
                        <option value="0">"MOVIE"</option>
                        <option value="1">"TV"</option>
                        <option value="2">"OTHER"</option>
                    </select>
                    <button class="btn btn-primary" on:click=add>"+ Add"</button>
                </div>
                <p class="dir-error">{move || error.get()}</p>

                <div class="cat-list">
                    <For each=move || state.categories.get() key=|c| c.slug.clone() let:c>
                        <div class="cat-row">
                            <span class=format!("cat-kind {}", kind_class(c.kind))>{c.kind.label()}</span>
                            <span class="cat-name">{c.name.clone()}</span>
                            <code class="cat-subdir">{c.subdir.clone()}</code>
                            <button
                                class="icon-btn danger"
                                title="Delete category"
                                on:click={
                                    let slug = c.slug.clone();
                                    move |_| del(slug.clone())
                                }
                            >
                                "🗑"
                            </button>
                        </div>
                    </For>
                    {move || {
                        state
                            .categories
                            .get()
                            .is_empty()
                            .then(|| view! { <p class="tree-empty">"— no categories yet —"</p> })
                    }}
                </div>
            </section>
        </div>
    }
}
