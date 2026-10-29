# Test strategy and coverage matrix

The suite covers current user-visible features, significant API boundaries and non-obvious helper behavior. It does not claim one test for every private function, 100% line coverage, or support for every browser/codec. External providers, physical devices and deployment environments have explicit gaps below.

## Run everything

```sh
python3 scripts/test.py
```

Add `--browser` for Google Chrome UI tests (`scripts/browser.cjs` sets `PW_EXPERIMENTAL_SERVICE_WORKER_NETWORK_EVENTS` so offline emulation reaches the service worker), with the Node `playwright` package available through normal resolution or `NODE_PATH`. See `development.md` for setup. All integration mutation tests use a newly created temporary catalogue and localhost server, generated video/audio/subtitles, fictional accounts and an ephemeral port. Existing production data and the running server are not test targets.

The HTTPS integration test also requires the `openssl` CLI to generate its disposable test certificate.

The runner builds a locked release executable and tests that executable. Each command's status and duration are written to `/tmp/plexv2-test-report.json`; `--report` changes the path. A failed step stops the pipeline and leaves an unsuccessful report. Python assertions must remain enabled: do not run integration tests with `python -O`.

## Layers

- Rust unit/integration tests cover parsing, cached matching, scanning invariants, migrations, credentials, library assignment, range parsing, subtitle timestamps and probe failure handling.
- Existing JS suites cover local/collection grouping, catalogue status, stream strategy and recommendations.
- `tests/new-features.cjs` covers filters, sort, language aliases, input guards, next-episode selection, countdown pause/cancel and subtitle delay.
- `tests/player-controls.cjs` contains named Node tests for player behavior, profile persistence, progress throttling/beacon fallback and removed controls.
- `scripts/integration.py` creates real synthetic media and decodes actual HLS/progressive output. `tests/api_regressions.py` adds named feature groups for the rest of the API, including failure paths and roles.
- `scripts/browser.cjs` drives actual Chrome, observes runtime errors, and uses visible UI for major flows. It verifies 0/1/2 audio-track visibility using three different source files.
- `tests/api-fuzz.py` sends ~2300 malformed requests (bad JSON, wrong types, huge/NUL/Unicode values, NaN/overflow queries) to every route of a disposable server and fails on any HTTP 500, dropped connection, hang or server exit. It skips the native folder chooser, logout and scan routes.
- `tests/self-update.py` starts a copy of the program from a temporary folder against a stub release server on localhost and checks the whole update: owner-only access, check before install, checksum and smoke-test refusals that change nothing, the swap with `.previous` kept, restart inside the same process with accounts intact, and the Docker, read-only and disabled variants. `scripts/package.py`, `scripts/build-deb.py` (installed in a clean Debian) and `install.sh` (including a tampered download) were exercised by hand; the release workflow itself runs on GitHub.
- `tests/title-editor-browser.cjs` (with `--browser`) checks the no-key experience in Chrome: unidentified titles get a name and default picture, series keep their seasons and episodes, and the name, details, cover and episodes can be edited by hand, survive full rescans, and apply to new episodes; viewers cannot edit. Rust tests cover the Wikidata choice rules (ambiguous titles are never guessed), parsing, stable local ids, cover content checks and edit validation; the API regression covers validation, locking and permissions.
- `tests/player-seek-browser.cjs` (with `--browser`) checks the seek bar in Chrome on a direct-play and a converted video: it is usable early, keeps following playback after a seek, and the hover preview label names the frame shown.
- `tests/setup-guide-browser.cjs` (with `--browser`) drives real Chrome through the first-run guide: folder checks, conversion folder refusal, hardware facts, a real scan, accounts, skip warnings on the key steps, remote access, the summary and re-run, the missing-key help dialog (owner and administrator wording, scan warning once per session), owner-only display, Escape and focus containment, reduced motion and 320–1440 px layout.
- `tests/telemetry-forwarding.py` runs a beta server against a stub collector on localhost (plain HTTP is accepted only for loopback endpoints) and checks heartbeat and error forwarding, schema validity against the real collector, rejection of private fields, owner opt-out, stable channel and `SMART_MEDIA_TELEMETRY=off`. The live production relay check (`telemetry-server/tests/live-relay.cjs --live`) remains an explicit manual step.
- `tests/help-renderer.py` checks HTML escaping, Unicode, code fences and generated-page parity. `render-help.py --check` guards against stale bundled help.

## Coverage matrix

