//! Modal shown (when "select files" is enabled) after picking a download folder:
//! probes the torrent's file list and lets the user choose which files to fetch
//! before the download starts.

use std::collections::HashSet;

use leptos::portal::Portal;
use leptos::prelude::*;

use crate::components::file_tree::FileTree;
use crate::types::{fmt_bytes, FileEntry};

#[component]
pub fn AddFileSelect(
    open: RwSignal<bool>,
    files: RwSignal<Vec<FileEntry>>,
    selected: RwSignal<HashSet<usize>>,
    probing: RwSignal<bool>,
    error: RwSignal<String>,
    #[prop(into)] on_confirm: Callback<()>,
    #[prop(into)] on_cancel: Callback<()>,
) -> impl IntoView {
    let files_sig = Signal::derive(move || files.get());

    let select_all = move |_| {
        selected.set(files.get().iter().map(|f| f.index).collect());
    };
    let select_none = move |_| selected.set(HashSet::new());

    let count = move || selected.get().len();
    let sel_size = move || {
        let sel = selected.get();
        files
            .get()
            .iter()
            .filter(|f| sel.contains(&f.index))
            .map(|f| f.length)
            .sum::<u64>()
    };
    let ready = move || !probing.get() && error.get().is_empty();
    let confirm_label = move || format!("Download {} file(s) · {}", count(), fmt_bytes(sel_size() as f64));

    view! {
        <Portal>
        {move || open.get().then(|| {
            view! {
                <div class="modal-overlay" on:click=move |_| on_cancel.run(())>
                    <div class="modal fsel-modal" on:click=|e| e.stop_propagation()>
                        <h3 class="modal-title">"Select files to download"</h3>

                        <Show when=move || probing.get() fallback=|| ()>
                            <div class="detail-loading">
                                <span class="spinner"></span>"fetching file list from peers…"
                            </div>
                        </Show>
                        {move || {
                            let e = error.get();
                            (!e.is_empty()).then(|| view! { <p class="dir-error">{e}</p> })
                        }}

                        <Show when=ready fallback=|| ()>
                            <div class="fsel-toolbar">
                                <button class="btn btn-ghost btn-sm" on:click=select_all>"✓ All"</button>
                                <button class="btn btn-ghost btn-sm" on:click=select_none>"✕ None"</button>
                            </div>
                            <FileTree files=files_sig selected=selected/>
                        </Show>

                        <div class="modal-actions">
                            <button class="btn btn-ghost" on:click=move |_| on_cancel.run(())>"Cancel"</button>
                            <button
                                class="btn btn-primary"
                                prop:disabled=move || !ready() || count() == 0
                                on:click=move |_| on_confirm.run(())
                            >
                                {confirm_label}
                            </button>
                        </div>
                    </div>
                </div>
            }
        })}
        </Portal>
    }
}
