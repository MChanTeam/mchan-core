# Milestone 1: Read-Only Forum Slice

## Goal

Prove the smallest useful MChan experience:

> A person can open MChan, choose an approved board, open a thread, and read its replies.

This milestone is intentionally read-only. It establishes the forum’s basic shape before adding authentication, posting, reporting, or moderation.

## User journey

```text
Home → Board → Thread → Replies
```

The journey must work with ordinary links and server-rendered HTML.

## In scope

- Public home page
- Approved board list
- Board page with thread list
- Thread page with replies
- Anonymous public author labels
- Seeded data for local development
- Working navigation between pages
- Working 404 page for unknown boards, threads, and routes
- One external CSS file

## Out of scope

Do not add these in this milestone:

- User accounts or authentication
- University email verification
- Thread or reply creation
- Reports or moderation actions
- Database persistence
- Search
- JavaScript interactions
- Profiles, reactions, uploads, or private messages

## Routes

| Route | Purpose | Expected result |
| --- | --- | --- |
| `GET /` | Show approved boards | Board names, descriptions, and links |
| `GET /boards/:slug` | Show one board | Board details and links to its threads |
| `GET /threads/:id` | Show one thread | Thread body and replies |
| Any unknown route | Show missing-page state | HTTP `404` with a link home |

## Seed data

Use deterministic local data so the pages are useful immediately after `cargo run`:

- One approved Engineering board
- At least two threads
- At least two replies on one thread
- Public author display such as `Anonymous`

The templates must not receive or display email addresses, account IDs, or other private identity data.

## Implementation sequence

1. Define the smallest board, thread, and reply data structures.
2. Add deterministic seed data.
3. Render the board list on `/`.
4. Add the board route and thread list.
5. Add the thread route and replies.
6. Add links between all three pages.
7. Replace the placeholder CSS with a readable forum layout.
8. Reuse the existing 404 behavior for missing resources.
9. Add focused route tests for success and missing-resource cases.

Keep the implementation small. Do not introduce a repository abstraction or database layer until this user journey works.

## Acceptance checklist

### Browser checks

- [ ] `/` shows the approved board.
- [ ] The board link opens the board page.
- [ ] The board page shows seeded threads.
- [ ] A thread link opens the thread page.
- [ ] The thread page shows the body and replies.
- [ ] Navigation links return to the previous level.
- [ ] Unknown board slugs show the 404 page.
- [ ] Unknown thread IDs show the 404 page.
- [ ] Pages load the external stylesheet.
- [ ] No public page exposes private identity data.

### Code checks

- [ ] `cargo test` passes.
- [ ] Route tests cover the main path.
- [ ] Route tests cover missing boards and threads.
- [ ] HTML is rendered through Askama templates.
- [ ] User-visible text is escaped by the template system.

## Completion signal

Milestone 1 is complete when a new developer can run:

```sh
cargo run
```

Then manually follow:

```text
/ → approved board → seeded thread → replies
```

without encountering a placeholder page, broken link, or missing route.

## Next milestone

After this slice is stable, add the read-only persistence layer: SQLite connection, migrations, approved boards, threads, and replies. Keep the existing routes and page behavior intact while replacing seeded data with database queries.
