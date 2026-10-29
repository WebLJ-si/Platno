# PlexV2 backlog

This is planned work, not a list of available features. Update the user guide and test matrix when completing an item.

## Automatic subtitle matching and synchronization

- [ ] Match subtitles to the exact release using file hash, language, episode identity, frame rate and duration instead of title alone.
- [ ] Detect whether subtitles are out of sync; estimate both constant offset and time drift.
- [ ] Offer automatic correction using a verified local tool such as ffsubsync or alass, behind an explicit user action.
- [ ] Show confidence, proposed offset/drift and a preview before applying; provide undo and preserve the original subtitle.
- [ ] Run synchronization as a bounded background job with cancellation and clear progress/errors. Do not silently install tools/models or overwrite media folders.
- [ ] Test correct files, positive/negative delay, frame-rate drift, wrong release, missing audio, long/short videos, Unicode paths and unavailable drives.

Requested 2026-10-29. Manual subtitle delay remains available in the player; automatic synchronization is not implemented.

## Open-source issue and support workflow

- [ ] Design a useful maintainer-facing issue workflow before reintroducing any “Report problem” UI.
- [ ] Prefer a GitHub issue template with reproduction steps, app/FFmpeg/browser versions, sanitized stream details and explicit user review before sharing.
- [ ] Clearly distinguish a household server administrator from a project maintainer. A local server admin is not expected to modify the codebase.
- [ ] Keep paths, titles, account identifiers, provider credentials and session cookies out of automatic exports by default.
- [ ] Add regression tests for redaction, opt-in sharing and actionable diagnostics.

Requested 2026-10-29. The former local playback-report button, endpoints and administrator report list have been removed. Stream statistics and server-health diagnostics remain available.

## Other separate projects

- [ ] Server-enforced parental/library restrictions with trustworthy rating data and bypass tests.
- [ ] PGS/VobSub subtitle burn-in with real fixtures and seek/ABR validation.
- [ ] Native browser/device verification for Safari, Firefox, iOS, Android and TV remotes.
- [ ] Build/run the Linux Docker image and test HTTPS deployment in an actual target environment.
- [ ] Revisit other ideas in `docs/feature-review.md` independently; that document records evaluation, not completion promises.

Manual skip markers and chapter navigation were removed at the user's request on 2026-10-29. They are not part of the current player. Legacy database tables from older builds may remain unused; no startup operation deletes historical user data.
