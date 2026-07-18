//! A collapsible-looking file tree built from a torrent's flat file list.
//! Each leaf shows a per-file progress bar and a selection checkbox bound to a
//! shared `selected` set. Reused by the detail inspector and the add-file picker.

use std::collections::{BTreeMap, HashSet};

use leptos::prelude::*;

use crate::types::{fmt_bytes, FileEntry};

/// A node in the path tree: named sub-directories plus files living at this level.
#[derive(Default)]
struct Node {
    dirs: BTreeMap<String, Node>,
    files: Vec<FileEntry>,
}

fn build_tree(files: &[FileEntry]) -> Node {
    let mut root = Node::default();
    for f in files {
        if f.components.is_empty() {
            continue;
        }
        let mut node = &mut root;
        for dir in &f.components[..f.components.len() - 1] {
            node = node.dirs.entry(dir.clone()).or_default();
        }
        node.files.push(f.clone());
    }
    root
}

fn render_file(
    f: &FileEntry,
    depth: usize,
    selected: RwSignal<HashSet<usize>>,
    interactive: bool,
) -> AnyView {
    let idx = f.index;
    let name = f.components.last().cloned().unwrap_or_default();
    let length = f.length;
    let pct = if length > 0 {
        (f.have_bytes as f64 / length as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let done = pct >= 99.95;
    let pad = format!("padding-left:{:.2}rem", 0.4 + depth as f32 * 0.95);
    let checked = move || selected.get().contains(&idx);
    let toggle = move |_| {
        selected.update(|s| {
            if !s.remove(&idx) {
                s.insert(idx);
            }
        });
    };

    view! {
        <label class="tree-file" class:excluded=move || !checked() style=pad>
            <input
                class="tree-check"
                r#type="checkbox"
                prop:checked=checked
                prop:disabled=!interactive
                on:change=toggle
            />
            <span class="tree-file-name">{name}</span>
            <span class="tree-file-bar">
                <span
                    class="tree-file-fill"
                    class:complete=done
                    style=format!("width:{pct:.1}%")
                ></span>
            </span>
            <span class="tree-file-size">{fmt_bytes(length as f64)}</span>
        </label>
    }
    .into_any()
}

fn render_node(
    node: &Node,
    depth: usize,
    selected: RwSignal<HashSet<usize>>,
    interactive: bool,
) -> AnyView {
    let mut rows: Vec<AnyView> = Vec::new();
    for (name, child) in &node.dirs {
        let pad = format!("padding-left:{:.2}rem", 0.4 + depth as f32 * 0.95);
        rows.push(
            view! {
                <div class="tree-dir" style=pad>
                    <span class="tree-caret">"▾"</span>
                    <span class="tree-dir-name">{name.clone()}</span>
                </div>
            }
            .into_any(),
        );
        rows.push(render_node(child, depth + 1, selected, interactive));
    }
    for f in &node.files {
        rows.push(render_file(f, depth, selected, interactive));
    }
    view! { <div class="tree-group">{rows}</div> }.into_any()
}

#[component]
pub fn FileTree(
    #[prop(into)] files: Signal<Vec<FileEntry>>,
    selected: RwSignal<HashSet<usize>>,
    /// When false, checkboxes are shown but disabled (read-only inclusion view).
    #[prop(default = true)] interactive: bool,
) -> impl IntoView {
    view! {
        <div class="file-tree">
            {move || {
                let files = files.get();
                if files.is_empty() {
                    return view! { <p class="tree-empty">"— no file list available —"</p> }.into_any();
                }
                render_node(&build_tree(&files), 0, selected, interactive)
            }}
        </div>
    }
}
