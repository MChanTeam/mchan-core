# Changelog

All notable changes to MChan are documented here.

## [Unreleased]

## [0.9.4] - 2026-08-16

- Bounded thread bumping so old active threads cannot stay near the top indefinitely.
- Returned unpinned threads to normal bump ordering alongside never-pinned threads.

## [0.9.3] - 2026-08-15

- Made authorized thread actions render inline alongside Reply, Report, Pin/Unpin, and Hide.

## [0.9.2] - 2026-08-14

- Made reply creation and thread bumping transactional, so failed replies no
  longer reorder boards.
- Ordered board threads with pinned threads first, then by their bump time.
- Split global admins from board-scoped moderators and renamed the global
  allowlist from `MCHAN_MODERATOR_EMAILS` to `MCHAN_ADMIN_EMAILS`; board
  moderators are assigned per board in SQLite.
- Added admin board-moderator assignment forms backed by SQLite records.
- Scoped browser moderation and pin/unpin actions to assigned boards; retained
  admin-only board/site bans and abuse-log access.
- Added optional best-effort Discord webhook notifications for successfully created pending reports; delivery failures do not affect report persistence.

## [0.9.1] - 2026-08-14

- Moved moderator web access to the whole `staff.mchan.fyi` host as a
  self-hosted Cloudflare Access application with an email/group Allow policy.
  The staff hostname uses the production Tunnel service
  `http://127.0.0.1:3001`, shares the production origin with `mchan.fyi`, and
  bypasses caching.
- Kept `mchan.fyi` public and anonymous. Every staff-host page receives the
  Cloudflare Access identity header, while the application still checks the
  normalized moderator email allowlist on every request as a second guard.
  Allowlisted staff requests render staff links and direct Hide controls;
  requests without that identity keep those controls hidden, and mutations
  retain the same header guard.

## [0.9] - 2026-08-14

- Opened MChan beta testing to the public and synchronized the package,
  homepage, README, and changelog release versions.
- Added board lifecycle management for creating, archiving, and restoring
  boards.
- Added direct moderator hide actions for threads and replies with optional
  notes.
- Added structured report and moderation reasons with transactional audit
  records.
- Added moderator and admin navigation links for staff tools.
- Simplified the homepage branding to use the `static/mlogo.png` wordmark
  directly, without the separate MChan heading or character icon.
- Centered the homepage MChan wordmark within the header.
- Removed the MChan girl character icon from the homepage header.
- Added the approved `/asid/` board for UiTM Dengkil discussion and made it a
  required product board: any configured startup allowlist retains `asid`, so
  stale VPS env-files cannot hide it from dev or production. Configured board
  slugs remain validated, while an unset allowlist retains migration statuses.
- Made the homepage wordmark scale between mobile and desktop viewport sizes.

## [0.8] - 2026-08-13

- Updated the README and repository documentation to reflect the current MChan
  closed beta, integrations, routes, deployment model, and local development
  workflow.
- Moved supporting project documents into `docs/` and cleaned generated local
  artifacts from the repository root.
- Added the homepage team credits for Anthonny “Kuumin”, Mxrza, Chifu, Forg,
  and JavanMyna.
- Added a “Powered by mchan-core” link to the official
  [mchan-core repository](https://github.com/MChanTeam/mchan-core).
- Standardized public branding on `MChan` and updated community wording to
  describe Malaysian higher-education communities.
- Added the Micon mascot beside the MChan homepage title.
- Simplified the beta moderation queue to Dismiss, Resolve, Hide, Lock, Board
  ban, and Site-wide ban by removing duplicate Remove and Quarantine controls.
- Added concise root `AGENTS.md` repository guidelines covering architecture,
  commands, conventions, runtime configuration, migrations, and deterministic
  testing patterns.
- Active boards now paginate at 20 threads per page.
- Archives now paginate at 50 threads per page.
- Added bounded SQLite `LIMIT`/`OFFSET` pagination queries.
- Limited recent-post previews to the displayed thread IDs.
- Added server-rendered Previous/Next pagination links.
- Invalid page parameters now return HTTP 400.

## [0.7] - 2026-08-12

- Added a lightweight health check for service monitoring and database availability.
- Expanded `GET /health` with service, package version, process uptime, and
  database status metadata for safe frequent polling.
- Added authenticated Discord moderation integration for handling existing reports.
- Discord moderators can dismiss or resolve reports and apply content, thread,
  board, and site moderation actions.
- Moderation actions are applied atomically and recorded in the moderation audit log.
- Added validation and predictable error handling for Discord moderation requests.
- Discord moderation operates independently from Miya and does not perform content
  classification.
- Added Miya text screening for allow, review, and block decisions.

## [0.6] - 2026-08-11

- Added optional JPEG, PNG, and WebP thread and reply attachments through
  `mchan-image`.
- Limited uploads to 20 MiB.
- Added display and thumbnail rendering for supported attachments.
- Added atomic media persistence with compensation when a post cannot be
  completed.
- Added this public changelog.

## [0.5]

Closed-beta baseline preceding the v0.6 release.
