//! Settings page: categories, TMDb provider config, and quality profiles.
//! Split into sections so each `view!` stays shallow.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{
    delete_category, delete_quality_profile, get_provider_info, get_settings, list_categories,
    list_quality_profiles, set_settings, set_tmdb_key, test_tmdb, upsert_category,
    upsert_quality_profile,
};
use crate::components::dashboard_state;
use crate::types::{
    Category, HdrPref, MediaKind, ProviderInfo, QualityProfile, Resolution, Settings,
};

const KINDS: [MediaKind; 3] = [MediaKind::Movie, MediaKind::Tv, MediaKind::Other];

fn kind_class(k: MediaKind) -> &'static str {
    match k {
        MediaKind::Movie => "k-movie",
        MediaKind::Tv => "k-tv",
        MediaKind::Other => "k-other",
    }
}

fn res_index(r: Resolution) -> usize {
    Resolution::ALL.iter().position(|x| *x == r).unwrap_or(0)
}

fn hdr_index(h: HdrPref) -> usize {
    HdrPref::ALL.iter().position(|x| *x == h).unwrap_or(1)
}

#[component]
pub fn SettingsPage() -> impl IntoView {
    view! {
        <div class="settings-page">
            <QueueSection/>
            <ProvidersSection/>
            <CategoriesSection/>
            <QualityProfilesSection/>
        </div>
    }
}

#[component]
fn QueueSection() -> impl IntoView {
    // Load the full settings so a save preserves rate limits / seeding goals.
    let settings = RwSignal::new(Settings::default());
    let loaded = RwSignal::new(false);
    let status = RwSignal::new(String::new());

    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(s) = get_settings().await {
                settings.set(s);
                loaded.set(true);
            }
        });
    });

    // Empty / invalid parses to 0 (unlimited — no queue).
    let on_max = move |e| {
        let v = event_target_value(&e).trim().parse::<u32>().unwrap_or(0);
        settings.update(|s| s.max_active_downloads = v);
        let s = settings.get();
        spawn_local(async move {
            match set_settings(s).await {
                Ok(()) => status.set("Saved.".into()),
                Err(e) => status.set(e.to_string()),
            }
        });
    };
    let max_val = move || match settings.get().max_active_downloads {
        0 => String::new(),
        v => v.to_string(),
    };

    view! {
        <section class="panel settings-section">
            <h2 class="page-title">"DOWNLOAD QUEUE"</h2>
            <p class="settings-hint">
                "Limit how many torrents download at once. Extras wait in a "
                <b>"QUEUED"</b>
                " state and start automatically as slots free up. Leave blank (0) for unlimited."
            </p>
            <div class="cat-form">
                <label class="qp-field">
                    <span>"MAX ACTIVE"</span>
                    <input
                        class="text-input"
                        r#type="number"
                        min="0"
                        placeholder="∞"
                        prop:value=max_val
                        prop:disabled=move || !loaded.get()
                        on:change=on_max
                    />
                </label>
                <span class="settings-hint">
                    {move || if loaded.get() { "0 = unlimited" } else { "loading…" }}
                </span>
            </div>
            <p class="add-status">{move || status.get()}</p>
        </section>
    }
}

#[component]
fn CategoriesSection() -> impl IntoView {
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

    let kind_idx = move || KINDS.iter().position(|k| *k == kind.get()).unwrap_or(2);

    view! {
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
                    prop:value=move || kind_idx().to_string()
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
    }
}

