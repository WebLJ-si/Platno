# Smart Media Scanner

A local Rust media library. It discovers video files, identifies shows and films, and plays them in a browser without modifying the media files.

## Start

Install [Rust](https://www.rust-lang.org/tools/install), including Cargo, on macOS, Windows, or Linux. A working C compiler is needed to build bundled SQLite (Xcode Command Line Tools on macOS; Visual Studio C++ Build Tools on Windows; a compiler toolchain on Linux). From this directory:

```sh
cargo run --release
```

Open **http://127.0.0.1:8787**. Choose **Scan & settings**, click **Browse folders…** on macOS to choose folders, or enter one absolute directory path per line, and start the scan. On Windows use paths such as `D:\Media\TV`. Leave the terminal running. On Windows and Linux, enter the paths manually. The first build downloads dependencies. TVmaze works without credentials; an internet connection is required for uncached metadata.

### Accounts and profiles

On first launch, create the server owner account with an email address in the browser; all accounts sign in by email. Passwords are stored as salted PBKDF2 hashes; sessions use random opaque cookies whose hashes are stored in SQLite. The owner can create administrators and users, and can change any account’s email or password from the activity page. Administrators can create users and view activity, while only the owner can choose critical server settings. Each account can add profiles, with playback history and playback preferences saved separately for each profile. Existing watch progress is assigned to the owner’s initial Default profile during setup. Do not expose the service to the internet without HTTPS and a trusted reverse proxy. For LAN or Docker deployment, bind with `SMART_MEDIA_HOST=0.0.0.0`; set `SMART_MEDIA_COOKIE_SECURE=1` when HTTPS terminates at a reverse proxy. Docker and HTTPS deployment examples are in [docs/deployment.md](docs/deployment.md). If the owner forgets the password, stop the service and run `./target/release/smart-media-scanner reset-owner-password` in a macOS/Linux terminal; it prompts for the replacement owner email and password without echoing the password. Set `SMART_MEDIA_DATA` the same way as the server if it uses a custom data directory. The command signs out existing owner sessions.

### Movie metadata

Create a [TMDB account and API credentials](https://developer.themoviedb.org/docs/getting-started), then copy your API **Read Access Token** (not the v3 API key). Set it before launching, without committing it or putting it in source files:

```sh
# macOS / Linux
export TMDB_TOKEN='your-read-access-token'
cargo run --release
```

```powershell
# Windows PowerShell
$env:TMDB_TOKEN = 'your-read-access-token'
cargo run --release
```

Alternatively, save the token in a private file named `tmdb-token` in the application data directory (restrict its permissions to your account). The environment variable takes precedence. The token is not stored in the browser or database. See `.env.example` for the variable names; the program does not load `.env` automatically. After adding a token, rescan the same folders: movies previously left unidentified because the token was missing are retried automatically. TMDB searches transmit the parsed title; TVmaze searches transmit the inferred show title. Neither provider receives the media path or video content.

## Playback

Install FFmpeg and ffprobe and make them available on `PATH` (on macOS, `brew install ffmpeg`). The application also checks the standard Homebrew locations. Optional `SMART_MEDIA_FFMPEG` and `SMART_MEDIA_FFPROBE` environment variables can specify the binaries. Choose a movie or a local file and press Play; the search view, collections, and episode list also offer file-level play buttons.

H.264/AAC MP4 files are sent directly using HTTP byte ranges; other formats are transcoded on demand to H.264/AAC HLS using FFmpeg. The player prefers Media Source Extensions with the locally bundled HLS.js library and falls back to a browser's native HLS support where needed. The first playlist is held until it contains three segments, or the conversion finishes for a short video, to avoid asking a browser to decode an incomplete stream. Slow input waits up to two minutes for the first playable playlist; HLS.js waits up to three minutes per playlist or segment and retries recoverable network failures. The player shows elapsed wait time while the stream is starting. HLS sessions write temporary segments to the system temporary directory and are cleaned up when stopped or inactive. If direct playback fails in a browser, choose **Use compatible playback**. Use the seek bar when transcoding; the server starts a new conversion at the selected position. Playback progress and completion history are stored in SQLite per profile, so Continue Watching works after refreshes and across browsers. The dashboard recommends local movies or shows using shared words in their provider descriptions, based on items in your watch history; recommendations stay local and do not send viewing history to metadata providers. Only files already in the SQLite library may be streamed; source media is never changed. The player supports season and episode navigation for identified TV episodes, playback speed, 1080p/720p/480p quality choices, and embedded text or matching local text subtitles. Auto uses direct playback for supported originals and otherwise serves a master HLS playlist with 480p, 720p, and 1080p renditions. HLS.js or native HLS chooses the rendition; fixed quality remains available. Adaptive HLS encodes all three renditions concurrently, so it costs more CPU than fixed quality. The legacy progressive fallback converts to 1080p. The owner can select an H.264 encoder reported by FFmpeg. Each new conversion smoke-tests the selected encoder and falls back to `libx264` when it cannot initialize; GPU and device access still depend on the host and Docker runtime. The player offers separate audio/subtitle selectors, chapter navigation, preferred audio language per profile, two simultaneous text subtitle tracks, subtitle delay, and saved volume/speed. PQ/HLG sources are tone-mapped to SDR during conversion using FFmpeg zscale and tonemap filters; a build without those filters cannot convert HDR correctly. Image-based subtitles and Jellyfin client compatibility remain unsupported.

The playback design follows Jellyfin's documented direct-play/transcoding model but does not incorporate Jellyfin source code. Jellyfin is a separate GPL-licensed application; FFmpeg is an external executable with its own license.

## Data and operation

SQLite is created and initialized on startup in the operating system's local application data directory under `smart-media-scanner/library.sqlite3`. Override this directory with `SMART_MEDIA_DATA`. It must be outside your media directories. To reset the application, stop it and remove or back up that directory; doing so also removes manual decisions and cached metadata. Paths are stored locally. Scan roots are retained in browser local storage. No artwork is written to media folders. Symlinks are not followed; permission errors appear in scan status. An inaccessible root never erases prior records. The page embeds a read-only snapshot of the SQLite library and cached movie collections at load time, so a refresh immediately shows saved results without scanning again. The current view is retained in the browser tab. Availability checks run in the background. Until checked, availability is unknown. Missing and temporarily inaccessible paths are both shown as unavailable, not as confirmed deletions.

The scan runs as a background server task and continues when the browser page is closed or refreshed, as long as the server is running. Scan status shows elapsed time, the current file and action, and the most recent 100 activity events. Cancel stops during discovery or between files; an in-progress metadata request may need to finish first. An unchanged file is not reprocessed; manual corrections stay locked even if a file changes. The scanner caches successful API responses for seven days and falls back to older cache on failure. TVmaze and TMDB are queried with timeouts and retries; only necessary title searches and IDs are sent. API content and artwork remain subject to the providers' terms: [TVmaze API](https://www.tvmaze.com/api), [TMDB API](https://developer.themoviedb.org/docs/getting-started).

## Matching and review

The parser handles `S01E02`, `1x02`, `Episode 2` inside a season folder, compact numbers such as `714` when the folder confirms season 7, multiple episodes such as `S01E01+E02`, ranges such as `S01E01-E03`, release years, and common release tags. It uses folder names where filenames lack show names. If the full filename title yields no TVmaze search results, the scanner can retry a corroborating, shorter parent-folder title; the selected episode must still match the provider catalogue. TVmaze returns fuzzy show candidates; the application compares normalized titles, requires a leading candidate margin, verifies numbered episodes in the aired catalogue, and only accepts an episode-title match automatically if its normalized title is unique. A unique episode title can identify a different position in the selected TVmaze aired catalogue when the filename uses another ordering; the original parsed number remains available in the review details. Automatic matches with confidence above 40% are applied when episode identification is supported; matches at or below 40%, missing provider matches, duplicate episode titles, conflicting numbering, and unavailable catalogues remain unverified regardless of score. The low threshold can still accept incorrect matches. Similarity scores are heuristics, not probabilities; select a candidate or enter a provider ID, season and episode, then save to lock the correction. You can review already identified files from their details. Local subtitle files sharing the video basename are listed.

Series detail compares matches above 40% or manually locked, currently available files against TVmaze's aired-order catalogue. Unavailable files are distinct from missing, unknown air dates are uncertain, future episodes are upcoming, and season 0 specials are excluded. Catalogues can be incomplete; stale cached catalogues are marked unverified. Multiple files assigned to one episode are shown as duplicates. The display is an inventory based on provider data, not proof of content or a calibrated accuracy estimate. Numbered entries in a classic complete collection can be identified as standalone animated films through TMDB when exact title, character context, animation genre and short runtime all agree; these appear in Movies.

The Movies view groups films by TMDB collection identity when at least two films from a collection are present; TMDB lookups run in the background and are cached. Locally identified numbered movie collections also appear as one card, including files without metadata. Other movies remain individual cards.

The **Collections** view also groups numbered filenames that share a specific folder, including matched and unmatched items. This is a local, unverified display grouping: numbers indicate filename order, not a verified episode catalogue or missing items. Unmatched files remain browsable in their associated show, local collection, or local folder; individual identification can still be edited.

## Current limitations

This is a first implementation, not the full specification in the original brief:

- TV uses TVmaze only and movie searches use TMDB only. There is no interchangeable adapter interface, alternative episode order, language selection, AKA lookup, cross-provider conflict resolution, or configurable thresholds. A title conflict is flagged but alternative DVD mapping is manual.
- Title-only episode matching is exact after normalization; numbered episodes can also match a minor title variation when the same numbered catalogue entry is clearly the best match; there is no fuzzy episode-title matching, neighboring-folder inference, embedded identification metadata, anime absolute numbering, date matching, split-file detection, or season-range inference. False matches and missed matches remain possible. A file without a year or episode number is treated as a potential TV episode, so movies without a year require review.
- There is no background metadata refresh for unchanged files or unlock button, migration system beyond initial schema creation, configurable cache policy, detailed scoring breakdown, or robust deletion classification. Unavailable does not distinguish deletion from an offline drive. A locked record can be edited again in review.
- Settings manage accounts and provider tokens, but there is no native folder picker on Windows or Linux, episode still/backdrop display, missing-season dashboard metric, season art, special toggle, scan ETA, or complete advanced review workflow. Movie artwork depends on TMDB configuration availability. Local subtitles are associated by basename; embedded playback tracks are read by FFprobe.
- Scans are sequential and folder reads and SQLite calls are synchronous; very large or slow network libraries may take time. Provider rate limiting is handled reactively by retry and backoff, with no proactive request pacing. Some errors are shown on the file's review entry rather than in scan status.
- An accessible catalogue is not a guarantee of completeness. Results based on stale catalogue cache are labeled unverified. Manual assignments are trusted, so verify provider and numbering before saving.

## New playback and library controls

- Select an audio language/track and chapter in the player. Changing the audio track restarts conversion at the current position.
- Use Space/K to play/pause, arrows or J/L to seek, F for fullscreen, M to mute, C to cycle subtitles, and Shift+N for the next local available episode. Ctrl/Cmd+K focuses library search. The PiP button uses the browser's Picture-in-Picture support.
- Near the end of an episode, the next-episode card appears. Automatic playback follows the profile setting and can be cancelled. The player skips duplicate entries for the current episode and unavailable files.
- Administrators can save per-file intro/recap/credits ranges from **Skip markers** in the player. Exact chapter names Intro, Recap, Credits, Opening credits, and End credits are used when no manual configuration exists. Detection by comparing episode audio is not implemented.
- Hover over the seek bar for a JPEG preview, generated on demand at 10-second intervals. Two jobs and 256 cached frames bound the work; frames are not pre-generated. Slow drives may not return a preview within the 15-second limit.
- Movie/show details offer watched/reset actions for all local files or the selected season; advanced file details have per-file actions. Reset clears saved progress. Status belongs to the active profile.
- Libraries can filter known genre/year/language/rating/runtime and unwatched status, sort titles, and choose a random title within the filters. Cached provider detail supplies extra fields when available. Unknown metadata is excluded from active filters. The added-date sort currently uses the last scan timestamp/mtime, not an immutable first-added date.
- **Stats** shows stream details; **Report** saves a local issue for administrators. Settings includes tool/path health, library size, largest files, files needing attention, recent reports, and account-session revocation. Reports are bounded to 500 and rate-limited per profile; no external telemetry is sent.
- Install the PWA from a supported browser over HTTPS or localhost. Its worker caches only the public offline page/icons/manifest, never authenticated HTML, APIs, artwork, or video. This provides an offline connection notice, not offline media playback.

HLS sessions use random identifiers, belong to their creating profile, and are limited to four concurrent sessions globally. This cap does not cover legacy progressive responses; there is no per-account quota or queue yet. See [the complete review of all 163 proposal entries](docs/feature-review.md) for implemented, partial, existing, and deferred work. In particular parental controls, PGS/VobSub burn-in, external integrations, and native TV clients remain separate work.

## Tests

```sh
cargo test
cargo clippy --all-targets -- -D warnings
node scripts/check-js.cjs
node tests/local-grouping.cjs
node tests/movie-collections.cjs
node tests/playback-strategies.cjs
node tests/new-features.cjs
cargo build
python3 scripts/integration.py
```

The HTTP suite needs Python 3, FFmpeg, and ffprobe. It generates fictional local media, starts a separate server on an ephemeral localhost port with its own temporary SQLite data directory, and checks real decoded HLS audio/video, HDR mapping, thumbnails, validation, profile isolation, roles, session revocation, and conversion limits. It never uses the real server database or media library. No provider credentials are required.

For the optional real Chrome UI suite, make the Node `playwright` package available (for example install it in a temporary tool directory and set `NODE_PATH` to that directory's `node_modules`), install Google Chrome, and run `python3 scripts/integration.py --browser`. It also tests the player, subtitles, diagnostics, keyboard use, and a 390-pixel mobile layout. The screenshot defaults to `/tmp/plexv2-mobile-regression.png`; set `PLEXV2_SCREENSHOT` to override it.

See [validation details and limits](docs/validation.md). These checks do not establish support for every browser/device or every real-world codec/HDR master.
