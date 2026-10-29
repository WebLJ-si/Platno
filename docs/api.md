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

Scanning uses `/api/scan` (POST: roots, optional library_id/full_scan), `/api/cancel` (POST) and `/api/status` (GET). `/api/choose-folder` opens a native macOS chooser and must only be called from explicit user interaction. Identification uses `/api/metadata/search`, `/api/metadata/match` (POST), and `/api/override` (POST). Subtitle lookup uses `/api/subtitles/status` and `/api/subtitles/search`.

Owner-only endpoints are `/api/owner/settings` (GET/POST), `/api/owner/encoders`, `/api/owner/accounts/credentials` (POST), `/api/owner/providers`, `/api/owner/providers/tmdb` (POST), and `/api/owner/providers/opensubtitles` (POST). The legacy `/api/subtitles/key` mutation is owner-only too. Provider status never returns stored credentials. Empty keys at the API boundary clear private files; the UI's empty input keeps the current value by not submitting it.

## Removed endpoints

`/api/playback/markers`, `/api/admin/playback/markers`, `/api/playback/report` and `/api/admin/playback/reports` no longer exist and return 404. There is no automatic subtitle-sync endpoint. Future matching/synchronization and maintainer issue handling are recorded in `TODO.md`.
