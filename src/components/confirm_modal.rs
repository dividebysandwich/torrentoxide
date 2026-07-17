use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{cancel_torrent, delete_torrent};
use crate::components::dashboard_state;

/// Single global confirmation dialog driven by `DashboardState::confirm`.
#[component]
pub fn ConfirmModal() -> impl IntoView {
    let confirm = dashboard_state().confirm;

    view! {
        {move || confirm.get().map(|data| {
            let delete_files = data.delete_files;
            let id = data.id;
            let title = if delete_files { "Delete torrent & files" } else { "Cancel torrent" };
            let message = if delete_files {
                format!(
                    "Permanently delete \u{201c}{}\u{201d} and ALL its downloaded files from disk? This cannot be undone.",
                    data.name
                )
            } else {
                format!(
                    "Cancel \u{201c}{}\u{201d}? Downloaded files will be kept on disk.",
                    data.name
                )
            };
            let confirm_label = if delete_files { "Delete files" } else { "Cancel torrent" };

            let do_confirm = move |_| {
                spawn_local(async move {
                    let _ = if delete_files {
                        delete_torrent(id).await
                    } else {
                        cancel_torrent(id).await
                    };
                });
                confirm.set(None);
            };

            view! {
                <div class="modal-overlay" on:click=move |_| confirm.set(None)>
                    <div class="modal confirm-modal" on:click=|e| e.stop_propagation()>
                        <h3 class="modal-title danger">{title}</h3>
                        <p class="modal-body">{message}</p>
                        <div class="modal-actions">
                            <button class="btn btn-ghost" on:click=move |_| confirm.set(None)>
                                "Keep"
                            </button>
                            <button class="btn btn-danger" on:click=do_confirm>
                                {confirm_label}
                            </button>
                        </div>
                    </div>
                </div>
            }
        })}
    }
}
