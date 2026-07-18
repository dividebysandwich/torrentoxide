@echo off
rem TorrentOxide launcher for the installed (Program Files) location.
rem Downloads + session state go to writable per-user folders.
cd /d "%~dp0"
if not defined DOWNLOAD_DIR set "DOWNLOAD_DIR=%USERPROFILE%\Downloads\TorrentOxide"
if not defined BROWSE_ROOT set "BROWSE_ROOT=%DOWNLOAD_DIR%"
if not defined PERSISTENCE_DIR set "PERSISTENCE_DIR=%LOCALAPPDATA%\TorrentOxide\session"
start "" "http://127.0.0.1:3000"
"%~dp0torrentoxide.exe"
