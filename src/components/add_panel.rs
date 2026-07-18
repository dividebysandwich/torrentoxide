use std::collections::HashSet;

use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::add_torrent;
use crate::components::add_file_select::AddFileSelect;
use crate::components::dir_picker::DirPicker;
use crate::types::{AddRequest, FileEntry};

/// Panel for adding torrents by magnet link, http(s) URL, or `.torrent` upload,
/// optionally choosing which files to download first.
#[component]
pub fn AddTorrentPanel() -> impl IntoView {
    let source = RwSignal::new(String::new());
    let paused = RwSignal::new(false);
    let select_files = RwSignal::new(false);
    let has_file = RwSignal::new(false);
    let status = RwSignal::new(String::new());
    let picker_open = RwSignal::new(false);
    let file_ref: NodeRef<html::Input> = NodeRef::new();

    // --- file-selection modal state ---
    let fsel_open = RwSignal::new(false);
    let chosen_dir = RwSignal::new(String::new());
    let probe_files = RwSignal::new(Vec::<FileEntry>::new());
    let probe_selected = RwSignal::new(HashSet::<usize>::new());
    let probing = RwSignal::new(false);
    let probe_error = RwSignal::new(String::new());

    let can_add = move || !source.get().trim().is_empty() || has_file.get();

    let open_picker = move |_| {
        if can_add() {
            status.set(String::new());
            picker_open.set(true);
        } else {
            status.set("Enter a magnet link / URL or choose a .torrent file first.".into());
        }
    };

    let on_dir_selected = Callback::new(move |dir: String| {
        if select_files.get_untracked() {
            // Probe the file list first, then let the user pick.
            chosen_dir.set(dir.clone());
            probe_files.set(Vec::new());
            probe_selected.set(HashSet::new());
            probe_error.set(String::new());
            probing.set(true);
            fsel_open.set(true);
            start_probe(dir, source, file_ref, probe_files, probe_selected, probing, probe_error);
        } else {
            perform_add(dir, source, paused.get(), None, file_ref, has_file, status);
        }
    });

    // Confirm the file selection → perform the add with only the chosen files.
    let confirm_selection = Callback::new(move |_: ()| {
        let mut indices: Vec<usize> = probe_selected.get().into_iter().collect();
        indices.sort_unstable();
        perform_add(
            chosen_dir.get(),
            source,
            paused.get(),
            Some(indices),
            file_ref,
            has_file,
            status,
        );
        fsel_open.set(false);
    });
    let cancel_selection = Callback::new(move |_: ()| fsel_open.set(false));

    view! {
        <div class="add-panel panel">
            <div class="add-row">
                <input
                    class="text-input grow"
                    r#type="text"
                    placeholder="magnet:?xt=urn:… or https://…/file.torrent"
                    prop:value=move || source.get()
                    on:input=move |e| source.set(event_target_value(&e))
                />
                <label class="file-btn">
                    <span>{move || if has_file.get() { "✓ .torrent" } else { "📄 .torrent" }}</span>
                    <input
                        r#type="file"
                        accept=".torrent"
                        node_ref=file_ref
                        style="display:none"
                        on:change=move |_| has_file.set(file_selected(file_ref))
                    />
                </label>
                <label class="pause-check">
                    <input
                        r#type="checkbox"
                        prop:checked=move || select_files.get()
                        on:change=move |e| select_files.set(event_target_checked(&e))
                    />
                    <span>"pick files"</span>
                </label>
                <label class="pause-check">
                    <input
                        r#type="checkbox"
                        prop:checked=move || paused.get()
                        on:change=move |e| paused.set(event_target_checked(&e))
                    />
                    <span>"paused"</span>
                </label>
                <button
                    class="btn btn-primary"
                    prop:disabled=move || !can_add()
                    on:click=open_picker
                >
                    "Add ▸"
                </button>
            </div>
            <p class="add-status">{move || status.get()}</p>
            <DirPicker open=picker_open on_select=on_dir_selected/>
            <AddFileSelect
                open=fsel_open
                files=probe_files
                selected=probe_selected
                probing=probing
                error=probe_error
                on_confirm=confirm_selection
                on_cancel=cancel_selection
            />
        </div>
    }
}

