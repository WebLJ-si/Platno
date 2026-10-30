# HTTP API reference

This describes the current internal browser API; it is not a versioned compatibility guarantee or Jellyfin API. JSON mutations use POST and `Content-Type: application/json`. Most endpoints require the same HttpOnly, SameSite session cookie as the UI. There are no bearer API keys. Keep clients same-origin and use HTTPS for remote access.

Responses generally use 400 for invalid values, 401 for missing/expired sessions, 403 for forbidden roles, 404 for absent resources, 409 for conflicts and 429 for limits. Extractor/schema errors can return 422. Some error responses have no JSON body; clients must not assume every error contains `error`.

## Public resources

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/` | Owner setup/sign-in page, or signed-in application snapshot |
| GET | `/help` | Bundled public user guide; no private catalogue data |
| GET | `/manifest.webmanifest`, `/sw.js`, `/offline.html` | PWA resources |
| GET | `/assets/icon-192.png`, `/assets/icon-512.png`, `/assets/hls-1.7.3.js` | Bundled public assets |
| GET | `/api/auth/setup-status` | `{configured: bool}` |
| POST | `/api/auth/setup` | First owner: email, password, confirm_password, optional display_name; rejected once configured |
| POST | `/api/auth/login` | Email/password; sets session cookie on success |

## Account and profile

| Method | Path | Input / purpose |
| --- | --- | --- |
| GET | `/api/auth/me` | Account identity, role, active profile, available profiles and PIN-set state |
| POST | `/api/auth/logout` | Revoke the current cookie/session |
| GET | `/api/auth/sessions` | Own account's sessions with current flag, creation and expiry |
| POST | `/api/auth/sessions` | `{others:true}` or `{id:"session-hash"}`; cannot use this to revoke current session |
| POST | `/api/profiles` | name, pin, confirm_pin; account-scoped unique profile name |
| POST | `/api/auth/profile` | profile_id, pin, optional confirm_pin for a legacy profile lacking a PIN |
| GET / POST | `/api/user/settings` | Read preferences; save one `{key,value}` string pair |
| GET | `/api/server/info` | Public-to-signed-in friendly server name |

Settings accept a fixed allowlist of values: playback quality/speed/volume, audio/subtitle languages, subtitle behavior/style, shelf visibility/limit, automatic next episode, accent and reduced motion. Unknown keys or invalid values are rejected. Settings belong to the active profile.

## Library and personal state

| Method | Path | Input / purpose |
| --- | --- | --- |
| GET | `/api/files` | Saved file records with availability and library IDs |
| GET | `/api/libraries` | Library metadata and auto-sort state |
| GET | `/api/libraries/{id}/files` | Files belonging to one existing library |
| GET | `/api/movie-collections` | Cached/in-progress collection mapping |
| GET | `/api/catalogue/{id}` | TVmaze aired catalogue and cached season artwork |
| GET | `/api/metadata/detail?kind=movie&id=1` | Provider detail for movie or tv |
| GET | `/api/watchlist` | Active profile's saved items |
| POST | `/api/watchlist/toggle` | `{path,enabled}`; enabling requires a catalogue path |
| GET | `/api/playback/progress?path=...` | One profile/path's progress and completion |
| GET | `/api/playback/progress` | Continue/history lists and watched count |
| POST | `/api/playback/progress` | path, position, duration, optional completed |
| GET | `/api/playback/watched` | Active profile's saved completion/position states |
| POST | `/api/playback/watched` | `{paths:[...],action:"watched"}` or `action:"reset"`; validates the complete batch before mutation |
| POST | `/api/playback/activity` | path, position, duration, mode, active; heartbeat used by the admin dashboard |

Fresh databases return an empty library list. `POST /api/admin/libraries` accepts `name`, `kind` (`mixed`, `movie`, `tv`, `video`, `other`), optional `id` to update, and optional `roots`; a successful response includes the saved `id`. Mixed libraries retain movies, series and videos together. An explicit scan `library_id` wins over automatic organization. Shared scans with automatic organization use the most specific configured folder, then existing membership, then a matching type or mixed library. Deleting a library, including the last one, preserves file records and media; memberships move to a compatible remaining library or are removed. No library is seeded or recreated on startup.

Progress positions/durations are finite seconds within the supported 0–86400 range. Reset removes profile progress. It does not delete media, watchlist entries or another profile's history.

## Playback

| Method | Path | Input / purpose |
| --- | --- | --- |
| GET | `/api/playback/info?path=...` | Duration, direct compatibility, codecs, audio_tracks, text subtitles, HDR flag and bitrate; no chapters |
| GET | `/api/playback/file?path=...` | Direct MP4 byte stream, with single HTTP Range support |
| GET | `/api/playback/transcode?path=...` | Progressive conversion; optional start, quality and validated audio ordinal |
| POST | `/api/playback/hls/start` | path, optional start, quality (`auto`, `480`, `720`, `1080`), optional audio ordinal; returns id/url |
| GET | `/api/playback/hls/{id}/{file}` | Creating profile's allowlisted playlist/segment files |
| POST | `/api/playback/hls/stop` | `{id}`; only the creating profile can stop it |
| GET | `/api/playback/subtitle?path=...&subtitle=embedded:3` | Converted WebVTT; subtitle can also be an allowlisted `file:N`, optional start |
| GET | `/api/playback/thumbnail?path=...&time=...` | JPEG preview with bounded work/cache |

Audio IDs are zero-based audio ordinals, not arbitrary FFprobe global stream IDs. Subtitle embedded IDs are validated global stream indices. Clients must use metadata supplied by the info endpoint. A single-track source still exposes its metadata to the API; the browser only shows a selector for two or more tracks.

## Administrator and owner

Administrators and owners can use `/api/admin/accounts` (GET/POST), `/api/admin/dashboard`, `/api/admin/health`, `/api/admin/libraries` (GET/POST), `/api/admin/libraries/delete` (POST), and `/api/admin/library-settings` (GET/POST). An administrator can create users, but cannot create another administrator.

Scanning uses `/api/scan` (POST: roots, optional library_id/full_scan), `/api/cancel` (POST) and `/api/status` (GET). `/api/admin/browse-folders?path=` (GET, administrators) lists the sub-folders of a folder for the in-page folder browser; without a path it opens the first existing quick place (Docker `/media`, mounts, home). Identification uses `/api/metadata/search`, `/api/metadata/match` (POST), and `/api/override` (POST). Subtitle lookup uses `/api/subtitles/status` and `/api/subtitles/search`.

No-key metadata and hand editing (administrator or owner): `POST /api/admin/title` (`{"paths":[...],"title","year","description","genres":[...],"poster"}` — every field optional; applies to all listed files, locks them against rescans; `poster` is a cover URL from the upload endpoint or `""` to remove it), `POST /api/admin/episode` (`{"path","season","episodes":[...],"title"}`), `POST /api/admin/covers` (raw JPEG, PNG or WebP body up to 8 MB, recognised by content; returns `{"id","url"}`), and `GET /api/covers/{name}` (any signed-in user). Without a provider key a title gets `provider: "local"` (named from the file name) or `"wikidata"` (movies, free source) and a numeric `provider_id` far above any TMDB or TVmaze id.

Updates (owner only): `GET /api/owner/update` (installed version, how it was installed, last check result; no network), `POST /api/owner/update/check` (asks GitHub for the latest published release and compares), `POST /api/owner/update/apply` (`{"version"}` must equal the version just found; downloads, verifies the SHA-256 against `SHA256SUMS`, smoke-tests the program with `--version`, swaps it in keeping `<name>.previous`, and restarts in place). Docker, `.deb` and read-only installs answer 409 with the manual command. `SMART_MEDIA_UPDATE_CHECK=off` disables it; `SMART_MEDIA_UPDATE_API` (https, or http on this machine) points it at another release server for testing.

First-run guide support: `GET /api/owner/system` (owner; memory, processor threads, graphics name, H.264 encoders, FFmpeg status, data folder, listen address, recommended and current conversion slots), `POST /api/admin/check-folder` (`{"path"}` → `{"ok","reason"}` with reason `ok`, `missing`, `not_folder`, `unreadable`, `timeout` or `invalid`), `GET /api/admin/provider-status` (`{"tmdb","opensubtitles"}` booleans, never the keys). `POST /api/owner/settings` also accepts `conversion_slots` (1–8) and `setup_guide` (`done`, `skipped`, `reset`). Errors for missing or rejected provider keys carry `code` (`tmdb_key_missing`, `tmdb_key_rejected`, `opensubtitles_key_missing`, `opensubtitles_key_rejected`) so the interface can explain how to add the key.

Owner-only endpoints are `/api/owner/settings` (GET/POST; keys `friendly_name`, `transcode_quality`, `hardware_encoder`, and `transcode_dir` — an absolute, writable folder outside all media folders, or empty to reset; GET also returns `effective_transcode_dir` and `transcode_dir_from_environment`, and `SMART_MEDIA_TRANSCODE_DIR` takes precedence), `/api/owner/encoders`, `/api/owner/accounts/credentials` (POST), `/api/owner/providers`, `/api/owner/providers/tmdb` (POST), and `/api/owner/providers/opensubtitles` (POST). The legacy `/api/subtitles/key` mutation is owner-only too. Provider status never returns stored credentials. Empty keys at the API boundary clear private files; the UI's empty input keeps the current value by not submitting it.

## Removed endpoints

`/api/playback/markers`, `/api/admin/playback/markers`, `/api/playback/report` and `/api/admin/playback/reports` no longer exist and return 404. Audio-based text-subtitle synchronization is available below. Exact-release subtitle discovery and maintainer issue handling remain in `TODO.md`.

## Local speech alignment

- `GET /api/playback/info`: `subtitle_sync.available` reports whether a local whisper-cli/model is detected.
- `POST /api/playback/subtitle-sync`: authenticated body `{request_id, path, subtitle, audio?, position?, refresh?}`. The request ID is 1–64 ASCII letters/digits/hyphens. Paths must be in the catalogue; subtitle IDs and audio ordinals are validated against local tracks. Runs at most one speech job globally, with finite subprocess time/output limits. Returns `{alignment:{offset,scale,confidence,anchors,samples},cached,local:true}`; media time equals subtitle time × scale + offset. Confidence is a match heuristic, not a probability. Rejects insufficient/inconsistent matches with 422 and occupied slots with 429. Source files remain unchanged.
- `POST /api/playback/subtitle-sync/cancel`: body `{request_id}`. Cancels only the matching active job owned by the current profile; returns `{cancelled}`.
- Profile setting `subtitle_auto_sync` accepts `true`/`false`, default true when the runtime is available. Undo is session-local; disabling the preference stops automatic reapplication on later playback.
- HLS start bodies and progressive conversion queries accept `compatible: true`. This forces software encoding and uses output-side seeking; auto HLS is requested as fixed 720p by the player in this mode.

Speech samples/transcripts are private temporary files. The cache contains only timing parameters, keyed by the profile and signatures of the source media, subtitle, audio selection, and speech model. Manual refresh recomputes an existing result.

### Owner remote access

`GET /api/owner/remote-access` reports configuration readiness, HTTPS listener and UPnP lease state. `POST` accepts `{ "enabled": true, "automatic": true }`; use `automatic: false` for manual router rules or `enabled: false` to stop. Requires an owner session and same-origin browser requests. The mapping targets only the configured HTTPS LAN endpoint; the API cannot forward arbitrary ports. `reachable: null` means internet reachability was not independently tested. See [remote access](remote-access.md).

### Optional diagnostic beta

Authenticated `GET /api/telemetry/config` reports availability/channel, recipient hostname and current policy. `POST /api/telemetry/report` accepts only sanitized, bounded enum/numeric diagnostics and queues an upload without blocking playback (202; 204 when disabled; 400 for unsafe fields; 429 for a full queue/rate cap). `POST /api/owner/telemetry` with `{"enabled":false}` disables server/browser reporting and invalidates queued generations; only the owner can change it. Stable/source builds are off by default. The independent collector, schema, retention, privacy notice and deployment requirements are documented in `BETA.md` and `telemetry-server/README.md`.
