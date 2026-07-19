//! Environment / `.env`-based configuration (server-only).

use std::path::{Path, PathBuf};

use anyhow::Context;

#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Default folder new torrents download into.
    pub download_dir: PathBuf,
    /// Root the remote directory browser is confined to.
    pub browse_root: PathBuf,
    /// Where librqbit persists its session state (for resume across restarts).
    pub persistence_dir: PathBuf,
    /// Optional auth. Auth is enabled only when BOTH username and password are set.
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
    /// Secret used to sign the session cookie. Random (per-run) if unset.
    pub session_secret: Option<String>,
    /// TMDb API key for metadata lookups (library / wanted / episode monitor).
    /// Overridable in the web UI; used by the PVR features.
    pub tmdb_api_key: Option<String>,
}

/// Read a non-empty env var, trimming whitespace; `None` if unset or blank.
fn env_opt(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
        _ => None,
    }
}

/// Create the directory (recursively) if needed, then canonicalize it.
fn ensure_dir(path: &Path) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory {}", path.display()))?;
    std::fs::canonicalize(path)
        .with_context(|| format!("failed to canonicalize {}", path.display()))
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let download_dir = env_opt("DOWNLOAD_DIR").unwrap_or_else(|| "./downloads".to_string());
        let download_dir = ensure_dir(Path::new(&download_dir))?;

        let browse_root = match env_opt("BROWSE_ROOT") {
            Some(p) => ensure_dir(Path::new(&p))?,
            None => download_dir.clone(),
        };

        let persistence_dir =
            env_opt("PERSISTENCE_DIR").unwrap_or_else(|| "./.rqbit-session".to_string());
        let persistence_dir = ensure_dir(Path::new(&persistence_dir))?;

        // Auth is only active when both are present.
        let auth_username = env_opt("AUTH_USERNAME");
        let auth_password = env_opt("AUTH_PASSWORD");

        Ok(Self {
            download_dir,
            browse_root,
            persistence_dir,
            auth_username,
            auth_password,
            session_secret: env_opt("SESSION_SECRET"),
            tmdb_api_key: env_opt("TMDB_API_KEY"),
        })
    }

    pub fn auth_enabled(&self) -> bool {
        self.auth_username.is_some() && self.auth_password.is_some()
    }

    /// Constant-time-ish credential check.
    pub fn check_credentials(&self, user: &str, pass: &str) -> bool {
        let (Some(u), Some(p)) = (&self.auth_username, &self.auth_password) else {
            return false;
        };
        // Compare both fields without early-exit on length/first-mismatch.
        ct_eq(user.as_bytes(), u.as_bytes()) & ct_eq(pass.as_bytes(), p.as_bytes())
    }
}

/// Constant-time byte comparison; returns true iff equal.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
