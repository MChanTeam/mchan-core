# MChan MVP Milestone

This document defines the first MChan milestone for the current four-person
team. It is a working agreement for the MVP. The team can update it when a
decision changes.

## Project goal

MChan is an anonymous, moderated imageboard for university-level communities. It
uses the compact, public thread-and-reply format associated with imageboards,
while adding stronger university verification and manual moderation.

The first closed test can focus on the Engineering Faculty community. The
design and data model must remain suitable for other faculties and
universities later.

The first MVP slice is intentionally text-only. It proves the public browsing
and discussion flow before media uploads and richer imageboard features are
added.

The MVP must prove this main product loop:

1. A person reads an approved board.
2. A verified university student creates a thread or reply.
3. Another verified user reports harmful or rule-breaking content.
4. A moderator reviews the report and takes action.

The long-term product direction is a genuine university-focused imageboard,
not a conventional forum with image uploads added later.
The public MVP is text-first, but its thread and reply presentation should
leave room for future imageboard-style post metadata and media.

## MVP scope

### Included

- Public home page and board list.
- Approved discussion boards.
- Text-first imageboard threads and replies.
- Approved university email verification.
- Anonymous public posting.
- Reports for threads and replies.
- Moderator review queue.
- Moderator actions.
- Administrator approval for new boards.
- Basic search.
- A working 404 page.
- Server-rendered HTML with one external CSS file.

### Not included

- Private messages.
- Public user profiles.
- Karma, reactions, badges, or followers.
- Image and video uploads.
- A mobile application.
- Recommendation feeds.
- A custom NLP or machine-learning moderation system.
- Full multi-university administration.

## Product and anonymity rules

- Anyone can read public content.
- Only verified students can create threads, reply, and report content.
- A post must not show the author's email address or account ID.
- The database must keep the real author link for moderation.
- Users do not have public profiles.
- An administrator must approve a board before it appears in the public list.
- Manual moderation is required for the MVP.
- User-provided HTML must be escaped.
- State-changing forms need CSRF protection.
- Verification, posting, replies, and reports need rate limits.

The team must define the identity disclosure policy before public launch. MChan
must not promise a level of anonymity that the system cannot provide.

## Technology

- Rust.
- Tokio.
- Axum.
- SQLite with SQLx and migrations.
- Askama, or another Rust template system.
- HTML and CSS.
- Minimal JavaScript.
- Docker.

The first deployment uses one Rust web process and one SQLite database. The
database layer must use normal SQL and migrations so that PostgreSQL can be
added later if needed.

## Team responsibilities

### Backend developer (Kuumin)

- Set up Tokio and Axum.
- Build application state, routes, errors, and the database layer.
- Build verification and session handling.
- Build threads, replies, reports, and moderation actions.
- Apply server-side validation, rate limits, CSRF protection, and audit logs.
- Keep private identity data out of public responses.
- Review security-sensitive code before it is merged.

### Frontend developer (Mxrza)

- Build the server-rendered page structure.
- Connect templates to backend routes.
- Build forms for threads, replies, reports, and moderation.
- Handle loading, empty, error, and permission states.
- Test the main user flows in the browser.

### Artist and designer (Chifuyu)

- Define the visual direction for MChan.
- Design the board, thread, reply, report, and moderation views.
- Define typography, colour, spacing, and layout rules.
- Prepare the 404 page and small visual assets.
- Review implemented pages for consistency and readability.

### Learning developer (JavanMyna)

- Work on small, clearly scoped issues.
- Add seed data and test data.
- Add and improve automated tests.
- Help with templates, CSS, documentation, and simple route work.
- Learn the Rust, Git, and code review workflow.

Critical authentication, privacy, and moderation code must receive backend
review before it is merged.

## Milestone plan

### Milestone 1: Foundation

- [x] Start the Tokio and Axum application.
- [ ] Add a shared HTML layout.
- [x] Add the catch-all 404 page.
- [x] Serve the external CSS file.
- [ ] Make the application run with Docker and Cargo.

