use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::components::{ParentRoute, Route, Router, Routes};
use leptos_router::StaticSegment;

use crate::components::dashboard::Dashboard;
use crate::components::feeds_page::FeedsPage;
use crate::components::layout::Layout;
use crate::components::library_page::LibraryPage;
use crate::components::settings_page::SettingsPage;
use crate::components::wanted_page::WantedPage;

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
                <ParentRoute path=StaticSegment("") view=Layout>
                    <Route path=StaticSegment("") view=Dashboard/>
                    <Route path=StaticSegment("library") view=LibraryPage/>
                    <Route path=StaticSegment("wanted") view=WantedPage/>
                    <Route path=StaticSegment("feeds") view=FeedsPage/>
                    <Route path=StaticSegment("settings") view=SettingsPage/>
                </ParentRoute>
            </Routes>
        </Router>
    }
}
