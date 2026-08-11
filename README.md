# MChan MVP

This document defines MChan's product direction and MVP agreement for the current
four-person team. It is a working document. Update it when an implementation
decision changes.

## Project vision

MChan is an open, anonymous Malaysian university imageboard. It is intended
primarily for Malaysian university students, but access is public. University
verification is not required.

Anyone can:

- Read public boards.
- Create a thread without an account.
- Reply without an account.
- Report a thread or reply without an account.
- Propose a board without an account.

MChan supports optional one-image attachments on threads and replies alongside
text-only posting. The first closed test may focus on the Engineering Faculty
community, but the design must remain suitable for other Malaysian universities.

Media processing is delegated by Core to the optional `mchan-image` service over
internal HTTP. The architecture and current behavior are described below.

The main product loop is:

1. A visitor opens an approved board.
2. The visitor creates an anonymous thread, with optional image media.
3. Other visitors reply with text and optional image media.
4. Visitors report content that violates the rules.
5. Moderators review reports and take action.
6. Inactive threads enter a public, read-only archive.

## Open Beta scope

The current Open Beta includes public boards, anonymous threads and replies,
thread-scoped poster IDs, post numbers, board reply counts with the three newest
replies, basic rate limits, public reports for threads and replies, a public
read-only archive, and optional image attachments. Text-only posting remains
available when image processing is disabled or offline; an image attempt in that
state reports that image processing is unavailable.

Threads and replies can carry one optional `post_media` record with processed
thumbnail/display paths, MIME type, width, and height. Core delegates media
processing to the optional `mchan-image` service over internal HTTP.

The beta also includes the protected moderation queue and the complete
moderation action set: dismiss, resolve, hide, remove, quarantine, and lock,
plus temporary board bans and site-wide bans. It includes encrypted post
origins, a protected decrypted abuse-log view, access auditing, automatic
30-day cleanup, and optional suspicious-use Cloudflare Turnstile checks.

Published policy source files are `PRIVACY.md` and `RULES.md`, rendered at
`/privacy` and `/rules`. Accounts, persistent pseudonyms, search, board
proposals, and author deletion tokens remain future work.

Moderator operation requires both Cloudflare Access authentication and an
allowlisted runtime email. The application must remain reachable only through
the Cloudflare Tunnel; see the runtime configuration section below.

The remaining broader MVP requirements below remain product direction, not the
current Open Beta launch gate.

## Product character

MChan should be:

- Independent and community-controlled.
- Human-scale and built by a small team.
- Anonymous or pseudonymous where appropriate.
- Fast, direct, dense, and usable.
- Locally rooted, personal, and slightly strange.
- Recognizable without looking professionally manufactured.

MChan rejects corporate social-media and SaaS-style design, algorithmic feeds,
engagement farming, follower and karma systems, surveillance-based advertising,
mandatory real-name identity, unnecessary abstraction, and premature platform
complexity. These principles guide product and technical choices. They are not a
branding manifesto.

## MVP scope

### Included

- A public home page and board list.
- Administrator-approved boards.
- Public board indexes and full thread pages.
- Anonymous thread creation without an account.
- Anonymous replies without an account.
- Thread-specific anonymous poster IDs.
- Reports for threads and replies.
- A protected moderator queue with dismiss, resolve, hide, remove, quarantine,
  and thread-lock actions.
- Temporary board-specific bans of up to 30 days and site-wide bans of up to
  365 days.
- Atomic audit records for every moderation action and ban.
- Encrypted post-origin records, a protected decrypted abuse-log view, and
  automatic 30-day cleanup.
- Basic rate limits.
- Board thread summaries with reply counts and the three newest replies.
- A public read-only archive at `/boards/{slug}/archive`; archived threads
  remain readable and reject new replies.
- Optional one-image posting and processed-media rendering for threads and
  replies.
- Optional suspicious-use Turnstile checks for thread and reply posting.
- Server-rendered HTML through the shared `templates/base.html` layout.
- One external CSS file.
- Minimal JavaScript.
- A working 404 page.
- `/privacy` and `/rules`, sourced from the root policy Markdown files.
- Docker and local Cargo workflows.

