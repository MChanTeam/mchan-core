# Moderation Specification

## Purpose

MChan moderation must remain small, auditable, and recoverable. Public reports
create review work; reports must never automatically hide content or delete
records.

The current Open Beta has a protected, read-only pending-report queue. The next
implementation work is moderation actions and audit history.

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

### Moderator queue

```text
GET /mod/reports
```

The queue:

- requires an allowlisted Cloudflare Access email;
- reads the `Cf-Access-Authenticated-User-Email` header;
- uses `MCHAN_MODERATOR_EMAILS`, a comma-separated lowercase-insensitive list;
- returns HTTP 403 when the header is absent or not allowlisted;
- displays pending thread and reply reports;
- orders reports oldest first by `created_at`, then `id`;
- links thread reports to the original post;
- links reply reports to the specific reply anchor;
- does not mutate reports or content.

The Access header is trusted only when the application origin is unreachable
except through the Cloudflare Tunnel. The VPS firewall must not expose port
3000 directly to the public internet.

## Data invariants

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
- a report never changes content status by itself;
- hidden content remains in the database for review and audit.

A report targets exactly one object: either a thread or a reply. The existing
SQLite check constraint enforces this.

## Next teammate task: moderation actions

Implement actions as a small vertical slice. Do not add dashboards, analytics,
automatic moderation, or a frontend framework.

Recommended endpoints:

```text
POST /mod/reports/{id}/dismiss
POST /mod/reports/{id}/resolve
POST /mod/reports/{id}/hide
POST /mod/reports/{id}/lock
```

Every action must:

1. authenticate the moderator using the same queue guard;
2. parse and validate the report ID;
3. load the report and require `status = 'pending'`;
4. apply the requested report/content transition atomically;
5. return a redirect to `/mod/reports`;
6. return a useful 404 or 409 when the report is missing or already handled;
7. never delete the original report or post row.

Suggested transition rules:

| Action | Report status | Content change |
| --- | --- | --- |
| Dismiss | `dismissed` | none |
| Resolve | `resolved` | none |
| Hide thread report | `resolved` | thread → `hidden` |
| Hide reply report | `resolved` | reply → `hidden` |
| Lock thread report | `resolved` | thread → `locked` |

The data layer should expose one transaction-oriented function per meaningful
action, or one carefully constrained action function. Keep SQL in `src/forum.rs`
and HTTP/authentication in `src/main.rs`.

## Audit migration

Before deploying actions, add a new SQLx migration for an audit table. It should
record at least:

```text
id
report_id
moderator_email
action
target type and target id
created_at
optional note
```

Audit rows must be inserted in the same transaction as the report/content
change. Audit records must not be editable through the public HTTP surface.

## Required behavior tests

Add deterministic tests for:

- missing moderator header → 403;
- unknown moderator email → 403;
- allowlisted moderator email → queue access;
- pending reports ordered oldest first;
- resolved and dismissed reports excluded from the queue;
- thread report and reply report links target the correct post;
- dismiss changes only the report status;
- hide thread hides the thread from public reads;
- hide reply hides the reply from public reads;
- lock keeps the thread readable but rejects new replies;
- repeated action on a handled report does not apply a second transition;
- audit row and moderation change commit or roll back together.

Tests must defend observable behavior and should use a fresh SQLite database or
an isolated fixture. Do not test source text or template formatting details.

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
- board bans or CAPTCHA before the queue/actions path is proven.

## Verification before merge

Run:

```bash
cargo fmt --all -- --check
cargo check
cargo test
```

Also smoke-test the protected queue and each action against a fresh SQLite
 database. Confirm that a backup can restore the moderation tables and content
statuses before enabling the workflow for Open Beta moderators.