### Milestone 2: Read-only imageboard foundation

- [x] Add the database connection and migrations.
- [x] Add approved boards.
- [x] Show the home page and board list.
- [x] Show a board index with compact thread summaries.
- [ ] Show each thread's original post and recent replies on the board index.
- [ ] Show omitted-reply counts without loading the full thread.
- [x] Show a full thread page with the original post and all replies.
- [ ] Support direct links to individual posts within a thread.
- [x] Keep the first vertical slice text-only and server-rendered.

### Milestone 3: Verified participation

- [ ] Add approved university email verification.
- [ ] Add secure sessions.
- [ ] Allow verified users to create threads.
- [ ] Allow verified users to reply.
- [ ] Show all posts as anonymous public content.

### Milestone 4: Moderation

- [ ] Allow verified users to report threads and replies.
- [ ] Add the moderator queue.
- [ ] Allow moderators to hide content.
- [ ] Allow moderators to lock threads.
- [ ] Allow moderators to suspend or ban accounts.
- [ ] Record moderation actions.

### Milestone 5: Administration and closed test

- [ ] Add board proposals.
- [ ] Allow an administrator to approve or reject boards.
- [ ] Add basic search.
- [ ] Add server-side validation and rate limits.
- [ ] Add CSRF protection and audit logs.
- [ ] Test the complete read, post, report, and moderation flows.
- [ ] Run a small closed test.

### Future Milestone 6: Imageboard capabilities

- [ ] Add optional image attachments with strict size, type, and dimension limits.
- [ ] Store media metadata separately from post text.
- [ ] Render original posts and replies using a shared compact post format.
- [ ] Show post numbers, timestamps, and safe reply references.
- [ ] Show image thumbnails in board indexes and thread pages.
- [ ] Load full-size images only when requested.
- [ ] Add lazy loading for images and other heavy media.
- [ ] Add an optional in-page expansion for omitted replies.
- [ ] Add catalog and archive views.
- [ ] Apply moderation and takedown rules to both text and media.

This milestone must not weaken university verification, anonymity, moderation,
or privacy rules. Media uploads are user content and require the same
validation, rate limiting, and moderation discipline as text.

## Suggested routes

| Page or action            | Route                                                    | Access        |
| ------------------------- | -------------------------------------------------------- | ------------- |
| Home and board list       | `GET /`                                                  | Public        |
| View a board              | `GET /boards/:slug`                                      | Public        |
| View a thread             | `GET /threads/:id`                                       | Public        |
| Search                    | `GET /search?q=...`                                      | Public        |
| Request verification      | `GET /auth/verify`                                       | Public        |
| Submit verification       | `POST /auth/verify`                                      | Public        |
| Create a thread           | `GET /boards/:slug/new` and `POST /boards/:slug/threads` | Verified user |
| Add a reply               | `POST /threads/:id/replies`                              | Verified user |
| Report content            | `POST /reports`                                          | Verified user |
| Moderator queue           | `GET /mod/reports`                                       | Moderator     |
| Review a report           | `GET /mod/reports/:id`                                   | Moderator     |
| Apply moderation action   | `POST /mod/actions`                                      | Moderator     |
| Review board proposals    | `GET /admin/boards`                                      | Administrator |
| Approve or reject a board | `POST /admin/boards/:id/action`                          | Administrator |

Routes can change during implementation. Keep them simple and consistent.

## Core data model

| Table                 | Main fields                                                                                          |
| --------------------- | ---------------------------------------------------------------------------------------------------- |
| `users`               | `id`, email value or hash, university, verification time, role, status, creation time                |
| `verification_tokens` | `id`, user ID, token hash, expiry time, used time                                                    |
| `boards`              | `id`, slug, name, description, status, creator, creation time                                        |
| `threads`             | `id`, board ID, author ID, title, body, status, creation time, update time                           |
| `replies`             | `id`, thread ID, author ID, body, status, creation time                                              |
| `reports`             | `id`, reporter ID, thread or reply ID, reason, details, status, reviewer, review time, creation time |

