//! Server-only modules (compiled under the `ssr` feature).

pub mod auth;
pub mod config;
pub mod engine;
pub mod events;
pub mod pvr;
pub mod upload;

use std::sync::Arc;

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use leptos::prelude::LeptosOptions;

use config::AppConfig;
use engine::Engine;
use pvr::Pvr;

/// Shared application state provided to Axum handlers and Leptos server fns.
#[derive(Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub engine: Arc<Engine>,
    pub config: Arc<AppConfig>,
    pub key: Key,
    pub pvr: Arc<Pvr>,
}

impl FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        state.leptos_options.clone()
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}
