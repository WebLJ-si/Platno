# Changelog

All notable changes to Platno are listed here. The newest release is first. Release notes on GitHub
are built from the matching section.

## [0.1.0]

First public release.

### Library and playback
- Scans folders in place and never modifies media files. Recognises series (TVmaze) and movies (TMDB, or the free Wikidata source when no key is set), with season and episode detection from file names.
- Works without any API key: titles keep a name taken from the file name and a default picture. Name, details, cover and episodes can be edited by hand and survive rescans.
- Web player with direct play, HLS conversion with FFmpeg (adaptive 480p/720p/1080p), HDR to SDR tone mapping, audio track and subtitle selection, two subtitles at once, local speech-based subtitle sync, picture-in-picture and keyboard shortcuts.
- Profiles with PINs, watchlist, continue watching, recommendations and per-profile settings.

### Setting up and running
- Animated first-run guide for the owner: media folders, conversion folder, memory and graphics, first scan, accounts, API keys and remote access. Every step can be skipped, with a clear list of what is lost; features that need a missing key explain how to add it.
- Owner-started updates from the Settings page (checks GitHub, verifies the SHA-256 checksum, keeps the previous version, restarts itself).
- Optional HTTPS remote access with automatic UPnP port mapping.
- Ready-to-run programs for Linux (x86_64, ARM64), macOS (Apple silicon and Intel) and Windows, `.deb` packages for Debian and Ubuntu, and a multi-architecture Docker image.
- Optional, clearly disclosed diagnostic beta; stable builds send nothing.

### Known limits
- The programs and packages are not code-signed: macOS and Windows show a warning on first start (see the README).
- Windows and macOS are built and unit-tested automatically but have had less hands-on testing than Linux.
- No per-library permissions or parental controls yet; see `docs/feature-review.md`.