### Not included in the MVP

- Normal user accounts.
- Persistent pseudonyms.
- Email verification or university verification.
- Account recovery.
- Public profiles.
- Private messages.
- Followers, karma, likes, reactions, or badges.
- Recommendation feeds or algorithmic ranking.
- Video or audio uploads.
- A mobile application.
- Automated moderation models.
- Complex moderator analytics.
- A sophisticated public transparency dashboard.
- Multiple archive privacy systems.
- Full multi-university administration.

Optional accounts and persistent pseudonyms may be considered later. The MVP
must not depend on them.

## Optional image architecture

Each thread or reply may include one optional image attachment. Accepted input is
JPEG, PNG, or WebP only; GIF and animated images are not supported. The upload
is limited to 20 MiB and an input image may be at most 10,000 × 10,000 pixels.

Core delegates media processing to the optional `mchan-image` service over
internal HTTP. `mchan-image` performs image decoding and processing; Core does
not decode uploads. The service returns processed WebP media: a display image
with a maximum 512 px longest dimension at Q80, and a thumbnail with a maximum
128 px longest dimension at Q80. Originals are never stored.

Core serves persisted processed media itself at same-origin `/images/...` URLs
from `<MCHAN_MEDIA_STORAGE_ROOT>/images`. The root defaults to `/data`, which
matches the Docker data volume, and local deployments can override it with
`MCHAN_MEDIA_STORAGE_ROOT`. Core and `mchan-image` mount the same named volume
at their service-specific roots (`/data` for Core and `/media` for the image
service), so paths returned by the processor are immediately retrievable; the
public paths are never derived from `MCHAN_IMAGE_SERVICE_URL`.
The processor writes only processed public media, using storage keys
`images/{image_id}/{variant}.webp` (for example, `display.webp` and
`thumbnail.webp`). Its container runs as UID `10001` with its storage volume
mounted read-write at `/media/images` and storage root `/media`. Core runs as
UID `1000` and mounts that same named volume read-only at the nested path
`/data/images` while retaining `/data` for SQLite and other application data.
The resulting processed media is public through Core's `/images/...` route.
The development and production volumes are distinct and are never shared.

Core owns processed-media lifecycle and deletion. If the database write fails
after processing succeeds, Core compensates by deleting the processed media.
When the adapter is disabled or offline, text-only posts remain available and
an image attempt returns an unavailable error.

## Anonymity and operational logs

Public posting does not require an account. Logged-out posts display:

- `Anonymous`.
- A temporary thread-specific poster ID.

Posts are anonymous to the public. MChan keeps limited private operational
records for abuse prevention, moderation, security, and lawful requests.

Keep public post data separate from restricted operational logs. The current
implementation stores an encrypted origin record for each post containing only
the limited abuse-prevention data needed by the service. The protected moderator
log view decrypts these records only for an authenticated, allowlisted moderator.
The response is marked `Cache-Control: no-store, private` and `Pragma: no-cache`;
access is recorded separately.

Origin records are retained for 30 days. Expired records are purged at startup
and hourly while the process is running. `MCHAN_ABUSE_KEY` is required to
encrypt and decrypt them and must be kept secret and stable for the lifetime of
the retained records.
Do not describe MChan as completely untraceable, and do not promise immunity
from identification.

## Post deletion

- Posts cannot be edited after publication.
- Authors cannot delete published posts in the current MVP.
- The original post of a thread cannot be deleted by its author after
  publication.
- Moderators and administrators can remove, redact, quarantine, or restrict
  any content when required for moderation, safety, privacy, or legal
  compliance.

"Permanent" means that the original poster cannot rewrite or erase the thread
after other people contribute. It does not mean that moderators cannot remove
unlawful or harmful material. Author deletion tokens are outside the current
MVP scope.

## Thread lifecycle and archives

