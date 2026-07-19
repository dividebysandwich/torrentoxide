//! Library page: scanned movies and TV shows on separate tabs.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{get_import_mode, get_library, import_now, rescan_library, set_import_mode};
use crate::types::{fmt_bytes, Library};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Movies,
    Shows,
}

#[component]
pub fn LibraryPage() -> impl IntoView {
    let library = RwSignal::new(Library::default());
    let status = RwSignal::new(String::new());
    let tab = RwSignal::new(Tab::Movies);
    let mode = RwSignal::new("move".to_string());

    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(l) = get_library().await {
                library.set(l);
            }
            if let Ok(m) = get_import_mode().await {
                mode.set(m);
            }
        });
    });

    let on_mode = move |e| {
        let m = event_target_value(&e);
        mode.set(m.clone());
        spawn_local(async move {
            let _ = set_import_mode(m).await;
        });
    };

    let rescan = move |_| {
        status.set("Scanning…".into());
        spawn_local(async move {
            match rescan_library().await {
                Ok(l) => {
                    status.set(format!("{} video file(s) scanned.", l.file_count));
                    library.set(l);
                }
                Err(e) => status.set(e.to_string()),
            }
        });
    };
    let import = move |_| {
        status.set("Importing finished downloads…".into());
        spawn_local(async move {
            match import_now().await {
                Ok(l) => {
                    status.set("Imported finished TV downloads into Show/Season folders.".into());
                    library.set(l);
                }
                Err(e) => status.set(e.to_string()),
            }
        });
    };

    let movies = move || library.get().movies;
    let shows = move || library.get().shows;
    let movie_count = move || library.get().movies.len();
    let show_count = move || library.get().shows.len();

    view! {
        <div class="settings-page">
            <section class="panel settings-section">
                <div class="files-head">
                    <h2 class="page-title">"LIBRARY"</h2>
                    <select class="sort-select" prop:value=move || mode.get() on:change=on_mode>
                        <option value="move">"MOVE"</option>
                        <option value="hardlink">"HARDLINK"</option>
                        <option value="copy">"COPY"</option>
                    </select>
                    <button class="btn btn-ghost btn-sm" on:click=import>"Import"</button>
                    <button class="btn btn-primary btn-sm" on:click=rescan>"Rescan"</button>
                </div>
                <p class="settings-hint">
                    "Movies and TV episodes found in the download folder. Category kind (Movie/TV) and folder layout drive classification; rescans run hourly or on demand. "
                    <b>"Import"</b>" organizes finished TV downloads into "<code>"Show/Season NN"</code>" folders — "
                    <b>"Move"</b>" (default) relocates the file for one clean copy and stops seeding that torrent; "
                    <b>"Hardlink"</b>" links it and keeps seeding (best when your download folder is separate from what your media server scans)."
                </p>
                <p class="add-status">{move || status.get()}</p>

                <div class="filter-chips lib-tabs">
                    <button
                        class="filter-chip"
                        class:active=move || tab.get() == Tab::Movies
                        on:click=move |_| tab.set(Tab::Movies)
                    >
                        {move || format!("MOVIES ({})", movie_count())}
                    </button>
                    <button
                        class="filter-chip"
                        class:active=move || tab.get() == Tab::Shows
                        on:click=move |_| tab.set(Tab::Shows)
                    >
                        {move || format!("TV SHOWS ({})", show_count())}
                    </button>
                </div>

                <Show when=move || tab.get() == Tab::Movies fallback=|| ()>
                    <div class="cat-list">
                        <For each=movies key=|m| m.path.clone() let:m>
                            <div class="cat-row">
                                <span class="cat-name">{m.title.clone()}</span>
                                <span class="rel-meta">
                                    {format!(
                                        "{} · {} · {}",
                                        m.year.map(|y| y.to_string()).unwrap_or_else(|| "—".into()),
                                        m.resolution,
                                        fmt_bytes(m.size as f64),
                                    )}
                                </span>
                            </div>
                        </For>
                        {move || movies().is_empty().then(|| view! { <p class="tree-empty">"— no movies —"</p> })}
                    </div>
                </Show>

                <Show when=move || tab.get() == Tab::Shows fallback=|| ()>
                    <div class="cat-list">
                        <For each=shows key=|s| s.title.clone() let:s>
                            <div class="cat-row">
                                <span class="cat-name">{s.title.clone()}</span>
                                <span class="rel-meta">{format!("{} episode(s)", s.episodes.len())}</span>
                            </div>
                        </For>
                        {move || shows().is_empty().then(|| view! { <p class="tree-empty">"— no shows —"</p> })}
                    </div>
                </Show>
            </section>
        </div>
    }
}
