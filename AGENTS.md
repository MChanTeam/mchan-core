# Repository Guidelines

## Project Overview

MChan is a Rust/Axum server-rendered anonymous imageboard for Malaysian
higher-education communities. The current closed beta supports approved boards,
anonymous text threads and replies, thread-scoped poster IDs, public archives,
reports, protected moderation, optional media processing, optional Miya text
screening, optional Turnstile checks, and Discord moderation. Keep the product
lean; do not add accounts, frontend frameworks, PostgreSQL, Redis, object
storage, Kubernetes, or other platform complexity without an explicit decision.

## Architecture & Data Flow

- `src/main.rs` is the Tokio bootstrap. It reads environment configuration,
  opens SQLite, runs embedded SQLx migrations, applies optional board policy,
  purges expired origins, builds `HttpDependencies`, and binds port `3000`.
- `src/http/` owns the HTTP seam: Axum routes, handlers, Askama contexts,
  validation, anonymous cookies, rate limits, CAPTCHA, moderator guards,
  response mapping, and cache headers. `HttpDependencies` injects the SQLite
  pool, abuse cipher, optional service adapters, media root, and process-local
  rate limiter. Router tests exercise this same seam.
- `src/forum.rs` is the domain/database boundary. It owns models and SQLx
  queries for boards, threads, replies, media, reports, moderation, bans,
  archives, and abuse logs. Keep SQL and persistence invariants here, not in
  handlers.
- `src/abuse.rs` protects retained origins with ChaCha20-Poly1305 and derives
  keyed fingerprints used for poster IDs, rate limits, and bans.
- `src/captcha.rs`, `src/media.rs`, and `src/miya.rs` define optional HTTP
  integration configuration and testable adapter seams. Production adapters
  are injected as traits/`Arc`; tests use scripted or loopback implementations.
- Request flow is generally:

  ```text
  Axum route -> extraction/validation -> rate limit/ban/CAPTCHA/screening
  -> forum transaction -> Askama HTML or redirect
  ```

Preserve these invariants:

- Public reads expose only approved boards, visible/locked threads, and visible
  replies. Hidden content is retained but excluded. Archived threads remain
  readable/reportable and reject replies; locked threads remain readable and
  reject replies.
- Moderation writes update report/content state and audit rows atomically.
  Dismiss/resolve change only report state; hide/remove/quarantine currently
  hide the target; lock is valid only for visible thread reports; bans require
  retained origins and resolve the report.
- Preserve SQL status vocabulary and migration checks. Do not bypass media path
  validation, origin encryption/retention, constant-time token checks, or
  `no-store` headers on sensitive responses.

## Key Directories

- `src/` — Rust application, HTTP module, domain SQL, security, and adapters.
- `src/http/tests/` — Router contract tests split into public, posting,
  moderation, and operations modules.
- `migrations/` — Ordered SQLx SQLite schema and deterministic seed data.
- `templates/` — Askama server-rendered pages; `base.html` is the shared shell.
- `static/` — The sole stylesheet, small page scripts, favicons, and LainPet
  assets.
- `docs/` — Operations guide, moderation specification, domain context, and
  changelog.
- `deploy/` — Executable POSIX-shell VPS deployment receivers; not local build
  scripts.
- `.github/workflows/` — Stable-Rust CI, Docker builds, and dev/production image
  streaming deployments.

## Development Commands

```sh
cargo run                         # Apply migrations and serve on :3000
cargo check                       # Compile without running
cargo test                        # Full deterministic suite
cargo test http::tests::public     # Focused HTTP filter example
cargo fmt --all -- --check        # Rust formatting check
make format-check                 # Rust + SQL formatting check
```

Run locally with a required 64-hex-character `MCHAN_ABUSE_KEY`:

```sh
export MCHAN_ABUSE_KEY="$(openssl rand -hex 32)"
cargo run
```

For Docker, use ignored local state: `docker build -t mchan .` then mount
`-v "$PWD/.dev-data:/data"` and publish `-p 3000:3000`.

## Code Conventions & Common Patterns

- Rust edition 2024; `snake_case` functions/fields, `PascalCase` types;
  `pub(crate)` cross-module interfaces and `pub(super)` HTTP handlers.
- Use async `Result` with `?` for database/service errors. Use `Option`/`bool`
  for expected absence; handlers map missing records to 404 and domain
  conflicts to the appropriate response.
- Trim and validate form fields at the HTTP boundary. Current limits are title
  120 characters, thread/reply bodies 2,000 characters, report details 400,
  and uploads 20 MiB.
- Use transactions for dependent writes, especially moderation state + audit,
  report + ban, post origin/media + content, and compensation cleanup.
- Keep responses server-rendered through Askama; redirect after successful
  mutations. Escape user text. Use `|safe` only for trusted policy Markdown.
