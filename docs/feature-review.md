# Feature proposal review

A review of the source document dated 29 Oct 2026. The document is a list of ideas; its internal priorities are not extra instructions from the user. Each proposal below has an explicit status. "Deferred" means a sensible separate development package, not an implemented feature.

Features were checked with Rust tests, an isolated HTTP/FFmpeg environment and a Chrome UI test. For the actual testing limits see `validation.md`.

Update of 29 Oct 2026: at the user's request, manual skip markers, chapter navigation and local problem reports were removed. The audio selector stays visible only when there are several tracks. Local speech-based synchronization of text subtitles is implemented; exact-release matching, translated subtitles and the maintainer issue workflow remain in `TODO.md`. A first-run setup guide, a configurable conversion folder and a configurable number of simultaneous conversions were added.

## First: the 12 highest-impact features

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| Audio track selection. The description marks it as not implemented, but it is essential: dubs, multilingual films, director's commentary. | Implemented | Audio track selection with language/codec/channels in HLS and progressive conversion. |
| Skip Intro / Recap / Credits. A button during playback. The intro is detected with ffmpeg (comparing audio or black frames across episodes) or by a manual time mark. | Removed at request | Manual markers and their APIs were removed. There is no automatic detection. |
| Next episode with a countdown. At the end of the credits a card with a countdown appears, like Netflix (post-play). | Implemented | A card, a 10-second countdown per profile setting, cancel and manual start. |
| Mark as watched / unwatched. One click for a movie, episode or series, plus progress reset. | Implemented | Transactional marking of a movie/file/series; unwatched deletes the position. |
| HLS adaptive bitrate. Several qualities in the master playlist so hls.js switches by network speed. | Implemented | Master playlist with 480/720/1080; hls.js chooses by network. Native HLS is chosen by the client. |
| Burn-in of image subtitles (PGS/VobSub). Very common in Blu-ray remuxes, not supported now. | Deferred | Needs verified PGS and VobSub samples, checks of geometry/colors and seek/ABR combinations. The current release keeps text subtitles. |
| HDR → SDR tone mapping. Without it HDR films look washed out after conversion. | Implemented | Automatic PQ/HLG detection and zscale/tonemap in both converters; requires the matching FFmpeg filters. |
| Trickplay. Preview thumbnails when dragging the slider, generated with ffmpeg. | Partly implemented | Slider previews are produced on demand in 5-second steps; the cache is bounded. Pre-generating sprite atlases is left for later. |
| Parental control. Age limits per profile (G/PG/12/16/18), a kids profile and library locks per profile. | Deferred | A security-critical package: needs reliable age ratings and server-side restriction of all lists, metadata, streams and profile switches. Hiding things in the UI would create a false sense of protection. |
| Secure remote access. Instructions or built-in support for a reverse proxy and HTTPS, preferably Cloudflare Tunnel or Tailscale. | Implemented | Instructions and a Caddy example for HTTPS and a private VPN, plus an optional built-in HTTPS listener with time-limited UPnP mapping; no tunnels are created. |
| Keyboard shortcuts. Space, ←/→, F, M, J/L, C for subtitles, Shift+N for the next episode. | Implemented | Space/K, arrows/J/L, F, M, C, Shift+N; Ctrl/Cmd+K for search. Input fields do not trigger commands. |
| Mobile and TV responsiveness and PWA. An installable app with an icon, manifest and full-screen display. | Partly implemented | Manifest, PNG icons, service worker, an unreachable-server screen and adapted mobile controls. Installing requires HTTPS or localhost. A physical TV client and remote are not verified. |

