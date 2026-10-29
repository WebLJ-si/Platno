# Platno Observatory

An independent Vercel-hosted diagnostics collector and private Google owner dashboard. It does not depend on WebLJ Dash. `api/index.js` validates a deliberately narrow report schema, rate-limits anonymous writes in durable Redis, stores private bounded reports with expiration, and protects reads/exports/deletions with an owner session.

## Deployment

The current deployment is https://platno-observatory.vercel.app. The directory is an independent Node 22+ project; deploy it as the Vercel project root, not the entire Rust repository. The Redis integration was provisioned on the free plan with automatic paid upgrades disabled. Vercel/Redis usage limits still apply; do not enable automatic upgrades without reviewing them.

Required production environment variables:

- `APP_URL`: your exact production origin, with no path, e.g. `https://platno-observatory.vercel.app`.
- `OWNER_EMAIL`: the sole permitted Google account's verified email. No default/fallback address is compiled into source. Changing it invalidates existing signed sessions.
- `SESSION_SECRET`: at least 32 random bytes encoded as a sufficiently long string; keep it private. Rotating it signs out sessions and rotates rate-limit hashing.
- `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`: OAuth Web application credentials.
- `UPSTASH_REDIS_REST_URL`, `UPSTASH_REDIS_REST_TOKEN`, or the Vercel integration aliases `KV_REST_API_URL`, `KV_REST_API_TOKEN`. Never expose these to the browser.

Create a Google Cloud OAuth client of type **Web application**. Set the authorized redirect URI to:

```
https://platno-observatory.vercel.app/api/auth/callback
```

Set its authorized JavaScript origin to `https://platno-observatory.vercel.app` if Google asks for one. If the consent screen is in Testing mode, add the intended owner as a test user. Add credentials privately through Vercel Project → Settings → Environment Variables, then redeploy. Do not commit or paste secrets into chat. Without those credentials, login returns 503; reports remain unreadable to anonymous users. The app verifies Google signature, issuer, audience, expiration, nonce, PKCE/state, verified email and the exact allow-list entry before granting access. It uses only `openid email`, not Gmail or Drive permissions.

```sh
npm ci
npm test
npm run check
npx vercel deploy --prod
```

Native `/api/ingest` must be publicly reachable (server reports do not have a Vercel account). Verify the production alias is excluded from preview-only deployment protection. Report reads still require the application's Google session. Do not publish a Vercel protection bypass token in the open-source client. Optional additional platform/WAF limits can complement the Redis limits.

## Data handling and uptime

The protocol accepts enum categories, bounded numbers, known code assets and geometry, not arbitrary error text, URLs, HTML or user metadata. Unknown properties and oversized reports are rejected. The masked replay canvas draws only generic boxes from validated geometry; it never renders uploaded HTML. Source builds/stable builds do not send diagnostics by default; diagnostic beta builds are automatic, disclosed and have server/browser opt-outs. See the root `BETA.md` before publishing a beta download.

Redis stores no raw IPs with reports. The rate limiter uses a keyed, daily rotating IP hash with a one-minute TTL and a global hourly cap. The public write protocol is intentionally not a secret, so events can be fabricated. Do not treat submitted telemetry as verified security evidence.

Reports expire after seven days, with at most 1,000 indexed error reports and 500 session summaries. Overflow records are deleted rather than just removed from indexes. Heartbeats update session summaries rather than filling error storage. The dashboard lists the latest 100 reports and recent observed sessions. It labels missing heartbeats accurately; it does not probe or expose users' private servers. Vercel/provider infrastructure logs are outside application-level deletion and can contain connection metadata.

## API

- `POST /api/ingest`: public, strict JSON schema, 180KB limit, 30 accepted attempts per network source per minute and 5,000 per hour globally. No read capability.
- `GET /api/health`: public generic liveness only.
- `GET /api/auth/login`, `GET /api/auth/callback`: Google OIDC flow.
- `GET /api/auth/me`, `GET /api/reports`, `GET /api/report?id=...`: owner session required.
- `POST /api/auth/logout`, `POST /api/delete?id=...`, `POST /api/purge`: owner session and exact same-origin request required.

The server never logs request/report bodies, cookies, OAuth tokens, private emails or raw client IPs itself. Platform request logs can still record their own metadata. Google login callbacks contain authorization codes in the URL; configure provider log retention appropriately.

## Verification

`npm test` covers strict schema rejection, exact verified owner identity, signed session integrity/audiences/owner changes, Google token signatures/issuer/audience/nonce/state and wrong accounts, private API reads, ingest size limits/rate-limit responses, and origin-protected deletion.

With Playwright available through normal resolution or `NODE_PATH`, run `node tests/ui.cjs` for dashboard replay, uptime, layouts and JavaScript errors. The root `tests/telemetry-browser.cjs` exercises the real media-server frontend against a stub collector configuration: automatic reports, hostile private DOM/messages/URLs not leaking, opt-outs/GPC and settings widths.

`node tests/live-relay.cjs --live` is an explicitly requested live connectivity check: it starts a disposable beta media-server catalogue and sends synthetic data through the actual Rust relay and deployed Vercel endpoint to Redis. It requires local ignored production/owner env files and retains one synthetic test report. It is intentionally absent from the default test command.

Real Google-account sign-in still requires configured OAuth credentials and a user sign-in; generated signing-key tests cannot prove the external consent screen setup. Environment files, `.vercel`, dependencies and tests are excluded from deployment/source publication as appropriate. Keep the Google credentials and sole owner's address in environment configuration, not source or engineering-memory notes.
