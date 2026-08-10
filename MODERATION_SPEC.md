# Moderation Specification

## Purpose

MChan moderation is small, auditable, and recoverable. Public reports create
review work; a report never automatically changes content status and no report
or post row is deleted by moderation.

The current text-first Open Beta implements the protected report queue, all
content/report actions, board and site bans, encrypted post origins, protected
decrypted-log access, and 30-day cleanup.

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

`MCHAN_ABUSE_KEY` is mandatory and must be exactly 64 hexadecimal characters
(32 bytes). Generate it once with:

```sh
openssl rand -hex 32
```

Keep the same secret in the runtime environment while retained origin records
exist. On the VPS, put it together with `MCHAN_MODERATOR_EMAILS` and the
persistent `DATABASE_URL` in the VPS-only `/etc/mchan/mchan.env`; the deployed
container must be started with `--env-file /etc/mchan/mchan.env`. The key and
moderator list must not be committed to the repository.

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
- hidden threads are excluded from boards and thread pages;
- hidden replies are excluded from thread pages;
- locked threads reject new replies;
- a lock action is valid only for a pending report targeting a visible thread;
- hide, remove, and quarantine have distinct audit action names but all set the
  target thread or reply status to `hidden`;
- dismiss sets only the report to `dismissed`;
- resolve sets only the report to `resolved`;
- handled reports cannot receive a second transition;
- hidden content remains in the database for review and audit;
- every successful action updates the report and, when applicable, the content
  status, then inserts its audit row in one transaction.

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

The implementation has eight deterministic tests covering:

- abuse-key encryption/decryption and tamper/invalid-key rejection;
- parsing the six content/report actions;
- atomic status transitions and one audit row per handled report;
- lock rejection for reply reports;
- unavailable targets remaining pending without an audit row;
- expired origins not authorizing bans;
- board/site ban limits and origin retention cleanup.

HTTP smoke coverage exercises all six content/report actions, board and site
bans, protected abuse-log access, access-audit insertion, cache-protection
headers, and startup retention purge.

## Non-goals

Do not add these in the moderation slice:

- user accounts;
- persistent pseudonyms;
- image processing;
- Redis or PostgreSQL;
- automatic report-count thresholds;
- automated moderation models;
- public moderator identities;
- deletion of reports or posts;
- complex analytics;
- archives, search, board proposals, or CAPTCHA.

Those remain deferred product scope, not missing moderation actions.
