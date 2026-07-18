//! Optional authentication: signed-cookie session + login/logout + middleware.

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::cookie::{Cookie, SameSite, SignedCookieJar};
use serde::Deserialize;

use crate::server::AppState;

pub const SESSION_COOKIE: &str = "toxide_session";

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

pub async fn login_page() -> Html<String> {
    Html(login_html(false))
}

pub async fn login_submit(
    State(state): State<AppState>,
    jar: SignedCookieJar,
    Form(form): Form<LoginForm>,
) -> Response {
    if state.config.check_credentials(&form.username, &form.password) {
        let mut cookie = Cookie::new(SESSION_COOKIE, form.username);
        cookie.set_path("/");
        cookie.set_http_only(true);
        cookie.set_same_site(SameSite::Lax);
        (jar.add(cookie), Redirect::to("/")).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, Html(login_html(true))).into_response()
    }
}

pub async fn logout(jar: SignedCookieJar) -> Response {
    (jar.remove(Cookie::from(SESSION_COOKIE)), Redirect::to("/login")).into_response()
}

/// Auth gate. No-op when auth is disabled. Otherwise: public assets + the login
/// routes pass through; everything else needs a valid signed session cookie —
/// unauthenticated `/api/*` gets 401, page loads redirect to `/login`.
pub async fn require_auth(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if !state.config.auth_enabled() {
        return next.run(req).await;
    }

    let path = req.uri().path();
    let public = path == "/login"
        || path == "/logout"
        || path == "/favicon.ico"
        || path.starts_with("/pkg/")
        || path.starts_with("/fonts/");
    if public {
        return next.run(req).await;
    }

    let jar = SignedCookieJar::from_headers(req.headers(), state.key.clone());
    if jar.get(SESSION_COOKIE).is_some() {
        return next.run(req).await;
    }

    if path.starts_with("/api/") {
        StatusCode::UNAUTHORIZED.into_response()
    } else {
        Redirect::to("/login").into_response()
    }
}

fn login_html(error: bool) -> String {
    let error_html = if error {
        r#"<p class="login-error">Invalid credentials. Try again.</p>"#
    } else {
        ""
    };
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1"/>
    <title>TorrentOxide · Login</title>
    <link rel="stylesheet" href="/pkg/torrentoxide.css"/>
    <style>
        :root {{ color-scheme: dark; --red:#ff2b4d; --cyan:#24e3ff; --ink:#d7e9f6; }}
        html, body {{ height: 100%; margin: 0; }}
        body {{
            background:
                radial-gradient(130% 90% at 50% -25%, rgba(255,43,77,.12), transparent 55%),
                linear-gradient(180deg, #05070e, #04060b 45%);
            color: var(--ink);
            font-family: "Rajdhani", "Segoe UI", system-ui, sans-serif;
            font-weight: 500; letter-spacing: .5px;
            display: flex; align-items: center; justify-content: center;
        }}
        .login-card {{
            position: relative; isolation: isolate;
            width: min(92vw, 380px); padding: 2.4rem 2.1rem;
            background: var(--red);
            clip-path: polygon(0 0, calc(100% - 16px) 0, 100% 16px, 100% 100%, 16px 100%, 0 calc(100% - 16px));
            filter: drop-shadow(0 0 14px rgba(255,43,77,.5));
        }}
        .login-card::before {{
            content:""; position:absolute; inset:2px; background:#0a0e18; z-index:-1;
            clip-path: polygon(0 0, calc(100% - 14px) 0, 100% 14px, 100% 100%, 14px 100%, 0 calc(100% - 14px));
        }}
        .login-title {{
            margin: 0 0 0.3rem; font-size: 1.8rem; letter-spacing: 3px; font-weight: 700;
            text-transform: uppercase; color: var(--cyan); text-shadow: 0 0 12px rgba(36,227,255,.5);
        }}
        .login-title b {{ color: var(--red); text-shadow: 0 0 12px rgba(255,43,77,.6); }}
        .login-sub {{ margin: 0 0 1.7rem; color: #6d7d93; font-size: 0.75rem; letter-spacing: 2px; text-transform: uppercase; }}
        .login-field {{ display: block; margin-bottom: 1rem; }}
        .login-field span {{ display: block; font-size: 0.68rem; color: #6d7d93; margin-bottom: 0.35rem; letter-spacing: 2px; text-transform: uppercase; }}
        .login-field input {{
            width: 100%; box-sizing: border-box; padding: 0.7rem 0.85rem;
            border: 1px solid rgba(36,227,255,.3); background: rgba(4,8,14,.85); color: var(--ink); font: inherit;
            clip-path: polygon(0 0, calc(100% - 8px) 0, 100% 8px, 100% 100%, 8px 100%, 0 calc(100% - 8px));
        }}
        .login-field input:focus {{ outline: none; border-color: var(--cyan); box-shadow: 0 0 12px -2px rgba(36,227,255,.5); }}
        .login-btn {{
            width: 100%; margin-top: 0.7rem; padding: 0.8rem; border: none; cursor: pointer;
            font: inherit; font-weight: 700; letter-spacing: 2px; color: #04060b; text-transform: uppercase;
            background: linear-gradient(120deg, var(--cyan), #12a8d8); box-shadow: 0 0 18px -2px rgba(36,227,255,.5);
            clip-path: polygon(0 0, calc(100% - 10px) 0, 100% 10px, 100% 100%, 10px 100%, 0 calc(100% - 10px));
        }}
        .login-btn:hover {{ filter: brightness(1.15); }}
        .login-error {{ color: #ff5e77; font-size: 0.82rem; margin: 0 0 1rem; letter-spacing: .5px; }}
    </style>
</head>
<body>
    <form class="login-card" method="post" action="/login">
        <h1 class="login-title">TORRENT<b>OXIDE</b></h1>
        <p class="login-sub">// authenticate to continue</p>
        {error_html}
        <label class="login-field">
            <span>Username</span>
            <input type="text" name="username" autocomplete="username" autofocus/>
        </label>
        <label class="login-field">
            <span>Password</span>
            <input type="password" name="password" autocomplete="current-password"/>
        </label>
        <button class="login-btn" type="submit">Enter</button>
    </form>
</body>
</html>"##
    )
}
