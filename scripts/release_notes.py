#!/usr/bin/env python3
"""Write the GitHub release notes for a version to stdout.

  release_notes.py VERSION [--repo OWNER/NAME] [--hub NAME]

Combines the matching CHANGELOG.md section with the download table, the Docker settings that must
be chosen, update instructions and checksum verification, so every release explains itself.
"""
import argparse, pathlib, re

ROOT = pathlib.Path(__file__).resolve().parents[1]


def changelog(version):
    text = (ROOT / 'CHANGELOG.md').read_text()
    base = version.split('-')[0]
    match = re.search(rf'^## \[{re.escape(version)}\][^\n]*\n(.*?)(?=^## \[|\Z)', text, re.M | re.S) or re.search(rf'^## \[{re.escape(base)}\][^\n]*\n(.*?)(?=^## \[|\Z)', text, re.M | re.S)
    return match.group(1).strip() if match else 'See the commit history for what changed.'


def notes(version, repo, hub):
    owner, name = repo.split('/')
    ghcr = f'ghcr.io/{owner.lower()}/platno'
    stable = '-' not in version
    tag = version
    files = lambda suffix, ext='': f'platno-{version}-{suffix}{ext}'
    image = f'{hub}/platno' if hub else ghcr
    hub_line = f'\nAlso on GitHub Container Registry: `{ghcr}:{tag}`' if hub else ''
    return f'''# Platno {version}

{changelog(version)}

## Download

| Computer | File |
| --- | --- |
| Windows 10 or 11 (64-bit) | [{files('windows-x86_64', '.zip')}](https://github.com/{repo}/releases/download/v{version}/{files('windows-x86_64', '.zip')}) |
| macOS (Apple silicon and Intel) | [{files('macos-universal', '.tar.gz')}](https://github.com/{repo}/releases/download/v{version}/{files('macos-universal', '.tar.gz')}) |
| Linux, 64-bit PC or server | [{files('linux-x86_64', '.tar.gz')}](https://github.com/{repo}/releases/download/v{version}/{files('linux-x86_64', '.tar.gz')}) |
| Linux, ARM64 (Raspberry Pi 4/5, ARM servers) | [{files('linux-aarch64', '.tar.gz')}](https://github.com/{repo}/releases/download/v{version}/{files('linux-aarch64', '.tar.gz')}) |
| Debian, Ubuntu (amd64) | [platno_{version}_amd64.deb](https://github.com/{repo}/releases/download/v{version}/platno_{version}_amd64.deb) |
| Debian, Ubuntu (arm64) | [platno_{version}_arm64.deb](https://github.com/{repo}/releases/download/v{version}/platno_{version}_arm64.deb) |

No Rust, compiler or other build tools are needed: the programs are ready to run. Unpack, start `platno`
(`platno.exe` on Windows), open http://127.0.0.1:8787 and create the owner account. Install
[FFmpeg](https://ffmpeg.org/download.html) as well so videos that browsers cannot play directly can be converted.

**One line on Linux and macOS:** `curl -fsSL https://github.com/{repo}/releases/latest/download/install.sh | sh`

**Debian and Ubuntu:** `sudo apt install ./platno_{version}_amd64.deb` installs the program, FFmpeg and a service that
starts at boot. Settings are in `/etc/default/platno`; data is in `/var/lib/platno`. Give the `platno` user read access to
your media folders (for example `sudo setfacl -R -m u:platno:rX /path/to/media`).

## Docker{' (this release)' if stable else ''}

Image: `{image}:{tag}`{'' if not stable else f' (also `{image}:latest`)'}{hub_line}
Architectures: `linux/amd64` and `linux/arm64`.

What you have to set:

| Setting | Required | What it is |
| --- | --- | --- |
| Media folder mount | **yes** | In the snippet below replace `/absolute/path/to/media` with your media folder on the host (with the repository's `compose.yaml`, set `PLEXV2_MEDIA_DIR` instead). It is mounted read-only at `/media`, so add `/media/...` folders in the first-run guide. |
| Volume `platno-data` | automatic | Keeps the accounts, library and settings at `/data`. Never delete it when updating. |
| Published port | no, default `8787` | The snippet publishes the port on `127.0.0.1` only; change it to `"8787:8787"` to reach Platno from other devices. With the repository's compose file `PLEXV2_PORT` changes the host port. |
| `SMART_MEDIA_COOKIE_SECURE=1` | only behind HTTPS | Set when a reverse proxy terminates HTTPS in front of Platno. |
| `TMDB_TOKEN` | no | Free TMDB token for movie posters and ratings. It can also be added in the app. |
| `SMART_MEDIA_TRANSCODE_DIR` | no | Folder for temporary conversion files; mount a volume or `tmpfs` there. |
| Permissions | **check** | The container runs as UID 10001. Your media folders must be readable by that user (or add `user: "UID:GID"` to the service). |

```yaml
services:
  platno:
    image: {image}:{tag}
    restart: unless-stopped
    ports:
      - "127.0.0.1:8787:8787"
    volumes:
      - platno-data:/data
      - /absolute/path/to/media:/media:ro
volumes:
  platno-data:
```

Start with `docker compose up -d`, then open http://127.0.0.1:8787. Update later with
`docker compose pull && docker compose up -d`. Reset a forgotten owner password with
`docker compose exec -it platno platno reset-owner-password`. There is no hardware video encoding
inside the container unless you pass a GPU device through; software conversion always works.

## Synology

Use Container Manager with the compose file above; see [docs/synology.md](https://github.com/{repo}/blob/v{version}/docs/synology.md) for folder permissions, the reverse proxy and what is not supported.

## Updating

- **Programs on Linux, macOS and Windows:** in Settings, Updates, press *Check for updates*, then *Install and restart*. The download is checked against `SHA256SUMS`; the old program is kept next to it as `platno.previous`.
- **Debian and Ubuntu:** install the new `.deb` over the old one.
- **Docker and Synology:** `docker compose pull && docker compose up -d`.

## Verify your download

`sha256sum -c SHA256SUMS --ignore-missing` (macOS: `shasum -a 256 -c SHA256SUMS --ignore-missing`).

## First start warnings

The programs are not code-signed, so the first start may show a warning. On macOS run
`xattr -d com.apple.quarantine platno` (or use the one-line installer, which avoids the warning). On Windows choose
*More info*, then *Run anyway*. Platno never changes your media files.

Full documentation: https://github.com/{repo}#readme
'''


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('version')
    parser.add_argument('--repo', default='WebLJ-si/Platno')
    parser.add_argument('--hub', default='weblj')
    args = parser.parse_args()
    print(notes(args.version.lstrip('v'), args.repo, args.hub))
