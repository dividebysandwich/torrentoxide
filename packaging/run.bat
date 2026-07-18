@echo off
rem TorrentOxide launcher — runs the server from this folder.
cd /d "%~dp0"
if not defined DOWNLOAD_DIR set "DOWNLOAD_DIR=%~dp0downloads"
echo TorrentOxide starting on http://127.0.0.1:3000  (close this window to stop)
start "" "http://127.0.0.1:3000"
"%~dp0torrentoxide.exe"
