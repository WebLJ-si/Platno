# PlexV2: detailed description of the user interface

This document describes the interface as currently implemented, based on the application code. Control names are given as they appear in the interface. Content and available buttons depend on the role, profile, selected media and the data in the library. The description deliberately does not specify the visual design.

## 1. First start: creating the owner account

If the server has no account yet, the **Create your server owner account** page opens:

- **Email**: the server owner's email address.
- **Display name**: optional display name.
- **Password**: the owner's password; at least 12 characters are required.
- **Confirm password**: the password entered again.
- **Create owner account**: creates the account and the initial profile. On failure a message is shown, for example about passwords that do not match or an invalid address.

After the owner account is created, the owner sees the first-run guide (section 3a).

## 2. Signing in and out

Once an owner exists, the **Sign in to your media server** page contains:

- **Email** and **Password** for signing in.
- **Sign in**, which checks the credentials and opens the app.
- A message after a wrong sign-in, or a temporary lockout after several wrong attempts.
- Instructions for recovering a forgotten owner password with a command in the server terminal. The password cannot be recovered by clicking in this form.

After signing in, the user signs out with **Sign out** in the profile menu. The sign-in page opens.

## 3. The common application frame

### Side menu

- **Overview**: the home view of the library and watching.
- **Libraries**: expands or collapses the library submenu. Below it are the individual libraries, for example **Movies**, **TV Shows** and **Videos**, with the number of their files. The names depend on the server settings. Clicking a name opens that library.
- **Watchlist**: the list of titles saved in the active profile.
- **Collections**: local groups of numbered files and other local folders.
- **Settings**: user settings; for administrators and the owner also server management.

The side area also shows the name of the server or app and a note that media stays where it is and is only read by the scanner.

### Header

- The heading says which view is open, for example **Overview**, the library name, **Watchlist**, **Collections** or **Settings**. A short description of the selected view is below it.
- **Search your library…** is the global search field. Typing opens results by titles and files. Clearing the search text returns to the previous view. **Ctrl+K** or **Cmd+K** puts focus in the search field.
- The **Profile menu** button opens the active profile's menu.

### Profile menu

It shows the active profile's name and the account's email address:

- **Switch profile** and buttons with the names of other profiles: choosing a profile. The chosen profile's PIN is required before switching.
- If there are no other profiles, **No other profiles yet.** is shown.
- **＋ Add profile**: opens the form for a new profile.
- **Sign out**: signs the current account out on the current device.

### Footer

- **Help & documentation**: opens the built-in user guide.
- The metadata providers TVmaze and TMDB are named, with links to their sites.

### Scan notice

While a scan runs, a notice appears at the top for administrators and the owner. It contains the phase, the number of files found or processed, elapsed time, the current activity, the current file and recent events. During file discovery it shows the number of files found; during processing it also shows progress. A stop is requested with **Cancel scan** in Settings.

## 3a. First-run guide

After creating the account, the owner sees a full-screen guide with a progress rail. It can be closed at any time (**Close guide**, or Escape) and run again from **Settings → Setup guide**. Only the owner sees it. Every step except the first and last can be skipped with **Skip this step**, which first shows what would be lost.

1. **Welcome**: overview of the steps and **Start**.
2. **Media folders**: for each library, a list of folders with a status (**Found**, **Not found**, **No permission**, **Not responding**, **Not a folder**, **Not a valid path**), an input with **Add**, **Choose** (system folder chooser on macOS) and **Remove**; a checkbox to sort videos automatically. **Save folders and continue** saves them.
3. **Other locations**: the read-only data folder, the **Conversion folder** input (**Use this folder**, **Reset**; refused if inside a media folder), and the FFmpeg status with installation hints.
4. **Memory and graphics**: detected memory, processor threads and graphics; the **Encoder** choice, **Picture quality** (faster start, balanced, best picture) and **Simultaneous conversions** (1 to 8, with a recommendation).
5. **First scan**: **Scan now** scans every library that has folders, with a progress bar and the current file, **Cancel scan**, and a summary of movies, episodes and other videos.
6. **Accounts**: the existing accounts and a form to add a person (email, display name, password and confirmation, role).
7. **Movie data (TMDB)** and 8. **Subtitles (OpenSubtitles)**: what each key enables and what is lost without it, numbered instructions with links, a password-style input and **Save and test**. A rejected key is not saved.
9. **Remote access**: the home-network listening status, and either the five required environment variables or the controls to turn remote access on and off.
10. **Finish**: a summary of what is configured and what is still open, with **Set up** buttons that jump to a step, and **Open my library**.

### Missing API key help

