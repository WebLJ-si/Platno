# Deployment

The application defaults to localhost. Create the owner account locally before granting remote network access. Keep server credentials and provider tokens outside Git.

## Docker

Install Docker Engine or Docker Desktop, then run from the repository:

```sh
export PLEXV2_MEDIA_DIR=/absolute/path/to/media
docker compose up -d --build
```

Temporary conversion files (HLS segments, speech-sync scratch files) go to the system temp folder by default. Set `SMART_MEDIA_TRANSCODE_DIR` or use Settings → Conversion folder to put them on a faster or larger disk; in Docker, mount a volume there. Folders inside a media folder are refused, and orphaned session folders older than an hour are removed at startup.

The host port defaults to 8787; set `PLEXV2_PORT` if another program already uses it.

Open `http://127.0.0.1:8787`. Scan `/media` from Settings. The media mount is read-only; SQLite and private provider configuration are in the `platno-data` volume. The container runs as UID 10001: its media directory must be readable by that UID. When binding an existing data directory, make it writable by that UID too. Source paths imported from a host database must use the container's `/media` paths to play correctly. GPU pass-through is platform-specific; software conversion always remains available. Adaptive HLS encodes three renditions concurrently and needs more CPU than fixed quality.

To update, pull the repository and run `docker compose up -d --build` again. Keep the data volume. Owner recovery uses `docker compose exec -it platno smart-media-scanner reset-owner-password`. 
### Published images

Release images are published for `linux/amd64` and `linux/arm64`:

- **Docker Hub:** [`weblj/platno`](https://hub.docker.com/r/weblj/platno), the default in `compose.yaml`.
- **GitHub Container Registry:** `ghcr.io/weblj-si/platno`, pushed by the release workflow (`.github/workflows/release.yml`). A new package starts private; make it public once in the package settings.

Use them without building: `docker compose pull && docker compose up -d` (set `PLEXV2_MEDIA_DIR`, and `PLEXV2_IMAGE` to pick another image or a version tag). Build your own checkout instead with `docker compose up -d --build`. How releases and the Docker Hub push work is described in [releasing.md](releasing.md). The image contains FFmpeg and runs as UID 10001; GPU encoders are not available inside it (software conversion is used).

## HTTPS reverse proxy

Use a domain whose DNS points to your server and install Caddy. Copy `deploy/Caddyfile.example`, replace `media.example.com` with your domain, and run Caddy as a service. Keep the application's port bound to localhost; expose only the proxy. Caddy manages HTTPS certificates for eligible public domains. Set `SMART_MEDIA_COOKIE_SECURE=1` before starting the app behind HTTPS (for Compose: export it, then recreate the container). Secure cookies require HTTPS; plain HTTP sign-in will stop working with that setting.

The example streams responses immediately and preserves same-origin byte ranges, HLS, and cookie authentication. If Caddy runs in another container, use a shared Docker network and proxy to `platno:8787`, rather than localhost. Do not add permissive CORS or cache authenticated API/media responses at the proxy.

A private VPN such as Tailscale is another deployment option. Set it up on your own devices, make the localhost service reachable through your VPN/proxy configuration, and use HTTPS for installed PWA access. This project does not automatically provision tunnels, open router ports, or create external accounts.

Sources: [Caddy reverse proxy](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy), [Dockerfile reference](https://docs.docker.com/reference/dockerfile/).

## Native service

Build with `cargo build --release --locked`. Run the absolute binary path under your OS service manager with `SMART_MEDIA_DATA` set to a persistent directory outside the media roots, and a PATH containing FFmpeg and ffprobe. Set the working directory and environment explicitly; service managers do not inherit your interactive shell. Linux systemd example:

```ini
[Unit]
Description=Platno media server
After=network-online.target

[Service]
User=media
ExecStart=/opt/platno/smart-media-scanner
Environment=SMART_MEDIA_DATA=/var/lib/platno
Environment=SMART_MEDIA_HOST=127.0.0.1
Environment=SMART_MEDIA_COOKIE_SECURE=1
Restart=on-failure
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

Create the service user and writable data directory first. Set `SMART_MEDIA_COOKIE_SECURE=0` for a local HTTP-only setup. macOS can use launchd with the same absolute binary and explicit environment; Windows can run the binary in a terminal or an independently configured service wrapper. No service is installed automatically.

## Backup and recovery

Stop the service before copying the entire data directory, including SQLite and private provider files, to access-controlled backup storage. Restore with the service stopped and preserve file ownership. Never copy only the SQLite file while it is live in WAL mode. Live database backup/restore from the UI is not implemented. The PWA offline page explains a lost connection; video downloads and offline viewing are not implemented.

## Built-in HTTPS and UPnP

The optional owner-controlled remote listener is separate from the HTTP/reverse-proxy deployment. It requires a reserved local IPv4 address, DNS hostname and externally issued/renewed trusted TLS certificate. Set `SMART_MEDIA_REMOTE_HOSTNAME`, `SMART_MEDIA_REMOTE_IP`, `SMART_MEDIA_REMOTE_PORT` (default 8943), `SMART_MEDIA_TLS_CERT`, and `SMART_MEDIA_TLS_KEY`, restart, then enable it in Settings. [Remote-access setup](remote-access.md) covers finite UPnP leases, manual rules, CGNAT, DNS, certificate renewal and testing from outside the LAN. Keep it disabled when a reverse proxy already handles public HTTPS; never forward port 8787 itself. Container deployments need explicit HTTPS port publishing and LAN multicast/router access for UPnP; those networking details are deployment-specific and have not been hardware-tested.
