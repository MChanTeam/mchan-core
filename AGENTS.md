# Repository Guidelines

## Project Overview

MChan is a Rust/Axum server-rendered anonymous Malaysian higher-education imageboard.
The current beta is text-first: users can browse approved boards, create
anonymous text threads, reply anonymously, receive thread-scoped public poster
IDs, see board reply counts and the three newest replies, read public archives,
report threads and replies, use the complete protected moderation flow, and
read the published policy pages. The public post model is media-ready through
`post_media`, but uploads are not enabled.

Optional suspicious-use Cloudflare Turnstile checks are part of the current
beta. Image uploads, search, board proposals, and accounts remain future work.
Keep the implementation lean and honest about unfinished scope. Do not
introduce accounts, frontend frameworks, PostgreSQL, Redis, object storage,
microservices, Kubernetes, or other platform complexity without an explicit
product decision.

## Architecture & Data Flow

- `src/main.rs` is the process bootstrap.
  - Starts Tokio and reads runtime environment configuration.
  - Creates the SQLite pool, runs embedded SQLx migrations, applies the optional
    board policy, and purges expired origins at startup and hourly.
  - Constructs explicit `http::HttpDependencies`, binds port `3000`, and serves
    the assembled Router.
- `src/http/` is the deep HTTP application module.
  - Owns Axum route assembly, private handlers and state, Askama contexts,
    validation, anonymous cookies, moderator access, rate limits, bans,
    Turnstile challenge flow, response mapping, and cache headers.
  - Exposes only the crate-private dependency constructor and Router builder.
  - Organizes public, posting, and moderation routes as private implementation
    modules; Router contract tests exercise the same HTTP seam as production.
- `src/forum.rs` is the forum data/domain boundary.
  - Owns `Board`, `Thread`, `Reply`, optional `Media`, report, moderation, ban,
    archive, and abuse-log models.
  - Owns SQLx queries and persistence invariants such as approved boards,
    visible/hidden/locked content, `archived_at` read-only behavior, board
    counts/recent-three replies, atomic moderation audits, and ban scopes.
  - Returns `Result` for database errors and `Option`/`bool` for absent domain
    records.
- `src/captcha.rs` owns optional Turnstile configuration, URL validation,
  siteverify requests, and the crate-private verifier port. Production uses the
  real HTTP adapter; Router tests use a scripted adapter.
- `HttpDependencies` contains the migrated SQLite pool, moderator allowlist,
  abuse cipher, optional CAPTCHA verifier, and a private process-local rate
  limiter, shared through `Arc`.
- Request flow is generally:

  ```text
  Axum route → extractor/form validation → rate limit/ban/CAPTCHA check
  → forum SQL function → Askama response/redirect
  ```
- `migrations/*.sql` are the schema and seed-data source of truth. Do not edit
  local `*.db` files as a substitute for migrations.
- Migration `0010_read_only_foundation.sql` adds `archived_at` and the optional
  `post_media` table used by the media-ready public shape.
- Anonymous identity uses an HttpOnly `mchan_anon` UUID cookie. A SHA-256 hash
  of that token plus the thread ID produces the public thread-scoped poster
  label; the stored `poster_id` is rendered to every viewer.
- Reports are stored as `pending`; moderator actions transition them to
  `resolved` or `dismissed` and record an audit row in the same transaction.

## Key Directories

- `src/` — Rust application code; `main.rs` is process bootstrap, `http/` is the
  deep HTTP module, `forum.rs` owns SQL/data and moderation, and `abuse.rs`
  protects origin records.
- `migrations/` — ordered SQLx migrations and deterministic seed content.
- `templates/` — standalone Askama HTML pages, including the moderation queue
  and restricted abuse-log view.
- `static/` — external CSS and the bundled LainPet JavaScript/assets.
- `.github/workflows/` — CI, Docker builds, and separate `dev`/production
  deployments.
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
MCHAN_ABUSE_KEY=$(openssl rand -hex 32) docker run --rm --name mchan \
  -p 3000:3000 -e MCHAN_ABUSE_KEY mchan
