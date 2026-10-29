# Development and architecture

PlexV2 is one Rust executable with an embedded browser UI. There is no separate frontend build or JavaScript package install for normal operation. Cargo compiles `static/index.html`, help, the Platno stylesheet/fonts, PWA assets and the bundled HLS.js into the binary. Rebuild and restart after changing them.

## Repository map

| Location | Responsibility |
| --- | --- |
| `src/main.rs` | Startup, SQLite media catalogue, scanner, provider metadata/cache, libraries and route registration |
| `src/auth.rs` | Accounts, PBKDF2 passwords, opaque sessions, role guard, profiles/PINs, settings, watchlist, watch progress and activity |
| `src/parser.rs` | Filename/folder parsing and title normalization |
| `src/playback.rs` | FFprobe metadata, audio selection, ranges, progressive conversion, HDR mapping and text subtitle conversion |
| `src/hls.rs` | HLS job ownership, conversion slots, process/session lifecycle, master/rendition playlists and segment delivery |
| `src/media_features.rs` | Bounded on-demand seek thumbnails |
| `src/diagnostics.rs` | Administrator tool/path health and catalogue statistics |
| `static/index.html` | Library UI, player, controls, rendering and profile preferences |
| `static/platno.css` | Shared Platno design, responsive layouts, navigation, authentication, player and form styling |
| `static/fonts/` | Self-hosted Archivo and JetBrains Mono with their SIL Open Font Licenses |
| `static/vendor/` | Locally bundled HLS.js and its license |
| `docs/user-guide.sl.md` | Canonical user guide, rendered into `static/help.html` |
| `scripts/render-help.py` | Deterministic guide renderer and generated-file drift check |
| `tests/` | Rust-adjacent browser-independent JS and HTTP feature regressions |
| `scripts/integration.py` | Disposable real-media/server/database test harness |
| `scripts/browser.cjs` | Real Chrome UI regressions, used by the isolated harness |
| `scripts/test.py` | One command to build and run the full local validation pipeline |
| `src/subtitle_sync.rs` | Bounded local speech alignment, profile-owned cancellation and signature-keyed timing cache |
| `scripts/setup-subtitle-sync.py` | Explicit, SHA-256 verified Whisper model setup |
| `TODO.md` | Future release matching, translated-subtitle alignment and maintainer issue workflow |

## Data and request flow

A scan walks configured roots without following symlinks, parses candidate videos and reuses unchanged records. Provider requests use a SQLite cache. Automatic matches can need review; manually confirmed matches are locked across scans. Persisted media paths are the allowlist for playback, thumbnails and subtitles.

The route guard restores a principal from an opaque cookie whose SHA-256 hash is stored in SQLite. Owner/admin/user restrictions are enforced at API boundaries. A profile is part of an account and has separate preferences, watchlist and progress. A profile PIN is not a parental content restriction.

Library browsing reads the saved catalogue without synchronously probing every mounted drive. Availability is checked separately. A path that cannot be opened is unavailable, not necessarily deleted.

On Play, the browser loads FFprobe metadata (cached by file signature), checks direct compatibility and restores profile progress. H.264/AAC MP4 uses range streaming. Conversion uses the selected audio ordinal and fixed quality or adaptive HLS. HLS jobs have random IDs and profile ownership; HLS and progressive conversions share four global slots. Temporary HLS segments are removed when a session stops or expires.

Auto HLS prepares three renditions concurrently. It is network-adaptive at the client, not a promise that the server can encode every source in real time. The progressive fallback is fixed quality and shares the same conversion slot cap. Tone mapping depends on FFmpeg filters and the source's HDR metadata.

Embedded text subtitles are converted to WebVTT. Local sidecars are selected from the catalogue's allowed basenames rather than arbitrary request paths. Client-side delay is applied from original cue times so adjustments do not accumulate. A second distinct text track uses a separate screen position.

## Current removal decisions

