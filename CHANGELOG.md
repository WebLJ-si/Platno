# Changelog

All notable changes to Platno are listed here. The newest release is first. Release notes on GitHub
are built from the matching section.

## [Unreleased]

### Fixes
- Fullscreen works: the player area goes fullscreen instead of the dialog, which browsers refuse.
- The seek bar keeps following playback after a seek, appears as soon as the length is known, and its hover preview shows the frame named by its label.
- Removed the "mark season watched" buttons; a series is still marked or reset as a whole, and each file individually.

### Folder browser
- Choosing a media folder now opens a folder list inside the page, which works in any browser on any system including Docker and Windows. The macOS-only server window and `/api/choose-folder` are gone.

### Faster video start
- Converted videos start in well under a second of waiting instead of about six (measured on a fast machine; a slow NAS processor is now the only limit). FFmpeg runs ahead of the player and is paused a few segments in front of it, so an idle or paused viewer costs no processor time. On Windows FFmpeg is still held to real time.
- H.264 videos that a browser already decodes (up to the chosen quality and 16 Mbit/s) are repackaged instead of re-encoded when playback starts from the beginning, which takes almost no processor time on a NAS.
- Auto quality repackages compatible H.264 as a single stream and only encodes the qualities the machine can afford (480p and 720p, plus 1080p with 8+ threads or a hardware encoder). On a 2-processor container the first segment arrives in about 0.6 s for H.264 and 2 s for HEVC, down from about 7 s.
- The player shows a spinner with what is happening and how many seconds have passed; media information is fetched while a title's page is open and alongside the saved position.
- The encoder check and the media information of a file are remembered, so starting a video no longer runs several extra FFmpeg and ffprobe processes.

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