```

The application listens on port `3000`. Useful local routes include `/`,
`/privacy`, `/rules`, `/boards/engineering`, `/boards/engineering/archive`,
`/boards/b`, `/threads/{id}`, `/mod/reports`, and `/mod/abuse-logs`. Moderator
routes require `Cf-Access-Authenticated-User-Email` and an allowlisted
`MCHAN_MODERATOR_EMAILS` value. Generate the required encryption key with
`openssl rand -hex 32` and provide it as `MCHAN_ABUSE_KEY`; VPS deployments
must load secrets from their environment file via `--env-file`. Trust the
Cloudflare identity header only when the origin is isolated behind the Tunnel.

`MCHAN_ENABLED_BOARD_SLUGS` is optional. Unset preserves the database's
approved boards. A configured comma-separated list is trimmed and deduplicated;
malformed entries or unknown slugs fail startup, and exactly the listed known
boards become approved. The dev VPS uses `engineering,b`; production uses
`b,pasum`. Random is `/b`; PASUM is `/pasum`.

Turnstile is optional. Set `MCHAN_TURNSTILE_SITE_KEY` and
`MCHAN_TURNSTILE_SECRET_KEY` together to enable it. The second thread attempt
(one prior attempt) and sixth reply (five prior attempts) are challenged in
separate 60-second namespaces; ordinary limits are two threads and ten replies
per minute. `MCHAN_TURNSTILE_VERIFY_URL` may override the default only with
HTTPS, except HTTP for `localhost`, `127.0.0.1`, or `::1`, without
credentials/fragments.

CI runs formatting, `cargo build`, `cargo test`, and a Docker build. A push to
`dev` runs `.github/workflows/rust.yml`, streams `mchan:ci` through its
dev-only forced SSH receiver, and uses `/etc/mchan/mchan.env`,
`/opt/mchan/data`, container `mchan-dev`, and loopback port `3000`. A push to
`main` runs `.github/workflows/production.yml` in GitHub Environment
`production`, streams `mchan:production` through a distinct forced receiver,
and uses the distinct `MCHAN_PRODUCTION_DEPLOY_KEY`,
`MCHAN_PRODUCTION_VPS_HOST`, `MCHAN_PRODUCTION_VPS_USER`, and
`MCHAN_PRODUCTION_VPS_KNOWN_HOSTS` names with `/etc/mchan/mchan-prod.env`,
`/opt/mchan/data-prod`, container `mchan-prod`, and loopback port `3001`.
The production Tunnel hostname must route to `http://localhost:3001`; dev
remains `http://localhost:3000`. Keep both ports loopback-only.

## Code Conventions & Common Patterns

- Rust edition 2024; use `snake_case` for functions/fields and `PascalCase` for
  types.
- Use `pub(crate)` for interfaces shared between `main.rs` and `forum.rs`; keep
  SQL row structs private.
- Keep HTTP concerns in handlers and SQL/domain concerns in `forum.rs`.
- Trim form strings before validation. Current limits include thread titles up
  to 120 characters, thread/reply bodies up to 2,000 characters, and the
  optional reporter message up to 400 characters.
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

- `src/main.rs` — runtime configuration, database/migration setup, board policy,
  retention scheduling, dependency construction, and TCP serving.
- `src/http/` — routes, private handlers/state, rendering, HTTP policy,
  request/response mapping, and Router contract tests.
- `src/forum.rs` — forum models, SQLx row mappings, public reads/writes,
  board counts/recent replies, archive queries, moderation transitions, bans,
  audit rows, and retention purge.
- `src/abuse.rs` — `MCHAN_ABUSE_KEY` validation, encrypted origin protection,
  fingerprints, and decryption.
- `src/captcha.rs` — optional Turnstile env parsing, HTTPS/loopback URL
  validation, and siteverify client.
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
- `migrations/0010_read_only_foundation.sql` — `archived_at` and `post_media`
  schema plus archived seed data.
- `templates/base.html` — shared page shell and policy footer.
- `templates/board.html` — board summaries, counts, recent-three replies, and
  archive link.
- `templates/archive.html` — public read-only archive index.
- `templates/thread.html` — thread/reply rendering, archive state, media,
  reply form, and reports.