When a feature needs a key that is missing or rejected (movie search or matching, the movie search in **Edit identification**, online subtitle search, and starting a scan without a TMDB key), a dialog explains what does not work, how to get the key step by step, and, for the owner, offers an input to add it immediately. Administrators who are not the owner are told to ask the owner. The scan warning appears once per browser session and offers **Scan anyway**.

## 4. Adding and switching a profile

### Add a profile

- **Profile name**: the new profile's name.
- **PIN** and **Confirm PIN**: entering and confirming a numeric PIN of four to eight digits.
- **Create profile**: creates the profile; errors are shown in the form.
- **Cancel**: closes the form without creating anything.

### Switching a profile

- The **Enter profile PIN** window asks for the chosen profile's PIN.
- For an older profile without a PIN, the window asks you to set and confirm one.
- **Continue** checks the PIN and switches the profile.
- **Cancel** closes the window.
- After several wrong attempts a temporary lockout may be shown.

History, watchlist and user settings belong to the active profile.

## 5. Overview

### Featured from your library

A title from the library is featured, with its name and basic details:

- **▶ Start watching** starts playing the matching file.
- **Explore library** opens the details of the featured title.
- If the library has no suitable title, **＋ Add your library** is shown instead and opens settings.

### Library at a glance

Counters are shown:

- **Movies**: recognized movies.
- **TV shows**: recognized series.
- **Episodes**: recognized episode files.
- **Watched**: the number of watched files in the active profile.

### Shelves on the home overview

- **Continue watching**: unfinished movies and episodes. The card shows the title, the position or episode details and progress; clicking opens playback. If there is no content, an explanation is shown.
- **You might like**: local recommendations based on the active profile's history. The card opens the title's details. With too little data, an empty state is shown.
- **Popular in your library**: titles ordered by watch history and the available library content. The card opens details.
- **Explore your shows**: series cards and an **All shows →** button for the series library.
- **Movie collections**: cards of movies or movie collections and an **All movies →** button for the movie library.

The first three shelves can be turned on or off individually in the profile settings.

## 6. Libraries

The user selects a library in the **Libraries** submenu. The view header shows the library's name and type, and the content shows the number of files. An administrator or the owner also sees **Manage libraries**, which opens settings.

### Movies and series: filters and sorting

- **Year**: filters by year.
- **Genre**: filters by genre.
- **Language**: filters by metadata language.
- **Minimum rating**: requires at least the chosen rating.
- **Time available**: selects titles whose known runtime fits the available time.
- **Sort**: **Title**, **Recently added**, **Year**, **Rating** or **Shuffle**.
- **Unwatched**: limits the view to titles with at least one unwatched file.
- **Surprise me**: opens a randomly chosen title from the current filtered results.
- The result count says how many titles match the filters. Unknown data does not satisfy a filter that requires it.
- A title card contains the title, the year if known, and the number of files; clicking opens details. The card can also be opened with the keyboard.
- If nothing matches the filters, a notice says that no titles match.

### Videos and other libraries

Instead of movie or series metadata, local folders are shown. A card opens the folder's details with a list of files. Each file offers **Edit identification** and **▶ Play**.

## 7. Search

Global search by titles and file names or paths shows:

- The number of matching files.
- **Movies**: cards of matching movies, if any.
- **TV shows**: cards of matching series, if any.
- **Files**: a list of matching files with title and path.
- Each file has **Edit identification** and **▶ Play**.
- If there are no matches, **No matching files** is shown. With more than 200 files, the first 200 are shown with a hint to search more precisely.

## 8. Watchlist

The **Your watchlist** view shows the active profile and the number of saved items. Each item has a title and the available year or season and episode details:

- **▶ Play**: plays the matching file; if the file is not in the current list, the button is disabled.
- **Remove**: removes the item from the list; it does not delete the file.

An empty list explains that a title can be added from its details.

## 9. Collections and local folders

**Collections** explains that local collections are inferred from the folder and numbered files and are not a confirmed catalogue. Cards of local collections are shown. If other files exist without reliable identification, **Other local folders** is shown too.

### Local collection details

- The collection title, the folder path and an explanation of the local order.
- A list of numbered files; a repeated number is marked **Repeated number**.
- For each file: the title, path, availability and whether the metadata match is confirmed.
- **Edit identification**: corrects the file's identification.
- **▶ Play**: plays the chosen file.
- **Close**: closes the details.

### Local folder details

The title, the path, a warning that the identity is not verified, and the list of files are shown. Each has **Edit identification** and **▶ Play**; **Close** closes the window.

When a TMDB-based collection is shown in the view, its details contain the title, the number of movies, movie cards and **Close**.

