use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::add_torrent;
use crate::components::dir_picker::DirPicker;
use crate::types::AddRequest;

/// Panel for adding torrents by magnet link, http(s) URL, or `.torrent` upload.
#[component]
pub fn AddTorrentPanel() -> impl IntoView {
    let source = RwSignal::new(String::new());
    let paused = RwSignal::new(false);
    let has_file = RwSignal::new(false);
    let status = RwSignal::new(String::new());
    let picker_open = RwSignal::new(false);
    let file_ref: NodeRef<html::Input> = NodeRef::new();

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
        perform_add(dir, source, paused.get(), file_ref, has_file, status);
    });

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
        </div>
    }
}

/// Perform the add once a directory is chosen: upload a selected file, else
/// submit the magnet/URL via the server function.
fn perform_add(
    dir: String,
    source: RwSignal<String>,
    paused: bool,
    file_ref: NodeRef<html::Input>,
    has_file: RwSignal<bool>,
    status: RwSignal<String>,
) {
    let _ = &file_ref;
    #[cfg(feature = "hydrate")]
    {
        if let Some(input) = file_ref.get() {
            if let Some(file) = input.files().and_then(|l| l.get(0)) {
                upload_file(file, dir, paused, status);
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
fn upload_file(file: web_sys::File, dir: String, paused: bool, status: RwSignal<String>) {
    let form = web_sys::FormData::new().unwrap();
    let _ = form.append_with_blob("file", &file);
    let _ = form.append_with_str("output_dir", &dir);
    let _ = form.append_with_str("paused", if paused { "true" } else { "false" });

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