| Feature / boundary | Verification | Important cases |
| --- | --- | --- |
| First owner setup | Rust + HTTP | confirmation, invalid credentials, single owner, repeated setup rejection |
| Login | Rust + HTTP + Chrome | valid, invalid, missing account, rate limit and expired-window recovery |
| Password hashing/recovery | Rust | salted round trip, length limits, malformed hashes, Unicode hex rejection, CLI data changes revoke sessions |
| Account roles | Rust + HTTP | owner/admin/user reads and mutations, admin cannot create admin, user cannot scan/edit/manage |
| Account credential changes | Rust + HTTP | confirmation, owner reauthentication, prior login revocation and replacement login |
| Session lifecycle | HTTP + Chrome | own list, current session, other-device revocation, cross-account no-op, logout, expiry |
| Profiles/PIN | Rust + HTTP | name/PIN validation, confirmation, uniqueness, legacy setup, wrong/correct PIN, five-failure block, cross-account rejection |
| Profile preferences | HTTP + Chrome + Node | each supported key, rejected values, finite volume, persistence and profile separation |
| Library management | Rust + HTTP | create/update, duplicate/invalid names, roots, type validation, membership |
| Safe library deletion | HTTP | source file survives, membership reassigned, last-of-type protection, unknown ID |
| Scanner | Rust + HTTP | incremental/full, locked correction preserved, target required, invalid roots/target, empty scan lifecycle, cancel |
| Offline/unavailable roots | Rust + HTTP | prior entries preserved, availability unknown before check, missing-file errors |
| Real-tree scanning | HTTP | Unicode names, deep nesting, ignored non-video files, symlink loops, unreadable folders, unchanged-file skipping, manual lock surviving a full rescan, removed media reported unavailable |
| Manual override validation | Rust + HTTP | unknown kind, non-positive provider ID, control characters, oversized title/season/episodes rejected |
| Temporary HLS cache | Rust | startup removes only idle, correctly named session directories |
| API robustness | HTTP fuzz | no 500s, hangs or crashes for malformed bodies and queries on any route |
| Filename parsing | Rust | movie/year/release tags, year-like titles (Blade Runner 2049, 2001), non-ASCII digits never panic, numeric titles after SxxExx are not ranges, SxxExx, NxNN, ranges, multi-episodes, folder context, extras |
| Provider matching | Rust with cached fiction | exact/fuzzy titles, remakes, episode ambiguity, stale cache and no TMDB key |
| Manual identification | Rust + HTTP + Chrome smoke | verified episode selection, lock persistence, malformed requests, review modal |
| Collection grouping | Rust + JS + HTTP | provider collection dedup, numbered folders, unmatched extras, cached mapping |
| Catalogue status/filtering | JS + Rust | present/missing/uncertain, duplicate/offline/upcoming states, filter consistency |
| Search | Node + Chrome | normalization including Slovenian accents, title search, clearing returns to library |
| Filters and sort | JS + Chrome | year/genre/language/rating/runtime/unwatched, unknown metadata, sorting and original card indices |
| Random title | JS + Chrome | active filters, opens a real matching detail dialog |
| Watchlist | Rust + HTTP + Chrome | add/remove, idempotence, invalid path, profile separation, empty state |
| Watched/reset | HTTP + Chrome | file batches, whole-title UI, completion, reset, invalid batch atomicity |
| Continue/resume | Rust + HTTP + Node + Chrome | stored progress, short-video completion, reset, close/pagehide persistence |
| Recommendations | JS | text overlap, watched title excluded, local-only algorithm |
| Media allowlist | HTTP | info/file/transcode/subtitle/preview refuse arbitrary paths |
| Direct playback | Rust + HTTP + Chrome | bytes, suffix/open ranges, invalid/overflow/multiple ranges, decoder compatibility |
| Audio selection | HTTP + Node + Chrome | hidden while loading/0/1 tracks, visible at 2, valid ordinal, invalid ordinal, decoded frequency confirms second track |
| Quality and audio | Node + Chrome | changing quality preserves alternate track; Auto direct decision respects audio |
| Adaptive HLS | HTTP + Chrome | three playlists, playable segments, actual decode and selected audio |
| Progressive conversion | HTTP | seek offset, H.264/AAC output, real decode |
| HLS session safety | Rust + HTTP | random IDs, profile ownership, stop, expired/unknown IDs, allowlisted names, four shared conversion slots, 16 repeated starts/stops, graceful SIGTERM with active HLS |
| HDR conversion | HTTP | synthetic PQ source, output BT.709, decode succeeds |
| Embedded subtitles | HTTP + Chrome | WebVTT text, invalid stream rejection, two tracks loaded |
| Sidecar subtitles | Rust + HTTP | allowed basename, UTF-8 content, seek retiming, invalid IDs/path input |
| Subtitle delay/style | Node + HTTP + Chrome | positive/negative/range checks, original times retained, zero clamping, preference persistence |
| Playback speed/volume/mute | Node + Chrome | supported speeds, invalid speeds, saved volume, element updates and shortcut |
| Keyboard | JS + Chrome | player actions, ignored input fields/modifiers, global focus, card Enter/Space |
| Next episode | JS | available next, duplicate/multi-episode skip, countdown pause, cancel, manual mode |
| Thumbnail previews | HTTP | JPEG content, cache reuse, invalid time/path; two-job bound is code-reviewed, not stress-measured |
| Playback strategy/retry | JS + Chrome | direct/MSE/native/progressive selection; actual Chrome HLS; slow-open Rust test |
| Stream statistics | Chrome | visible panel with current state |
| Activity dashboard | HTTP | heartbeat, active/off toggle and administrator reads |
| Server health | HTTP + Chrome | tools, paths, catalogue totals and diagnostics UI |
| Provider private storage | HTTP + Rust | status contains no token, private file permissions, missing-key errors; no live provider calls |
| Mobile and accessibility | Chrome | 320/390/768/1024/1920px library widths, mobile library menu open/select/Escape, 390px guide, full-height player, focus, semantic labels, appearance/reduced motion |
| PWA | HTTP + Chrome | assets, manifest, installed worker, offline fallback and return online |
| Help/docs | Python + HTTP + Chrome | generated parity, escaping, public access, readable mobile layout |
| Removed features | Node + HTTP + Chrome | no marker/report/chapter controls or implementations, old routes 404, fresh database lacks removed tables |
| Docker configuration | Manual commands | Compose syntax, a real image build, healthy start, FFmpeg filters, scan and HLS conversion inside the container (arm64); amd64 and the publishing workflow are untested |

