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

## Run with Docker (recommended)

The easiest way to self-host. Requires **Docker** with the **Compose** plugin.

```sh
git clone https://github.com/dividebysandwich/torrentoxide.git
cd torrentoxide
docker compose up -d --build
```

Then open <http://localhost:3000>.

> The first build compiles the whole Rust app (several minutes). Later `docker compose up -d`
> starts instantly.

What the compose setup does:

- Builds a small multi-stage image from the `Dockerfile`.
- Publishes the web UI on port **3000**.
- Mounts two host directories so your data survives container rebuilds:
  - `./downloads` → `/data/downloads` — your files (also the directory-browser root)
  - `./data/session` → `/data/.rqbit-session` — resume state

### Enable authentication (optional)

Create a `.env` file next to `docker-compose.yml` (Compose reads it automatically):

```sh
AUTH_USERNAME=admin
AUTH_PASSWORD=change-me
SESSION_SECRET=$(openssl rand -hex 32)
```

Restart with `docker compose up -d`. Leave these blank (or omit the file) for no auth.

### Everyday commands

```sh
docker compose logs -f          # follow logs
docker compose down             # stop and remove the container
docker compose up -d --build    # rebuild after pulling new code
```

### Customization

- **Paths / port** — set `DOWNLOADS_DIR` and `SESSION_DIR` in `.env` to relocate the host
  volumes; edit the `ports:` mapping (e.g. `8080:3000`) to change the published port.
- **Peer connectivity** — outbound connections + DHT work out of the box. On Linux, for the
  best connectivity uncomment `network_mode: host` in `docker-compose.yml` (and drop the
  `ports:` mapping).

> **No buildx?** `docker compose build` needs the BuildKit/buildx plugin (bundled with
> standard Docker installs). If it's missing, build with the classic builder instead:
> `DOCKER_BUILDKIT=0 docker build -t torrentoxide:latest .` then `docker compose up -d`.

## Run from source

```sh
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos

cp .env.example .env      # optional — sensible defaults apply without it
cargo leptos watch        # dev, with auto-reload
# or
cargo leptos serve        # build once and serve
```

Then open <http://127.0.0.1:3000>.

For a standalone production binary:

```sh
cargo leptos build --release
# run ./target/release/torrentoxide with LEPTOS_SITE_* env vars set
# (see the Dockerfile for the exact variables)
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

> In Docker these default to `/data/downloads` and `/data/.rqbit-session` (set by the image);
> you normally only set `AUTH_USERNAME`, `AUTH_PASSWORD`, and `SESSION_SECRET` via `.env`.


## Security notes

- The directory browser and all save paths are canonicalized and confined to
  `BROWSE_ROOT` (path-traversal and symlink escapes are rejected).
- Authentication uses a stateless signed cookie. Logging out clears the browser cookie;
  set a strong `SESSION_SECRET` and serve behind HTTPS in production.