Thread lifecycle is board-specific. Fast boards may use bump limits, reply
limits, and automatic expiry. Slower or information-focused boards may keep
threads active longer.

When a thread expires, MChan will:

1. Remove it from the active board index.
2. Make it read-only.
3. Move it into a public archive.
4. Preserve its text and processed media.
5. Keep it searchable inside MChan.
6. Prevent new replies.

MChan does not want community history to disappear by default. External
search-engine indexing can be controlled per board later; it is not an MVP
implementation requirement.

## Moderation

Site-wide minimum rules apply to every board. They cover:

- Illegal content.
- Doxxing and personal information.
- Credible threats.
- Targeted harassment.
- Spam.
- Malware.
- Explicit sexual media and pornography.
- Sexual solicitation.
- Abuse of the service.
- Anonymous or unverifiable accusations against identifiable private
  individuals.

Public figures, public organizations, universities, companies, and documented
public events may be discussed when relevant. Each board may add stricter topic,
format, and quality rules. A board cannot weaken the site-wide rules.

For the current implementation:

- Every report is created as `pending` and enters the protected moderator queue.
- Moderators may dismiss or resolve a report without changing content status.
- Hide, remove, and quarantine all set the reported thread or reply to
  `hidden`; the content stays in the database and disappears from public reads.
- Lock applies only to a visible thread. A locked thread remains readable but
  rejects new replies. A reply report cannot be locked.
- Every successful moderation action writes an atomic audit record. A handled
  report cannot be acted on a second time.
- A moderator may issue a board ban for 1–30 days or a site-wide ban for
  1–365 days when a retained protected origin is available. Bans use the
  encrypted origin fingerprint and resolve the report with a matching audit
  record.

Moderator access is protected by Cloudflare Access identity plus the
`MCHAN_MODERATOR_EMAILS` allowlist. The identity header is trusted only when the
origin is isolated behind the Cloudflare Tunnel.

## New-board proposals

Anyone may propose a new board without an account. Every proposal requires
administrator approval.

The MVP may use a basic proposal form and an administrator review page. It must
not add voting, automatic board creation, or a complex governance system.

## Technical constraints

Preserve the current lean stack:

- Rust.
- Tokio.
- Axum.
- SQLite.
- SQLx and migrations.
- Askama or another Rust server-side template system.
- HTML and CSS.
- Minimal JavaScript.
- Docker.
- One lean deployment initially.

Do not add a frontend framework. Do not make PostgreSQL, Redis, object storage,
microservices, Kubernetes, or cloud architecture current MVP requirements. They
may become necessary later, but they are not needed for this milestone.

## Team responsibilities

The current four-person team is:

### Backend developer (Kuumin)

Focus on:

- Routes and the database.
- Anonymous posting and thread-specific IDs.
- Media-processing integration and lifecycle.
- Reports and moderation.
- Rate limits.
- Restricted operational logs.
- Security review.
- Server-side validation.

### Frontend developer (Mxrza)

Focus on:

- Server-rendered board and thread views.
- Thread and reply forms.
- Image upload fields.
- Thumbnail and image display.
- Reply references.
- Reports and moderation forms.
- Archive and search views.
- Error and empty states.

### Artist and designer (Chifuyu)

Focus on:

- MChan's independent visual identity.
- Dense imageboard layouts.
- Board, thread, reply, catalog, archive, report, and moderation views.
- Typography, spacing, colour, icons, and small visual assets.
- Avoiding corporate or app-like presentation.

### Learning developer (JavanMyna)

Keep the small-task and learning role. Suggested areas:

- Seed data.
- Automated tests.
- Templates.
- CSS.
- Documentation.
- Simple routes.
- Image-validation test fixtures.
- Archive and search test cases.

Security-sensitive code, restricted logs, media processing, and moderation code
must receive backend review before merge.

## Milestone plan

A checked item means that the repository currently contains that part of the
implementation. An unchecked item is planned work. Requirements in this
section are not complete merely because they are listed.

### Milestone 1: Foundation

