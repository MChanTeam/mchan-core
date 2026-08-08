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

MChan is not a verified-student forum. It is not a conventional forum that may
add images later. The imageboard format is a core product requirement from the
start. The first closed test may focus on the Engineering Faculty community,
but the design must remain suitable for other Malaysian universities.

The main product loop is:

1. A visitor opens an approved board.
2. The visitor creates an anonymous thread, with an image when the board
   requires or permits one.
3. Other visitors reply with text and optional images.
4. Visitors report content that violates the rules.
5. Moderators review reports and take action.
6. Inactive threads enter a public, read-only archive.

The current repository contains a read-only, database-backed slice of this
journey. It does not yet implement anonymous posting, uploads, moderation,
search, or archives. Those features are MVP work, not later optional ideas.

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
- One optional image attachment per reply, except on text-only boards.
- Board-level posting modes:
  - **Image required**: a new thread must include an image; replies follow the
    board's permitted media rules.
  - **Image optional**: a post may contain text, one image, or both.
  - **Text only**: uploads are rejected.
- Thread-specific anonymous poster IDs.
- Post numbers, timestamps, and reply references.
- Reports for threads and replies.
- A basic moderator queue.
- Moderator removal actions, thread locking, and temporary board bans.
- Basic rate limits.
- CAPTCHA only when behaviour appears suspicious or abuse is elevated.
- Minimal encrypted abuse logs.
- Public read-only archives.
- Search across active and archived threads.
- Administrator review of board proposals.
- Server-rendered HTML.
- One external CSS file.
- Minimal JavaScript.
- A working 404 page.
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

## Images are an MVP requirement

MChan must process uploads before public display. The MVP media rules are:

- Accept JPEG, PNG, WebP, and animated GIF.
- Permit one attachment per post. Text-only boards permit none.
- Limit static images to 5 MB.
- Limit animated GIFs to 10 MB.
- Decode, process, and sanitize every upload.
- Remove metadata where possible.
- Reject files that cannot be decoded safely.
- Do not retain the original uploaded file.
- Generate a thumbnail with a maximum size of 512 px.
- Generate a processed display image with a maximum longest side of 2048 px.
- Preserve the aspect ratio.
- Do not upscale smaller images.
- Keep processed media when a thread enters the archive.
- Keep video and audio outside the MVP.

The upload milestone includes safe storage, browser rendering, image expansion,
and media removal when moderators remove a post. Image support is not a future
feature to be deferred beyond the MVP.

## Anonymity and operational logs

Public posting does not require an account. Logged-out posts display:

- `Anonymous`.
- A temporary thread-specific poster ID.

The poster ID remains consistent within one thread. It must not create a
permanent identity across threads. It must not be presented as proof that two
people are different. It must not expose the user's IP address.

Posts are anonymous to the public. MChan keeps limited private operational
records for abuse prevention, moderation, security, and lawful requests.

Keep public post data separate from restricted operational logs. MChan may keep
minimal encrypted records containing:

- Post identifier.
- Exact posting timestamp.
- IP address or necessary network information.
- Basic anti-abuse signals.
- Uploaded-media hash.
- Reports and moderator actions.

Normal retention is 30 days. Extend retention only when necessary for a serious
report, an active investigation, or a lawful preservation request. Do not
describe MChan as completely untraceable, and do not promise immunity from
identification.

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

For the MVP:

- Ordinary reports enter a moderator queue.
- Report count can affect priority but must not automatically remove content.
- Urgent content may be quarantined when credible safety signals exist.
- Board moderators handle routine board enforcement.
- Central moderators handle serious abuse, cross-board incidents, legal matters,
  and site-wide bans.
- Board moderators may issue temporary board-specific bans.
- Central moderators control long-term and site-wide bans.

The software should stay basic. It needs reports, a queue, actions, locks,
bans, audit records, and access controls. It does not need an enterprise
trust-and-safety platform.

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
- Media processing and storage.
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
- [ ] Add a shared HTML layout.

### Milestone 2: Read-only imageboard foundation

- [x] Load approved boards from the database.
- [x] Show the home page and board list.
- [x] Show a board index with compact thread summaries.
- [x] Show a full thread page with the original post and replies.
- [ ] Show recent replies and reply counts in board thread summaries.
- [ ] Support direct links to individual posts within a thread.
- [ ] Add the read-only archive view.
- [ ] Add the public imageboard post shape needed for media rendering.

The current slice supports read-only browsing and anonymous text thread creation.
It does not yet implement anonymous replies, uploads, moderation, search, or
archives. Those features are MVP work, not later optional ideas.

### Milestone 3: Open anonymous posting

- [x] Create threads without an account.
- [ ] Reply without an account.
- [x] Add server-side validation for thread creation.
- [ ] Generate thread-specific anonymous IDs.
- [ ] Add post numbers and reply references.
- [ ] Add basic rate limits.
- [ ] Add a suspicious-use CAPTCHA integration point.

### Milestone 4: Image uploads

- [ ] Allow one image per post.
- [ ] Add image-required, image-optional, and text-only board modes.
- [ ] Validate JPEG, PNG, WebP, and animated GIF files.
- [ ] Decode and sanitize images safely.
- [ ] Remove metadata where possible.
- [ ] Generate thumbnails up to 512 px.
- [ ] Generate processed images up to a 2048 px longest side.
- [ ] Enforce the 5 MB static and 10 MB animated GIF limits.
- [ ] Store only safe processed media.
- [ ] Remove media when a post is moderated out.
- [ ] Render thumbnails and processed images in the browser.
- [ ] Add safe image expansion.