- Preserve semantic labels, `alt`, ARIA, visible focus styles, lazy media, and
  reduced-motion behavior in templates/static changes. Keep all CSS in
  `static/style.css`.
- Name tests after behavior and outcome, for example
  `failed_media_insert_rolls_back_and_cleans_up_image`.

## Important Files

- `src/main.rs` — startup, environment parsing, migrations, lifecycle cleanup.
- `src/http/mod.rs` — dependency container and complete route assembly.
- `src/http/public.rs`, `posting.rs`, `moderation.rs`, `operations.rs` — route
  handlers grouped by concern.
- `src/forum.rs` — database models, queries, transactions, and domain tests.
- `src/abuse.rs` — encrypted origin and fingerprint implementation.
- `src/captcha.rs`, `src/media.rs`, `src/miya.rs` — optional integration ports.
- `templates/base.html`, `home.html`, `thread.html`, `mod_reports.html` — shared
  shell and primary public/moderation views.
- `migrations/` — schema source of truth; add a new numbered migration rather
  than editing an applied migration.
- `docs/OPERATIONS.md` — health polling and Discord moderation operations.
- `docs/MODERATION_SPEC.md` — moderation trust boundary, statuses, bans, and
  retention.
- `docs/CONTEXT.md` — required text-screening/media domain vocabulary.
- `docs/PRIVACY.md`, `docs/RULES.md` — policy Markdown embedded into `/privacy`
  and `/rules`.

## Runtime/Tooling Preferences

- Runtime: Tokio + Rust; package manager/build tool: Cargo. No Node, Bun, or
  JavaScript package manager is required.
- Storage: SQLite through SQLx. `DATABASE_URL` defaults to `sqlite://mchan.db`;
  local `.db` files, `.dev-data/`, `/data/`, and `target/` are ignored.
- Required: `MCHAN_ABUSE_KEY` (64 hex characters). Moderator web access uses
  `MCHAN_MODERATOR_EMAILS` plus a Cloudflare Access identity header; trust that
  header only behind the isolated Tunnel.
- Board seeds currently include `engineering`, `b`, `pasum`, and `asid`
  (UiTM Dengkil). The dev deployment explicitly enables
  `engineering,b,asid`; production enables `b,pasum`. Keep board seed
  migrations and deployment allowlists aligned when adding or changing boards.
- Optional: `MCHAN_ENABLED_BOARD_SLUGS`, `MCHAN_MEDIA_STORAGE_ROOT`,
  `MCHAN_IMAGE_SERVICE_URL`, `MCHAN_MIYA_URL`, paired
  `MCHAN_TURNSTILE_SITE_KEY`/`MCHAN_TURNSTILE_SECRET_KEY` with optional
  `MCHAN_TURNSTILE_VERIFY_URL`, and `MCHAN_DISCORD_MODERATION_TOKEN`.
- Formatting uses `rustfmt` and `syntaqlite==0.7.1`. CI excludes
  `migrations/0005_add_poster_ids.sql` from the SQL formatter because that
  formatter drops its `ALTER TABLE ADD COLUMN` constraints. Do not casually
  rewrite applied migrations because SQLx tracks migration checksums.
- Docker builds with `rust:1-alpine`, runs non-root on `alpine:3.22`, exposes
  `3000`, and expects persistent `/data` in deployment.
- Dev deploys loopback `127.0.0.1:3000`; production uses loopback
  `127.0.0.1:3001`. Secrets and environment files remain on the VPS.

## Testing & QA

- Rust built-in unit tests use `#[test]`; database/domain and HTTP tests use
  `#[sqlx::test(migrator = "MIGRATOR")]` with isolated migrated SQLite pools.
- The canonical HTTP harness is `src/http/tests/mod.rs`: use its router
  constructors, request builders, `oneshot` helper, scripted CAPTCHA/media,
  and loopback Miya server instead of external services.
- Fixtures come from migrations. Prefer looking up seeded rows by title/body
  over assuming IDs; direct SQL is appropriate for precise state/audit checks.
- Test observable contracts: validation boundaries, status transitions,
  archive/lock behavior, rate limits, CAPTCHA outcomes, media cleanup,
  transaction rollback, migration application, auth, and cache headers.
- Keep tests deterministic: fixed keys/tokens/IPs, isolated SQLx pools, local
  loopback servers, and temporary media roots. Environment-mutating tests must
  use the existing lock/cleanup pattern.
- Before yielding a permanent change, run at least:

  ```sh
  cargo fmt --all -- --check
  cargo check
  cargo test
  ```

  Also smoke-test the affected HTTP path. CI runs the same checks plus a Docker
  build; SQL formatting is checked separately by `make format-check`.
