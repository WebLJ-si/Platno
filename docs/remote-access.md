# Remote access over HTTPS

Platno can ask a trusted home router to forward a TCP port using **UPnP**, similar to Plex. Automatic mapping is **off by default**. It forwards only a separate HTTPS listener, never the ordinary HTTP port. The owner enables or disables it in **Settings → Remote access**. All viewers still sign in with an account.

Port forwarding does not provide encryption or a certificate. Unlike Plex, this project has no hosted account service, certificate service, relay, or public reachability probe. You need a DNS hostname and a trusted certificate before using public access. A VPN is an alternative when public forwarding is unavailable.

## Native HTTPS with automatic router forwarding

1. Finish the owner account setup locally. Use a separate account for each viewer.
2. Reserve a stable LAN IPv4 address for the server in your router, for example `192.168.1.50`.
3. Point a DNS hostname you control (for example `media.example.com`) to your public IPv4 address. Use a dynamic DNS updater if your ISP changes this address.
4. Obtain a trusted PEM certificate chain and matching private key for that hostname. A DNS-01 ACME client can issue/renew certificates without opening an HTTP challenge port. Configure renewal with the appropriate plugin for your DNS provider. Keep the private key outside the repository and media directories, readable only by the server account. Do not use a self-signed certificate for public access or bypass browser certificate errors.
5. Set these environment variables in the service/shell that launches the app (`.env` files are not loaded automatically):

   ```sh
   export SMART_MEDIA_REMOTE_HOSTNAME=media.example.com
   export SMART_MEDIA_REMOTE_IP=192.168.1.50
   export SMART_MEDIA_REMOTE_PORT=8943
   export SMART_MEDIA_TLS_CERT=/private/path/fullchain.pem
   export SMART_MEDIA_TLS_KEY=/private/path/privkey.pem
   ```

6. Restart the server, open **Settings → Remote access**, select **Automatic · UPnP**, and click **Enable remote access**. UPnP must be supported/enabled on your trusted router. Allow inbound TCP to the HTTPS port in the server firewall. Keep the existing HTTP listener on loopback; never forward port 8787 directly.
7. Use the exact HTTPS address shown in Settings, including the router-selected external port. Your certificate covers the hostname independently of the port number. Test using mobile data or a different internet connection; some routers cannot reach their public address from inside the LAN.

The server binds the HTTPS listener to the specified local IP only. Certificate files are reloaded every minute when a replacement parses successfully, so an ACME renewal can atomically replace them. The app checks that the PEM/key pair loads; browsers check certificate trust, validity dates, and hostname. It does not request, issue, or automatically renew certificates itself. A missing/unreadable replacement retains the previous TLS configuration. Certificate renewal must be monitored through your ACME client.

The secure listener rejects unexpected Host names, blocks account setup, adds Secure to its session cookies, and sends HSTS. Changing settings requires an owner session. Browser mutations reject cross-site origins. Proxy headers cannot turn plain HTTP into a trusted TLS connection.

## Lease lifecycle and status

- The router chooses an available external port. Only TCP is mapped.
- Each lease lasts ten minutes and is renewed approximately every four minutes while the server runs. Infinite leases are rejected.
- Mapping ownership is checked by its random per-session description, internal destination, protocol, and external port before renewal or removal. Unrelated router rules are not deliberately replaced or removed.
- Disable stops the HTTPS listener and removes a confirmed owned mapping. Clean server shutdown does the same. A crash normally leaves only the remainder of the finite lease. If a router violates its lease contract or a response is lost, inspect its mapping table; the UI reports cleanup errors instead of claiming removal succeeded.
- Successfully enabled settings are persisted locally and restored on restart. Automatic mode can receive a different external port on restart; check the displayed address again.
- Renewal failure or an unexpected listener exit stops remote HTTPS. Re-enable from the local app to retry. Failed startup remains disabled in the current process and is reported in Settings.
- **Router mapped** means the router confirmed a finite rule. It does **not** mean that internet connectivity, DNS, TLS trust, or the firewall has been independently verified. The API's `reachable` field remains `null`.

This implementation supports UPnP routers that allow mapping inspection and finite leases. NAT-PMP/PCP-only routers, permanent-only mappings, and routers with inaccessible/very large mapping tables are not supported by automatic mode. Discovery uses the configured LAN interface and has a timeout. No public IP-check service receives your server address.

## Manual forwarding

Choose **Manual TCP forwarding** and enable the listener. Forward your chosen external TCP port to the local HTTPS address in Settings. If the external port differs from the local port, use that external port in your remote URL. The app cannot discover or delete a rule you created yourself; remove it in the router when it is no longer needed. Disabling still closes the application's HTTPS listener.

Alternatively use the existing `deploy/Caddyfile.example` with a reverse proxy that issues and renews HTTPS certificates. Keep the built-in remote listener disabled in that deployment, set `SMART_MEDIA_COOKIE_SECURE=1`, bind the HTTP app to loopback, and forward only the proxy's TLS port. Never configure simultaneous conflicting mappings. Caddy's usual automatic HTTPS setup also needs its documented ACME challenge ports unless using DNS challenges.

## When it does not connect

A private or carrier-grade router WAN address is rejected for automatic mode. CGNAT generally needs an ISP public address, VPN, or separately managed tunnel. Double NAT may need changes on both routers. This app does not offer a Plex-style relay. Check the certificate/domain, DNS address, firewall, router UPnP support, and the HTTPS URL's external port. Opening more ports will not solve CGNAT.

## API

Owner only, authenticated:

- `GET /api/owner/remote-access`: configuration readiness, listener/mapping state, address, lease duration, and diagnostics. Never returns private key contents or paths.
- `POST /api/owner/remote-access`: `{"enabled":true,"automatic":true}` for UPnP; `automatic:false` for manual forwarding; `{"enabled":false}` to stop. Configuration errors return 400. A failed setup closes its listener and does not persist an enabled state.

There is no arbitrary target IP/port in this API: the fixed server configuration controls the destination. Enabling access is an explicit owner action, rather than an automatic side effect of installing or visiting the app.
