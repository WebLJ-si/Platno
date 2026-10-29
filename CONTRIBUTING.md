# Contributing to Platno

Platno is an open-source project backed by [WebLJ.si](https://weblj.si). Everyone is welcome to take part, whether you report a bug, improve the documentation or send code.

## Reporting problems and ideas

Open an [issue](https://github.com/WebLJ-si/Platno/issues). For a bug, say what you did, what you expected, what happened, and your system (operating system, browser, how you installed Platno, the version shown in Settings → Updates, and FFmpeg version if playback is involved). Remove private paths, tokens and account details before posting.

## Sending changes

1. Fork the repository and branch from `test`. `main` only receives changes that have already passed there.
2. Keep a change focused, and add or update a test with it. Tests live in `tests/` and `src/`; `docs/testing.md` explains what each covers.
3. Run the suite before opening a pull request:

   ```sh
   python3 scripts/test.py            # Rust, JavaScript and HTTP/FFmpeg tests (needs FFmpeg, Python 3, Node)
   python3 scripts/test.py --browser  # adds real Chrome UI tests (needs the Node playwright package)
   ```
4. Open the pull request against `test`. CI runs the same suite and builds every program as downloadable artifacts, so you can try your change on Windows, macOS and Linux without compiling.

## Good first areas

- Testing on Windows, Synology and Raspberry Pi and reporting what breaks.
- Translating the user guide (`docs/user-guide.md`); the interface is English for now.
- Items in [TODO.md](TODO.md) and [docs/feature-review.md](docs/feature-review.md).

## Ground rules

Platno never modifies media files, never sends data anywhere without the owner turning it on, and never collects people's provider keys. Changes that break those promises will not be merged. Releases are cut by the maintainers, see [docs/releasing.md](docs/releasing.md).
