# Repository Guidelines

## Project Overview

MChan is a Rust/Axum server-rendered anonymous Malaysian university imageboard.
The current beta is text-first: users can browse approved boards, create
anonymous threads, reply anonymously, receive thread-scoped public poster IDs,
report threads and replies, and use the complete protected moderation flow.
Image uploads, archives, search, board proposals, CAPTCHA, and accounts remain
deferred.

Keep the implementation lean and honest about unfinished scope. Do not introduce accounts, frontend frameworks, PostgreSQL, Redis, object storage, microservices, Kubernetes, or other platform complexity without an explicit product decision.

## Architecture & Data Flow

- `src/main.rs` is the HTTP boundary and process entrypoint.
  - Starts Tokio.
  - Reads `DATABASE_URL` with fallback `sqlite://mchan.db`.
  - Requires `MCHAN_ABUSE_KEY` (64 hexadecimal characters) for encrypted post
    origins and purges expired origins at startup and hourly.
  - Creates the SQLite pool and runs embedded SQLx migrations.
  - Builds the Axum router, validates forms, applies rate limits and active
    bans, sets the anonymous cookie, renders Askama templates, and maps errors
    to HTTP responses.
- `src/forum.rs` is the forum data/domain boundary.
  - Owns `Board`, `Thread`, `Reply`, report, moderation, ban, and abuse-log
    models.
  - Owns SQLx queries and persistence invariants such as approved boards,
    visible/hidden/locked content, atomic moderation audits, and ban scopes.
  - Returns `Result` for database errors and `Option`/`bool` for absent domain
    records.
- `AppState` contains the SQLite pool and a process-local in-memory rate limiter, shared through `Arc`.
- Request flow is generally:

  ```text
  Axum route → extractor/form validation → rate limit/ban check → forum SQL
  function → Askama response/redirect
  ```
- `migrations/*.sql` are the schema and seed-data source of truth. Do not edit local `*.db` files as a substitute for migrations.
- Anonymous identity uses an HttpOnly `mchan_anon` UUID cookie. A SHA-256 hash of that token plus the thread ID produces the public thread-scoped poster label; the stored `poster_id` is rendered to every viewer.
- Reports are stored as `pending`; moderator actions transition them to
  `resolved` or `dismissed` and record an audit row in the same transaction.

## Key Directories

- `src/` — Rust application code; `main.rs` is the entrypoint, `forum.rs` is
  the SQL/data and moderation module, and `abuse.rs` protects origin records.
- `migrations/` — ordered SQLx migrations and deterministic seed content.
- `templates/` — standalone Askama HTML pages, including the moderation queue
  and restricted abuse-log view.
- `static/` — external CSS and the bundled LainPet JavaScript/assets.
- `.github/workflows/` — CI, Docker build, and `dev` branch deployment.
- `.dev-data/` — ignored local development database directory.

## Development Commands

```bash
# Run the server; applies pending migrations at startup
cargo run

# Compile
cargo check

# Run the current test command
cargo test

# Rust formatting check
cargo fmt --all -- --check

# Format Rust and SQL
make format

# Check Rust and SQL formatting
make format-check

# Local Docker workflow
docker build -t mchan .
docker run --rm --name mchan -p 3000:3000 mchan
```

The application listens on port `3000`. Useful local routes include `/`,
`/boards/engineering`, `/boards/b`, `/threads/{id}`, `/mod/reports`, and
`/mod/abuse-logs`. Moderator routes require
`Cf-Access-Authenticated-User-Email` and an allowlisted
`MCHAN_MODERATOR_EMAILS` value. Generate the required encryption key with
`openssl rand -hex 32` and provide it as `MCHAN_ABUSE_KEY`; VPS deployments
must load secrets from `/etc/mchan/mchan.env` via `--env-file`. Trust the
Cloudflare identity header only when the origin is isolated behind the Tunnel.

CI runs formatting, `cargo build`, `cargo test`, and a Docker build. A push to
`dev` also deploys the image to the configured VPS through the restricted SSH
receiver.

## Code Conventions & Common Patterns

- Rust edition 2024; use `snake_case` for functions/fields and `PascalCase` for
  types.
- Use `pub(crate)` for interfaces shared between `main.rs` and `forum.rs`; keep
  SQL row structs private.
- Keep HTTP concerns in handlers and SQL/domain concerns in `forum.rs`.
- Trim form strings before validation. Current limits include thread titles up
  to 120 characters and thread/reply bodies up to 10,000 characters.
- Use `Result<..., sqlx::Error>` in forum functions and propagate errors with
  `?`.
- Use `Option`/`bool` for expected missing records; handlers turn missing
  resources into the rendered 404 response.
- Filter public reads by board/content status. Approved boards and visible
  content are public; hidden content is excluded; locked threads remain
  readable but reject replies.
