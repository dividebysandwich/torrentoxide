#![recursion_limit = "512"]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use std::sync::Arc;

    use axum::extract::DefaultBodyLimit;
    use axum::middleware;
    use axum::routing::{get, post};
    use axum::Router;
    use axum_extra::extract::cookie::Key;
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};

    use torrentoxide::app::{shell, App};
    use torrentoxide::server::config::AppConfig;
    use torrentoxide::server::engine::Engine;
    use torrentoxide::server::{
        auth,
        events::sse_handler,
        upload::{probe_handler, upload_handler},
        AppState,
    };

    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,librqbit=info".into()),
        )
        .init();

    let config = Arc::new(AppConfig::from_env().expect("failed to load configuration"));
    let engine = Engine::new(config.clone())
        .await
        .expect("failed to start torrent engine");

    // Leptos options come from cargo-leptos env vars (LEPTOS_SITE_ADDR etc.).
    let conf = get_configuration(None).expect("failed to read leptos configuration");
    let mut leptos_options = conf.leptos_options;

    // For packaged/standalone builds (archives, .deb, .msi) the leptos env vars
    // aren't set, so pin the values that are fixed for this app and self-locate
    // the `site/` assets next to the executable.
    if std::env::var_os("LEPTOS_OUTPUT_NAME").is_none() {
        // Otherwise leptos defaults this to "leptos_config" → wrong /pkg/*.js name.
        leptos_options.output_name = "torrentoxide".into();
    }
    if std::env::var_os("LEPTOS_SITE_ROOT").is_none() {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(candidate) = exe.parent().map(|d| d.join("site")) {
                if candidate.is_dir() {
                    leptos_options.site_root = candidate.to_string_lossy().into_owned().into();
                }
            }
        }
    }

    let addr = leptos_options.site_addr;

    let key = match &config.session_secret {
        Some(secret) => Key::derive_from(secret.as_bytes()),
        None => {
            if config.auth_enabled() {
                log!("WARNING: SESSION_SECRET is not set — sessions will reset on restart.");
            }
            Key::generate()
        }
    };

    let app_state = AppState {
        leptos_options: leptos_options.clone(),
        engine,
        config: config.clone(),
        key,
    };

    let routes = generate_route_list(App);

    let app = Router::new()
        .route("/api/events", get(sse_handler))
        .route(
            "/api/upload",
            post(upload_handler).layer(DefaultBodyLimit::max(26 * 1024 * 1024)),
        )
        .route(
            "/api/probe",
            post(probe_handler).layer(DefaultBodyLimit::max(26 * 1024 * 1024)),
        )
        .route("/login", get(auth::login_page).post(auth::login_submit))
        .route("/logout", post(auth::logout))
        .leptos_routes(&app_state, routes, {
            let opts = leptos_options.clone();
            move || shell(opts.clone())
        })
        .fallback(leptos_axum::file_and_error_handler::<AppState, _>(shell))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth::require_auth,
        ))
        .with_state(app_state);

    if config.auth_enabled() {
        log!("Authentication is ENABLED (username/password required).");
    } else {
        log!("Authentication is DISABLED (set AUTH_USERNAME + AUTH_PASSWORD to enable).");
    }
    log!("TorrentOxide listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app.into_make_service())
        .await
        .expect("server error");
}

#[cfg(not(feature = "ssr"))]
fn main() {
    // The wasm/hydrate build has no binary entry point.
}
