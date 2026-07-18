// The Leptos `view!` macros nest deeply enough (esp. the SVG traffic graph) to
// exceed the default type-recursion limit of 128.
#![recursion_limit = "512"]

pub mod api;
pub mod app;
pub mod components;
pub mod types;

#[cfg(feature = "ssr")]
pub mod server;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::App;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
