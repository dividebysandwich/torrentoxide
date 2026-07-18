# TorrentOxide

A self-hostable web-driven BitTorrent client built in Rust with
[**Leptos**](https://leptos.dev) (SSR + hydration) and the 
[**librqbit**](https://github.com/ikatson/rqbit) torrent engine.

![stack](https://img.shields.io/badge/rust-leptos%200.8-orange) ![engine](https://img.shields.io/badge/engine-librqbit%208-blueviolet)

<img width="995" height="716" alt="image" src="https://github.com/user-attachments/assets/d095c6a3-3173-49bc-9105-7f6fce474195" />

<img width="651" height="515" alt="image" src="https://github.com/user-attachments/assets/f0003ecb-9fd9-4536-88d0-d2b777e67266" />

## Features

- **Live torrent list** — per-torrent progress bar, download/upload speeds, ETA, and a
  dual-series (down/up) traffic **sparkline** per row.
- **Global traffic graph** — animated pulsing-gradient area chart of aggregate up/down.
- **Add torrents** by magnet link, `.torrent` file upload, or http(s) URL.
- **Remote directory browser** — pick the save folder on the *server* (handy when the UI
  runs on another machine), confined to a configurable root, with folder creation.
- **Controls** — pause, resume, cancel (keep files), and cancel & delete files
  (with a confirmation dialog).
- **Live updates** over Server-Sent Events (no manual refresh).
- **Optional auth** — a styled login page + signed session cookie, enabled only when
  credentials are set in `.env`.
- **Persistence** — torrents resume across restarts.
- **Dark, responsive** neon/cyberpunk theme.

## Requirements

```sh
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos
```

## Run

```sh
cp .env.example .env      # optional — sensible defaults apply without it
cargo leptos watch        # dev, with auto-reload
# or
cargo leptos serve        # build once and serve
```

Then open <http://127.0.0.1:3000>.

For a production build:

```sh
cargo leptos build --release
# run the produced ./target/release/torrentoxide binary with LEPTOS_SITE_* env vars set
```

## Configuration (`.env`)

| Variable            | Default            | Purpose                                                        |
| ------------------- | ------------------ | -------------------------------------------------------------- |
| `DOWNLOAD_DIR`      | `./downloads`      | Default folder new torrents download into.                     |
| `BROWSE_ROOT`       | = `DOWNLOAD_DIR`   | Root the remote directory browser is confined to.              |
| `PERSISTENCE_DIR`   | `./.rqbit-session` | Where session state is stored so torrents resume on restart.   |
| `LEPTOS_SITE_ADDR`  | `127.0.0.1:3000`   | Address the server binds to.                                   |
| `AUTH_USERNAME`     | *(unset)*          | Set together with `AUTH_PASSWORD` to require login.            |
| `AUTH_PASSWORD`     | *(unset)*          | Auth is **disabled** unless both are set.                      |
| `SESSION_SECRET`    | *(random)*         | Signs session cookies; set a long random value in production.  |


## Security notes

- The directory browser and all save paths are canonicalized and confined to
  `BROWSE_ROOT` (path-traversal and symlink escapes are rejected).
- Authentication uses a stateless signed cookie. Logging out clears the browser cookie;
  set a strong `SESSION_SECRET` and serve behind HTTPS in production.