## 10. Movie or series details

The title window shows the title, type and provider, the year, rating, runtime and description when the data is available. While loading, or when data is missing, matching text is shown. **Close** closes the window.

### Common actions

- **✓ Mark movie watched** or **✓ Mark series watched**: marks the matching files as watched.
- **Mark unwatched / reset**: clears the watched mark and saved position for the matching files.
- **＋ Add to Watchlist**: adds the title to the active profile's list. If it is already added, **✓ In Watchlist** is shown; clicking again removes it.
- Below the actions, a confirmation of the marking or reset is written.

### Movie

- **▶ Play movie**: plays the movie's chosen primary file.

### Edit details

Administrators and the owner see **✎ Edit details** next to the title's actions. It opens a form for the title's **Title**, **Year**, **Genres**, **Description** and **Cover picture** (upload, or **Remove custom cover**), with **Save changes**. For series it also lists every file with **Season**, **Episode** (for example `3, 4` for a double episode) and **Title** fields and a **Save episode** button per row. Edits apply to all files of the title, are protected from rescans, and **Back** returns to the details. Titles without an online match show "Named from file name" as their source, a default picture and **No rating**; their episodes are listed from the file names.

### Series

- **Season**: selects the season.
- **Episode**: selects an episode from the season; a missing episode is marked **Missing**.
- **▶ Play episode**: plays the chosen episode when a local file is available for it. Otherwise the button is disabled.

### Aired episode catalogue

For a series, the provider's catalogue of aired episodes is shown:

- **All episodes**: shows every episode with the total count.
- **Missing**: shows missing episodes and their count.
- **Uncertain**: shows uncertain, duplicate, unreachable or otherwise unclear cases and their count.
- **Season N**: expands or collapses a season; it also states the number of episodes.
- Each episode can show the number, title, status, summary, air date and the path of the local file. The title of a confirmed present episode is a play button.
- If the catalogue is not reachable, a notice is shown and the selectors use the locally recognized episodes.

### Bonus material

If bonus files exist, a **Bonus material** section is shown. It expands a list of the matching files; each has the path and **▶ Play**. Bonus content does not count as an episode.

### Advanced settings · Local file details

The section expands to review all of the title's files. For each it shows the path, availability, watched mark, match state, identification explanation and detected subtitles:

- **Edit identification**: opens the manual identification correction.
- **▶ Play**: plays this file.
- **Watched**: marks the single file as watched.
- **Reset**: resets the single file's position and watched state.

## 11. Edit identification

The window shows the chosen file's path and a form for choosing the right metadata. The button may also be visible to a regular user, but the server does not allow the user role to search for and save an identification; an administrator or the owner is needed for the correction:

- **Search for**: **Movie · TMDB** or **TV show · TVmaze**.
- **Title**: the search title; initially filled with the known title or the one extracted from the file name.
- **Search** or Enter: looks for results.
- Each result is a button with the title and possibly the year. Choosing a movie saves the match; choosing a series opens the episode choice.
- For a series, a season and episode are chosen from the catalogue and the choice is confirmed. The match is saved as manually confirmed for future scans.
- **Back to search** may appear after a save error.
- **Close** closes the window; the states for searching, no results and errors are shown too. Without a TMDB key, movie search shows the missing-key help (section 3a).

## 12. Video player

### Top area

- **Back to library** or the back arrow: closes the player and returns to the library or the previous view.
- The title of the current video.
- For episodes, season and episode selectors when the matching series is available; choosing switches to another local episode.
- A message about preparing, loading, waiting for the disk, reconnecting or an error.
- **Try again**: retries playback after an error.

### Resuming playback

If a suitable saved position exists, **Continue watching?** opens:

- **Continue watching** resumes at the saved position.
- **Start from beginning** starts at the beginning and resets the saved position.

### Timeline

- Display of the current and total time.
- A slider to jump to the chosen position.
- Moving the pointer along the timeline can show the time and a thumbnail preview.

### Playback buttons and selectors