- [x] Start the Tokio and Axum application.
- [x] Add the SQLite connection and migrations.
- [x] Add the catch-all 404 page.
- [x] Serve the external CSS file.
- [x] Verify the local Cargo build and test commands.
- [x] Verify the Docker build and run workflow.
- [x] Add a shared HTML layout in `templates/base.html`.

### Milestone 2: Read-only imageboard foundation

- [x] Load approved boards from the database.
- [x] Show the home page and board list.
- [x] Show a board index with compact thread summaries.
- [x] Show a full thread page with the original post and replies.
- [x] Show reply counts and the three newest replies in board summaries.
- [x] Support direct links to individual posts within a thread.
- [x] Add the read-only archive view at `/boards/{slug}/archive`.
- [x] Add the public `post_media` shape and optional media rendering.

The current slice supports read-only browsing, anonymous text and optional
image threads and replies, thread-scoped public poster IDs, post numbers, board
counts/recent replies, public archives, processed media rendering, policy pages,
and the complete protected moderation flow. Search and board proposals remain
future work.

### Milestone 3: Open anonymous posting

- [x] Create threads without an account.
- [x] Reply without an account.
- [x] Add server-side validation for thread creation.
- [x] Generate thread-specific anonymous IDs.
- [x] Add post numbers and reply references.
- [x] Add basic rate limits.
- [x] Add an optional suspicious-use CAPTCHA integration point.

Turnstile is enabled only when both `MCHAN_TURNSTILE_SITE_KEY` and
`MCHAN_TURNSTILE_SECRET_KEY` are configured. It challenges the second thread
attempt after one prior attempt, or the sixth reply after five prior attempts,
in separate namespaced 60-second suspicious windows. Normal limits remain two
threads and ten replies per minute. The verification URL defaults to Cloudflare
and may be overridden only with HTTPS, except loopback HTTP for local testing.

### Milestone 4: Optional image attachments

- [x] Allow one optional image per thread or reply.
- [x] Accept JPEG, PNG, and WebP input only; reject GIF and animation.
- [x] Limit uploads to 20 MiB and input images to 10,000 × 10,000 pixels.
- [x] Delegate decoding and processing to `mchan-image` over internal HTTP.
- [x] Generate WebP display media up to a 512 px longest dimension at Q80.
- [x] Generate WebP thumbnails up to a 128 px longest dimension at Q80.
- [x] Never store originals; retain only processed media.
- [x] Let Core own media lifecycle/deletion and compensate DB failures.
- [x] Keep text-only posting available when the adapter is unavailable.
- [x] Render thumbnails in lists and display media on full-thread pages.

### Milestone 5: Moderation

- [x] Accept public reports for threads and replies.
- [x] Add the authenticated moderator queue.
- [x] Add dismiss and resolve report actions with audit records.
- [x] Add hide, remove, quarantine, and lock actions with exact status rules.
- [x] Add temporary board bans (1–30 days).
- [x] Add central site-wide bans (1–365 days).
- [x] Record audit records for every moderation action and ban.
- [x] Protect encrypted abuse-log access with decryption, access auditing, and
  `no-store`/`no-cache` response headers.
- [x] Apply 30-day origin retention with startup and hourly cleanup.
- [x] Add deterministic moderation and abuse-crypto tests (8 tests).

Keep the implementation simple. Do not build complex analytics or an automated
moderation platform.

### Milestone 6: Remaining archives, search, and closed test work

- [x] Add the public read-only archive route and archived thread behavior.
- [x] Retain archive records and processed-media fields when present.
- [ ] Search active and archived threads.
- [ ] Add the board proposal form and administrator approval flow.
- [ ] Add end-to-end tests for the remaining main flows.
- [ ] Run a small closed test.

Optional accounts are not an MVP milestone. Archive browsing and optional image
attachments are already implemented.

## Current routes

These routes are implemented in the current Open Beta:

