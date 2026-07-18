# TorrentOxide

A self-hostable web-driven BitTorrent client built in Rust with
[**Leptos**](https://leptos.dev) (SSR + hydration) and the 
[**librqbit**](https://github.com/ikatson/rqbit) torrent engine.

![stack](https://img.shields.io/badge/rust-leptos%200.8-orange) ![engine](https://img.shields.io/badge/engine-librqbit%208-blueviolet)

<img width="1344" height="1053" alt="image" src="https://github.com/user-attachments/assets/14c89598-8933-4bea-ba15-29637a83ef18" />

<img width="651" height="515" alt="image" src="https://github.com/user-attachments/assets/f0003ecb-9fd9-4536-88d0-d2b777e67266" />

<img width="788" height="533" alt="image" src="https://github.com/user-attachments/assets/7f8a349e-d9bc-40ed-b880-c2e08ef32be5" />


## Features

- **Cyberpunk aesthetic** - Torrent clients don't have to look like an office tool.
- **No complex setup** - Just configure a folder for the downloads, run and visit the webpage.
- **Simple operation** - Add torrents, pause/resume, remove, or delete both torrent and downloaded data.
- **.torrent and Magnet support** - Just copy-paste a magnet link and press Download. Or upload a .torrent file, your choice.
- **Ideal for running on your NAS** - Just point to your media folder and pick the right sub-folder for whether you're downloading a TV show, Movie, or anything else.
- **Basic authentication support** - Protect access with a username/password in `.env`. Or don't, if you don't expose it to the outside.
- **Live updates** over Server-Sent Events (no manual refresh).
- **Persistence** - torrents resume across restarts.

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

## Arch Linux

Install paru, if you haven't already:

```sh
sudo pacman -S --needed base-devel
git clone https://aur.archlinux.org/paru.git
cd paru
makepkg -si
```

You can also use yay instead of paru, if you prefer:

```sh
sudo pacman -S --needed base-devel git
git clone https://aur.archlinux.org/yay.git
cd yay
makepkg -si
```

Then install torrentoxide:

```sh
paru -S torrentoxide
```

or

```sh
yay -S torrentoxide
```

## Prebuilt downloads

| Platform | Artifacts |
| --- | --- |
| **Windows** (x86_64) | `.zip` (portable) · `.msi` (installer + Start Menu shortcut) |
| **Linux** (x86_64) | `.tar.gz` (portable) · `.deb` (installs a `systemd` service) |
| **Linux ARM64 / Raspberry Pi** (Pi 3/4/5, Zero 2 W) | `.tar.gz` (portable) · `.deb` (systemd service) |
| **macOS** (universal — Intel + Apple Silicon) | `.tar.gz` (portable) |

> **Raspberry Pi:** use the `linux-arm64` build on a **64-bit** OS (Raspberry Pi OS 64-bit /
> Ubuntu; Bookworm or newer). 32-bit installs aren't supported — reflash 64-bit, or build
> from source / run the Docker image (the `Dockerfile` builds natively on the Pi).

Each package bundles the server binary **and** its `site/` web assets; the binary
locates them next to itself, so the portable archives just need extract + run:

```sh
# Linux / macOS
tar xzf torrentoxide-<version>-<platform>.tar.gz
cd torrentoxide-<version>-<platform> && ./run.sh      # → http://127.0.0.1:3000
```

On Windows, unzip and run `run.bat` (or install the `.msi` and launch **TorrentOxide**
from the Start Menu). The `.deb` installs a service — start it with
`sudo systemctl enable --now torrentoxide` and configure it via
`/etc/torrentoxide/torrentoxide.env`.

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