## Player

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| A separate audio and subtitle selector with language and codec labels (e.g. "English · EAC3 5.1") | Implemented | Audio track selection with language/codec/channels in HLS and progressive conversion. |
| Default audio and subtitle languages per profile (e.g. original audio, Slovenian subtitles) | Implemented | Audio language per profile and the existing subtitle language/mode settings. |
| Picture-in-Picture and a mini player | Partly implemented | Browser PiP with a WebKit fallback; support depends on the browser. A mini player while browsing is not implemented. |
| Chromecast, AirPlay and DLNA playback | Deferred | Each protocol needs device discovery, a receiver/session, secure access to the stream and a test on a physical device. |
| Chapters from MKV/MP4 with marks on the timeline | Removed at request | The chapter selector and its playback-info data were removed. |
| Remember the last speed and volume | Implemented | Speed and volume are stored in the profile; volume also locally as a fallback. |
| Volume normalization and night mode (dynamic compression): quiet speech, loud explosions | Deferred | Needs a verified audio chain for both converters and loudness measurements; we do not change audio without a clear setting. |
| Surround sound: keep 5.1 and downmix to stereo if the client cannot cope | Partly existing | Compatible originals stay unchanged; the HLS downmix to stereo is verified. Multichannel delivery needs a client capability handshake. |
| Manual subtitle delay (+/- seconds) and audio delay | Partly implemented | Manual delay of text subtitles ±30 seconds; audio delay is not implemented. |
| Subtitle style: background, outline, shadow, position | Partly implemented | Size, color, dark/transparent background and shadow. Dual subtitles have separate positions; an arbitrary style editor does not exist yet. |
| Two subtitles at once (language learning) | Implemented | Two different text tracks with a shared time offset and separate positions. |
| "Stats for nerds" display: codec, bitrate, direct/transcode, reason for transcoding | Implemented | Stream data, selected track, quality, mode and the reason for conversion in the player. |
| A "Report a problem" button during playback that records stream data | Removed at request | The UI, API, local storage of reports and the administrator list were removed. A future workflow is in TODO.md. |
| Skip ±10 s with a double click or tap (mobile) | Partly implemented | Double-clicking the left/right half skips ±10 s. Mobile swipe gestures are not implemented. |
| Gesture controls: brightness, volume, skipping by dragging | Deferred | Needs tests of conflicts with scrolling, system gestures and accessibility on real phones. |
| Save the position when the tab closes too (sendBeacon) | Implemented | The position is sent on close/pagehide/visibilitychange; a keepalive fallback and a per-profile local key. |
| Ambilight / ambient background mode | Deferred | A decorative feature with extra GPU use; not essential for reliable playback. |

## Search, browsing and discovery

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| Filters: genre, year, rating, language, country, studio, runtime, unwatched | Partly implemented | Genre, year, rating, language, runtime, unwatched and sorting. Country/studio do not have uniform stored data yet. Unknown values are clearly excluded from active filters. |
| Sorting: added, year, rating, alphabetical, random | Implemented | Title, scan date/mtime, year, rating and stable random sorting. An unchanging first-added date is not stored yet. |
| Search by actors, directors and writers (a person page with filmography) | Deferred | Needs stored credits/people and provider links; the current index does not have that data. |
| Actor page: all films you have locally | Deferred | Needs stored credits/people and provider links; the current index does not have that data. |
| Trailers: automatic preview on hover, like Netflix, or a "Trailer" button | Deferred | Needs separate provider video references and decisions about external playback and privacy. Automatic hover video is not a default improvement. |
| "More like this" from genre, cast and TMDB keywords | Partly existing | Local recommendations from descriptions and history already exist. Credits/keywords are not stored, so using them is not yet possible. |
| A "Because you watched X" shelf | Partly existing | Local recommendations from descriptions and history already exist. Credits/keywords are not stored, so using them is not yet possible. |
| Random movie ("Surprise me") with a choice of mood or genre | Partly implemented | Surprise me respects active filters; genre works when metadata is known. Mood is not modeled. |
| Manual collections created by the user | Deferred | Needs its own membership model, editing and per-profile permissions. Existing TMDB/local collections remain. |
| Smart collections with rules (e.g. "comedies after 2010 rated above 7") | Deferred | Needs its own membership model, editing and per-profile permissions. Existing TMDB/local collections remain. |
| Playlists, shared or personal | Deferred | Needs its own membership model, editing and per-profile permissions. Existing TMDB/local collections remain. |
| Tags/labels and manual sorting | Deferred | Needs its own membership model, editing and per-profile permissions. Existing TMDB/local collections remain. |
| "Just added" and "New this week" shelves | Deferred | The first-added time must be separate from the last scan; sorting by existing data is implemented, and we do not show a wrong shelf. |
| A "Recently watched" shelf with the option to remove from Continue Watching | Partly implemented | Continue Watching and per-profile history exist; reset removes the position. A separate history shelf is not implemented. |
| A "Top 10 in your library" shelf by the whole server's watch history | Deferred | The current history keeps the last state, not all watch events; a server-wide Top 10 cannot be computed honestly from it. |
| IMDb, Rotten Tomatoes and Metacritic ratings next to TMDB | Deferred | Needs an additional provider/key and verified terms of use. TMDB/TVmaze ratings remain. |
| Showing the age rating and short warnings (violence, language) | Deferred | A reliable model of regional ratings and warnings is missing; tied to parental control. |
| Choosing by time ("I have 90 minutes") | Implemented | A time filter of 30/60/90/120/180 minutes considers only known runtimes. |
| Global search with Ctrl+K and suggestions while typing | Partly implemented | Ctrl/Cmd+K focuses the existing global search; a separate suggestions window is not implemented. |
| Typo-tolerant (fuzzy) search | Deferred | Relevance ranking and a test on large libraries are needed; the existing normalized title/file search remains. |
| A hero carousel ("billboard") on Overview with silent background playback | Partly existing | Overview has a static hero. Rotation and automatic playback are decorative and need motion control and a network cost review. |

