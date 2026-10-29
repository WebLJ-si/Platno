# PlexV2 backlog

This is planned work, not a list of available features. Update the user guide and test matrix when completing an item.

## Automatic subtitle matching and synchronization

- [ ] Match subtitles to the exact release using file hash, language, episode identity, frame rate and duration instead of title alone.
- [x] Estimate constant offset and, when independent distant speech samples agree, time drift using local Whisper transcription and subtitle-text matching.
- [x] Offer automatic correction with profile opt-out, manual retry, cancellation, match evidence and session-local undo; preserve originals.
- [x] Bound local inference and cache only timing parameters per profile and source signature.
- [x] Test offset/drift estimation, ambiguous dialogue, Unicode normalization, source preservation, cache invalidation, actual speech inference, subtitle allowlists, and seek-time correction.
- [ ] Add reliable alignment for translated subtitles using an independently verified non-text speech alignment method.
- [ ] Add end-to-end real speech tests for long-video drift, changing edits, additional languages, physical devices and disconnected network drives.

Implemented 2026-10-29 for same-language text matching. Manual delay remains available. Exact-release discovery and translated-subtitle alignment remain separate work.

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
- [ ] Test the Linux Docker image on amd64 and with HTTPS in an actual target environment (the arm64 image builds and runs; see `docs/validation.md`).
- [ ] Revisit other ideas in `docs/feature-review.md` independently; that document records evaluation, not completion promises.

Manual skip markers and chapter navigation were removed at the user's request on 2026-10-29. They are not part of the current player. Legacy database tables from older builds may remain unused; no startup operation deletes historical user data.
