# Platno on a Synology NAS

The supported way is **Docker** through Container Manager (DSM 7.2 or newer; on DSM 7.0/7.1 the app is called Docker). Platno does not ship a Synology package (`.spk`), because it cannot be tested here; the Docker image is the same program as the release downloads, and works on Intel/AMD (x86_64) and ARM64 models.

## Install with Container Manager

1. **Create folders.** In File Station make `docker/platno` for Platno's data. Your media stays where it is (for example `/volume1/video`).
2. **Create a project.** Container Manager → *Project* → *Create*. Name it `platno`, choose `/volume1/docker/platno` as the path, and paste this compose file:

   ```yaml
   services:
     platno:
       image: ghcr.io/weblj-si/platno:latest
       restart: unless-stopped
       ports:
         - "8787:8787"
       user: "1026:100"          # your DSM user's UID:GID, see "Permissions" below
       volumes:
         - /volume1/docker/platno:/data
         - /volume1/video:/media:ro
   ```
3. **Start it**, then open `http://NAS-ADDRESS:8787` and create the owner account. In the first-run guide add folders under `/media`, for example `/media/Movies`.

`ports: "8787:8787"` makes Platno reachable from your whole home network. Use `"127.0.0.1:8787:8787"` if you only publish it through DSM's reverse proxy.

## Permissions

The container must be able to read your media and write `/data`.
- Find your DSM user's numbers over SSH with `id YOUR_USER` (typically `uid=1026(...) gid=100(users)`) and put them in `user:` as above. That user needs read access to the shared folder (Control Panel → Shared Folder → Permissions) and write access to `docker/platno`.
- Alternatively keep the default user (UID 10001) and give it read permission on the media shared folder and write permission on the data folder.

If the first-run guide says a folder is "Not found" or "No permission", it is almost always this.

## HTTPS from outside the house

Use DSM's reverse proxy: Control Panel → Login Portal → Advanced → *Reverse Proxy* → Create: source `HTTPS`, your hostname, port `443`; destination `HTTP`, `localhost`, port `8787`. Attach a certificate from *Security → Certificate*. Then add `SMART_MEDIA_COOKIE_SECURE: "1"` under `environment:` in the compose file. Forward only the HTTPS port on your router. Never forward 8787 directly. Platno's own remote-access feature (see [remote-access.md](remote-access.md)) is not needed behind DSM's proxy.

## Updating

Container Manager → Project → `platno` → *Action* → *Stop*, then *Build*/*Pull* and *Start* again (or over SSH: `docker compose pull && docker compose up -d` in the project folder). The data folder is kept. The in-app update button shows these instructions instead of installing, because the container is read-only by design.

## Limits

- Hardware video encoding is not enabled in the container. Software conversion works but is slow on small NAS processors; prefer direct play (H.264/AAC MP4) or let your players handle formats directly where possible.
- Intel models with QuickSync can pass `/dev/dri` into the container (`devices: ["/dev/dri:/dev/dri"]`) and choose the QuickSync encoder in the first-run guide. This is untested.
- DSM can restart on a different internal IP; use a fixed address or the NAS hostname.
- Running the Linux program directly on DSM (without Docker) is possible on x86_64 and ARM64 models, but DSM's own FFmpeg is limited, so it is not recommended or tested.
