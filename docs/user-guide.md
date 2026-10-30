# PlexV2: user guide

PlexV2 is a local video library with a web player. The server reads the folders you choose, tries to recognize movies and series, and lets you play them in the browser. It never renames, moves or overwrites your media files while scanning or playing.

## Installing

Download the ready-to-run program for your computer from the [releases page](https://github.com/WebLJ-si/Platno/releases/latest); no Rust or other build tools are needed. On Linux and macOS `curl -fsSL https://github.com/WebLJ-si/Platno/releases/latest/download/install.sh | sh` does it in one line. On Debian and Ubuntu install the `.deb` with `sudo apt install ./platno_VERSION_amd64.deb`; it sets up a service that starts at boot. With Docker use the image `ghcr.io/weblj-si/platno`. To build from source see the reference. Then continue with the first start below, replacing `cargo run --release --locked` by `platno`.

## Updating

The owner opens Settings, Updates, presses Check for updates, and then Install and restart. Platno downloads the official release, checks it against the published checksum, keeps the old program next to it as `platno.previous`, and restarts; accounts and libraries are untouched. Nothing is checked or installed unless the owner presses a button. Docker and Synology installs are updated with `docker compose pull && docker compose up -d`, and `.deb` installs by installing the newer `.deb`; there the Updates panel shows the exact command. `SMART_MEDIA_UPDATE_CHECK=off` removes the feature.

## First start

You need Rust with Cargo, plus FFmpeg and ffprobe that the server can reach. From the project root run:

```sh
cargo run --release --locked
```

Open http://127.0.0.1:8787 and create the owner account. Enter a valid email address, a password of at least 12 characters and the password confirmation. The display name can differ from the email. The server must stay running while you use it.

There is only one owner. The owner or an administrator adds further accounts in Settings. The owner can create an administrator; an administrator can create regular users.

By default the data directory is the system application data directory for smart-media-scanner. SMART_MEDIA_DATA can change it. Do not use a media folder as the data directory.

## Libraries and folders

A new installation starts without libraries. In the setup guide, give your first library a name, choose Automatic · mixed media and add one shared media folder. Movies and series are identified from file names, episode numbers and online matches; camera clips and unmatched videos stay browsable by folder. All of them remain in that one library. Add more libraries only when you want them, either during setup or in Settings. Other available types are Movies, TV shows, Videos and Other.

In the Folders field enter one absolute path per line. The path must exist on the computer or container where the server runs. A path on your phone or another computer is not a path on the server.

Example paths:

```text
/Volumes/Media/Movies
/home/media/TV
D:\Media\Movies
```

Browse opens a folder list in the page: it shows the folders of the computer or container that runs Platno (including drives on Windows and /media in Docker), so it works in any browser on any system. You can also type the path by hand. In the example Docker setup your media folders appear under /media; host paths are not automatically the same inside the container.

For separate collections, create named libraries and assign folders to each. A scan with a selected library puts every file in that library. With “Use library folders” and automatic organization enabled, the most specific matching folder wins; existing memberships are kept when no folder rule matches, then new files use a matching media type or mixed library. No libraries are created automatically. If automatic organization is off, choose a target library.

Any library can be deleted, including the last one. This keeps media, identification and watch history. Membership moves to another library of the same type or a mixed library when one exists; otherwise the files remain in Overview until assigned again. Libraries are not recreated on restart. Upgrades remove only untouched, empty legacy default libraries and preserve configured or populated libraries.

## Scanning

Scan added or changed files checks the selected folders and skips unchanged files. Full library scan rechecks unlocked files. Manually confirmed identifications stay protected in both cases.

Scanning keeps running on the server after you close the tab. Progress, the current file and recent activity appear in the notice at the top of the page. Cancel requests a stop; a network lookup that has already started may finish first.

A disconnected drive does not mean the files are deleted. A folder that cannot be reached does not delete earlier records. After startup, availability can be unknown for a short time until the check finishes.

## Without API keys

Platno works without any key. Series use TVmaze, which needs none. When no TMDB key is saved, movies are looked up on Wikidata, a free source, and matched only when the title (and year, if known) points to exactly one film; they get a name, year, runtime, genres and a short description, but no poster or rating. Anything that cannot be matched keeps the name taken from its file name and a default picture. Seasons and episodes are read from the file names either way, so every file stays in the library.

An administrator or the owner can open any title, press Edit details, and change the name, year, genres and description, upload a cover (JPEG, PNG or WebP up to 8 MB), and fix the season, episode number and title of every file. Edits apply to the whole title and are protected from rescans, and new episodes of an edited show join it. Adding a TMDB key later upgrades movies that were named from Wikidata or file names on the next scan, except those you edited by hand. Set `SMART_MEDIA_WIKIDATA=off` to stop the Wikidata lookups; the title searched is the only thing sent.

## Recognizing movies and series

Series use TVmaze and movies use TMDB. For movies, add a TMDB API Read Access Token in the owner settings. This is not the same as the shorter v3 API key. The token is kept on the server and is never printed back in the form.

For series, names with a season and episode help:

```text
TV/Example Show/Season 01/Example.Show.S01E02.mkv
TV/Example Show/Season 01/Example.Show.S01E03-E04.mkv
Movies/Example Movie (2020)/Example.Movie.2020.mkv
```

A match result is not a guarantee that the content is right. If a title is wrong, open its details or Advanced settings and choose Edit identification. Search for the correct title and confirm the result; for a series, choose the episode in TVmaze order. Confirming locks the choice for later scans.

The series catalogue shows present, missing and uncertain episodes. Green means confirmed present, red a missing episode, and yellow also covers unreachable files, duplicates or any other uncertain status. The All, Missing and Uncertain filters open the matching seasons. A provider's catalogue can be incomplete.

## Browsing, search and filters

Libraries opens the list of libraries in the side menu. In a movie or series library you can filter by year, genre, language, minimum rating, runtime and unwatched. Filters use known data. If runtime or genre is missing, the title does not match an active filter on that value.

Sort supports title, year, rating, Recently added and Shuffle. Recently added currently uses the time of the last scan (mtime), not an unchanging first-added date.

Surprise me picks a random title from the current filtered results. With no results there is nothing to pick. Global search looks through normalized titles and file names or paths; it is not an actor search and is not automatically tolerant of every typing mistake.

Collections also contain local groups inferred from folders and numbered files. Such a group is not by itself a confirmed episode catalogue.

## Watchlist and watch history

In a title's details use Add to Watchlist. The watchlist is stored on the server for the active profile. Remove takes a title off the list; it does not delete the file and does not reset watch history.

Mark movie/series watched marks local files as watched. For a series it covers all local files of the series. Per-file actions also exist in the advanced details.

Mark unwatched / reset is shown only when at least one of the selected local files is marked watched. The button removes the saved position and the watched mark for the selected files. This also affects Continue Watching. Resetting does not touch the history of other profiles.

Continue Watching offers to resume a saved position. When you reopen a video you can choose Continue watching or Start from beginning. The position is saved periodically and sent once more when the player closes or the tab is hidden. If power is lost suddenly, the last second is not guaranteed.

## Playback and quality

The bottom of the player contains the timeline, back 10 seconds, play/pause and forward 10 seconds. Use the Episodes icon at the top to choose a season or episode. In the bottom right corner are volume (hover the speaker for the slider), Picture-in-Picture, the Settings gear and Fullscreen, as in other video players. Settings groups quality, speed, subtitles and synchronization, the audio track and stream statistics, and opens above the corner. The header shows the show, season, episode number and episode title. The controls fade after 5 seconds without mouse movement (not while the pointer is on them or a panel is open), and subtitles stay above them. Escape closes the open panel first; clicking outside the panel also dismisses it.

Play opens the player. Compatible H.264/AAC MP4 files are sent directly using HTTP byte-range requests. Other formats are converted to H.264/AAC when needed. Playback never changes the original.

With HLS conversion, Auto prepares 480p, 720p and 1080p renditions, and the player picks the quality to suit conditions. Three renditions at once use more processor time. On a weak server choose a fixed 480p or 720p. Conversion cannot create detail that is not in the original.

On first sign-in the owner gets a first-run guide: library folders, other locations, memory and graphics (encoder, quality, simultaneous conversions), the first scan, accounts, the TMDB and OpenSubtitles keys, and remote access. Every step can be skipped; for the keys, the guide says which features stay off without them. When you later use a feature that needs a key, the app shows exact instructions for adding it. You can run the guide again in Settings (Setup guide).

The owner can choose the folder for temporary conversion files (and for speech-based subtitle sync) in Settings under Conversion folder, or with the environment variable `SMART_MEDIA_TRANSCODE_DIR`. Use a fast disk with free space; the folder must be outside your media folders so the scanner does not index temporary files. If the variable is set, it takes precedence over the setting.

If direct playback fails, the player tries a compatible conversion. Compatibility mode manually switches to software H.264/AAC conversion at the current position. With Auto it uses a fixed 720p and, when the index is poor, decodes the file from the start up to the chosen time. This can take longer but avoids broken fast seeking. FFmpeg must support the input codec; encrypted or irreparably damaged files cannot be guaranteed to play. A long load can come from a disconnected drive, a network folder or a slow conversion; wait for the message and use Try again.

HLS and progressive conversion share at most four simultaneous slots on the server (the owner can change this number in the first-run guide or in Settings). A busy-slots message means you should close an unused playback or try later. It is not a queue or a per-account limit.

HDR PQ/HLG is mapped to SDR during conversion. FFmpeg must include the zscale and tonemap filters. Correct colors on every real HDR recording or screen are not guaranteed just because a file is marked HDR.

## Audio

The Audio track selector is visible only when the file has at least two audio tracks. It is hidden during the initial information read and with zero or one track.

Several tracks are usually embedded in the same MKV or MP4 file, for example the original, a dub and a commentary. The selector shows language, codec, channels and title when they are recorded. Separate external AC3/AAC files cannot be attached as replacement audio.

Switching audio restarts the conversion at the current position and may load briefly. Preferred audio language in the profile settings selects a matching track if there is one; otherwise the first track is used. Choosing a language does not create missing synchronization.

## Subtitles

The player supports embedded text tracks and detected local SRT, VTT, ASS or SSA files next to the video file. Advanced ASS animations and styling are not fully preserved when converting to WebVTT. Image-based PGS/VobSub subtitles are not supported.

Choose the main track in Subtitles. If more tracks are available you can add Second subtitles; two different tracks are shown at separate heights. The same track is never shown twice.

Subtitle appearance (size, color, background, font, position and text shadow) is in the player settings under Subtitle appearance and in Settings, Profile preferences. It is saved to your profile and applies at once. There is no manual delay field: use Sync subtitles instead.

In the profile settings you choose the default language and mode (off, preferred language or always), plus size, color, background and shadow. The default language only works if the track is labeled well enough.

Settings includes searching local subtitles and OpenSubtitles. For online search the owner sets an application API key. The current feature searches and links to the result at the provider; it does not guarantee automatic download, matching or synchronization.

Automatic detection and correction of out-of-sync subtitles is listed in the TODO. For now use the manual delay or a verified external tool on a copy of the subtitle.

## Keyboard, PiP and next episode

Space or K toggles play/pause, even when a button in the player has focus. Left arrow or J skips back 10 seconds; right arrow or L skips forward 10 seconds. Up and Down arrows change the volume; the number keys 0 to 9 jump to that tenth of the video. F toggles fullscreen, M mutes, C cycles through subtitles, Shift+N opens the next locally available episode. Ctrl+K or Cmd+K focuses the library search.

While you type in input fields, selects and text areas, the player shortcuts do not fire. You can also open library cards with Enter or Space when they have focus.

PiP relies on browser and operating-system support. If there is no support or the video is not ready yet, a message is shown. It is not guaranteed on every device.

At the end of an episode a card for the next local episode appears. If Auto-play next is on in the profile, a ten-second countdown starts. Play now starts immediately; Keep watching cancels the automatic move. The countdown waits while paused. The app does not download missing episodes.

The current player has no manual skip markers or chapter navigation.

## Profiles and accounts

You choose a profile through the avatar. Add profile asks for a name and a PIN of 4 to 8 digits entered twice. Switching to a profile requires its PIN; for an older profile without a PIN, set one first. After five wrong attempts a fifteen-minute lockout follows.

A profile has its own history, watchlist and settings. This is not parental control. A PIN alone does not mean age limits, different library permissions or protection from the account owner.

In Settings under Signed-in devices choose Show sessions. You see the sessions of your own account and can choose Sign out other devices. It does not reveal other accounts' sessions. Sign out signs out the current device.

The owner changes account details in the administrator area. To change their own credentials they must give the current password. Changing a password signs out the existing sessions of that account.

If the owner has forgotten the password, stop the server and run the interactive recovery in a terminal with the same SMART_MEDIA_DATA:

```sh
./target/release/smart-media-scanner reset-owner-password
```

## Diagnostics and troubleshooting

Stats in the player shows the current mode, the reason for conversion, codecs and quality information. The data is meant to help you understand the stream; estimated bitrate or buffer is not a measurement of actual picture quality.

An administrator opens Check server health in Settings: it checks FFmpeg, ffprobe, the TMDB configuration and the library folders, and shows the number and total size of files, the largest files and up to a hundred unrecognized files. Needs attention is not proof that a file is damaged.

Server activity shows active playbacks. The amount of data is an estimate from file size and position, not measured network traffic. The scan notice has a separate log of its progress.

If a video cannot be opened, check the drive connection, the server path and permissions. If a video stutters, try a lower fixed quality and check the computer's load. If the web page cannot be reached, check that the server is running, the port, and access over the home network or VPN.

There is currently no built-in problem report. When reporting on GitHub, describe the steps, the expected and actual behavior, and the versions of your system, browser and FFmpeg. Remove private paths, tokens and account data before posting. A local administrator is not necessarily a developer of the project.

## Mobile devices, PWA and remote access

The web interface adapts to smaller screens; the mobile player uses the full available viewport. Physical phones and TV remotes are not equivalent to testing a resized window in Chrome.

On a supported browser you can install the app from its menu. You need HTTPS or localhost. The PWA caches only the public screen for an unreachable server, the icons and the manifest. Private libraries and video are not stored for offline viewing.

For access from outside the computer use a properly configured HTTPS reverse proxy or a private VPN. SMART_MEDIA_COOKIE_SECURE=1 is for HTTPS; over plain HTTP such cookies prevent normal sign-in. Details are in docs/deployment.md in the repository.

## Updating and backup

After changes to HTML/JS a new Rust build is needed, because the web interface is embedded in the executable. Then stop the old instance and start the new one with the same data settings. Refreshing the browser alone does not load new code into an already running server.

Before a backup stop the server and copy the whole data directory, including the private provider files. While SQLite WAL is live, do not copy only library.sqlite3. Copies contain private data; keep them with appropriate permissions.

Do not delete the data directory as a routine troubleshooting step: you would lose accounts, manual identifications and history. To restore, use a verified copy with the server stopped.

## Automatic subtitle sync with speech

The server can create a locally timestamped transcript from short audio samples and compare it with the chosen text subtitles. It needs whisper-cli and the ggml-base.bin model. On macOS install the tool with brew install whisper-cpp and run python3 scripts/setup-subtitle-sync.py. With a custom data directory add --data-dir or set SMART_MEDIA_WHISPER_MODEL; SMART_MEDIA_WHISPER sets the path to the tool. The Docker image already includes both, so nothing needs installing there. Elsewhere the model is downloaded only when you explicitly run the setup script, and its SHA-256 is verified.

Subtitles are never synchronized automatically. When you press Sync subtitles the server listens to up to five short clips (10 seconds each), one from each fifth of the video, preferring spots where the subtitles say someone is talking. A clip without recognisable speech is replaced by another random spot (at most 12 tries). At least three distinct matching passages of speech determine the offset; clips from different parts of the video also reveal gradual timing drift. A poor match, translated subtitles or a different edit can prevent a reliable correction; the timing you had stays in place.

Sync subtitles starts the alignment, and pressing it again listens to new random clips. Cancel sync stops the current profile's job. Undo sync restores the original timestamps during the current playback. The setting is separate for the two displayed subtitle tracks and follows seeks in the video. Original files are never overwritten. Samples and transcripts stay on the server in a temporary directory; only the determined timing correction is cached, separately per profile, video, subtitle, audio and model.

## Remote access

In Settings → Remote access the owner chooses automatic UPnP mapping or manual TCP port forwarding. The feature is off by default. It uses a separate HTTPS server; do not forward the regular HTTP port 8787 to the internet. All viewers must sign in. Initial owner-account setup is not allowed through the remote interface.

Before turning it on, reserve a fixed local IPv4 address for the server and set a DNS name that points to your public IP. For that name you need a trusted TLS certificate and private key. Obtain and renew the certificate with an ACME client, preferably using a DNS challenge. The application does not issue certificates itself. Keep the key outside the repository and the library, with restricted permissions. Do not ignore browser certificate warnings.

Set the variables in the process environment and restart the server; .env files are not loaded automatically. Example:

```sh
export SMART_MEDIA_REMOTE_HOSTNAME=media.example.com
export SMART_MEDIA_REMOTE_IP=192.168.1.50
export SMART_MEDIA_REMOTE_PORT=8943
export SMART_MEDIA_TLS_CERT=/private/path/fullchain.pem
export SMART_MEDIA_TLS_KEY=/private/path/privkey.pem
```

Then in Settings → Remote access choose Automatic · UPnP and Enable remote access. Your home router must support UPnP and time-limited mappings; the firewall must allow TCP on the chosen local HTTPS port. Use the HTTPS address shown, with the host name and the external port. The DNS record must point to your public IP. Check reachability over mobile data.

The mapping is valid for ten minutes and renews about every four minutes while running. Disable remote access closes HTTPS and removes the confirmed own mapping; the same happens on a proper shutdown. Renewed certificates are reloaded successfully from the same files every minute. A successful Router mapped status confirms the rule on the router, not verified reachability from the internet. CGNAT, double NAT, wrong DNS, the certificate or a firewall can still prevent the connection. NAT-PMP/PCP and a relay through an intermediary are not supported in this version.

With Manual TCP forwarding you forward the external TCP port to the shown local HTTPS address yourself. If the external port is different, correct the port in the address you use away from home. Remove the manual rule on the router; the application does not manage it. After a restart a successfully saved setting is restored, but an automatically chosen external port may change.

## Beta diagnostics

The stable version and a regular build from source do not send diagnostics by default. A clearly labeled diagnostic beta automatically sends technical error categories, uptime and a masked reconstruction of the interface at the time of an error. The BETA.md notice must be available before download; the app also shows the notice during use.

In Settings → Beta diagnostics a browser can turn off reporting or only the replay. Do Not Track and Global Privacy Control turn off reporting from the browser. The owner can turn off reporting for the whole server; the SMART_MEDIA_TELEMETRY=off variable also prevents it. A single browser's choice does not turn off the owner's server reports. Data already sent cannot be recalled.

The report does not include names, email, accounts, passwords, file paths, original URLs, media titles, library content, form values, images, audio or video. The replay shows only rectangles and the kinds of interactions. Random session identifiers are not a permanent user identifier. Hosting can still process IP addresses; complete anonymity cannot be promised. The private dashboard keeps a limited number of reports for at most seven days. A missing heartbeat signal is not reliable proof that the server is down.
