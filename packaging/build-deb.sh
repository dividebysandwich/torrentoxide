#!/usr/bin/env bash
# Build a .deb from the release binary + built site assets.
# Usage: packaging/build-deb.sh <version>   (run from the repo root)
set -euo pipefail

VERSION="${1:?usage: build-deb.sh <version> [deb-arch]}"
DEB_ARCH="${2:-amd64}"

# The Debian control "Version" must start with a digit and use a limited charset.
# Keep the (possibly non-numeric, e.g. a branch name) VERSION for the file name,
# but derive a valid control version from it.
case "$VERSION" in
  [0-9]*) DEB_VERSION="$VERSION" ;;
  *)      DEB_VERSION="0.0.0~$VERSION" ;;
esac
DEB_VERSION="$(printf '%s' "$DEB_VERSION" | tr -cd 'A-Za-z0-9.+~-')"

case "$DEB_ARCH" in
  amd64) RID="linux-x86_64" ;;
  arm64) RID="linux-arm64" ;;   # 64-bit Raspberry Pi OS (Pi 3/4/5, Zero 2)
  armhf) RID="linux-armv7" ;;
  *)     RID="linux-${DEB_ARCH}" ;;
esac
BIN="target/release/torrentoxide"
SITE="target/site"

[ -x "$BIN" ] || { echo "missing $BIN — run 'cargo leptos build --release' first" >&2; exit 1; }
[ -d "$SITE" ] || { echo "missing $SITE" >&2; exit 1; }

PKG="$(mktemp -d)"
trap 'rm -rf "$PKG"' EXIT

# --- payload ---
install -Dm755 "$BIN" "$PKG/usr/lib/torrentoxide/torrentoxide"
mkdir -p "$PKG/usr/lib/torrentoxide/site"
cp -r "$SITE/." "$PKG/usr/lib/torrentoxide/site/"
# on PATH: symlink resolves (via current_exe) to the real binary, so the
# adjacent site/ dir is still found.
mkdir -p "$PKG/usr/bin"
ln -s ../lib/torrentoxide/torrentoxide "$PKG/usr/bin/torrentoxide"
install -Dm644 packaging/linux/torrentoxide.service "$PKG/lib/systemd/system/torrentoxide.service"
install -Dm644 packaging/linux/torrentoxide.env "$PKG/etc/torrentoxide/torrentoxide.env"
install -Dm644 README.md "$PKG/usr/share/doc/torrentoxide/README.md"

# --- control metadata ---
SIZE_KB="$(du -sk "$PKG" | cut -f1)"
mkdir -p "$PKG/DEBIAN"
cat > "$PKG/DEBIAN/control" <<EOF
Package: torrentoxide
Version: ${DEB_VERSION}
Architecture: ${DEB_ARCH}
Maintainer: TorrentOxide <torrentoxide@users.noreply.github.com>
Section: net
Priority: optional
Depends: libc6, ca-certificates
Installed-Size: ${SIZE_KB}
Homepage: https://github.com/dividebysandwich/torrentoxide
Description: Self-hosted web BitTorrent client
 TorrentOxide is a self-hostable BitTorrent client with a modern web UI,
 built with Leptos and the librqbit engine. Installs a systemd service that
 serves the UI on port 3000.
EOF

# preserve user edits to the config file across upgrades
echo "/etc/torrentoxide/torrentoxide.env" > "$PKG/DEBIAN/conffiles"

cat > "$PKG/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = "configure" ]; then
    systemctl daemon-reload >/dev/null 2>&1 || true
    echo "TorrentOxide installed. Enable + start it with:"
    echo "  sudo systemctl enable --now torrentoxide"
    echo "Then open http://localhost:3000 (edit /etc/torrentoxide/torrentoxide.env for auth)."
fi
exit 0
EOF

cat > "$PKG/DEBIAN/prerm" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = "remove" ]; then
    systemctl stop torrentoxide >/dev/null 2>&1 || true
    systemctl disable torrentoxide >/dev/null 2>&1 || true
fi
exit 0
EOF

cat > "$PKG/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
systemctl daemon-reload >/dev/null 2>&1 || true
exit 0
EOF

chmod 0755 "$PKG/DEBIAN/postinst" "$PKG/DEBIAN/prerm" "$PKG/DEBIAN/postrm"

mkdir -p dist
OUT="dist/torrentoxide-${VERSION}-${RID}.deb"
dpkg-deb --root-owner-group --build "$PKG" "$OUT"
echo "built $OUT"
