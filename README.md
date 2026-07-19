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
- **Built-in PVR** - Sonarr/Radarr-style automation: categories, quality profiles, Torznab indexers + RSS auto-download, a media library, a wanted list with quality upgrades, an episode monitor, and a release calendar ([see below](#media-library--automation-pvr)).

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

## Media library & automation (PVR)

Beyond a plain torrent client, TorrentOxide has a Sonarr/Radarr-style automation
layer, reachable from the top navigation. All of its data lives in an embedded
database at `<PERSISTENCE_DIR>/pvr.redb`. Nothing here is required — the client
works fine without touching any of it.

### Settings

- **Categories** — map a name (e.g. *Movies*, *TV Shows*) to a sub-folder under
  the **media library root** (`LIBRARY_ROOT`), tagged as **Movie**, **TV**, or
  **Other**. A download can target a category, which is where the finished
  download is **imported** (the download itself always lands in `DOWNLOAD_DIR`
  first). The torrent list can be filtered by category, and the category *kind*
  drives how the library classifies files and organizes TV.
- **Quality profiles** — define what releases are acceptable and how upgrades are
  chosen: a minimum and a cutoff resolution, an HDR preference
  (*ignore / prefer / require*), required languages, and whether upgrades are
  allowed. Resolution is primary, HDR a strong secondary preference — e.g.
  *"accept SDR now, upgrade to HDR when it appears"* = min 720p, cutoff 1080p,
  HDR *prefer*, language *english*.
- **Providers** — a free **TMDb API key** (from
  [themoviedb.org](https://www.themoviedb.org/settings/api)) powers library
  identification, the calendar, and the episode monitor. A key set here overrides
  the `TMDB_API_KEY` `.env` value.

### Feeds & indexers

- **Torznab indexers** — point at a Jackett/Prowlarr endpoint (e.g.
  `http://127.0.0.1:9117/api/v2.0/indexers/all/results/torznab/`) with its API
  key; **Test** verifies the connection. Indexers power the manual **Search** and
  the wanted-list monitor.
- **RSS feeds** — subscribe to an RSS/Torznab feed with a category, a quality
  profile, and an auto-download toggle. A background poller (interval
  configurable on the page) grabs acceptable new items into the category;
  episodes already on disk are skipped and grabs are de-duplicated. Feeds can be
  edited or deleted.
- A **grab history** shows what was fetched and from where.

### Wanted & Calendar

- **Wanted** — search TMDb and add a **movie** or **series** to monitor, with a
  quality profile + category. A background monitor runs a few times a day: for a
  series it uses TMDb air dates to find aired-but-missing episodes, searches your
  indexers, and grabs the best acceptable release (upgrading anything below the
  cutoff). **Without a Torznab indexer configured, the wanted list only powers
  the Calendar.**
- **Calendar** — a month grid of upcoming/recent episode air dates for your
  monitored series (via TMDb).

### Library & import

- **Library** — scans the media library (`LIBRARY_ROOT`) into **Movies** and
  **TV Shows** (separate tabs), excluding the incoming download area. Classification
  uses the category *kind* first, then folder layout (`TV Shows/<Show>/…`), then
  filename parsing (handles `S01E04`, `1x04`, `SxxMxx` specials, and absolute anime
  numbering); episodes are grouped by show. Rescans run hourly, on demand, and
  right after an import.
- **Import** — every download (automated grabs **and** manual adds) lands in
  `DOWNLOAD_DIR` first (staged under `.incoming`), then is moved into the library
  **automatically as soon as it finishes** — no button press needed (a Library-page
  button and a periodic sweep are fallbacks):
  - **Automated grabs** go to their category folder; TV is organized into
    `<category>/<Show>/Season NN/<Show> - SxxEyy.ext`, matching an existing show
    folder when possible.
  - **Manual adds** move as-is into the folder you chose.

  Pick the mode on the Library page:
  - **Move** *(default)* — relocates the file into the library so there's a single
    clean copy, and forgets the torrent so it isn't re-downloaded.
  - **Hardlink** — links the file into the library and keeps the download seeding
    from `DOWNLOAD_DIR` (requires `DOWNLOAD_DIR` and `LIBRARY_ROOT` on the same
    filesystem — which they are when `DOWNLOAD_DIR` is a sub-folder of it).
  - **Copy** — a full second copy in the library (doubles disk), keeps seeding.

## Configuration (`.env`)

| Variable            | Default            | Purpose                                                        |
| ------------------- | ------------------ | -------------------------------------------------------------- |
| `LIBRARY_ROOT`      | = `DOWNLOAD_DIR`   | Media library root: category folders live under it, finished downloads are organized into them, and the file browser is confined to it. |
| `DOWNLOAD_DIR`      | `./downloads`      | Incoming folder new torrents download into (staged under `.incoming`) before being moved into the library on completion. |
| `PERSISTENCE_DIR`   | `./.rqbit-session` | Where session + PVR state is stored (`pvr.redb` lives here).   |
| `LEPTOS_SITE_ADDR`  | `127.0.0.1:3000`   | Address the server binds to.                                   |
| `TMDB_API_KEY`      | *(unset)*          | Optional TMDb key for the library/wanted/calendar features (also settable in the UI). |
| `AUTH_USERNAME`     | *(unset)*          | Set together with `AUTH_PASSWORD` to require login.            |
| `AUTH_PASSWORD`     | *(unset)*          | Auth is **disabled** unless both are set.                      |
| `SESSION_SECRET`    | *(random)*         | Signs session cookies; set a long random value in production.  |

> In Docker these default to `/data/downloads` and `/data/.rqbit-session` (set by the image);
> you normally only set `AUTH_USERNAME`, `AUTH_PASSWORD`, and `SESSION_SECRET` via `.env`.


## Security notes

- The directory browser and all save paths are canonicalized and confined to
  `LIBRARY_ROOT` (path-traversal and symlink escapes are rejected).
- Authentication uses a stateless signed cookie. Logging out clears the browser cookie;
  set a strong `SESSION_SECRET` and serve behind HTTPS in production.