#[component]
fn ProvidersSection() -> impl IntoView {
    let key = RwSignal::new(String::new());
    let info = RwSignal::new(ProviderInfo::default());
    let status = RwSignal::new(String::new());

    let reload = move || {
        spawn_local(async move {
            if let Ok(i) = get_provider_info().await {
                info.set(i);
            }
        });
    };
    Effect::new(move |_| reload());

    let save = move |_| {
        let k = key.get();
        spawn_local(async move {
            match set_tmdb_key(k).await {
                Ok(()) => {
                    key.set(String::new());
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
            match test_tmdb().await {
                Ok(()) => status.set("✓ TMDb key works.".into()),
                Err(e) => status.set(format!("✗ {e}")),
            }
        });
    };
    let placeholder = move || {
        if info.get().tmdb_key_set {
            "•••••••• (set — enter to replace)".to_string()
        } else {
            "not set".to_string()
        }
    };

    view! {
        <section class="panel settings-section">
            <h2 class="page-title">"PROVIDERS"</h2>
            <p class="settings-hint">
                "TMDb powers library identification and the wanted / episode monitor. A key set here overrides the .env value."
            </p>
            <div class="cat-form">
                <span class="prov-label">"TMDb API key"</span>
                <input
                    class="text-input grow"
                    r#type="password"
                    placeholder=placeholder
                    prop:value=move || key.get()
                    on:input=move |e| key.set(event_target_value(&e))
                />
                <button class="btn btn-primary" on:click=save>"Save"</button>
                <button class="btn btn-ghost" on:click=test>"Test"</button>
            </div>
            <p class="add-status">{move || status.get()}</p>
        </section>
    }
}

#[component]
fn QualityProfilesSection() -> impl IntoView {
    let profiles = RwSignal::new(Vec::<QualityProfile>::new());
    let name = RwSignal::new(String::new());
    let min_res = RwSignal::new(Resolution::R720);
    let cutoff_res = RwSignal::new(Resolution::R1080);
    let hdr = RwSignal::new(HdrPref::Prefer);
    let langs = RwSignal::new(String::new());
    let upgrade = RwSignal::new(true);
    let error = RwSignal::new(String::new());

    let reload = move || {
        spawn_local(async move {
            if let Ok(ps) = list_quality_profiles().await {
                profiles.set(ps);
            }
        });
    };
    Effect::new(move |_| reload());

    let add = move |_| {
        let n = name.get().trim().to_string();
        if n.is_empty() {
            error.set("Profile name is required.".into());
            return;
        }
        let prof = QualityProfile {
            id: String::new(),
            name: n,
            min_resolution: min_res.get(),
            cutoff_resolution: cutoff_res.get(),
            hdr: hdr.get(),
            languages: langs
                .get()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            preferred_groups: Vec::new(),
            blocked_groups: Vec::new(),
            upgrade_allowed: upgrade.get(),
        };
        error.set(String::new());
        spawn_local(async move {
            match upsert_quality_profile(prof).await {
                Ok(()) => {
                    name.set(String::new());
                    langs.set(String::new());
                    reload();
                }
                Err(e) => error.set(e.to_string()),
            }
        });
    };
    let del = move |id: String| {
        spawn_local(async move {
            let _ = delete_quality_profile(id).await;
            reload();
        });
    };

    view! {
        <section class="panel settings-section">
            <h2 class="page-title">"QUALITY PROFILES"</h2>
            <p class="settings-hint">
                "Define what releases are acceptable and how upgrades are chosen. Resolution is primary; HDR is a strong secondary preference (accept SDR now, upgrade to HDR when it appears)."
            </p>
            <div class="qp-form">
                <input
                    class="text-input grow"
                    r#type="text"
                    placeholder="name (e.g. HD-HDR)"
                    prop:value=move || name.get()
                    on:input=move |e| name.set(event_target_value(&e))
                />
                <label class="qp-field">
                    <span>"MIN"</span>
                    <select
                        class="sort-select"
                        prop:value=move || res_index(min_res.get()).to_string()
                        on:change=move |e| {
                            let i = event_target_value(&e).parse::<usize>().unwrap_or(0);
                            min_res.set(Resolution::ALL[i.min(Resolution::ALL.len() - 1)]);
                        }
                    >
                        {Resolution::ALL
                            .iter()
                            .enumerate()
                            .map(|(i, &r)| view! { <option value=i.to_string()>{r.label()}</option> })
                            .collect_view()}
                    </select>
                </label>
                <label class="qp-field">
                    <span>"CUTOFF"</span>
                    <select
                        class="sort-select"
                        prop:value=move || res_index(cutoff_res.get()).to_string()
                        on:change=move |e| {
                            let i = event_target_value(&e).parse::<usize>().unwrap_or(0);
                            cutoff_res.set(Resolution::ALL[i.min(Resolution::ALL.len() - 1)]);
                        }
                    >
                        {Resolution::ALL
                            .iter()
                            .enumerate()
                            .map(|(i, &r)| view! { <option value=i.to_string()>{r.label()}</option> })
                            .collect_view()}
                    </select>
                </label>
                <label class="qp-field">
                    <span>"HDR"</span>
                    <select
                        class="sort-select"
                        prop:value=move || hdr_index(hdr.get()).to_string()
                        on:change=move |e| {
                            let i = event_target_value(&e).parse::<usize>().unwrap_or(1);
                            hdr.set(HdrPref::ALL[i.min(HdrPref::ALL.len() - 1)]);
                        }
                    >
                        {HdrPref::ALL
                            .iter()
                            .enumerate()
                            .map(|(i, &h)| view! { <option value=i.to_string()>{h.label()}</option> })
                            .collect_view()}
                    </select>
                </label>
                <input
                    class="text-input"
                    r#type="text"
                    placeholder="languages (csv, e.g. english)"
                    prop:value=move || langs.get()
                    on:input=move |e| langs.set(event_target_value(&e))
                />
                <label class="pause-check">
                    <input
                        r#type="checkbox"
                        prop:checked=move || upgrade.get()
                        on:change=move |e| upgrade.set(event_target_checked(&e))
                    />
                    <span>"upgrade"</span>
                </label>
                <button class="btn btn-primary" on:click=add>"+ Add"</button>
            </div>
            <p class="dir-error">{move || error.get()}</p>
            <div class="cat-list">
                <For each=move || profiles.get() key=|p| p.id.clone() let:p>
                    <div class="cat-row">
                        <span class="cat-name">{p.name.clone()}</span>
                        <span class="qp-summary">
                            {format!(
                                "{} → {} · HDR {}{}",
                                p.min_resolution.label(),
                                p.cutoff_resolution.label(),
                                p.hdr.label(),
                                if p.upgrade_allowed { " · upgrades" } else { "" },
                            )}
                        </span>
                        <button
                            class="icon-btn danger"
                            title="Delete profile"
                            on:click={
                                let id = p.id.clone();
                                move |_| del(id.clone())
                            }
                        >
                            "🗑"
                        </button>
                    </div>
                </For>
                {move || {
                    profiles
                        .get()
                        .is_empty()
                        .then(|| view! { <p class="tree-empty">"— no quality profiles yet —"</p> })
                }}
            </div>
        </section>
    }
}