- `templates/policy.html` — rendered wrapper for root policy Markdown.
- `templates/mod_reports.html` — protected queue and moderation/ban forms.
- `templates/abuse_logs.html` — restricted decrypted-origin view.
- `PRIVACY.md` and `RULES.md` — root policy sources rendered at `/privacy` and
  `/rules`.
- `docs/MODERATION_SPEC.md` — current moderation contract and operational setup.
- `static/style.css` — shared visual styling.

## Runtime/Tooling Preferences
- `MCHAN_ABUSE_KEY` is required at startup and must be 64 hexadecimal
  characters; generate it with `openssl rand -hex 32`. `MCHAN_MODERATOR_EMAILS`
  is the case-insensitive comma-separated moderator allowlist.
- Optional Turnstile uses paired `MCHAN_TURNSTILE_SITE_KEY` and
  `MCHAN_TURNSTILE_SECRET_KEY`; `MCHAN_TURNSTILE_VERIFY_URL` defaults to
  Cloudflare and accepts only HTTPS except loopback HTTP.
- The Docker image creates `/data`, but application startup defaults to a
  relative `sqlite://mchan.db`; deployments must explicitly provide
  `DATABASE_URL` and persistent storage when persistence matters.
- Dev VPS runtime uses `/etc/mchan/mchan.env`, mounts `/opt/mchan/data` to
  `/data`, and passes `--env-file /etc/mchan/mchan.env` to container `mchan-dev`.
- Production VPS runtime is isolated: `/etc/mchan/mchan-prod.env`, mounted
  `/opt/mchan/data-prod` to `/data`, `--env-file /etc/mchan/mchan-prod.env`,
  container `mchan-prod`, and loopback port `3001`. Never commit either env
  file or its secrets.
- Runtime stack: Tokio, Axum, Askama, SQLx, and SQLite.
- Docker uses a multi-stage `rust:1-alpine` build and an Alpine runtime as non-root user `mchan`.
- No Node/Bun runtime or package manager is required.
- SQL formatting uses `syntaqlite==0.7.1`; Rust formatting uses `rustfmt`.
- `syntaqlite 0.7.1` drops `ALTER TABLE ADD COLUMN` constraints when formatting `0005_add_poster_ids.sql`. CI explicitly excludes that already-applied migration from the SQL formatting check. Do not reformat or rewrite applied migrations casually; SQLx checks migration checksums.
- Local SQLite files, `.dev-data`, logs, environment files, editor files, and build artifacts are ignored by `.gitignore`.

## Testing & QA

The repository currently has 58 deterministic tests covering forum reads/writes,
anonymous IDs, archive and media-ready mapping, Turnstile configuration and
verification, board policy, moderation transactions, and the HTTP interface.
The moderation-domain subset covers encrypted-origin round-trips and tamper
rejection, action parsing, atomic status and audit transitions, lock target
validation, unavailable targets, expired-origin ban rejection, board/site ban
limits, and retention cleanup.

Router contract tests use fresh migrated SQLite databases and in-process HTTP
requests. They cover public board/thread/archive and policy reads, disabled and
missing resources, thread/reply creation, anonymous cookies, poster identity,
validation, archived/locked writes, bans, namespaced limits, scripted Turnstile
allow/reject/unavailable outcomes with draft preservation, reports, moderator
guards and actions, protected abuse-log decryption and access auditing, and
`no-store`/`no-cache` headers.

For changes, at minimum run:

```bash
cargo fmt --all -- --check
cargo check
cargo test
```

Also smoke-test the affected HTTP path. Important general contracts include:

- approved and unknown board routes, archive routes, and policy routes;
- thread/reply creation and validation errors;
- 404 behavior for missing resources;
- thread-scoped anonymous-ID consistency and cross-thread separation;
- 429 behavior for thread, reply, and report limits;
- optional CAPTCHA thresholds, verification failures, and draft preservation;
- report insertion and status filtering;
- migration application against a fresh database and an existing database.

Add focused deterministic tests when introducing new behavior, especially for
validation, rate-limit boundaries, archive state, media mapping, CAPTCHA
configuration, status transitions, migrations, and report/moderation actions.
Avoid tests that only assert source text or incidental defaults.
