# Changelog

All notable changes to Platno are listed here. The newest release is first. Release notes on GitHub
are built from the matching section.

## [0.2.3]

### Diagnostics
- The diagnostic beta (opt-in, see BETA.md) now also reports layout problems: a page wider than the screen, controls overlapping each other or a title, clipped button text and controls outside the screen, with a wireframe that marks the boxes involved and no text. It also reports stalls, slow requests, 403/404-type responses, policy violations and failed styles or pictures, and server events for FFmpeg failures, scan errors and speech-sync tool failures.
- A `:beta` Docker image (same program, diagnostics on) is published next to the normal image; the normal image sends nothing.

### Fixes
- For videos without subtitles the settings panel could open over the timeline because the height of the control bar was never measured.

## [0.2.2]

### Player
- A pause, however long, no longer restarts the converted stream: about 30 seconds stay buffered and playback resumes in under half a second (measured with pauses up to 200 seconds).
- The browser tooltip listing the keyboard shortcuts no longer pops up over the video.
- A card in the bottom right shows 90 seconds before the end of an episode: a countdown to the end, the next episode's name, Go to next and Close. "Don't show again" asks first and saves to your profile; turn it back on in Settings under Profile preferences.
- The header shows the episode title next to the show name, using the catalogue's name for series matched online (rescan once to fill in existing episodes).
- Subtitle appearance (size, color, background, font, position, text shadow) is available in the player settings as well as in Settings. The manual subtitle delay field is gone.
- Subtitles are no longer synchronized automatically. Sync subtitles now listens to up to five 10-second clips from different parts of the video and replaces any clip without speech with another random one.
- Switching to another episode keeps the player in fullscreen.
- On Continue watching cards without artwork the play button moved to the bottom right, so it no longer covers the first letters of the title.
- Volume, Picture-in-Picture, settings and fullscreen sit together in the bottom right corner, like other video players. Hover the speaker for the volume slider; volume is no longer inside the settings panel. The settings panel opens above the corner.

## [0.2.1]

### Player
- Picture-in-Picture and Fullscreen are icon buttons beside the settings gear instead of text buttons inside the panel.
- The play button and the 10-second buttons are real icons, centered in their circles; the volume and back buttons match.
- Pausing shows a short animated pause icon and gently dims the picture. A pause no longer restarts playback unless it lasted over a minute and a half, so resuming does not rebuffer.
- The header shows the episode title next to the season and episode.
- Controls hide after 5 seconds of no mouse movement, but stay while the pointer is on them. Subtitles sit above the controls and move down when the controls hide.
- Leaving the player with the back arrow also leaves fullscreen.
- Space always plays or pauses, even right after clicking a control. Up/Down change the volume and 0 to 9 jump through the video.

### Brand
- New Platno logo (a P whose bowl is a play button) in the sidebar, the browser tab icon, the installed-app icon and the offline and help pages.

### Docker
- The image includes the whisper.cpp speech engine and the verified base model, so automatic subtitle sync works without any setup.

## [0.2.0]

### Player and interface
- Simplified playback controls: timeline, back 10 seconds, play/pause and forward 10 seconds; episode navigation and grouped settings sit at the top.
- Subtitles, synchronization, audio, quality, speed, fullscreen and advanced options are organized in the player settings panel.
- Consistent dark dropdowns throughout setup, settings, filters and playback, with readable long labels and native keyboard behavior.
- Neutral keyboard focus and text selection, without selected-looking highlights on headings, content blocks or settings links.

### Libraries and setup
- New installations start without three predefined libraries. Choose one shared folder in setup for a mixed library containing movies, series and other videos.
- Add, name and organize your own libraries by folder. Explicit scan targets and the most specific folder rules preserve the intended organization.
- Any library can be removed, including the last one, while keeping media and watch history. Existing configured or populated libraries are preserved on upgrade.
- Camera clips and unmatched videos without episode evidence no longer become fabricated local series.
- Folder paths typed during loading are preserved when an earlier directory request finishes late.


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