The player audio selector starts hidden and becomes visible only for two or more embedded audio tracks. Chapter navigation, manual/automatic skip-marker UI, and local playback reports were removed on 2026-10-29. Their routes and new-table initialization were removed too. Old databases can retain dormant `playback_markers` or `playback_reports` tables; startup deliberately does not destroy historical user data. No remaining application feature reads or writes them.

Do not reintroduce local reports as a substitute for an open-source maintainer issue workflow. `TODO.md` records requirements for a future opt-in, sanitized issue workflow and exact-release subtitle discovery. Local speech-based offset/drift correction is implemented in `subtitle_sync.rs`; translated-subtitle alignment remains separate work.

## Environment

| Variable | Meaning |
| --- | --- |
| `SMART_MEDIA_DATA` | Persistent database/private configuration directory, outside media roots |
| `SMART_MEDIA_HOST` | Bind address; defaults to localhost |
| `SMART_MEDIA_PORT` | HTTP port; defaults to 8787 |
| `SMART_MEDIA_COOKIE_SECURE` | `1` adds Secure to the cookie; use only with HTTPS |
| `SMART_MEDIA_FFMPEG` | Optional explicit FFmpeg executable |
| `SMART_MEDIA_FFPROBE` | Optional explicit ffprobe executable |
| `TMDB_TOKEN` | TMDB API Read Access Token; overrides the private token file |
| `OPENSUBTITLES_API_KEY` | OpenSubtitles application key; overrides the private key file |

The application does not automatically load `.env`. The Compose file interpolates its environment separately. Never commit real provider files, cookies, a production database or test output containing private catalogue paths.

## Editing and testing

Use `cargo run` for a local debug server; keep a separate disposable `SMART_MEDIA_DATA` when experimenting. Avoid a second process pointing at a real active catalogue for mutation tests.

Run the complete non-browser pipeline:

```sh
python3 scripts/test.py
```

For Chrome UI checks, install Google Chrome and make the Node `playwright` package resolvable. One isolated package installation option is:

```sh
npm install --prefix /tmp/plexv2-test-tools playwright
export NODE_PATH=/tmp/plexv2-test-tools/node_modules
python3 scripts/test.py --browser
```

The runner executes Rust tests, strict Clippy, script parsing/behavior suites, generated-help drift checks, a locked release build and actual HTTP/FFmpeg regressions. The browser option adds UI flows against the same isolated release executable. A JSON report with command results and timings is written to `/tmp/plexv2-test-report.json` by default; override with `--report`.

Edit `docs/user-guide.sl.md` and then run `python3 scripts/render-help.py`. Commit the generated `static/help.html` too. `--check` fails if the source and bundle drift. The renderer intentionally supports a small, escaped Markdown subset: headings, paragraphs and fenced code; do not add raw HTML expecting it to execute.

See `docs/testing.md` for coverage and limitations. Tests are organized around observable behavior and failure boundaries, not a claim of one test per private function or complete branch coverage.

## Contribution workflow

Keep changes focused. Add a failing regression when fixing a reproducible behavior bug, cover both the authorized action and relevant invalid/unauthorized action, and update public docs when behavior changes. Use synthetic fixtures and cached fictional provider responses instead of a contributor's actual media library.

Use `git diff --check`, inspect staged files, and keep generated artifacts, credentials and private output out of commits. No automatic deployment or restart is performed by the test runner. A successful test build does not update an already running binary.

## Reliability and operational limits

Child processes have bounded captured output and timeouts; cancelled streams schedule termination and wait for child exit before releasing their conversion permit. One HLS reaper manages idle sessions; explicit stop and SIGTERM/SIGINT shut down active HLS children before removing their temporary directories. Direct streams, FFprobe, subtitle conversion, password hashing and preview jobs have separate concurrency limits. The server returns a busy response rather than enqueueing work without bound. Preview files are published atomically; expired authentication attempts and old probe-cache entries are pruned periodically.

Scanner writes and library/account operations use transactions where a partial update would lose associations or invalidate credentials. The scanner rechecks manual locks at the actual write. Availability is rechecked after a minute when file lists are requested; unreachable filesystem calls may still block in the operating system. SQLite access is still synchronous in several async request handlers and uses a busy timeout; heavy database lock contention is an outstanding architecture limitation. Abrupt process or machine termination may leave HLS temporary directories, because graceful cleanup cannot run after SIGKILL or power loss. Service managers and monitored backups remain necessary; this is not proof of multi-day stability.