| Page or action | Route | Access |
| --- | --- | --- |
| Home and board list | `GET /` | Public |
| Community rules | `GET /rules` | Public |
| Privacy policy | `GET /privacy` | Public |
| View a board | `GET /boards/{slug}` | Public |
| Read-only board archive | `GET /boards/{slug}/archive` | Public |
| New-thread form | `GET /boards/{slug}/new` | Public |
| Create a thread | `POST /boards/{slug}/threads` | Public |
| View a thread | `GET /threads/{id}` | Public |
| Add a reply | `POST /threads/{id}/replies` | Public unless archived |
| Report a thread | `POST /threads/{id}/report` | Public |
| Report a reply | `POST /replies/{id}/report` | Public |
| Moderator queue | `GET /mod/reports` | Authenticated moderator |
| Apply a report action | `POST /mod/reports/{id}/{action}` | Authenticated moderator |
| Protected abuse logs | `GET /mod/abuse-logs` | Authenticated moderator |

`action` is one of `dismiss`, `resolve`, `hide`, `remove`, `quarantine`, or
`lock`. It also accepts `ban-board` and `ban-site`; those actions require a
form field named `days` from 1–30 or 1–365 respectively. Moderator requests
require a Cloudflare Access identity header whose normalized email appears in
`MCHAN_MODERATOR_EMAILS`.

An archived thread is selected by its non-null `archived_at` value. It remains
readable and reportable, but reply creation rejects it; the archive index is
read-only. Search, board-proposal, administrator-review, and author-deletion
routes are not implemented. The MVP must not add verification routes or make
university verification part of access control.

## Lean data model

The target MVP does not require `users`, `verification_tokens`, university email
hashes, verified author IDs, or reporter account IDs. Keep public post data
separate from restricted operational logs.

Suggested concepts are intentionally broad. This is a milestone agreement, not a
complete database design.

### `boards`

- `id`
- `slug`
- `name`
- `description`
- `status`
- `media_mode`
- `created_at`

### `threads`

- `id`
- `board_id`
- `title`
- `body`
- `status`
- `bumped_at`
- `created_at`
- Anonymous poster identifier material.
- Encrypted or separately protected abuse-log reference.

### `replies`

- `id`
- `thread_id`
- `body`
- `status`
- `created_at`
- Anonymous poster identifier material.
- Encrypted or separately protected abuse-log reference.

### `post_media`

The optional processed-media record is attached to exactly one thread or reply:

- `id`
- `thread_id` or `reply_id`
- `thumbnail_path`
- `display_path`
- `mime_type`
- `width`
- `height`

The `post_media` table stores processed media paths and metadata returned by the
optional image processor. Originals are never stored.

### `reports`

- `id`
- Target post.
- Reason.
- Details.
- Status.
- Reviewer.
- Review time.
- Creation time.

Reports do not require a reporter account ID. Report priority may use report
count, but count alone must not remove content.

### `board_proposals`

- `id`
- Proposed slug.
- Name.
- Description.
- Reason.
- Status.
- Review information.
- Creation time.

### `moderation_actions`

- `id`
- Target.
- Action type.
- Broad reason.
- Moderator.
- Creation time.

Restricted operational records are separate from these public-facing concepts.
They contain only the limited encrypted abuse data described above and follow
its retention policy.

## Imageboard vocabulary

- **Board**: An administrator-approved topic community with a stable slug and a
  posting media mode.
- **Thread**: A discussion on a board with one original post and replies.
- **Original post (OP)**: The first post that creates a thread.
- **Reply**: A post added to an existing thread.
- **Post**: The shared public shape of an OP or reply: anonymous display, body,
  number, timestamp, references, moderation state, and optional processed
  `post_media`.
- **Board index**: The active list of threads on a board, including reply
  counts and the three newest replies for each summary.
- **Catalog**: A denser board view with many thread summaries and media. It is a
  later presentation improvement, not a separate ranking system.
- **Archive**: A public, read-only view of threads selected by `archived_at`.

## Current project structure

The implementation is intentionally small:

```text
src/
  main.rs              # Runtime configuration, database lifecycle, TCP serving
  http/                # Deep HTTP module: routes, handlers, rendering, Router tests
  forum.rs             # Board, post, archive, report, moderation, ban, and log queries
  abuse.rs             # Encrypted origin protection and decryption
  captcha.rs           # Turnstile configuration plus production/test verifier seam
migrations/
  ...                  # SQLite schema and seed migrations
  0009_complete_moderation.sql
  0010_read_only_foundation.sql  # archived_at and post_media
templates/
  base.html            # Shared shell and policy footer
  home.html             # Public home and board list
  board.html            # Board summaries, counts, recent replies, archive link
  archive.html          # Public read-only archive
  thread.html           # Public thread/reply rendering, media, and reports
  policy.html           # Rendered privacy/rules wrapper
  mod_reports.html      # Protected report queue and action forms
  abuse_logs.html       # Protected decrypted-origin view
static/
  style.css
PRIVACY.md              # Root source rendered at /privacy
RULES.md                # Root source rendered at /rules
```

Do not create an authentication subsystem for the MVP. Keep operational logs
behind separate access controls from public post rendering.

## Local setup

### Requirements

- Docker.
- Rust and Cargo for local development.

### Run with Docker

Build the image:

```sh
docker build -t mchan .
```

Start the container with a temporary abuse key:

```sh
MCHAN_ABUSE_KEY=$(openssl rand -hex 32) \
docker run --rm \
  --name mchan \
  -p 3000:3000 \
  -e MCHAN_ABUSE_KEY \
  -v "$PWD/data:/data" \
  mchan
```

Open <http://localhost:3000>. The container listens on port `3000`. Stop it with
`Ctrl+C`. Build the image again after a code change.

### Moderator access, board policy, Turnstile, and VPS runtime configuration

MChan does not have application admin accounts. Moderator access requires both:

1. an email allowed by the Cloudflare Access application; and
2. that email in the runtime `MCHAN_MODERATOR_EMAILS` environment variable.

The moderator identity is supplied by
`Cf-Access-Authenticated-User-Email`. Trust that header only when the origin
cannot be reached directly and the VPS is published through the Cloudflare
Tunnel.

`MCHAN_ENABLED_BOARD_SLUGS` is optional. When unset, startup preserves the
approved boards already in the database. When set, it accepts a comma-separated
list, trims whitespace, and removes duplicates; an empty/malformed entry or an
unknown slug fails startup. A valid list makes exactly those known boards
approved and archives the other boards. The deployment policy is:

- dev VPS: `MCHAN_ENABLED_BOARD_SLUGS=engineering,b`
- production VPS: `MCHAN_ENABLED_BOARD_SLUGS=b,pasum`

Random remains the `/b` board, and PASUM is `/pasum`. Keep both application
ports loopback-only; the Tunnel is the only public path.

`MCHAN_ABUSE_KEY` is also mandatory. Generate a 32-byte (64-hex-character) key
once and keep it secret and stable while retained origin records exist:

```sh
openssl rand -hex 32
```

`MCHAN_MEDIA_STORAGE_ROOT` is optional. When unset, Core uses `/data`; set it
when running locally with a different shared directory. Mount that same
directory as the storage root for `mchan-image`.

Turnstile is optional. Set both `MCHAN_TURNSTILE_SITE_KEY` and
`MCHAN_TURNSTILE_SECRET_KEY` to enable it; if both are absent, CAPTCHA is
disabled. Supplying only one key or an empty key is a startup configuration
error. `MCHAN_TURNSTILE_VERIFY_URL` optionally overrides the default Cloudflare
siteverify endpoint. The override must be HTTPS; HTTP is accepted only for
loopback `localhost`, `127.0.0.1`, or `::1` testing, with no credentials or URL
fragment.

When enabled, the second thread attempt (one prior attempt) or sixth reply
(five prior attempts) is challenged in its separate namespaced 60-second
window. A missing or failed challenge returns the form with submitted text
preserved; an unavailable verifier returns `503`. Successful verification then
proceeds through the ordinary two-thread or ten-reply per-minute limit.

