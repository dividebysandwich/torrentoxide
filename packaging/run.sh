#!/usr/bin/env sh
# TorrentOxide launcher — runs the server from this folder.
# The binary finds its ./site assets automatically; downloads go to ./downloads.
cd "$(dirname "$0")" || exit 1
: "${DOWNLOAD_DIR:=./downloads}"
export DOWNLOAD_DIR
echo "TorrentOxide starting on http://127.0.0.1:3000  (press Ctrl+C to stop)"
exec ./torrentoxide