- Hide, remove, and quarantine all map the target to `hidden`; lock is valid
  only for a visible thread. Dismiss and resolve change only report status.
- Use transactions when a write requires multiple dependent database operations,
  including moderation status plus audit writes and bans.
- Keep responses server-rendered through Askama. Use redirects after successful
  mutations where the flow does not require a special response page.
- Keep CSS in `static/style.css`; do not introduce inline styling or a frontend
  framework for small UI changes.
- SQL status values are constrained in migrations. Preserve existing status
  vocabulary unless changing the schema deliberately.
- The current rate limiter is intentionally process-local and single-server:
  threads 2/minute, replies 10/minute, reports 5/minute.

## Important Files

- `src/main.rs` — startup, routes, handlers, validation, identity cookie, rate
  limits, moderator guard, action dispatch, decryption, and cache headers.
- `src/forum.rs` — forum models, SQLx row mappings, public reads/writes,
  moderation transitions, bans, audit rows, and retention purge.
- `src/abuse.rs` — `MCHAN_ABUSE_KEY` validation, encrypted origin protection,
  fingerprints, and decryption.
- `migrations/0001_create_boards.sql` — boards schema and Engineering seed.
- `migrations/0002_create_threads.sql` — threads schema and seed threads.
- `migrations/0003_create_replies.sql` — replies schema and seed replies.
- `migrations/0004_add_random_board.sql` — permanent approved `/b/` Random board.
- `migrations/0005_add_poster_ids.sql` — stored poster labels with legacy
  `Anonymous` defaults.
- `migrations/0006_create_reports.sql` — pending/resolved/dismissed reports and
  target constraint.
- `migrations/0007_seed_random_board_content.sql` — additional deterministic
  board content.
- `migrations/0008_create_moderation_actions.sql` — initial audit table.
- `migrations/0009_complete_moderation.sql` — complete action vocabulary,
  encrypted origins, bans, and abuse-log access audit table.
- `templates/thread.html` — thread/reply rendering, reply form, and reports.
- `templates/mod_reports.html` — protected queue and moderation/ban forms.
- `templates/abuse_logs.html` — restricted decrypted-origin view.
- `MODERATION_SPEC.md` — current moderation contract and operational setup.
- `static/style.css` — shared visual styling.

## Runtime/Tooling Preferences
- `MCHAN_ABUSE_KEY` is required at startup and must be 64 hexadecimal
  characters; generate it with `openssl rand -hex 32`. `MCHAN_MODERATOR_EMAILS`
  is the case-insensitive comma-separated moderator allowlist.
- The Docker image creates `/data`, but application startup defaults to a
  relative `sqlite://mchan.db`; deployments must explicitly provide
  `DATABASE_URL` and persistent storage when persistence matters.
- VPS deployments keep runtime values in `/etc/mchan/mchan.env` and pass
  `--env-file /etc/mchan/mchan.env`; never commit that file or its secrets.
- Runtime stack: Tokio, Axum, Askama, SQLx, and SQLite.
- Docker uses a multi-stage `rust:1-alpine` build and an Alpine runtime as non-root user `mchan`.
- No Node/Bun runtime or package manager is required.
- SQL formatting uses `syntaqlite==0.7.1`; Rust formatting uses `rustfmt`.
- `syntaqlite 0.7.1` drops `ALTER TABLE ADD COLUMN` constraints when formatting `0005_add_poster_ids.sql`. CI explicitly excludes that already-applied migration from the SQL formatting check. Do not reformat or rewrite applied migrations casually; SQLx checks migration checksums.
- Local SQLite files, `.dev-data`, logs, environment files, editor files, and build artifacts are ignored by `.gitignore`.

## Testing & QA

The repository has eight deterministic tests covering encrypted-origin
round-trips and tamper rejection, moderation action parsing, atomic status and
audit transitions, lock target validation, unavailable targets, expired-origin
ban rejection, board/site ban limits, and retention cleanup.

The moderation milestone has also been HTTP-smoke-tested for all six
content/report actions, board and site bans, protected abuse-log access and
access auditing, `no-store`/`no-cache` headers, and startup retention purge.

For changes, at minimum run:

```bash
cargo fmt --all -- --check
cargo check
cargo test
```

Also smoke-test the affected HTTP path. Important general contracts include:

- approved and unknown board routes;
- thread/reply creation and validation errors;
- 404 behavior for missing resources;
- thread-scoped anonymous-ID consistency and cross-thread separation;
- 429 behavior for thread, reply, and report limits;
- report insertion and status filtering;
- migration application against a fresh database and an existing database.

Add focused deterministic tests when introducing new behavior, especially for
validation, rate-limit boundaries, status transitions, migrations, and
report/moderation actions. Avoid tests that only assert source text or
incidental defaults.