For local Cargo development, export the runtime configuration:

```sh
export MCHAN_MODERATOR_EMAILS=you@example.com
export MCHAN_ABUSE_KEY='paste-the-output-of-openssl-rand-here'
# Optional:
export MCHAN_MEDIA_STORAGE_ROOT="$PWD/data"
export MCHAN_TURNSTILE_SITE_KEY='site-key'
export MCHAN_TURNSTILE_SECRET_KEY='secret-key'
cargo run
```

For local Docker testing:

```sh
docker run --rm \
  --name mchan \
  -p 3000:3000 \
  -e MCHAN_MODERATOR_EMAILS=you@example.com \
  -e MCHAN_ABUSE_KEY='paste-the-64-hex-character-key-here' \
  -e MCHAN_TURNSTILE_SITE_KEY='site-key' \
  -e MCHAN_TURNSTILE_SECRET_KEY='secret-key' \
  mchan
```

CI deployments stream only the Docker image to a VPS-side forced SSH receiver;
runtime environment variables are never transferred from GitHub Actions.
Keep each environment's runtime values outside the repository and pass its
environment file to the container with `--env-file`.

- **Dev:** A push to `dev` runs `.github/workflows/rust.yml`, streams
  `mchan:ci`, and uses `/etc/mchan/mchan.env`. Run container `mchan-dev` with
  persistent data at `/opt/mchan/data` and bind its application to loopback
  port `3000`. Its GitHub names are `MCHAN_DEPLOY_KEY`,
  `MCHAN_VPS_HOST`, `MCHAN_VPS_USER`, and `MCHAN_VPS_KNOWN_HOSTS`; use a
  distinct forced receiver from production.
- **Production:** A push to `main` runs `.github/workflows/production.yml` in
  the GitHub Environment `production`, streams `mchan:production`, and uses
  the distinct `MCHAN_PRODUCTION_DEPLOY_KEY`,
  `MCHAN_PRODUCTION_VPS_HOST`, `MCHAN_PRODUCTION_VPS_USER`, and
  `MCHAN_PRODUCTION_VPS_KNOWN_HOSTS` secret/vars. Use
  `/etc/mchan/mchan-prod.env`, persistent data at `/opt/mchan/data-prod`,
  container `mchan-prod`, and loopback port `3001`, with a distinct forced
  receiver.

The two VPS stacks are intentionally isolated:

| Environment | Core | Network | Core bind and nested media mount | Image service | Image volume |
| --- | --- | --- | --- | --- | --- |
| Dev | `mchan-dev` (`127.0.0.1:3000:3000`) | `mchan-dev-internal` | `/opt/mchan/data:/data`; `mchan-image-media-dev:/data/images:ro` | `mchan-image-dev` | `mchan-image-media-dev:/media/images` |
| Production | `mchan-prod` (`127.0.0.1:3001:3000`) | `mchan-internal` | `/opt/mchan/data-prod:/data`; `mchan-image-media:/data/images:ro` | `mchan-image` | `mchan-image-media:/media/images` |

Each environment's Core receives its matching env file with `--env-file`
(`/etc/mchan/mchan.env` for dev and `/etc/mchan/mchan-prod.env` for
production). Both image services listen on port `3001` inside their matching
network, and Core uses the shared internal URL
`http://mchan-image:3001`; the dev image service uses the `mchan-image`
network alias, while the production service is itself named `mchan-image`.
Core runs as UID `1000`; the image
service runs as UID `10001`. The image volume is read-write only for the image
service and read-only for Core, so `/data` is never obscured and the SQLite
bind remains unchanged.

The named media volumes are persistent deployment data. Core redeployments
replace containers without deleting either the named volume or the
`/opt/mchan/data*` database bind. Image-service redeployments likewise reuse
the matching named volume, so processed media survives either service being
recreated. Dev media stays in `mchan-image-media-dev` and production media
stays in `mchan-image-media`; neither environment can read the other's media.
The version-controlled `deploy/deploy-mchan` and
`deploy/deploy-mchan-prod` scripts are installed as
`/usr/local/bin/deploy-mchan` and `/usr/local/bin/deploy-mchan-prod` by the
corresponding forced SSH receivers. Before replacing Core they require the
matching network and media volume to exist and validate them, then verify the
candidate container and retain the old container's original mounts and network
for rollback.

