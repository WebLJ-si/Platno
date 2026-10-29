# Validation · 2026-10-29

## Automated checks

- `cargo test`: **53 passed**. Existing parsing, scanner, cached matching, role/profile/PIN/watchlist checks remain covered, with additional malformed-credential, cookie, PIN, range, Unicode subtitle and FFprobe failure checks.
- `cargo clippy --all-targets -- -D warnings`: passes without warnings.
- `cargo build --release --locked`: succeeds on this Apple Silicon macOS host.
- `node scripts/check-js.cjs`: all inline application JavaScript parses.
- `node --check static/sw.js`: worker parses.
- `node tests/local-grouping.cjs`, `node tests/movie-collections.cjs`, `node tests/playback-strategies.cjs`: pass.
- `node --test tests/player-controls.cjs`: **23 named tests pass** for audio/quality, seeking, play/pause, speed, volume, mute, subtitle delay, progress/beacon, PiP/fullscreen API branches and removed UI. These API-branch tests use controlled browser objects, not physical OS windows.
- `python3 tests/help-renderer.py`: **5 tests pass** for escaping, Unicode, headings, code fences and generated parity.
- `python3 scripts/render-help.py --check`: the bundled public help page matches its canonical guide.
- `node tests/new-features.cjs`: filtering, sorting, language-code aliases, keyboard input guards, next-episode selection, countdown/cancellation/pause behavior, and nonaccumulating subtitle delay pass.
- `python3 scripts/integration.py`: passes **27 additional named API feature groups** alongside the existing real-media checks; passes with an isolated temporary database, generated fictional media, and a separate ephemeral-port server. It checks unauthorized access, direct HTTP byte ranges, multiple audio tracks, all three adaptive HLS renditions, real segment decoding, correct selected audio via decoded frequency, HDR-to-SDR BT.709 output, cached JPEG previews, invalid input rejection, atomic watched/reset, short-video completion, removed-feature 404s and database absence, tool health, profile isolation, HLS ownership/limits, the shared conversion limit, 16 repeated HLS start/stop/cleanup cycles, graceful SIGTERM during active HLS, account-session revocation, and ordinary-user role boundaries.
- `python3 scripts/integration.py --browser`: Google Chrome headless on this host checks login, library navigation, global search after visiting an empty library, ordinary-user status polling, reload, watched/reset UI, 0/1/2-track audio-selector visibility, real HLS playback and audio restart, loading text subtitles and applying delay, shortcuts, stream statistics, close-progress persistence, diagnostics, account sessions, keyboard-accessible cards, desktop/mobile layouts, and the PWA offline fallback and help page. The final run targets the release executable using `PLEXV2_TEST_BINARY`.
- `PLEXV2_MEDIA_DIR=/tmp docker compose config --quiet`: passes.
- `git diff --check`: passes before publishing.

The reproducible full command is `python3 scripts/test.py --browser` with Playwright available. A JSON command report is written to `/tmp/plexv2-test-report.json`. The feature-to-test matrix is in [testing.md](testing.md).

The HTTP/Chrome tests use generated files and fake test-only accounts. They do not touch the user's actual library, database, credentials, or running server. Test servers are terminated and temporary directories removed afterward. `PLEXV2_TEST_BINARY` can select a release binary instead of the default debug executable. Browser screenshots are written under `/tmp` by default.

## Limits

Chrome mobile viewport emulation does not establish support on physical iPhones, Android phones, or TV remotes. Native Safari/Firefox/Edge, fullscreen/PiP OS behavior, and installation as a standalone app have not been verified in this run. The browser-independent strategy tests cover the native-HLS fallback decision, not actual Safari decoding.

HDR tests use a synthetic PQ-tagged source and confirm conversion/decoding/output metadata, not reference-display color accuracy for every HDR master. FFmpeg must include zscale and tonemap. The image-subtitle burn-in proposals remain unimplemented; there is no claim of PGS/VobSub playback.

The local Docker daemon was unavailable. Compose syntax was checked, but the Linux image was not built/run, and hardware acceleration in a container was not tested. The Caddy HTTPS example was reviewed against official documentation, not deployed with a live domain/certificate. External metadata/subtitle integrations were not tested against paid/authenticated provider accounts.

No live process on the user's existing port was stopped or replaced during testing. Restart the existing server with the newly built executable to load the compiled-in HTML and routes. This avoids interrupting an unknown existing scan or playback session.

Manual skip markers, chapter navigation and local playback reports have been removed at user request. Their former routes return 404, fresh databases no longer create their tables, and no UI references remain. Audio selection remains available only for multi-track media. Local speech-based text-subtitle alignment is implemented; exact-release discovery, translated-subtitle alignment and maintainer issue work remain in [TODO.md](../TODO.md).

See [feature-review.md](feature-review.md) for an explicit status for every proposal, including deferred larger modules and partial implementations. No claims are made for parental restrictions, native clients, social synchronization, offline videos, or absent provider integrations.

The reliability review also exercised simulated SQLite failures and rollback, concurrent failed-login counters, locked identification during scanning, cancellation and reaping of child processes, bounded process output, bounded scan error history, preview cache eviction, stale authentication cleanup and reconnection of a media path. This was a short automated run, not a multi-day soak, production load test or fault injection against a real network filesystem.

## Speech and compatibility validation — 2026-10-29

The complete `scripts/test.py --browser --speech` run passed: 58 Rust tests, strict Clippy, 27 Node control tests, syntax/help checks, the existing HTTP/API/Chrome suite and real Whisper inference. New fixtures exercise decoded HLS and progressive output for MPEG-4/PCM AVI, FLV/MP3, VP9/Opus WebM, 10-bit H.264 MP4, and odd-dimension silent FFV1. The speech fixture verifies offset recovery, at least three matching anchors, preserved subtitle bytes, signature-based cache reuse/invalidation and subtitle allowlist rejection. A separate real Chrome run verifies automatic sync changes actual cue timestamps, undo restores them, cancellation preserves timing, and compatibility/mobile controls work.

Long-video rate correction is covered by synthetic anchor tests; a real long-video drift corpus, translated dialogue, all languages, damaged/DRM files, Docker speech-engine installation and physical/native devices were not tested. Passing these fixtures does not establish universal codec support.
