use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::StaticSegment;

use crate::components::dashboard::Dashboard;

/// The HTML document shell used for SSR + hydration.
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options=options.clone()/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/torrentoxide.css"/>
        <Title text="TorrentOxide"/>
        <Router>
            <Routes fallback=|| view! { <p class="notfound">"404 — not found"</p> }>
                <Route path=StaticSegment("") view=Dashboard/>
            </Routes>
        </Router>
    }
}