The public Cloudflare Tunnel route must map the production hostname to
`http://localhost:3001`; the dev route remains `http://localhost:3000`.
Do not expose either port publicly. Env files must be readable only by the
deployment/runtime account. Do not put moderator emails, Turnstile secrets,
or the abuse key in `Dockerfile`, migrations, or committed workflow
configuration.

### Run locally

```sh
export MCHAN_MODERATOR_EMAILS=you@example.com
export MCHAN_ABUSE_KEY='paste-the-64-hex-character-key-here'
cargo run
```

Open <http://localhost:3000>.

### Current repository status

The current code implements public home, board, policy, thread, new-thread,
archive, reply, and report routes. It serves `/static`, uses the shared Askama
base layout, loads approved boards and seeded content from SQLite, and has a
fallback 404 response.

The Open Beta has anonymous text and optional-image posting, thread-scoped poster
IDs, post numbers, board reply counts and recent-three summaries, public
read-only archives, processed `post_media` rendering, basic rate limits, the
protected pending-report queue, all six content/report actions, board and site
bans, encrypted post origins, protected decrypted abuse logs with access
auditing, 30-day startup/hourly cleanup, and optional suspicious Turnstile
checks. Search, board proposals, accounts, persistent pseudonyms, and author
deletion tokens remain future work.
The repository currently has 58 deterministic tests covering the forum,
moderation, archive, media shape, Turnstile, board policy, and HTTP contracts.
Router tests use fresh migrated SQLite databases and in-process requests to
cover public reads, writes, cookies, rate limits, bans, CAPTCHA outcomes,
reports, moderator actions, abuse-log access, auditing, and cache headers.

## Git workflow

- Create a branch for each task.
- Keep each branch focused on one change.
- Pull the latest target branch before opening a pull request.
- Resolve conflicts in the branch before requesting review.
- Ask the relevant team member to review the change.
- Merge only after the checks pass and the change has approval.

Suggested branch names:

```text
feature/anonymous-posting
feature/optional-image-media
feature/moderation-queue
docs/mvp-readme
```

## Definition of done for the broader MVP

The current beta already satisfies the foundation, browsing, anonymous posting,
archive, optional image attachments, policy, and moderation criteria above. The
broader MVP still requires:

- Search includes active and archived threads.
- Administrators can approve a board proposal.
- Main flows have automated tests beyond the current suite.

The current implementation also provides:

- Anyone can browse approved boards without an account.
- Anyone can create a text thread without an account.
- Anyone can reply without an account.
- Anonymous posts receive thread-specific IDs.
- Anyone can report a thread or reply.
- Moderators can review reports and apply dismiss, resolve, hide, remove,
  quarantine, and thread-lock actions.
- Moderators can issue board bans up to 30 days and site-wide bans up to 365
  days when a retained origin is available.
- Every moderation action and abuse-log access is audited.
- Restricted encrypted operational logs are decrypted only in the protected
  view, sent with no-store/no-cache headers, and purged after 30 days.
- Expired threads enter a public, read-only archive.
- Invalid input produces a useful response.
- The 404 page works.
- Database migrations create a fresh database.
## Later features

Possible later features include:

- Optional accounts.
- One persistent pseudonym per account.
- A choice between anonymous and pseudonymous posting on supported boards.
- Saved threads.
- Private preferences.
- Notifications.
- Improved catalog views.
- Search across active and archived threads.
- Administrator board proposals and approval.
- Support for more universities.
- Better moderation tooling.
- Carefully evaluated small moderation models.
- Data export and privacy controls.

Do not add followers, public popularity metrics, algorithmic feeds, influencer
features, or corporate social-network features.