### Milestone 5: Moderation

- [ ] Accept public reports for threads and replies.
- [ ] Add the moderator queue.
- [ ] Add hide, remove, quarantine, and lock actions.
- [ ] Add temporary board bans.
- [ ] Add central site-wide ban capability.
- [ ] Record moderation audit actions.
- [ ] Protect encrypted abuse-log access.
- [ ] Apply the normal 30-day operational-log retention.

Keep the implementation simple. Do not build complex analytics or an automated
moderation platform.

### Milestone 6: Archives, search, and closed test

- [ ] Add board-specific expiry, bump, or reply-limit rules.
- [ ] Move expired threads into a public read-only archive.
- [ ] Retain processed archive media.
- [ ] Search active and archived threads.
- [ ] Add the board proposal form and administrator approval flow.
- [ ] Add end-to-end tests for the main flows.
- [ ] Run a small closed test.

Optional accounts are not an MVP milestone.

## Suggested routes

These routes are suggestions for the target MVP. They do not claim that the
routes already exist.

| Page or action | Suggested route | Access |
| --- | --- | --- |
| Home and board list | `GET /` | Public |
| View a board | `GET /boards/:slug` | Public |
| View a thread | `GET /threads/:id` | Public |
| View archives | `GET /boards/:slug/archive` | Public |
| Search | `GET /search?q=...` | Public |
| Create a thread | `POST /boards/:slug/threads` | Public |
| Add a reply | `POST /threads/:id/replies` | Public |
| Delete own reply | `POST /posts/:id/delete` | Token holder |
| Report content | `POST /reports` | Public |
| Propose a board | `POST /board-proposals` | Public |
| Moderator queue | `GET /mod/reports` | Moderator |
| Apply moderation action | `POST /mod/actions` | Moderator |
| Review board proposals | `GET /admin/boards` | Administrator |
| Approve or reject proposal | `POST /admin/boards/:id/action` | Administrator |

Routes can change during implementation. Keep them simple and consistent. The
MVP must not add verification routes or make university verification part of
access control.

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

### `media`

- `id`
- Post or thread reference.
- Stored processed-file reference.
- Thumbnail reference.
- Media type.
- Width.
- Height.
- Size.
- Animation information.
- Content hash.
- Creation time.

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
  number, timestamp, references, moderation state, and optional processed media.
- **Board index**: The active list of threads on a board.
- **Catalog**: A denser board view with many thread summaries and media. It is a
  later presentation improvement, not a separate ranking system.
- **Archive**: A public, read-only view of threads that are no longer active.

## Suggested project structure

Keep the number of modules small until the application has real complexity.

```text
src/
  main.rs              # Tokio runtime, Axum startup, and routes
  forum.rs             # Board, thread, and reply data access
  boards/              # Board pages and proposal flow when needed
  threads/             # Thread and reply handlers when posting is added
  media/               # Upload processing and safe storage
  moderation/          # Reports and moderation actions
  db/                  # Database helpers when needed
  migrations/          # SQLx migrations
static/
  style.css
  ...
templates/
  base.html            # Add when a shared layout is needed
  board.html
  home.html
  thread.html
  ...
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

Start the container:

```sh
docker run --rm --name mchan -p 3000:3000 mchan
```

Open <http://localhost:3000>. The container listens on port `3000`. Stop it with
`Ctrl+C`. Build the image again after a code change.

### Run locally

```sh
cargo run
```

Open <http://localhost:3000>.

### Current repository status

The current code implements the read-only routes `GET /`,
`GET /boards/{slug}`, and `GET /threads/{id}`. It serves `/static`, uses
Askama templates, loads approved boards and seeded threads and replies from
SQLite, and has a fallback 404 response.

The target anonymous MVP does not include public user accounts or university
verification. Anonymous posting, media, reports, moderation, search, archives,
and board proposals are not implemented yet.

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
feature/image-uploads
feature/moderation-queue
docs/mvp-readme
```

## Definition of done

The MVP is ready for a small closed test when:

- Anyone can browse approved boards without an account.
- Anyone can create a thread without an account.
- Anyone can reply without an account.
- Anonymous posts receive thread-specific IDs.
- Image-required boards require an image for new threads.
- Image-optional boards support text-only or image posts.
- Text-only boards reject uploads.
- Static images and GIFs are validated and processed safely.
- Original uploads are not retained.
- Board and thread pages show thumbnails and processed media correctly.
- Anyone can report a thread or reply.
- Moderators can review reports.
- Moderators can remove or quarantine content.
- Moderators can lock threads.
- Rate limits work.
- Restricted operational logs follow the retention policy.
- Expired threads enter a public, read-only archive.
- Search includes active and archived threads.
- Administrators can approve a board proposal.
- Invalid input produces a useful response.
- The 404 page works.
- Database migrations create a fresh database.
- Main flows have automated tests.

## Later features

Possible later features include:

- Optional accounts.
- One persistent pseudonym per account.
- A choice between anonymous and pseudonymous posting on supported boards.
- Saved threads.
- Private preferences.
- Notifications.
- Improved catalog views.
- Improved search.
- Support for more universities.
- Better moderation tooling.
- Carefully evaluated small moderation models.
- Data export and privacy controls.

Do not add followers, public popularity metrics, algorithmic feeds, influencer
features, or corporate social-network features.