## Metadata and library

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| Several providers: besides TMDB also Trakt, OMDb, Fanart.tv and TheTVDB (for series) | Deferred | A new adapter, identities, cache, subscriptions and rules for resolving conflicting results are a separate development package. |
| Anime support (AniDB/AniList, absolute episode numbers) | Deferred | A new adapter, identities, cache, subscriptions and rules for resolving conflicting results are a separate development package. |
| A music library (albums, artists, lists, lyrics) and a background audio player | Deferred | New media types need their own models, metadata and clients. Arbitrary video libraries are already supported. |
| A photo and home-video library (dates, albums, optionally faces) | Deferred | New media types need their own models, metadata and clients. Arbitrary video libraries are already supported. |
| Podcasts and audiobooks with a remembered position | Deferred | New media types need their own models, metadata and clients. Arbitrary video libraries are already supported. |
| Documentaries, stand-up and a YouTube archive as special library types | Deferred | New media types need their own models, metadata and clients. Arbitrary video libraries are already supported. |
| Several versions of the same film (1080p, 4K, director's cut) with a choice at playback | Partly existing | Local files with the same provider identity are available in the details; quality/release is not yet normalized for a dedicated selector. |
| Detecting duplicate files and suggesting a merge | Deferred | The same path/provider is not proof of the same file. Needs cost-bounded hash/signature comparisons and a safe review before merging. |
| Detecting damaged files (ffprobe error) with a warning | Partly existing | FFprobe returns a useful error on playback. Full periodic checks of large media would need a queue. |
| Importing and exporting .nfo files (Kodi/Emby) | Deferred | Needs a Kodi/Emby-compatible parser and precedence rules; export must not change media folders unintentionally. |
| Image capture: poster, background, logo, episode thumbnail | Deferred | Needs safe upload/storage and permissions. Provider posters and previews already work. |
| Manually uploading your own posters and backgrounds | Deferred | Needs safe upload/storage and permissions. Provider posters and previews already work. |
| Editing metadata: title, description, genres, tags | Partly existing | Manual identification and locking the provider link are supported; free editing of fields/tags is not. |
| Subtitles, posters and nfo in the folder next to the file as a preferred source | Deferred | Needs a Kodi/Emby-compatible parser and precedence rules; export must not change media folders unintentionally. |
| Live folder watching (inotify / FSEvents / ReadDirectoryChanges) instead of manual scanning | Deferred | Network and removable drives need a unified scheduler with debounce/backoff; currently manual incremental and full scans are verified. |
| Scheduled automatic scans (e.g. every night at 3:00) | Deferred | Network and removable drives need a unified scheduler with debounce/backoff; currently manual incremental and full scans are verified. |
| Automatically detecting a newly connected drive | Deferred | Network and removable drives need a unified scheduler with debounce/backoff; currently manual incremental and full scans are verified. |
| A report of unrecognized files ("Needs attention") | Implemented | Administrator diagnostics show up to 100 files for review with reasons. |
| Storage use per library, the longest and largest films | Partly implemented | Diagnostics show the total size and the 10 largest files. A per-library breakdown and the longest films are not yet there. |
| A list of missing episodes for all series in one place (Wanted) | Partly existing | A single series' seasons/catalogue already mark missing episodes; a global Wanted list and export are not implemented. |
| Detecting lower quality (e.g. 480p, CAM) with an upgrade warning | Deferred | Needs a read technical index of all files; names such as CAM/480p are not reliable proof of quality by themselves. |

## Subtitles

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| Downloading subtitles from OpenSubtitles straight into the folder next to the file | Partly existing | Search and local tracks exist. Downloading requires provider download/login rights and an explicit policy for writing next to the file. |
| Automatically downloading missing subtitles in the desired language | Deferred | Needs a supported provider API, match verification and a confirmed write policy for media folders. |
| Podnapisi.net support (handy for Slovenian subtitles) | Deferred | Needs a supported provider API, match verification and a confirmed write policy for media folders. |
| Scoring subtitle matches by file hash | Deferred | Needs a supported provider API, match verification and a confirmed write policy for media folders. |
| Automatic subtitle synchronization (e.g. ffsubsync or alass) | Partly implemented | Local Whisper compares timestamped speech with the subtitle text, estimates offset/drift when there are enough matches, and supports cancel, re-measure and undo. Originals stay unchanged; translated subtitles and exact-release matching remain separate work. |
| Converting SRT/ASS/SSA to WebVTT with the style kept | Partly existing | FFmpeg converts the text to WebVTT; advanced ASS styling/animations are not fully preserved. |
| ASS animation support (e.g. JASSUB) for anime | Deferred | A new adapter, identities, cache, subscriptions and rules for resolving conflicting results are a separate development package. |
| Manually uploading subtitles during playback | Deferred | Needs a bounded upload, validation, per-profile permissions and separate storage. |
| Generating subtitles locally with Whisper when none exist | Deferred | Needs large local models, resource management and checks of translation quality; we add no hidden model downloads. |
| Translating subtitles into Slovenian with a local model | Deferred | Needs large local models, resource management and checks of translation quality; we add no hidden model downloads. |

## Users, profiles and security

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| Profile avatars and colors (a choice from a gallery) | Partly existing | Profiles have colored initials; a gallery/upload is not implemented. |
| A kids profile with a limited library and no settings | Deferred | A security-critical package: needs reliable age ratings and server-side restriction of all lists, metadata, streams and profile switches. Hiding things in the UI would create a false sense of protection. |
| A viewing time limit (e.g. 2 hours/day for a child) | Deferred | Needs a daily usage log and a server-side playback limit, not just a counter in the browser. |
| An activity log per user (who watched what) | Existing | The administrator sees active and recent profiles/playbacks; this is not an immutable audit log. |
| Two-factor authentication (TOTP) | Deferred | An authentication package with credential storage/verification, expiry/revocation and recovery. We do not ship an unverified security mechanism. |
| Quick Connect: sign in on a TV with a 6-digit code from a phone | Deferred | An authentication package with credential storage/verification, expiry/revocation and recovery. We do not ship an unverified security mechanism. |
| Invitations for new users with a link | Deferred | An authentication package with credential storage/verification, expiry/revocation and recovery. We do not ship an unverified security mechanism. |
| Per-library permissions: who sees which | Deferred | A security-critical package: needs reliable age ratings and server-side restriction of all lists, metadata, streams and profile switches. Hiding things in the UI would create a false sense of protection. |
| A limit on simultaneous streams per user | Partly implemented | HLS and progressive conversions share a global number of slots (the owner sets 1–8, default 4); HLS has per-profile ownership. A queue and per-user quotas are not implemented. |
| A bitrate limit for remote access | Partly existing | Manual quality and ABR allow a lower bitrate. Server-side local/remote classification and a hard quota are not implemented. |
| SSO/OIDC (Authelia, Authentik, Google) and LDAP | Deferred | An authentication package with credential storage/verification, expiry/revocation and recovery. We do not ship an unverified security mechanism. |
| API keys for integrations | Deferred | An authentication package with credential storage/verification, expiry/revocation and recovery. We do not ship an unverified security mechanism. |
| A list of active sessions with sign-out of other devices | Implemented | A profile sees its account's sign-in sessions and can sign out other devices. |
| Sign-in with passkeys (WebAuthn / passkeys) | Deferred | An authentication package with credential storage/verification, expiry/revocation and recovery. We do not ship an unverified security mechanism. |
| Brute-force protection at the IP level, not just per account | Partly existing | Account and PIN limits exist. An IP limit needs correctly configured proxy trust. |
| An administrator audit log | Deferred | Needs defined events, bounded retention and integrity; the current scan/activity view is not an audit log. |
| Automatic SQLite database backup with restore from the interface | Partly documented | Instructions for backup/restore with the server stopped. Automatic online backup and UI restore need a safe plan for WAL and rollback. |

## Social features

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| Watch Together / SyncPlay: synchronized watching with several users and chat | Deferred | A standalone social package: new events/permissions and persistent history. External channels need accounts and settings; a per-profile watchlist already exists. |
| Recommendations between users ("Anja watched this") | Deferred | A standalone social package: new events/permissions and persistent history. External channels need accounts and settings; a per-profile watchlist already exists. |
| Ratings (stars or a like) per profile | Deferred | A standalone social package: new events/permissions and persistent history. External channels need accounts and settings; a per-profile watchlist already exists. |
| Content requests ("Please add this film") that the administrator sees | Deferred | A standalone social package: new events/permissions and persistent history. External channels need accounts and settings; a per-profile watchlist already exists. |
| Shared watch lists (e.g. a family one) | Deferred | A standalone social package: new events/permissions and persistent history. External channels need accounts and settings; a per-profile watchlist already exists. |
| Notifications: new episode, new film, request fulfilled (push, email, Discord, Telegram) | Deferred | A standalone social package: new events/permissions and persistent history. External channels need accounts and settings; a per-profile watchlist already exists. |
| Private notes on a film | Deferred | A standalone social package: new events/permissions and persistent history. External channels need accounts and settings; a per-profile watchlist already exists. |
| A yearly summary ("Your year in film", like Spotify Wrapped) | Deferred | A standalone social package: new events/permissions and persistent history. External channels need accounts and settings; a per-profile watchlist already exists. |

## Integrations and automation

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| Trakt sync (scrobble, watchlist, ratings) | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |
| Letterboxd export and import | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |
| Sonarr / Radarr / Prowlarr and Overseerr / Jellyseerr connections | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |
| Webhooks on events (playback start, new item, error) | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |
| Home Assistant integration (e.g. dim the lights during playback) | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |
| Discord Rich Presence | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |
| Tautulli-style statistics (most watched, hours, devices) | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |
| Prometheus/Grafana metrics (CPU, streams, transcoding) | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |
| OpenAPI/Swagger documentation for the API | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |
| A plugin system or script hooks | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |
| External players: opening in VLC/mpv, M3U export for IPTV apps | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |
| Partial Jellyfin API compatibility, which would open many existing apps | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |

## Conversion and performance

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| A conversion cache (do not transcode the same film several times) | Deferred | Conversions are tied to the user's track/seek/quality. A persistent cache needs quotas, invalidation and a job life cycle. |
| Pre-conversion ("Optimized versions") at night | Deferred | Conversions are tied to the user's track/seek/quality. A persistent cache needs quotas, invalidation and a job life cycle. |
| A limit on simultaneous transcodes and a queue | Partly implemented | HLS and progressive conversions share a global number of slots (the owner sets 1–8, default 4); HLS has per-profile ownership. A queue and per-user quotas are not implemented. |
| Hardware acceleration detection: NVENC, QSV, VAAPI, VideoToolbox, AMF with an automatic test | Partly existing | Listing and a smoke test of VideoToolbox/NVENC/QSV with a libx264 fallback exist. VAAPI/AMF need a physical target environment. |
| Direct Stream: only a container change (MKV → fMP4), without re-encoding video | Deferred | Needs capability negotiation and a separate remux plan; the current verified common targets are H.264/AAC and the original compatible MP4. |
| HEVC/AV1 direct play where the browser supports it | Deferred | Needs capability negotiation and a separate remux plan; the current verified common targets are H.264/AAC and the original compatible MP4. |
| 4K with HEVC where the client is capable | Deferred | Needs capability negotiation and a separate remux plan; the current verified common targets are H.264/AAC and the original compatible MP4. |
| Checking browser compatibility (MediaCapabilities API) before choosing the mode | Deferred | Needs capability negotiation and a separate remux plan; the current verified common targets are H.264/AAC and the original compatible MP4. |
| Smart caching of thumbnails and posters (WebP/AVIF, different sizes) | Partly implemented | Posters are lazy-loaded; new JPEG previews have a bounded server cache. A WebP/AVIF pipeline does not exist yet. |
| Lazy loading and a virtualized list for libraries with thousands of titles | Partly existing | Posters load lazily; virtualizing the whole list needs measurement and a change to focus/navigation. |
| An index for fast search (SQLite FTS5) | Deferred | A normalized content index and a paginated API are needed first; the existing search runs in the client. |
| Measuring disk speed at the first playback with advice | Deferred | Automatically reading large files during playback would compete with the stream; an explicit diagnostic test is needed. |

## Download and offline

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| Downloading a file for offline use (with permission) | Deferred | Needs permissions, quotas, safe on-device storage and version reconciliation. A PWA by itself does not mean offline videos. |
| Mobile Sync: a pre-prepared smaller version for travel | Deferred | Needs permissions, quotas, safe on-device storage and version reconciliation. A PWA by itself does not mean offline videos. |
| Exporting a list with per-device synchronization | Deferred | Needs permissions, quotas, safe on-device storage and version reconciliation. A PWA by itself does not mean offline videos. |

## Interface and accessibility

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| Light and dark themes, and automatic by system | Deferred | Colors are still partly written directly in CSS; they must first be converted systematically to semantic tokens. |
| Slovenian, English and other localizations (i18n) | Deferred | The interface has a lot of inline HTML/JS text. All strings must be centralized, then localized fully. The documentation and interface are currently English only. |
| TV mode with arrow-key and remote navigation (10-foot UI) | Partly implemented | Cards are focusable and support Enter/Space. Full spatial navigation and a 10-foot layout need testing on a TV. |
| Accessibility: ARIA labels, focus, contrast, screen readers | Partly implemented | Control labels, focus, keyboard, status messages and reduced motion; this is not a full WCAG audit. |
| Audio descriptions, if available as a track | Partly implemented | The selector shows the language/title/channels of all audio tracks, including audio description when labeled as such. |
| Hover cards with a short description | Deferred | Stable data and verified playback come first. The existing load/error statuses remain. |
| Skeleton loading instead of empty pages | Deferred | Stable data and verified playback come first. The existing load/error statuses remain. |
| Clients for Android TV, Apple TV, Fire TV, LG/Samsung (or at least a Capacitor/Tauri package) | Deferred | Standalone native projects need packaging, signing, updates and devices. An installable web app is implemented. |
| A desktop app (Tauri) with a built-in mpv for direct play without browser limits | Deferred | Standalone native projects need packaging, signing, updates and devices. An installable web app is implemented. |
| Card colors from the poster (ColorThief) | Deferred | Decorative; extracting colors from remote posters needs a safe CORS/cache pipeline. |
| Detecting at startup whether the server is reachable, with a clear error description | Implemented | A stable startup warning with retry and a PWA offline screen; no endless reload loop. |

## Administration and maintenance

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| A dashboard: CPU, RAM, disk, active streams, FFmpeg processes | Partly existing | Active streams and per-profile activity; health/library data added. CPU/RAM/FFmpeg system metrics are not there yet. |
| Viewing logs in the interface with search and filters | Partly existing | The scan log and server-health diagnostics; a global server log viewer with search does not exist yet. |
| Health checks: missing ffmpeg, TMDB token, folder accessibility | Implemented | Check server health in Settings reports FFmpeg, ffprobe, the TMDB configuration and the state of the library folders; the first-run guide also checks FFmpeg and folders. |
| Automatic updates or a notice about a new version | Deferred | Needs versioned releases, signing and rollback. Docker has a documented manual update. |
| A Docker image and a docker-compose example | Implemented | A multi-stage Dockerfile and Compose with an unprivileged user, read-only media and a persistent data volume; the image is built and run in verification, and a GitHub Actions workflow can publish it to GitHub Container Registry on a version tag. Nothing is published automatically. |
| An installation wizard on first start (TMDB key, libraries, language) | Implemented | An animated first-run guide for the owner covers folders, conversion folder, memory and graphics, the first scan, accounts, both API keys and remote access, with skip warnings and in-context key help. The interface language is English. |
| A service (systemd / launchd / Windows service) with instructions | Partly documented | Native service instructions and a systemd example; an automatic installer for all operating systems does not exist yet. |
| Importing history from Plex, Jellyfin and Trakt | Deferred | Needs a documented integration contract, authorization, migrations and tests against the target system. We do not expose media URLs without authorization. |
| Exporting and importing settings | Deferred | Needs a versioned format, validation of provider identities and merge rules that do not overwrite users. |
| A quiet low-priority scan mode for slow drives | Deferred | Needs a platform I/O scheduler and measurements. Manual cancel/incremental scans already exist. |

## Features that set PlexV2 apart from Plex

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| "Previously on": a spoiler-free summary of the previous episode, local or via an API | Deferred | Needs a licensed/obtained source of summaries and verifiable spoiler protection; we do not invent episode content. |
| Detecting duplicates between libraries with a visual quality comparison | Deferred | The same path/provider is not proof of the same file. Needs cost-bounded hash/signature comparisons and a safe review before merging. |
| Spoiler protection: hide descriptions and thumbnails of unwatched episodes | Deferred | Needs a uniform watched status across catalogues, descriptions, recommendations and images; hiding with CSS alone is not enough. |
| "How many hours you watched" statistics by genre, month and series | Deferred | The last position is not a count of hours actually watched; a persistent event history is needed. |
| Slovenian films: a special shelf with local works and RTV/SFC labels | Deferred | Needs verified countries/languages and RTV/SFC identities; a title or the user's language is not proof of origin. |
| An automatic inventory of missing seasons with export of a shopping or search list | Partly existing | A single series' seasons/catalogue already mark missing episodes; a global Wanted list and export are not implemented. |

## Suggested order of implementation

| Proposal | Assessment | Implementation / reason |
| --- | --- | --- |
| Short term (a few days): audio tracks, mark as watched, keyboard shortcuts, next episode with a card, filters and sorting. | Assessed | The priorities are the source document's recommendation. Implemented packages and deferred integrations are listed one by one above. |
| Medium term: skip intro, trickplay, parental control, adaptive bitrate, subtitle burn-in, Trakt, notifications. | Assessed | The priorities are the source document's recommendation. Implemented packages and deferred integrations are listed one by one above. |
| Long term: HDR tone mapping, Watch Together, TV apps, music and photos, a plugin system, compatibility with the Jellyfin API. | Assessed | The priorities are the source document's recommendation. Implemented packages and deferred integrations are listed one by one above. |
