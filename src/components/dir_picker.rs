use leptos::portal::Portal;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{browse_dir, make_dir};
use crate::components::dashboard_state;
use crate::types::DirListing;

/// Remote directory browser modal, confined server-side to `BROWSE_ROOT`.
#[component]
pub fn DirPicker(
    open: RwSignal<bool>,
    #[prop(into)] on_select: Callback<String>,
) -> impl IntoView {
    let state = dashboard_state();
    let listing = RwSignal::new(None::<DirListing>);
    let new_name = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());

    let load = move |path: Option<String>| {
        spawn_local(async move {
            match browse_dir(path).await {
                Ok(l) => {
                    listing.set(Some(l));
                    error.set(String::new());
                }
                Err(e) => error.set(e.to_string()),
            }
        });
    };

    // Load the starting directory whenever the modal opens.
    Effect::new(move |_| {
        if open.get() && listing.get_untracked().is_none() {
            let start = state.defaults.get_untracked().download_dir;
            load((!start.is_empty()).then_some(start));
        }
    });

    let create_folder = move |_| {
        let name = new_name.get();
        if name.trim().is_empty() {
            return;
        }
        let base = match listing.get() {
            Some(l) => l.path,
            None => return,
        };
        spawn_local(async move {
            match make_dir(base, name).await {
                Ok(l) => {
                    listing.set(Some(l));
                    new_name.set(String::new());
                    error.set(String::new());
                }
                Err(e) => error.set(e.to_string()),
            }
        });
    };

    let select_current = move |_| {
        if let Some(l) = listing.get() {
            if l.writable {
                on_select.run(l.path);
                open.set(false);
                listing.set(None);
            }
        }
    };

    let close = move |_| {
        open.set(false);
        listing.set(None);
    };

    view! {
        // Teleport the modal to <body> so it escapes the .add-panel backdrop-filter
        // stacking context (otherwise later sibling torrent rows paint over it).
        <Portal>
        {move || open.get().then(|| {
            view! {
                <div class="modal-overlay" on:click=close>
                    <div class="modal dir-picker" on:click=|e| e.stop_propagation()>
                        <h3 class="modal-title">"Choose download folder"</h3>
                        <div class="dir-current">
                            <span class="dir-current-label">"PATH"</span>
                            <code>{move || listing.get().map(|l| l.path).unwrap_or_else(|| "…".into())}</code>
                        </div>

                        <div class="dir-toolbar">
                            <button
                                class="btn btn-ghost btn-sm"
                                prop:disabled=move || listing.get().and_then(|l| l.parent).is_none()
                                on:click=move |_| {
                                    if let Some(parent) = listing.get().and_then(|l| l.parent) {
                                        load(Some(parent));
                                    }
                                }
                            >
                                "⬑ Up"
                            </button>
                            <span class="dir-hint">
                                {move || {
                                    let ro = listing.get().map(|l| !l.writable).unwrap_or(false);
                                    if ro { "this folder is read-only" } else { "" }
                                }}
                            </span>
                        </div>

                        <div class="dir-list">
                            <For
                                each=move || listing.get().map(|l| l.entries).unwrap_or_default()
                                key=|e| e.path.clone()
                                children=move |entry| {
                                    let path = entry.path.clone();
                                    let writable = entry.writable;
                                    view! {
                                        <button
                                            class="dir-entry"
                                            class:readonly=!writable
                                            on:click=move |_| load(Some(path.clone()))
                                        >
                                            <span class="dir-icon">"▸"</span>
                                            <span class="dir-name">{entry.name}</span>
                                            {(!writable).then(|| view! { <span class="dir-ro">"ro"</span> })}
                                        </button>
                                    }
                                }
                            />
                            {move || {
                                let empty = listing.get().map(|l| l.entries.is_empty()).unwrap_or(true);
                                empty.then(|| view! { <p class="dir-empty">"no sub-folders here"</p> })
                            }}
                        </div>

                        <div class="dir-newfolder">
                            <input
                                class="text-input"
                                placeholder="new folder name"
                                prop:value=move || new_name.get()
                                on:input=move |e| new_name.set(event_target_value(&e))
                            />
                            <button class="btn btn-ghost btn-sm" on:click=create_folder>"+ Create"</button>
                        </div>

                        <p class="dir-error">{move || error.get()}</p>

                        <div class="modal-actions">
                            <button class="btn btn-ghost" on:click=close>"Cancel"</button>
                            <button
                                class="btn btn-primary"
                                prop:disabled=move || listing.get().map(|l| !l.writable).unwrap_or(true)
                                on:click=select_current
                            >
                                "Select this folder"
                            </button>
                        </div>
                    </div>
                </div>
            }
        })}
        </Portal>
    }
}