## Known gaps

Live TMDB/OpenSubtitles queries are not part of deterministic tests. Cached fictional responses exercise matching without leaking titles/paths or requiring paid credentials. Full external-provider behavior needs separate opt-in integration tests with legitimate credentials and rate limits.

Chrome mobile viewport emulation is not physical iOS/Android/TV validation. Native Safari HLS, Firefox/Edge, OS fullscreen/Picture-in-Picture, hardware encoders, real HDR reference colors, slow network-share throughput and standalone PWA installation are not established by this suite. PiP/fullscreen browser APIs remain feature-detected; existing tests cover the surrounding player, not all operating-system behaviors.

The native macOS folder chooser is intentionally not automatically invoked: it would open a system dialog. Manual identification provider success is exercised with fictional cached Rust fixtures; online search requires real provider availability.

A successful unit or HTTP test is not a performance/load audit. In particular concurrent HLS slot rejection is checked, but long-running CPU/GPU/disk stress and thumbnail contention are not benchmarked.

Historical removed-feature tables in an upgraded database are left unused rather than destructively deleted. Fresh databases no longer create them. The test suite checks fresh-database absence and API removal.

The reliability regressions additionally cover a manual lock arriving during scan identification, database rollback on simulated failures, concurrent login attempt updates, child process time/output limits, cancellation and reaping, preview cache bounds, reconnecting a file, and administrative polling under slow/failing responses. The full run is isolated and does not substitute for a multi-day production soak or device testing.

Speech/conversion extensions: `python3 scripts/test.py --browser --speech` includes `tests/media-compatibility.py --speech`. Without `--speech`, the codec fixtures still run and the real speech stage is explicitly skipped. Actual speech testing requires local whisper-cli and the base model; use macOS say or set PLEXV2_SPEECH_FIXTURE to a clear speech WAV. Tests cover independently selected subtitle corrections, transformed seek coordinates, cancellation ownership, positive/negative offset and rate fits, ambiguous dialogue refusal, real transcription/cache invalidation, and decoded HLS/progressive output across five distinct input cases.

Remote-access coverage: `tests/remote-access.py` starts a disposable server with a temporary trusted test certificate and verifies real TLS hostname/chain validation, Secure/HttpOnly cookies, HSTS, owner permissions, setup blocking, browser-origin rejection, owner disable through the remote connection, restart persistence, cleanup, and failure rollback. It never discovers a real router. Rust tests use an isolated SOAP router to verify finite lease creation/renewal/removal, permanent-lease rejection, and refusal to touch another application's or an expired lease's port. Hardware UPnP, actual WAN reachability, ACME renewal, CGNAT, and router interoperability require deployment testing. `scripts/test.py` includes both regression layers.
