//! Library page: movies and shows discovered by scanning the download tree.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{get_library, rescan_library};
use crate::types::{fmt_bytes, Library};

#[component]
pub fn LibraryPage() -> impl IntoView {
    let library = RwSignal::new(Library::default());
    let status = RwSignal::new(String::new());

    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(l) = get_library().await {
                library.set(l);
            }
        });
    });

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

    let movies = move || library.get().movies;
    let shows = move || library.get().shows;

    view! {
        <div class="settings-page">
            <section class="panel settings-section">
                <div class="files-head">
                    <h2 class="page-title">"LIBRARY"</h2>
                    <button class="btn btn-primary btn-sm" on:click=rescan>"Rescan"</button>
                </div>
                <p class="settings-hint">
                    "Movies and TV episodes found in the download folder (identified by filename). Rescans run hourly, or on demand."
                </p>
                <p class="add-status">{move || status.get()}</p>

                <span class="detail-card-title">"MOVIES"</span>
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

                <span class="detail-card-title lib-subhead">"TV SHOWS"</span>
                <div class="cat-list">
                    <For each=shows key=|s| s.title.clone() let:s>
                        <div class="cat-row">
                            <span class="cat-name">{s.title.clone()}</span>
                            <span class="rel-meta">{format!("{} episode(s)", s.episodes.len())}</span>
                        </div>
                    </For>
                    {move || shows().is_empty().then(|| view! { <p class="tree-empty">"— no shows —"</p> })}
                </div>
            </section>
        </div>
    }
}