- **Play or pause**: plays or pauses; clicking the video also toggles playback.
- **Back 10 seconds** and **Forward 10 seconds**: move by ten seconds. Double-clicking the left or right side of the video also moves playback.
- **Mute or unmute**: turns sound off or on.
- **Volume**: sets the volume.
- **Audio track**: choice of an embedded audio track; shown only if at least two tracks are available. Tracks can include language, codec, channel count and title. A change may restart the conversion at the current position.
- **Subtitles**: turns the main text subtitle track off or chooses one.
- **Second subtitles**: turns the second subtitle track off or chooses one; the selector appears when at least two tracks are available.
- **Subtitle delay**: manual subtitle offset in seconds, from −30 to +30, added to the speech synchronization.
- **Sync subtitles**: realigns the chosen text tracks from short local audio samples. **Cancel sync** stops its own job; **Undo sync** restores the original timestamps for the current playback.
- **Automatically sync subtitles with local speech recognition**: a profile setting for automatic alignment after choosing a subtitle; it requires a local Whisper and model.
- **Quality**: **Auto**, **1080p**, **720p** or **480p**.
- **Playback speed**: **0.5×**, **0.75×**, **1×**, **1.25×**, **1.5×** or **2×**.
- **Compatibility mode**: restarts playback with software H.264/AAC conversion and robust seeking; Auto uses 720p.
- **Stats**: shows or hides details about the playback mode, the reason for conversion, quality, codecs, audio, original bitrate, HDR, HLS, estimated bandwidth and buffer, when available.
- **PiP**: tries to turn picture-in-picture on or off; a message is shown if the mode is unsupported.
- **Fullscreen**: turns fullscreen mode on or off.

### Next episode

At the end of a suitable episode, a **Next episode** window shows the title and details of the next local episode:

- **Play now** starts the next episode.
- **Keep watching** cancels the automatic move.
- If automatic continuation is on in the profile, a ten-second countdown is shown; it does not count down while paused.

### Keyboard

- **Space** or **K**: play/pause.
- **J** or left arrow: ten seconds back.
- **L** or right arrow: ten seconds forward.
- **M**: mute or unmute.
- **F**: fullscreen.
- **C**: cycle subtitles.
- **Shift+N**: next local episode.

This version has no manual markers for skipping the intro, recap or credits, no chapter navigation and no problem-report button.

## 13. Settings: each profile's settings

### Profile preferences

Settings are saved automatically for the active profile:

- **Preferred playback quality**: the default quality, from **Auto** to the individual fixed options.
- **Playback speed**: the default playback speed.
- **Subtitle language**: **Ask each time** or a chosen language: English, Croatian, Slovenian, German, Spanish, French, Italian, Japanese, Korean, Portuguese, Russian or Chinese.
- **Automatically play the next episode**: turns automatic continuation on or off.
- **Continue watching items**: the maximum number of items on the continue shelf.
- On saving, a message about success or failure is shown.

### More profile settings

- **Preferred audio language**: the first/original track or a preferred language if such a track exists.
- **Subtitle behavior**: **Off by default**, **Use preferred language** or **Always show a subtitle when available**.
- **Subtitle size**, **Subtitle background**, **Subtitle shadow** and **Subtitle color**: subtitle display options.
- **Show Continue watching**, **Show recommendations** and **Show popular in your library**: individually turn the matching shelves in Overview on or off.
- **Accent color** and **Reduce motion**: personal appearance and reduced-movement options.

## 14. Settings: libraries and scanning

This part is visible to administrators and the owner.

### Libraries

- **Automatically sort recognized movies and shows into their matching libraries**: automatically distributes recognized movies and series. If the option is off, scanned content goes to the chosen target library.
- **Scan folders (one path per line)**: the list of server folders, one path per line.
- **Save sorting and folders**: saves the automatic sorting and the folders.
- **Browse folder…**: opens a folder list in the page. It shows the folders of the machine that runs Platno with quick places (Docker `/media`, mounts, home, Windows drives), an *Up* button and an editable path. It lists folders only, skips hidden ones and works on every system.

For each existing library:

- A **Library name** field and a type selector **Movies**, **TV shows**, **Videos** or **Other**.
- **Save**: saves the library's name, type and folders.
- **Delete**: asks for confirmation and deletes the library; media files stay and membership moves to another library of the same type. The last library of each type cannot be deleted.
- The number of files and the current type.
- **Folders (one per line)**: this library's folders.
- **Browse…**: adds a folder with the chooser on a supported server.
- **Scan this library**: saves the current library and starts a scan of its listed folders.

### Add a library

- A field for the new library's name.
- A type selector: **Videos**, **Movies**, **TV shows** or **Other**.
- **Create**: creates the library.

### Scan

- A selector for the target library that receives content assigned by the scan.
- **Scan added or changed files**: incremental scan.
- **Full library scan**: rechecks unlocked files.
- **Cancel scan**: requests that the scan stop.
- An explanation says that manually confirmed identifications stay protected during a rescan.

## 15. Settings: server and providers

This part is visible to the owner.

### Updates

**Updates** shows the installed version and **Check for updates**. When a newer release exists it shows the version, its notes and a link to the release page, and either **Install and restart** (programs you installed yourself) or the exact command to run (Docker, `.deb`, read-only folder). Installing downloads and verifies the release, then the page reloads when the server is back.

