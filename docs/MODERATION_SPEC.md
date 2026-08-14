# Moderation Specification

## Purpose

MChan moderation is small, auditable, and recoverable. Public reports create
review work; a report never automatically changes content status and no report
or post row is deleted by moderation.

The current beta implements the protected report queue, all content/report
actions, board and site bans, encrypted post origins, protected decrypted-log
access, 30-day cleanup, public read-only archives, and optional suspicious-use
Turnstile checks. Archive state and challenge behavior are documented here
because they affect moderation and reporting paths.

## Runtime configuration and trust boundary

Moderator access requires both:

1. the email is allowed by the Cloudflare Access application; and
2. its normalized lowercase value appears in the comma-separated
   `MCHAN_MODERATOR_EMAILS` environment variable.

The application reads the identity from
`Cf-Access-Authenticated-User-Email` (case-insensitive HTTP header lookup).
This header is trustworthy only when the origin cannot be reached directly and
the VPS is published through the Cloudflare Tunnel. The VPS firewall must not
expose port `3000` to the public internet.

### Cloudflare Access and the moderator UI session

Cloudflare Access must protect the moderator bootstrap and moderator paths
without putting the public site behind Access. Configure the Access application
for these origin paths:

```text
/authenticate
/admin
/admin/*
/mod/*
```

Leave public pages, including `/`, `/boards/*`, and `/threads/*`, outside this
Access application so they remain anonymously readable. The origin must still
be reachable only through the Cloudflare Tunnel; do not expose port `3000`
directly.

`GET /authenticate` is the Access-protected bootstrap for the moderator UI. It
validates the `Cf-Access-Authenticated-User-Email` header with the same
allowlist guard described above, sets an opaque HMAC-authenticated
`__Host-mchan-moderator` cookie, and returns a `303` redirect to `/`. The
cookie is valid for eight hours (`Max-Age=28800`) and has `Path=/`, `Secure`,
`HttpOnly`, and `SameSite=Lax` attributes. The home and thread pages may
accept a valid cookie for their `is_moderator` UI flag and show UI-only
controls; the cookie is never an authorization credential. Every moderator
mutation route continues to require the current Cloudflare Access identity
header, and must not authorize the mutation from the cookie.

The cookie is checked against the current lowercase
`MCHAN_MODERATOR_EMAILS` allowlist, so removing an address takes effect without
waiting for cookie expiry. Do not expose or rely on the token's internal
format.

For production on the VPS, keep `MCHAN_ABUSE_KEY`, `MCHAN_MODERATOR_EMAILS`,
and the persistent `DATABASE_URL` in `/etc/mchan/mchan-prod.env`, and start the
deployed container with `--env-file /etc/mchan/mchan-prod.env`. The development
environment file remains `/etc/mchan/mchan.env`; it is not the production
deployment file. Neither file belongs in the repository.

`MCHAN_ABUSE_KEY` is mandatory and must be exactly 64 hexadecimal characters
(32 bytes). Generate it once with:

```sh
openssl rand -hex 32
```

Keep the same secret in the runtime environment while retained origin records
exist. The production VPS uses `/etc/mchan/mchan-prod.env` (as above); the
development environment uses `/etc/mchan/mchan.env`. The key and moderator
list must not be committed to the repository.

### Optional Turnstile checks

Turnstile is enabled only when both `MCHAN_TURNSTILE_SITE_KEY` and
`MCHAN_TURNSTILE_SECRET_KEY` are present. If both are absent, it is disabled;
supplying only one key or an empty key is a startup configuration error.
`MCHAN_TURNSTILE_VERIFY_URL` optionally overrides the default Cloudflare
siteverify endpoint. The override must be HTTPS, except HTTP for loopback
`localhost`, `127.0.0.1`, or `::1`, and it may not contain credentials or a
fragment.

The second thread attempt (one prior suspicious attempt) and sixth reply (five
prior suspicious attempts) are challenged in separate action namespaces with
60-second windows. Ordinary limits remain two threads and ten replies per
client per minute. Missing or failed challenges return the form with submitted
text preserved; verifier failures return HTTP 503. A successful challenge
proceeds through the ordinary limit.

## Current interface

### Public reporting

```text
POST /threads/{id}/report
POST /replies/{id}/report
```

Accepted reasons:

```text
spam
harassment
doxxing
threats
illegal
other
```

Invalid reasons return HTTP 400. Reports are rate-limited to five per client per
60 seconds. Successful reports redirect to the affected thread and create a
`pending` row in `reports`.

### Moderator queue and actions

```text
GET  /mod/reports
POST /mod/reports/{id}/{action}
GET  /mod/abuse-logs
```

