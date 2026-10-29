# Development and architecture

PlexV2 is one Rust executable with an embedded browser UI. There is no separate frontend build or JavaScript package install for normal operation. Cargo compiles `static/index.html`, help, PWA assets and the bundled HLS.js into the binary. Rebuild and restart after changing them.

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
| `static/vendor/` | Locally bundled HLS.js and its license |
| `docs/user-guide.sl.md` | Canonical user guide, rendered into `static/help.html` |
| `scripts/render-help.py` | Deterministic guide renderer and generated-file drift check |
| `tests/` | Rust-adjacent browser-independent JS and HTTP feature regressions |
| `scripts/integration.py` | Disposable real-media/server/database test harness |
| `scripts/browser.cjs` | Real Chrome UI regressions, used by the isolated harness |
| `scripts/test.py` | One command to build and run the full local validation pipeline |
| `TODO.md` | Explicit future work, including subtitle synchronization and maintainer issue workflow |

## Data and request flow

A scan walks configured roots without following symlinks, parses candidate videos and reuses unchanged records. Provider requests use a SQLite cache. Automatic matches can need review; manually confirmed matches are locked across scans. Persisted media paths are the allowlist for playback, thumbnails and subtitles.

The route guard restores a principal from an opaque cookie whose SHA-256 hash is stored in SQLite. Owner/admin/user restrictions are enforced at API boundaries. A profile is part of an account and has separate preferences, watchlist and progress. A profile PIN is not a parental content restriction.

Library browsing reads the saved catalogue without synchronously probing every mounted drive. Availability is checked separately. A path that cannot be opened is unavailable, not necessarily deleted.

On Play, the browser loads FFprobe metadata (cached by file signature), checks direct compatibility and restores profile progress. H.264/AAC MP4 uses range streaming. Conversion uses the selected audio ordinal and fixed quality or adaptive HLS. HLS jobs have random IDs and profile ownership; HLS and progressive conversions share four global slots. Temporary HLS segments are removed when a session stops or expires.

Auto HLS prepares three renditions concurrently. It is network-adaptive at the client, not a promise that the server can encode every source in real time. The progressive fallback is fixed quality and shares the same conversion slot cap. Tone mapping depends on FFmpeg filters and the source's HDR metadata.

Embedded text subtitles are converted to WebVTT. Local sidecars are selected from the catalogue's allowed basenames rather than arbitrary request paths. Client-side delay is applied from original cue times so adjustments do not accumulate. A second distinct text track uses a separate screen position.

## Current removal decisions

The player audio selector starts hidden and becomes visible only for two or more embedded audio tracks. Chapter navigation, manual/automatic skip-marker UI, and local playback reports were removed on 2026-10-29. Their routes and new-table initialization were removed too. Old databases can retain dormant `playback_markers` or `playback_reports` tables; startup deliberately does not destroy historical user data. No remaining application feature reads or writes them.

Do not reintroduce local reports as a substitute for an open-source maintainer issue workflow. `TODO.md` records requirements for a future opt-in, sanitized workflow and automatic subtitle release matching/offset/drift correction. Neither is implemented.

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
