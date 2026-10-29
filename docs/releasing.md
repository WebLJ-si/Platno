# Releases, branches and updates

## Branches

| Branch | Purpose |
| --- | --- |
| `main` | Stable. Everything on it passes the full suite. Releases are cut from it. |
| `test` | Try things first. Pushing runs the whole suite and builds every program, `.deb` and a Docker image tagged `test`, **but publishes no release**. Download the build artifacts from the workflow run, or `docker pull ghcr.io/weblj-si/platno:test`. |

Work on `test`, check it, then merge to `main`. The in-app updater only follows published releases, never the test branch.

## What a release contains

Pushing a tag `vX.Y.Z` (the tag must equal the version in `Cargo.toml`) runs `.github/workflows/release.yml`:

1. Builds static programs for Linux x86_64 and ARM64 (musl), a universal macOS program, and Windows x86_64.
2. Builds `platno_X.Y.Z_amd64.deb` and `_arm64.deb` and installs each in a clean Debian to prove they work.
3. Creates archives (`.tar.gz`, `.zip`), the bare programs the in-app updater downloads, `install.sh`, and `SHA256SUMS`.
4. Assembles the multi-architecture Docker image from those programs and pushes `ghcr.io/weblj-si/platno:X.Y.Z` and `:latest` and, when the repository has the secrets `DOCKERHUB_USERNAME` (`weblj`) and `DOCKERHUB_TOKEN` (a Docker Hub access token with write rights), `weblj/platno` on Docker Hub too. Without those secrets publish Docker Hub by hand after the release: download `platno-X.Y.Z-linux-x86_64` and `-aarch64` into `docker-bin/` as `platno-linux-amd64` and `platno-linux-arm64`, then `docker buildx build -f Dockerfile.release --platform linux/amd64,linux/arm64 -t weblj/platno:X.Y.Z -t weblj/platno:latest --push .`. New GHCR packages start private: make `platno` public once in the package settings.
5. Creates the GitHub release with notes written by `scripts/release_notes.py`: the changelog section, a download table, every Docker setting, update and verification instructions.

A tag with a hyphen (`v0.2.0-rc.1`) is published as a pre-release and does not become `latest`.

## Cutting a release

1. Update the version in `Cargo.toml` (and `Cargo.lock`), add a `## [X.Y.Z]` section to `CHANGELOG.md`.
2. Merge to `main`, wait for CI to pass.
3. `git tag vX.Y.Z && git push origin vX.Y.Z`. Check the Actions run, then the release page.
4. Preview the notes locally with `python3 scripts/release_notes.py X.Y.Z`.

## How the in-app update works

*Settings → Updates* (owner only) asks `api.github.com` for the latest published release when the owner presses *Check for updates*; nothing happens in the background. *Install and restart* downloads `platno-X.Y.Z-<platform>`, checks its SHA-256 against `SHA256SUMS` from the same release, runs it once with `--version`, moves the old program to `<name>.previous`, puts the new one in place and restarts itself in the same process (the service manager is not involved). Failures before the swap change nothing. Docker, `.deb` and read-only installs show the right manual command instead. `SMART_MEDIA_UPDATE_CHECK=off` removes the feature. To go back, stop Platno and rename `platno.previous` to `platno`.

Updates are not code-signed, only checksummed: they protect against damaged or substituted downloads, not against a compromised GitHub account. Signing is future work.

## Not published automatically

- Windows and macOS programs are unsigned (SmartScreen and Gatekeeper warn on first start).
- No `.spk` for Synology, no Homebrew formula, no `.rpm`, no Windows installer. See [synology.md](synology.md).