The queue and every action:

- require the moderator guard described above;
- return HTTP 403 when the identity header is absent or not allowlisted;
- display pending thread and reply reports oldest first by `created_at`, then
  `id`;
- link thread reports to the original post and reply reports to their anchors;
- return a redirect to `/mod/reports` after a successful action;
- return a useful 404, 409, or 422 for missing, already handled, invalid-target,
  invalid-duration, or missing-origin requests;
- never delete the original report or post row.

`action` accepts `dismiss`, `resolve`, `hide`, `remove`, `quarantine`, `lock`,
`ban-board`, and `ban-site`. Ban actions require a form field named `days`.
Board bans accept 1–30 days; site-wide bans accept 1–365 days.

## Status and transition invariants

Existing status values are the source of truth:

```text
reports:  pending | resolved | dismissed
threads:  visible | hidden | locked
replies:  visible | hidden
```

Public behavior:

- visible threads and replies are rendered;
- locked threads remain readable;
- archived threads remain readable and reportable;
- hidden threads are excluded from boards and thread pages;
- hidden replies are excluded from thread pages;
- locked threads reject new replies;
- archived threads reject new replies regardless of lock state;
- a lock action is valid only for a pending report targeting a visible thread;
- hide, remove, and quarantine have distinct audit action names but all set the
  target thread or reply status to `hidden`;
- dismiss sets only the report to `dismissed`;
- resolve sets only the report to `resolved`;
- handled reports cannot receive a second transition;
- hidden content remains in the database for review and audit;
- every successful action updates the report and, when applicable, the content
  status, then inserts its audit row in one transaction.

Archive state is represented by a non-null `threads.archived_at` value added by
migration `0010_read_only_foundation.sql`. The archive index at
`GET /boards/{slug}/archive` selects those rows; the normal board index excludes
them. The thread page remains readable, but reply creation returns the archived
result and no new reply is inserted. Existing reports remain available.

A report targets exactly one object: either a thread or a reply. The SQLite
check constraint enforces this.

## Bans

Ban actions use the encrypted origin fingerprint stored for the reported post.
If the origin has expired or is absent, the ban is rejected and the report
remains pending without a ban or audit row.

- `ban-board` creates a board-scoped ban and resolves the report. It accepts
  1–30 days and blocks matching posts on that board.
- `ban-site` creates a site-scoped ban and resolves the report. It accepts
  1–365 days and blocks matching posts on every board.

Both ban rows and their `ban_board`/`ban_site` audit rows are committed with the
report transition. Active bans expire at their stored deadline; revoked bans
are not active.

## Encrypted abuse logs

Each post stores a protected origin record containing an encrypted client key,
an anti-abuse fingerprint, nonce, creation time, and retention deadline. The
public post views never display the decrypted key.

`GET /mod/abuse-logs` requires the same moderator guard, records an
`abuse_log_accesses` audit row, decrypts only retained records, and returns the
sensitive identifiers in a restricted view. The response includes:

```text
Cache-Control: no-store, private
Pragma: no-cache
```

Origin records have a 30-day retention deadline. Expired records are purged
once during startup and hourly while the process runs. The log query returns
only retained records (up to the current implementation's 100-row view limit).

## Tests and verification

The repository currently has 58 deterministic tests. They cover forum
reads/writes, anonymous IDs, validation and rate limits, archive state,
media-ready row mapping, optional Turnstile configuration/verification,
policy rendering, moderation contracts, and the assembled HTTP interface.

The moderation subset has eight deterministic tests covering:

- abuse-key encryption/decryption and tamper/invalid-key rejection;
- parsing the six content/report actions;
- atomic status transitions and one audit row per handled report;
- lock rejection for reply reports;
- unavailable targets remaining pending without an audit row;
- expired origins not authorizing bans;
- board/site ban limits and origin retention cleanup.

Router contract tests use fresh migrated SQLite databases and in-process HTTP
requests. They exercise public archive reads, archived reply rejection, policy
routes, draft-preserving namespaced Turnstile outcomes, content/report actions,
board and site bans, protected abuse-log access and auditing, and
cache-protection headers. Domain tests cover retention cleanup.

## Non-goals

Do not add these in the moderation slice:

- user accounts;
- persistent pseudonyms;
- image upload processing;
- search;
- board proposals;
- automatic report-count thresholds;
- automated moderation models;
- public moderator identities;
- deletion of reports or posts;
- complex analytics.

Public archives and optional Turnstile are implemented adjacent to moderation;
keep their current routes and invariants intact rather than omitting them from
the current product.
