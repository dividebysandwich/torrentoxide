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
        || path.starts_with("/pkg/");
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
        :root {{ color-scheme: dark; }}
        html, body {{ height: 100%; margin: 0; }}
        body {{
            background: radial-gradient(1200px 800px at 20% -10%, #131033 0%, #08070f 55%, #05040a 100%);
            color: #e8ecff;
            font-family: ui-monospace, "JetBrains Mono", "Fira Code", monospace;
            display: flex; align-items: center; justify-content: center;
        }}
        .login-card {{
            width: min(92vw, 380px);
            padding: 2.4rem 2rem;
            border-radius: 18px;
            background: rgba(18, 16, 40, 0.72);
            border: 1px solid rgba(120, 90, 255, 0.35);
            box-shadow: 0 0 40px rgba(120, 60, 255, 0.25), inset 0 0 30px rgba(0, 240, 255, 0.05);
            backdrop-filter: blur(12px);
        }}
        .login-title {{
            margin: 0 0 0.35rem; font-size: 1.6rem; letter-spacing: 2px; font-weight: 800;
            background: linear-gradient(90deg, #00f0ff, #a56bff, #ff5cf0);
            -webkit-background-clip: text; background-clip: text; color: transparent;
        }}
        .login-sub {{ margin: 0 0 1.6rem; color: #8b8fb5; font-size: 0.8rem; letter-spacing: 1px; }}
        .login-field {{ display: block; margin-bottom: 1rem; }}
        .login-field span {{ display: block; font-size: 0.72rem; color: #9aa0cc; margin-bottom: 0.35rem; letter-spacing: 1px; text-transform: uppercase; }}
        .login-field input {{
            width: 100%; box-sizing: border-box; padding: 0.7rem 0.85rem;
            border-radius: 10px; border: 1px solid rgba(120, 130, 200, 0.3);
            background: rgba(8, 8, 20, 0.75); color: #e8ecff; font: inherit;
        }}
        .login-field input:focus {{ outline: none; border-color: #00f0ff; box-shadow: 0 0 0 2px rgba(0,240,255,0.25); }}
        .login-btn {{
            width: 100%; margin-top: 0.6rem; padding: 0.75rem; border: none; cursor: pointer;
            border-radius: 10px; font: inherit; font-weight: 700; letter-spacing: 1px; color: #05040a;
            background: linear-gradient(90deg, #00f0ff, #a56bff); text-transform: uppercase;
            box-shadow: 0 0 20px rgba(0, 240, 255, 0.35);
        }}
        .login-btn:hover {{ filter: brightness(1.1); }}
        .login-error {{ color: #ff6b9d; font-size: 0.8rem; margin: 0 0 1rem; }}
    </style>
</head>
<body>
    <form class="login-card" method="post" action="/login">
        <h1 class="login-title">TORRENTOXIDE</h1>
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