### Server

- **Server name** and **Save server name**: the name displayed in the app.
- **Transcoding encoder**: automatic choice or the available hardware encoders.
- **Save encoder**: saves the choice. An explanation of the software fallback is also shown.

### Metadata and subtitle providers

- **TMDB Read Access Token**: the token field, the status **configured** or **not configured** and any note that the value comes from the environment. **Save TMDB token** saves the entered token.
- **OpenSubtitles API key**: the same status and a key field. **Save OpenSubtitles key** saves the key.
- Existing keys are never printed in the form. The save result or an error is shown.

### Transcoding quality

- **Balance**: **Faster start · lower bitrate**, **Balanced** or **Higher picture quality · more CPU and bandwidth**.
- **Save transcoding balance**: saves the server-wide conversion setting.

### Conversion folder

- **Folder (absolute path)** and **Save conversion folder**: where temporary conversion and speech-sync files go. The folder must be writable and outside every media folder. The currently used folder is shown; if `SMART_MEDIA_TRANSCODE_DIR` is set the field is disabled.

### Setup guide

- **Run the setup guide**: opens the first-run guide again (section 3a).

## 16. Settings: signed-in devices

**Signed-in devices** is available to a user for their own account:

- **Show sessions**: lists the current sessions, marks **This device** or **Other session** and shows the sign-in and expiry times.
- **Sign out other devices**: signs out the other sessions of this account.
- An empty state or a loading error may be shown.

## 17. Settings: subtitles

The **Subtitles** section contains **Search subtitles**. A regular user sees the form, but the server rejects their online OpenSubtitles search; a local list may be shown from already loaded data:

- An explanation of locally detected subtitles and the OpenSubtitles search; the online provider receives the search title.
- The **Settings** button next to the provider explanation opens settings.
- **Title**: the search title.
- **Subtitle language**: **All languages** or one of the offered languages.
- **Search** or Enter: looks for local and online results.
- **Detected on your computer**: matching local files with the path and the names of detected subtitles.
- **OpenSubtitles results**: release or file name, language, rating and number of ratings, download count and any **Hearing impaired** label.
- **View on OpenSubtitles**: opens the result at the provider, if the link is valid.
- States with no results and errors are written out too. Without an OpenSubtitles key the missing-key help appears (section 3a). This part does not guarantee automatic download or synchronization of subtitles.

## 18. Settings: diagnostics and accounts

### Diagnostics – administrator and owner

- **Check server health** checks the FFmpeg and ffprobe tools, the TMDB configuration, the number and total amount of media files and the accessibility of the library folders.
- **Library folders**: the list of folders and their accessibility.
- **Needs attention**: an expandable list of up to 100 titles that need review, with the path and the reason.
- **Largest files**: an expandable list of the largest files.
- While checking, or on an error, the matching message is shown.
- **Currently watching**: for each active playback shows the user, profile, file, mode, position and watch time. The data estimate is not a measurement of actual network traffic.
- If there is no recent activity, an empty state is shown.

### Accounts – administrator and owner

The list shows the user name, role, email address and number of profiles.

For each account the owner additionally sees:

- **Email address**, **New password**, **Confirm new password**.
- For their own account also **Current owner password** and instructions for recovering a forgotten password in the terminal.
- **Save credentials**: saves the new credentials; the change signs out the existing sessions of this account.

### Add an account – administrator and owner

- **Email address**, optional **Display name**, **Temporary password** and **Confirm password**.
- **Role**: **User · can watch media**; the owner can also choose **Administrator · can manage the server**.
- **Create account**: creates the account and the initial profile. A confirmation or an error is shown.

## 19. Help and an unreachable server

- **Help & documentation** opens a standalone page with the user guide for use and maintenance.
- If the public offline page was already stored and the server is unreachable, the browser can show the **Your media server is unavailable** page with instructions for reconnecting. This is not offline access to the private library or videos.
- If loading a signed-in session fails, **Could not load your library** can appear with the **Try again** and **Return to sign in** buttons.

## 20. Differences between roles

- **User**: uses libraries, search, the player, profiles, Watchlist, profile settings, the available subtitles in the player and their own sessions. Buttons for editing identification and online subtitle search may be visible, but the server rejects these actions for the user role.
- **Administrator**: in addition manages libraries and scanning, sees diagnostics and activity and can create a regular user account.
- **Owner**: in addition manages server settings, provider keys and account credentials, and can create an administrator.

Individual buttons may appear only after data has loaded, or when the matching files, episodes, tracks, profiles or permissions are actually available.
