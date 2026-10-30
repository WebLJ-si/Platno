# Diagnostic beta: read before downloading

This beta includes **automatic diagnostic reporting** to the Platno Observatory operated by this project's maintainer. The diagnostic beta download and release notes must link to this notice before download. By choosing that clearly labeled beta, you acknowledge the reporting described here. This notice is not a substitute for explaining the behavior in the app; beta startup and the interface display it too.

Source builds and the `private` image send nothing. **While Platno is in beta the official Docker images (`latest`, version tags and `beta`) send technical errors, layout reports and heartbeats automatically**; the owner can switch this off in Settings or with `SMART_MEDIA_TELEMETRY=off`, and each browser can switch off its own reporting. The server owner is told once, in a dialog after signing in, and answers for the whole server (keep it on, or turn it off for everyone); the choice is remembered and can be changed in Settings, Beta diagnostics. Other accounts are never asked and there is no banner. A consent prompt is planned for after the beta. After sign-in, participating browsers also attach a masked interface replay to error reports. No third-party analytics SDK or remote recording script is loaded.

## What is sent

- A random, short-lived session ID, app version, error category, screen category (such as library, player, or settings), code locations (asset/line/column), and HTTP status/route category when relevant.
- App/server process uptime; approximate browser family/version, OS family, CPU thread count, viewport, online state and browser playback capabilities.
- Player technical state, quality/mode, minute-bucket position/duration, and media error code.
- Up to 100 recent interface events and at most two wireframe snapshots: rectangles representing generic buttons, inputs, panels and media placeholders. No original text, attributes, HTML, pictures or actual screen pixels are recorded.
- Layout problems found by the interface itself: the category (page wider than the screen, controls overlapping each other or a title, button text cut off, a control outside the screen), how many elements are involved, and a wireframe of the screen with the involved boxes marked. Only rectangles are sent, never their text, names or pictures.
- Technical failure categories from the server: FFmpeg stopped unexpectedly, a scan error, speech-sync tool failure, plus the existing HTTP 5xx and panic notices. Each is reported at most five times per ten minutes.
- Heartbeats about every five minutes while reporting is active. A missing heartbeat is not proof of downtime: the app might be closed, disconnected, backgrounded, or opted out.

## What is excluded

Names, emails, account/profile IDs, credentials, cookies, provider tokens, media titles, file paths, original URLs/query strings, library text, raw console/error messages, request/response bodies, audio, video, subtitle text, images and form values are excluded. Login/setup pages do not load the recorder. PIN/profile switching dialogs and password/email controls are excluded from the replay. Error messages are classified locally; only fixed categories and safe code locations are sent.

Random IDs reset with the app/browser run and are not a persistent installation identifier. This minimizes identification, but is **not a promise of absolute anonymity**: the hosting/network providers necessarily process connection IP addresses and technical metadata can sometimes be distinctive. The application does not store raw IPs with reports. Its temporary rate-limit keys use a daily rotating keyed hash; those keys expire after one minute. Vercel and the storage provider have their own infrastructure logs and policies.

## Controls and retention

Settings → Beta diagnostics lets each browser disable error reporting or replay separately. Do Not Track and Global Privacy Control disable browser reporting. The server owner can disable all future server/browser reporting in Settings, or start the app with `SMART_MEDIA_TELEMETRY=off`. A browser opt-out does not independently disable the owner's server-level error and uptime reporting. Turning reporting off clears pending replay and invalidates queued uploads; an upload already in flight cannot be recalled.

The owner-only Google dashboard stores reports privately for up to seven days, with a cap of 1,000 retained errors and 500 session summaries. Its list shows the most recent 100 reports and labels those counts as a window, not lifetime totals. The operator can delete reports, export sanitized JSON, or delete retained indexed records. Old records may expire sooner under capacity limits. Public ingestion is rate-limited but, like other anonymous telemetry endpoints, reports can be fabricated and are not trusted security evidence.

## Scope and limits

The reporter captures browser exceptions/rejections, console error categories, failed same-origin scripts, styles and pictures, policy violations, API server failures (5xx, slow requests over 20 seconds, 403/404/408/413/429), playback failures and stalls longer than ten seconds, layout problems, and best-effort Rust panic notifications. It does not guarantee capture of all crashes: power loss, OS termination, aborts/OOM, offline failures, and failures before initialization may be unavailable. It never delays playback for a network upload; queues and upload rates are bounded and failures are dropped without an offline backlog. Uptime means observed process/session uptime, not independent probing of private media servers.

## Building a diagnostic beta

Publish this notice alongside the download, use a clearly labeled diagnostic-beta artifact, and embed only the public collector URL:

```sh
PLATNO_RELEASE_CHANNEL=beta \
PLATNO_TELEMETRY_ENDPOINT=https://platno-observatory.vercel.app/api/ingest \
cargo build --release --locked
```

For an existing source build, set `SMART_MEDIA_RELEASE_CHANNEL=beta` and `SMART_MEDIA_TELEMETRY_ENDPOINT` to your collector's HTTPS `/api/ingest` URL. `SMART_MEDIA_TELEMETRY=off` overrides both. No collector secret or Google credential belongs in the media-server binary. Operator setup and tests live in `telemetry-server/README.md`.