/// Kick off a file-list probe (URL/magnet via server fn, upload via `/api/probe`).
#[cfg(feature = "hydrate")]
fn start_probe(
    dir: String,
    source: RwSignal<String>,
    file_ref: NodeRef<html::Input>,
    files: RwSignal<Vec<FileEntry>>,
    selected: RwSignal<HashSet<usize>>,
    probing: RwSignal<bool>,
    error: RwSignal<String>,
) {
    let finish = move |result: Result<Vec<FileEntry>, String>| {
        probing.set(false);
        match result {
            Ok(list) => {
                selected.set(list.iter().filter(|f| f.included).map(|f| f.index).collect());
                files.set(list);
            }
            Err(e) => error.set(e),
        }
    };

    // Uploaded .torrent → probe via multipart; otherwise magnet/URL server fn.
    if let Some(file) = file_ref.get().and_then(|i| i.files()).and_then(|l| l.get(0)) {
        let form = web_sys::FormData::new().unwrap();
        let _ = form.append_with_blob("file", &file);
        let _ = form.append_with_str("output_dir", &dir);
        spawn_local(async move {
            let outcome = match gloo_net::http::Request::post("/api/probe").body(form) {
                Ok(req) => match req.send().await {
                    Ok(r) if r.ok() => r
                        .json::<Vec<FileEntry>>()
                        .await
                        .map_err(|e| format!("bad response: {e}")),
                    Ok(r) => Err(format!("probe failed ({})", r.status())),
                    Err(e) => Err(format!("probe error: {e}")),
                },
                Err(e) => Err(format!("request error: {e}")),
            };
            finish(outcome);
        });
    } else {
        let src = source.get();
        spawn_local(async move {
            let outcome = crate::api::probe_url(src, dir).await.map_err(|e| e.to_string());
            finish(outcome);
        });
    }
}

#[cfg(not(feature = "hydrate"))]
fn start_probe(
    _dir: String,
    _source: RwSignal<String>,
    _file_ref: NodeRef<html::Input>,
    _files: RwSignal<Vec<FileEntry>>,
    _selected: RwSignal<HashSet<usize>>,
    _probing: RwSignal<bool>,
    _error: RwSignal<String>,
) {
}

/// Perform the add once a directory (and optional file selection) is chosen.
fn perform_add(
    dir: String,
    source: RwSignal<String>,
    paused: bool,
    only_files: Option<Vec<usize>>,
    file_ref: NodeRef<html::Input>,
    has_file: RwSignal<bool>,
    status: RwSignal<String>,
) {
    let _ = &file_ref;
    #[cfg(feature = "hydrate")]
    {
        if let Some(input) = file_ref.get() {
            if let Some(file) = input.files().and_then(|l| l.get(0)) {
                upload_file(file, dir, paused, only_files, status);
                input.set_value("");
                has_file.set(false);
                return;
            }
        }
    }

    let src = source.get();
    if src.trim().is_empty() {
        status.set("Enter a magnet link / URL or choose a .torrent file.".into());
        return;
    }
    let _ = &has_file;
    spawn_local(async move {
        match add_torrent(AddRequest {
            source: src,
            output_dir: dir,
            paused,
            only_files,
        })
        .await
        {
            Ok(()) => {
                source.set(String::new());
                status.set(String::new());
            }
            Err(e) => status.set(e.to_string()),
        }
    });
}

#[cfg(feature = "hydrate")]
fn file_selected(file_ref: NodeRef<html::Input>) -> bool {
    file_ref
        .get()
        .and_then(|i| i.files())
        .map(|l| l.length() > 0)
        .unwrap_or(false)
}

#[cfg(not(feature = "hydrate"))]
fn file_selected(_file_ref: NodeRef<html::Input>) -> bool {
    false
}

#[cfg(feature = "hydrate")]
fn upload_file(
    file: web_sys::File,
    dir: String,
    paused: bool,
    only_files: Option<Vec<usize>>,
    status: RwSignal<String>,
) {
    let form = web_sys::FormData::new().unwrap();
    let _ = form.append_with_blob("file", &file);
    let _ = form.append_with_str("output_dir", &dir);
    let _ = form.append_with_str("paused", if paused { "true" } else { "false" });
    if let Some(indices) = only_files {
        let csv = indices.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let _ = form.append_with_str("only_files", &csv);
    }

    spawn_local(async move {
        match gloo_net::http::Request::post("/api/upload").body(form) {
            Ok(req) => match req.send().await {
                Ok(r) if r.ok() => status.set(String::new()),
                Ok(r) => status.set(format!("upload failed ({})", r.status())),
                Err(e) => status.set(format!("upload error: {e}")),
            },
            Err(e) => status.set(format!("request error: {e}")),
        }
    });
}