## Platno interface

The 2026-10-29 redesign follows the supplied Platno HTML references: near-black surfaces, lime accent, condensed Archivo headings, JetBrains Mono metadata, a featured title and four library statistics, and colored title cards when artwork is missing. Reference content is illustrative; displayed titles, counts, progress, and actions use the existing APIs. The stored `amber` accent preference now maps to Platno lime; blue and mint remain available.

The shared stylesheet is served publicly at `/assets/platno.css` and is used by both the authenticated UI and Rust-generated account forms. Fonts are embedded in the executable and served from `/assets/`, so loading the interface makes no external font request. The help stylesheet is generated by `scripts/render-help.py`; edit the renderer rather than the generated HTML. The offline page remains self-contained.

At 760px and below, the sidebar becomes a bottom tab bar, and Libraries opens a dismissible menu above it. Library selection and Escape close that menu; moving across the breakpoint resets its presentation. Use `event.composedPath()` for outside-click detection because navigation re-renders and detaches the original clicked button. Settings get section anchors after rendering, preserving all role-dependent controls. Media queries adapt the existing player controls and detail dialog without changing playback endpoints.

Validate with `node scripts/check-js.cjs`, the existing JavaScript/Rust suites, and `NODE_PATH=<path-to-playwright-node_modules> python3 scripts/integration.py --browser`. The browser suite covers the mobile library menu and horizontal overflow at 320, 390, 768, 1024, and 1920px as well as playback, filters, settings, offline mode, and authentication.

Artwork ambiance: `applyArtworkGlow()` samples a 16×16 anonymous-CORS image in a browser canvas and applies an alpha-weighted average RGB value to `--artwork-color` on the current detail/featured container. A bounded 64-entry promise cache avoids repeated sampling. Unsupported CORS/canvas reads resolve without interrupting artwork display; a low-opacity blurred artwork layer provides a visual fallback. Image callbacks verify the element is still connected and its source is unchanged before applying color. Featured cards combine a blurred cover background with a separate sharp 2:3 poster using `object-fit: contain`; the poster stacks above the copy on phones.

## Local speech synchronization and recovery conversion

`subtitle_sync.rs` validates the library path, subtitle ID and selected audio stream, then decodes one or two 75-second mono 16 kHz WAV samples. A local [whisper.cpp CLI](https://github.com/ggml-org/whisper.cpp/tree/master/examples/cli) supplies timestamped text. Unicode-normalized phrase matches must be distinct, unambiguous, and consistent before a robust offset/rate fit is accepted. It requires three agreeing anchors and a match score of at least 0.72; this score is a heuristic, not a probability. A rate fit requires anchors separated by at least five minutes. The cache is bounded to 256 timing-only entries keyed by profile, media/subtitle signatures, audio selection and model signature. Temporary samples/transcripts use a private temporary directory.

There is one active inference slot; cancellation checks both the profile and client-generated request ID. Child process runtime and output are bounded using `processes::output`; cancellation drops the running future and schedules child termination/reaping. The player retains original cue times, applies offset/rate independently per selected subtitle track, accounts for converted-stream seek offsets and keeps manual delay additive. It reloads subtitle cues around the inverse corrected seek position. Undo suppresses automatic reapplication in the current playback; the profile preference opts out across playback sessions.

FFmpeg input probing/timestamp recovery and stereo AAC output are shared across conversion paths. H.264 10-bit or non-4:2:0 video is not advertised as directly playable. Compatibility mode uses libx264 and output-side seeking, avoiding broken input indexes (including FLV files that emit no frames with input-side `-ss 0`). The player awaits old HLS-session cleanup before starting a replacement, retaining correct behavior at the four-slot conversion limit. See the [FFmpeg codec documentation](https://ffmpeg.org/ffmpeg-codecs.html) for decoder error-handling limits.