Useful status values include:

- Boards: `pending`, `approved`, `rejected`, `archived`.
- Posts: `visible`, `hidden`, `locked`.
- Reports: `open`, `dismissed`, `actioned`.
- Users: `active`, `suspended`, `banned`.

## Imageboard vocabulary and direction

MChan uses these terms:

- **Board**: An approved topic community, identified by a stable slug.
- **Thread**: A discussion on a board containing one original post and replies.
- **Original post (OP)**: The first post that creates a thread.
- **Reply**: A post added to an existing thread.
- **Post**: The shared public shape of an OP or reply: anonymous display,
  body, creation time, moderation status, and eventually optional media.
- **Catalog**: A compact board view showing many thread summaries, thumbnails,
  and a limited number of recent replies.
- **Archive**: A read-only view of threads that are no longer active.

The current MVP may keep `threads` and `replies` as separate storage concepts.
Future imageboard work should make their public rendering consistent through a
shared post shape. Do not add media tables or upload handling until the
validation, storage, moderation, and privacy requirements are defined.

The board index is intentionally a compact overview. The full thread page is
the canonical one-page reading view for every reply and attachment in that
thread. A user should not need a separate page for each reply.

## Moderation

Report reasons include:

- Harassment or bullying.
- Threats or danger.
- Doxxing or personal information.
- Hate or discrimination.
- Spam.
- Sexual or illegal content.
- Other rule violation.

MVP moderator actions include:

- Hide a thread or reply.
- Lock a thread.
- Dismiss a report.
- Suspend an account for a fixed period.
- Ban an account.
- Add a short moderation note.

Moderation actions must be auditable. A simple blocked-word filter can provide
a warning or review signal, but it must not be the only moderation layer.

## Suggested project structure

```text
src/
  main.rs              # Tokio runtime and Axum startup
  app_state.rs         # Shared database and configuration state
  error.rs             # Application errors and responses
  auth/
    handlers.rs
    middleware.rs
    service.rs
  boards/
    handlers.rs
    service.rs
  threads/
    handlers.rs
    service.rs
  moderation/
    handlers.rs
    service.rs
  models/
  db/
    migrations/
static/
  style.css
templates/
  base.html
  home.html
```

Keep the number of modules small until the application has real complexity.

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

Open <http://localhost:3000>.

The container listens on port `3000`. Stop it with `Ctrl+C`.

Build the image again after a code change:

```sh
docker build -t mchan .
```

### Run locally

```sh
cargo run
```

Open <http://localhost:3000>.

### Docker files

- `Dockerfile` uses Alpine Linux for the build and runtime images.
- `.dockerignore` removes local files from the Docker build context.

## Git workflow

- Create a branch for each task.
- Keep each branch focused on one change.
- Pull the latest target branch before opening a pull request.
- Resolve conflicts in the branch before requesting review.
- Ask the relevant team member to review the change.
- Merge only after the checks pass and the change has approval.

Suggested branch names:

```text
feature/board-list
feature/thread-page
fix/verification-rate-limit
docs/mvp-milestone
```

## Definition of done

The MVP is ready for a small closed test when:

- A verified test user can browse approved boards.
- A verified test user can create a thread and reply.
- Public pages do not show email addresses or private account data.
- A user can report a thread or reply.
- A moderator can review a report and hide or lock content.
- An administrator can approve a board.
- Invalid form input returns a useful error page.
- Unauthenticated users cannot post.
- Suspended and banned users cannot post.
- The application has a working 404 page.
- Database migrations can create a fresh local database.
- The main flows have automated tests.

## Later features

- More faculties and universities.
- Better search indexing.
- Thread sorting and pagination improvements.
- Catalog and archive views.
- Imageboard media attachments with strict limits.
- Post numbers, timestamps, and reply references.
- Moderator analytics.
- A carefully evaluated small moderation model.
- Data export and deletion controls.
- Privacy policy and formal community guidelines.
