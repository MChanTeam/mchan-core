# MChan

MChan is a server-rendered anonymous Malaysian higher-education imageboard. Visitors can
read approved boards, create text threads, reply anonymously, and report content
without accounts. Moderators review reports through protected web and Discord
interfaces.

The current release is an open beta. The application is intentionally small:
Rust, Tokio, Axum, Askama, SQLx, and SQLite. It does not require a frontend
framework, Node, PostgreSQL, Redis, or object storage.

## Current beta

Implemented:

- Approved boards with public indexes and thread pages.
- Anonymous threads and replies with thread-scoped public poster IDs.
- Optional JPEG, PNG, and WebP attachments through the external `mchan-image`
  processor. Text-only posting continues when the processor is disabled or
  unavailable.
- Board reply counts and the three newest replies on each board summary.
- Public, read-only archives selected by `archived_at`.
- Public reports for threads and replies, limited to five reports per client per
  60 seconds.
- Protected moderator reports and abuse-log views.
- Dismiss, resolve, hide, remove, quarantine, lock, board-ban, and site-ban
  actions with atomic audit records.
- Encrypted post-origin records, protected access auditing, and 30-day cleanup.
- Optional Cloudflare Turnstile checks for suspicious posting activity.
- Optional Miya text screening. Screening can allow, flag for review, or block a
  submission; it does not replace human moderation.
- Optional authenticated Discord moderation for existing reports.
- Health and changelog endpoints.

Not implemented:

- Accounts, persistent pseudonyms, or institutional verification.
- Search.
- Board proposals or administrator proposal review.
- Author deletion tokens.
- Video, audio, or multi-file uploads.
- Public moderator identities, popularity metrics, recommendation feeds, or
  automated report-count takedowns.

## Repository layout

```text
src/
  main.rs              Runtime configuration, migrations, and TCP serving
  http/                Axum routes, handlers, rendering, and HTTP contract tests
  forum.rs             SQLite domain queries and moderation invariants
  abuse.rs             Encrypted origin protection and fingerprints
  captcha.rs           Turnstile configuration and verification
  media.rs             HTTP adapter for optional image processing
  miya.rs              HTTP adapter for optional text screening
migrations/            Ordered SQLx migrations and seed data
templates/             Askama HTML templates
static/                CSS, JavaScript, and bundled static assets
docs/                  Operations, moderation, context, policy, and changelog documents
deploy/                VPS-side deployment scripts
Dockerfile             Multi-stage production image
Makefile               Rust and SQLite formatting commands
```

## Local development

Requirements: Rust/Cargo. Docker is optional. No Node or package manager is
required.

The default database URL is `sqlite://mchan.db`. Use `DATABASE_URL` to point to a
different SQLite database. Local databases and development data are ignored by
Git; `.dev-data/` is the recommended location for disposable local state.

`MCHAN_ABUSE_KEY` is required and must be exactly 64 hexadecimal characters.
Generate one with:

```sh
openssl rand -hex 32
```

Run the application directly:

```sh
export MCHAN_ABUSE_KEY='paste-a-64-character-key-here'
export MCHAN_ADMIN_EMAILS='admin@example.com'
# Board-moderator assignments are lowercase email rows in SQLite.
# Optional board policy (startup always retains required `asid`):
# export MCHAN_ENABLED_BOARD_SLUGS='engineering,b,asid'
# Optional integrations:
# export MCHAN_IMAGE_SERVICE_URL='http://127.0.0.1:3001'
# export MCHAN_MIYA_URL='http://127.0.0.1:8000'
# export MCHAN_TURNSTILE_SITE_KEY='site-key'
# export MCHAN_TURNSTILE_SECRET_KEY='secret-key'
# export MCHAN_DISCORD_MODERATION_TOKEN='shared-secret'
cargo run
```

Startup applies all embedded migrations. The server listens on port `3000`.
`MCHAN_ENABLED_BOARD_SLUGS` is optional; when set, it is a trimmed,
deduplicated comma-separated list of known board slugs, and startup always
retains the required `asid` board even when a stale VPS env-file omits it.
Malformed or unknown configured slugs still fail startup. The deployment lists
are `engineering,b,asid` on dev and `b,pasum,asid` on production.

### Optional integrations

- `MCHAN_IMAGE_SERVICE_URL` enables the `mchan-image` HTTP processor. The core
  service sends uploads for validation and processing, stores only processed
  media metadata, and serves the resulting files from `/images/...` under
  `MCHAN_MEDIA_STORAGE_ROOT` (default `/data`).
- `MCHAN_MIYA_URL` enables text screening. An unavailable screening service does
  not silently block publication; the post is published with a screening audit
  note for moderator attention.
- Set both `MCHAN_TURNSTILE_SITE_KEY` and `MCHAN_TURNSTILE_SECRET_KEY` to enable
  Turnstile. The second thread attempt and sixth reply are challenged in separate
  60-second namespaces. Ordinary limits are two threads and ten replies per
  minute. `MCHAN_TURNSTILE_VERIFY_URL` may override the verification endpoint
  subject to the HTTPS/loopback validation described in
  [`docs/MODERATION_SPEC.md`](docs/MODERATION_SPEC.md).
- `MCHAN_DISCORD_MODERATION_TOKEN` enables the authenticated internal Discord
  moderation endpoint. See [`docs/OPERATIONS.md`](docs/OPERATIONS.md); keep the
  endpoint on a private network or localhost/restricted proxy.

### Docker

Build and run with disposable ignored development data:

```sh
docker build -t mchan .
MCHAN_ABUSE_KEY=$(openssl rand -hex 32) \
MCHAN_ADMIN_EMAILS='admin@example.com' \
  docker run --rm \
  --name mchan \
  -p 3000:3000 \
  -e MCHAN_ABUSE_KEY \
  -e MCHAN_ADMIN_EMAILS \
  -v "$PWD/.dev-data:/data" \
  mchan
```

Open <http://localhost:3000>. For persistent deployments, use a dedicated
persistent directory and an environment file outside the repository.

## Roles and access

MChan has exactly two web roles; it has no accounts, sessions, or RBAC
framework. Global admins are the normalized lowercase emails listed in the
comma-separated `MCHAN_ADMIN_EMAILS` environment variable. Board moderators are
normalized lowercase email assignments in SQLite's `board_moderators` table and
are scoped to those boards. Admins are global. Assigned moderators can view and
handle reports, and directly hide, lock, or pin content, only on their assigned
boards.

| Page or action | Route | Access |
| --- | --- | --- |
| Admin home and board management | `/admin*` | Admins only |
| Home and board list | `GET /` | Public; staff links/controls for admins or assigned moderators |
| Health check | `GET /health` | Public |
| Community rules | `GET /rules` | Public |
| Privacy policy | `GET /privacy` | Public |
| Changelog | `GET /changelog` | Public |
| View a board | `GET /boards/{slug}` | Public; assigned moderators get own-board controls |
| Read-only archive | `GET /boards/{slug}/archive` | Public |
| New-thread form | `GET /boards/{slug}/new` | Public |
| Create a thread | `POST /boards/{slug}/threads` | Public |
| View a thread | `GET /threads/{id}` | Public; assigned moderators get own-board controls |
| Add a reply | `POST /threads/{id}/replies` | Public unless locked/archived |
| Report a thread | `POST /threads/{id}/report` | Public |
| Report a reply | `POST /replies/{id}/report` | Public |
| Processed media | `GET /images/...` | Public |
| Moderator queue | `GET /mod/reports` | Admins: all boards; moderators: assigned boards |
| Apply report action | `POST /mod/reports/{id}/{action}` | Admins: all actions; moderators: own-board non-ban actions |
| Direct hide content | `POST /mod/threads/{id}/hide` or `/mod/replies/{id}/hide` | Admins or assigned board moderators, own board |
| Pin/unpin thread | `POST /mod/threads/{id}/pin` or `/unpin` | Admins or assigned board moderators, own board |
| Protected abuse logs | `GET /mod/abuse-logs` | Admins only |
| Discord moderation | `POST /internal/discord/moderate` | Separate bearer token |

Home, board, and thread pages remain public and anonymous. They inspect the
Cloudflare Access identity only to render staff links and direct hide/lock/pin
controls for the matching global admin or assigned board moderator; they do not
create a browser session. Protected browser routes use the same identity
header. Trust that header only when the origin is isolated behind the
Cloudflare Tunnel. Discord moderation is separately authenticated with
`MCHAN_DISCORD_MODERATION_TOKEN`.

## Deployment

CI runs formatting, `cargo build`, `cargo test`, and a Docker build. The `dev`
workflow deploys the dev image to loopback port `3000`; the `main` workflow
deploys production to loopback port `3001`. Runtime environment files and
secrets stay on the VPS, outside GitHub Actions and this repository.

- [`docs/OPERATIONS.md`](docs/OPERATIONS.md) — health polling and Discord
  moderation operations.
- [`docs/MODERATION_SPEC.md`](docs/MODERATION_SPEC.md) — moderation contract,
  trust boundary, bans, retention, and verification behavior.
- [`deploy/deploy-mchan`](deploy/deploy-mchan) — dev deployment receiver script.
- [`deploy/deploy-mchan-prod`](deploy/deploy-mchan-prod) — production deployment
  receiver script.

Keep both application ports loopback-only. Production's Cloudflare Tunnel must
route to `http://localhost:3001`; dev remains on `http://localhost:3000`.

## Releasing

To prepare a release, first edit the `## [Unreleased]` notes in
[`docs/CHANGELOG.md`](docs/CHANGELOG.md). Then run the release helper with the
new package version:

```sh
make release VERSION=9.2
```

The helper updates `Cargo.toml` and `Cargo.lock`, promotes the Unreleased notes
to the versioned changelog entry, and leaves a new empty Unreleased section.
The homepage reads its version from the package metadata automatically, so no
manual homepage or version synchronization is needed. Inspect the generated
diff, run the consistency check, then commit and tag the release:

```sh
git diff
make release-check
git add Cargo.toml Cargo.lock docs/CHANGELOG.md
git commit -m "Release v9.2"
git tag v9.2
```

CI runs `make release-check` before building and testing both development and
production workflows.

## Verification and formatting

Run the standard checks before submitting a change:

```sh
cargo fmt --all -- --check
cargo check
cargo test
```

For Rust and SQL formatting setup/checks:

```sh
make install-formatters
make format-check
```

Router contract tests exercise the assembled HTTP router with fresh migrated
SQLite databases. The suite covers public reads, posting and validation,
anonymous cookies and IDs, archives, media-ready mapping, Turnstile outcomes,
reports, bans, moderation actions, abuse-log access, Discord moderation, health
responses, and cache headers.

## Project policies

`docs/PRIVACY.md` and `docs/RULES.md` are the published policy sources rendered
at `/privacy` and `/rules`. The domain vocabulary and screening terminology are in
[`docs/CONTEXT.md`](docs/CONTEXT.md). Keep public post data separate from
restricted operational logs, preserve existing SQL status vocabulary, and keep
HTTP concerns in `src/http/` while SQL and domain invariants remain in
`src/forum.rs`.
