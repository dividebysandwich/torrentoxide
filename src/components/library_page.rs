//! Library page. Scans the download tree into movies/shows in a later phase.

use leptos::prelude::*;

#[component]
pub fn LibraryPage() -> impl IntoView {
    view! {
        <div class="page-stub panel">
            <h2 class="page-title">"LIBRARY"</h2>
            <p class="page-stub-note">"Media library scanning arrives in a later phase."</p>
        </div>
    }
}
