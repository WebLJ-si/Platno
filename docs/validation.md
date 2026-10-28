# Validation · 2026-10-29

## Automated checks

- `cargo test`: **35 passed**. Existing parsing, scanner, cached matching, role/profile/PIN/watchlist checks remain covered, with additional marker-range and HLS filename checks.
- `cargo clippy --all-targets -- -D warnings`: passes without warnings.
- `cargo build --release --locked`: succeeds on this Apple Silicon macOS host.
- `node scripts/check-js.cjs`: all inline application JavaScript parses.
- `node --check static/sw.js`: worker parses.
- `node tests/local-grouping.cjs`, `node tests/movie-collections.cjs`, `node tests/playback-strategies.cjs`: pass.
- `node tests/new-features.cjs`: filtering, sorting, language-code aliases, keyboard input guards, next-episode selection, countdown/cancellation/pause behavior, and nonaccumulating subtitle delay pass.
- `python3 scripts/integration.py`: passes with an isolated temporary database, generated fictional media, and a separate ephemeral-port server. It checks unauthorized access, direct HTTP byte ranges, multiple audio tracks and chapters, all three adaptive HLS renditions, real segment decoding, correct selected audio via decoded frequency, HDR-to-SDR BT.709 output, cached JPEG previews, invalid input rejection, atomic watched/reset, short-video completion, explicit empty marker configuration, report retention/rate limiting, tool health, profile isolation, HLS ownership/limits, account-session revocation, and ordinary-user role boundaries.
- `python3 scripts/integration.py --browser`: Google Chrome headless on this host checks login, library navigation and reload, watched/reset UI, chapter/audio controls, real HLS playback and audio restart, loading text subtitles and applying delay, shortcuts, stream statistics, playback reports, close-progress persistence, diagnostics, account sessions, keyboard-accessible cards, desktop/mobile layouts, and the PWA offline fallback. The final run targets the release executable using `PLEXV2_TEST_BINARY`.
- `PLEXV2_MEDIA_DIR=/tmp docker compose config --quiet`: passes.
- `git diff --check`: passes before publishing.

The HTTP/Chrome tests use generated files and fake test-only accounts. They do not touch the user's actual library, database, credentials, or running server. Test servers are terminated and temporary directories removed afterward. `PLEXV2_TEST_BINARY` can select a release binary instead of the default debug executable. Browser screenshots are written under `/tmp` by default.

## Limits

Chrome mobile viewport emulation does not establish support on physical iPhones, Android phones, or TV remotes. Native Safari/Firefox/Edge, fullscreen/PiP OS behavior, and installation as a standalone app have not been verified in this run. The browser-independent strategy tests cover the native-HLS fallback decision, not actual Safari decoding.

HDR tests use a synthetic PQ-tagged source and confirm conversion/decoding/output metadata, not reference-display color accuracy for every HDR master. FFmpeg must include zscale and tonemap. The image-subtitle burn-in proposals remain unimplemented; there is no claim of PGS/VobSub playback.

The local Docker daemon was unavailable. Compose syntax was checked, but the Linux image was not built/run, and hardware acceleration in a container was not tested. The Caddy HTTPS example was reviewed against official documentation, not deployed with a live domain/certificate. External metadata/subtitle integrations were not tested against paid/authenticated provider accounts.

No live process on the user's existing port was stopped or replaced during testing. Restart the existing server with the newly built executable to load the compiled-in HTML and routes. This avoids interrupting an unknown existing scan or playback session.

See [feature-review.md](feature-review.md) for an explicit status for every proposal, including deferred larger modules and partial implementations. No claims are made for parental restrictions, native clients, social synchronization, offline videos, or absent provider integrations.
