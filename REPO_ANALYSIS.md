# MChan Core — repository analysis

**Subject:** `giffithzero/mchan-core-giffith-` @ `31bdbcc` (branch `arena/00765316-mchan-core-giffith`)
**Date of analysis:** 2026-10-16
**Analyst:** Arena agent (static analysis session)

---

## 0. Method and limits

| Step | Result |
| --- | --- |
| `git` inspection | Shallow, single-commit clone (`31bdbcc`, 2026-10-12, "Merge pull request #56 … hold-flagged-posts") |
| Upstream comparison | Fork is **byte-identical** to `MChanTeam/mchan-core@main` (`compare` → `status: identical, ahead 0, behind 0`) |
| Migrations | **Verified**: all 17 applied cleanly in order to an in-memory SQLite DB (13 tables, 34 objects, seeds present) |
| `make release-check` | **Verified**: `python3 scripts/release.py --check` → exit 0 (Cargo.toml / Cargo.lock / changelog / home template consistent at 0.11.0) |
| `cargo fmt / check / test` | **Not run** — no Rust toolchain in this sandbox, and only github.com / npm / pypi are reachable, so rustup could not be installed. All Rust findings below are from reading the source, not from executing it. |
| Issue/PR context | Upstream issue tracker read for roadmap context (fork has issues disabled) |

Where I state a behaviour, it is from the code as written; where I could not execute something I say so.

---

## 1. Snapshot

| Property | Value |
| --- | --- |
| Language / edition | Rust 2024 (needs ≥ 1.85) |
| Framework | Axum 0.8 (multi-listener), Askama 0.16 templates, SQLx 0.8 + SQLite |
| Runtime deps | 15 (askama, axum, chacha20poly1305, getrandom, hmac, pulldown-cmark, reqwest, serde, serde_json, sha2, sqlx, time, tokio, tower-http, uuid) |
| Prod Rust | **8,123 lines** in `src/` |
| Test Rust | **10,503 lines** (56% of `src/`): 8,492 in dedicated test files + 2,011 in-file |
| Test functions | **169** (40 `#[test]`, 129 `#[sqlx::test]`) |
| Migrations | 17, forward-only, checksum-tracked |
| Routes | 30 public/internal-core + 7 private Telegram = 37 |
| Front-end | 1 stylesheet (880 lines), 3 small vanilla scripts (978 lines incl. the Lain mascot), no Node/npm |
| Ops | 2 Docker deployments behind Cloudflare Tunnel (dev `:3000`, prod `127.0.0.1:3001`), Discord moderation API, Discord report webhook, optional Miya screening, optional `mchan-image` processor, optional Turnstile |
| Release | v0.11.0 (2026-10-12) |

**Verdict:** this is a well-disciplined, deliberately small production service, not a toy. The domain layer, transactional invariants, and test suite are better than most projects at this size. The weaknesses are concentrated in *operational hardening* (unbounded in-memory/DB state, two timeout-less HTTP clients, no CSRF/edge-header defense-in-depth) and *documentation drift* (the docs are treated as the spec, so stale numbers there matter more than usual). No blockers for running it; several cheap fixes with high leverage.

---

## 2. Architecture

```
main.rs  ── env parsing, SQLite open, embedded migrations, board policy,
            startup+hourly purge loop, two listeners
   │
   ├─ http/mod.rs        HttpDependencies (pool, cipher, adapters, in-process RateLimiter)
   │     ├─ public.rs     home, /boards/{slug}[/archive], /threads/{id}, policy pages
   │     ├─ posting.rs    create thread/reply/report + "canonical application helpers"
   │     │                 (validation, bans, rate, CAPTCHA, origin, Miya, media)
   │     ├─ moderation.rs /admin*, /mod/reports*, /mod/*/hide|pin, /mod/abuse-logs
   │     ├─ operations.rs /health, /internal/metrics, /internal/discord/moderate
   │     ├─ telegram.rs   /internal/telegram/*  (separate authenticated listener)
   │     └─ embed.rs      OpenGraph origin/summary helpers
   │
   ├─ forum.rs           domain + SQL + transactions + in-file domain tests
   ├─ abuse.rs           ChaCha20-Poly1305 origin encryption + HMAC fingerprints
   ├─ captcha.rs         Turnstile config/verify adapter (trait + test double)
   ├─ media.rs           mchan-image HTTP adapter, strict response validation
   └─ miya.rs            text-screening adapter (allow / review / block)
```

Key design facts worth crediting:

- **One posting pipeline, two front doors.** `posting.rs` exposes `validate_*`, `ensure_*_rate`, `ensure_not_banned_*`, `protect_origin`, `evaluate_miya`, `canonical_process_media`; the web handlers and the private Telegram API both call them. Telegram deliberately skips only Turnstile, and that is documented.
- **Domain boundary held.** All SQL lives in `forum.rs`; handlers never build queries. Multi-row effects (post + origin + media + outbox/report; moderation + audit; ban + report resolution) are single transactions.
- **Optional integrations are ports.** `MediaProcessor` / `CaptchaVerifier` traits, `Arc<dyn …>` injection, loopback/scripted doubles in tests. Missing/failing Miya degrades to "publish + report as unchecked" rather than failing closed or silently blocking.
- **Media is out-of-process.** Core never writes image files: it uploads bytes to `mchan-image`, validates the returned storage keys against a strict shape (`images/<image_id>/<variant>.webp`, safe path segments, ≤512/≤128 px, `image/webp`, non-zero size), stores only metadata, and serves read-only files from a `:ro` volume. The compensation path (`cleanup_media`) is invoked on every failure and on idempotent replays.
- **Projection outbox** with leased delivery (`lease_token` + `expires_at`, token-checked ack, 7-day retention purge) gives the Telegram service a durable change feed without coupling it to Core's tables.
- **Machine idempotency** keyed by `(service, operation, opaque_key)` + a domain-separated request hash (length-prefixed SHA-256 over principal/board/title/body/media bytes), so exact replays return the original ID and conflicting payloads are rejected with no side effects. A rare in-flight race is handled by polling for a concurrent commit for up to ~5 s.

### Data model (17 migrations)

`boards` → `threads` → `replies` with status CHECK vocabularies (`boards: pending|approved|rejected|archived`, `threads: visible|hidden|locked`, `replies: visible|hidden`, `reports: pending|resolved|dismissed`) plus:

- `reports` (exactly one of thread/reply, `details`, `held`), `moderation_actions` (8-value action CHECK), `direct_moderation_actions` (9-value reason CHECK, `direct_hide` audit without a report)
- `post_origins` (unique per post, encrypted client key + fingerprint + nonce, `retain_until` default `+30 days`, retention index), `bans` (scope CHECK board|site, expiry, `revoked_at`), `abuse_log_accesses`
- `post_media` (partial unique indexes per thread/reply), `board_moderators` (lowercase-email CHECK, PK `(board_slug, email)`)
- `machine_idempotency`, `projection_outbox` (with an elaborate CHECK tying kind → id shape)

Everything needed for the documented invariants is enforced in SQL as well as in Rust, which is why the migration set is trustworthy.

---

## 3. What is genuinely strong

1. **Docs-as-spec, enforced by tests.** `AGENTS.md`, `docs/MODERATION_SPEC.md`, `docs/CONTEXT.md` (with an explicit *avoid these words* vocabulary list) and `docs/OPERATIONS.md` describe the trust boundary, statuses, and retention; the HTTP contract tests assert those same promises (held → 202/`"held": true`, no projection event until approval, one audit row per handled report, 403 without identity, `no-store` on sensitive responses, ban limits 1–30 / 1–365).
2. **Security fundamentals done properly.** ChaCha20-Poly1305 with a versioned AAD and domain-separated keys (`mchan-abuse-encryption-v1` / `mchan-abuse-fingerprint-v1`), HMAC-SHA256 fingerprints, constant-time bearer comparison, strict key parsing (exactly 64 hex chars, no trimming), path/DTO whitelisting for every integration response, `|safe` used at exactly one place (trusted policy Markdown), and no secret ever logged.
3. **Test engineering.** 169 tests, deterministic: `#[sqlx::test]` with an isolated migrated pool per test, scripted Miya/webhook servers on `127.0.0.1:0`, scripted Turnstile/media doubles, a real router harness (`oneshot`) so tests exercise the same seam production does, an env-mutating lock pattern, and behaviour-named tests (`failed_media_insert_rolls_back_and_cleans_up_image`).
4. **Deployment rigour.** Read-only rootfs, `--cap-drop ALL`, `no-new-privileges`, `--pids-limit`, 256 MB cap, nodev/noexec tmpfs, non-root Alpine runtime, health-gated deploy with rollback-by-rename (`deploy-mchan-prod`), image streaming over SSH with pinned known-hosts and a temp key, dev/prod state split, and no secrets in the repo (`.env*` ignored, documented as VPS-only).
5. **Migration discipline.** Forward-only, additive, CHECK-constrained; the CI/Makefile exclusion of `0005_add_poster_ids.sql` from the SQL formatter is justified in a comment (formatter drops `ALTER TABLE ADD COLUMN` constraints) rather than silently ignored.
6. **Zero dead markers.** No `TODO`/`FIXME`/`XXX`/`HACK` anywhere in `src/`. The code is uniform in style (rustfmt-enforced) and the layering is respected without exceptions.

---

## 4. Findings

Severity: **H** = fix soon (abuse/robustness under load), **M** = worth scheduling, **L** = polish/consistency.

### 4.1 Security and trust boundary

**H-1 · Two HTTP clients have no timeout.**
`src/captcha.rs:126` (`Client::new()`) and `src/http/mod.rs:506` (`report_webhook_client: Client::new()`). Miya gets 2 s and `mchan-image` 30 s, but these two have *no* timeout. The webhook case is worse because `notify_report` is awaited inline in the public report handlers (`src/http/posting.rs:763`, `:814`) *before* the redirect is returned — a hanging Discord endpoint holds the reporter's request (and its task/connection) open, several times per posted report. Captcha is on the posting path and a hung `siteverify` does the same for the poster.
*Fix:* `.timeout(Duration::from_secs(3))` on both, and/or `tokio::spawn` the webhook so it is truly best-effort as the docs claim. A regression test with a non-responding loopback listener would lock this in.

**H-2 · No CSRF / cross-site defence at the application layer.**
Every mutation is a plain POST form (`/mod/reports/{id}/{action}`, `/mod/threads/{id}/hide|pin`, `/admin/*`, posting, reporting). Grep confirms there is no CSRF token, no `Origin` check, no `Sec-Fetch-Site` check, and no `SameSite`/cookie handling in `src/`. Moderation authorization is *entirely* the `Cf-Access-Authenticated-User-Email` header injected by the edge. That works only while (a) the origin is unreachable directly and (b) the Cloudflare Access session cookie semantics prevent cross-site use; if the Access session cookie is `SameSite=None`, a third-party form POST would be accepted and applied as the moderator, and the app would never know.
*Fix:* cheap defense-in-depth — reject mutations whose `Sec-Fetch-Site` is not `same-origin`/`none` (with an `Origin` check fallback), or add a per-session double-submit token. Verify the Access cookie's `SameSite` attribute and document it in `docs/MODERATION_SPEC.md` next to the existing trust-boundary text.

**M-3 · Client identity is taken from unauthenticated proxy headers.**
`client_key` (`src/http/mod.rs:668`) prefers `cf-connecting-ip`, then the first `x-forwarded-for` entry, else `"local"`. That value becomes the HMAC fingerprint behind poster IDs, rate limits, and *all* bans and IP retention. The docs are explicit that the origin must be isolated, but there is no code-level guard: if the port were ever exposed (a mis-scoped firewall rule, a stray `--publish`, a debug port-forward), anyone could self-select a fingerprint, bypass every rate limit and ban, and poison `post_origins`.
*Fix:* accept these headers only when the peer address is loopback (or a configured proxy CIDR), or require an edge-injected shared secret header; add a startup warning when the bind address is not loopback and no proxy secret is configured.

**M-4 · Rate limiter is an in-process map that never evicts.**
`src/http/mod.rs:410-451`: `HashMap<String, VecDeque<Instant>>`, entries pruned *inside* a bucket, but buckets are never removed and `captcha_context` (`:698`) calls `suspicious()` on ordinary page GETs, which creates an entry for every new client key. Long-lived process + Cloudflare's address diversity ⇒ monotonically growing memory (order of ~100 B per distinct key/namespace), invisible to `/health`.
*Fix:* sweep stale buckets on write (cheap: keep a count and `retain` occasionally) or move to a small fixed-window/counter structure with a cap.

**L-5 · Template render panics instead of 500.**
14 `template.render().unwrap()` sites in handlers (`src/http/public.rs`, `posting.rs`, `moderation.rs`). The realistic trigger is the `malaysia_post_time` filter parsing `created_at` (`src/http/mod.rs:60-78`); any malformed/legacy timestamp turns a page into a panicked task (connection reset) rather than a 500. Low probability, trivial to fix, and the fix also removes the last `unwrap`s from request paths.

### 4.2 Correctness and behaviour

**M-6 · Thread pages load every reply, unpaged.**
`forum::load_thread` (`src/forum.rs:2758`) runs `SELECT … FROM replies … ORDER BY created_at, id` with **no LIMIT** and derives `reply_count` from the fetched rows, then renders all of them. Board pages are paged (20), archives (50), and the Telegram snapshot is paged — thread pages are the outlier. Bumping is capped at 300 replies (`THREAD_BUMP_LIMIT`), but nothing caps reply *count*: a long-lived thread accumulates replies indefinitely and each view pays for all of them in rows, allocations and HTML. Upstream issue **#52 "Improve pagination"** is open on exactly this ground.
*Fix:* page replies (e.g. newest N with a "view all" continuation) or at minimum bound the query and count separately.

**M-7 · Approving a held reply can rewind the thread's position.**
`publish_held_target` (`src/forum.rs:2025`) approves a held reply by calling `publish_reply` (`:687`), which does `SET bumped_at = (SELECT created_at FROM replies WHERE id = ?)` — i.e. the held reply's *original* submission time, not the approval time, and unconditionally. If the thread has since been bumped by a later reply, approving an older held reply moves the thread *down* the board listing. (The held thread path is fine: it sets `bumped_at = CURRENT_TIMESTAMP` and emits the withheld `thread_created` event.)
*Fix:* `bumped_at = MAX(COALESCE(bumped_at, created_at), <reply.created_at>)`, or use the approval timestamp to match the thread path and the `docs/MODERATION_SPEC.md` wording.

**L-8 · Hiding a reply leaves stale ordering.**
`apply_direct_hide` (`src/forum.rs:2058`) and `update_target_status` hide the reply but never recompute the parent thread's `bumped_at`, so a thread can keep the position it earned from a now-hidden reply (and a hidden reply can still be one of the three "recent replies" previewed on the board — the preview query filters `status='visible'`, so the *preview* is right, the *ordering* is not). Same pattern for `hide/remove/quarantine` via the report queue.

**L-9 · Locked/archived replies pay for screening and media before being rejected.**
`create_reply` (`src/http/posting.rs:573`) runs Miya screening and uploads the attachment to `mchan-image`, then discovers `Locked`/`Archived`/`NotFound` and cleans up the processed image. `forum::classify_reply_target` already exists (`src/forum.rs:536`) and is used by the Telegram path — the web path should use it (or move the check into `create_reply` before the external calls). Today a locked thread still costs one Miya call plus one image round-trip per attempt, and the client sees `409` after a wasted upload.

**L-10 · `machine_idempotency` never expires.**
`src/forum.rs:435-486` inserts/reads/updates; there is no `DELETE` anywhere, and `main.rs`'s hourly janitor purges only abuse logs and acknowledged outbox events, even though migration 0016 created `idx_machine_idempotency_created_at` (an index that exists for a purge that was never written). Consequences: unbounded growth on a busy machine API, and every opaque key is reserved forever — a caller that reuses a key after a year still gets a 409. Note also the key namespace is global per `(service, operation, key)`, so two different Telegram principals must never choose the same key; that is worth stating in the API docs.
*Fix:* purge rows older than N days (e.g. 1–7) in the same hourly task, and document the key namespace rule.

**L-11 · `/health` uptime measures the wrong interval.**
`PROCESS_START.get_or_init(Instant::now)` (`src/http/operations.rs:64, 93, 126`) initialises lazily on the *first* health/metrics request, not at process start, so `uptime_seconds` is time-since-first-poll. `docs/OPERATIONS.md` explicitly promises "seconds since this process started". Under a real poller the delta is seconds; after a long idle gap (a restarted poller, a paused container) it becomes wrong, and monitoring that alerts on uptime would be misled.
*Fix:* set the `OnceLock` in `main.rs` (or use a `LazyLock` initialised at startup) and assert the semantics in a test.

**L-12 · Deep page offsets accepted.**
`page_offset` (`src/http/public.rs:7-29`) accepts any positive integer page; `?page=100000000` becomes `OFFSET 2e9`, a full index walk for a page that cannot exist. Bounded cost per request, trivially spammable.

### 4.3 Observability, delivery and CI

**M-13 · No logging or tracing layer.**
No `tracing`/`log` dependency; 12 `eprintln!` sites and nothing else. No access logs, no request IDs, no structured events, no log levels, nothing correlating a slow request with an upstream. `/health` + `/internal/metrics` cover "is it up / how much content", not "what happened". For a service that already streams deploys and moderate traffic, this is the biggest operational gap after H-1.
*Fix:* add `tracing` + `tracing-subscriber` + `tower-http`'s `TraceLayer`, keep the existing `no-store`/metrics behaviour, and log the existing failure points (Miya, media, webhook, outbox lease) with structured fields. 1–2 crates, no architectural change.

**M-14 · Outbox delivery is coupled to the Telegram listener.**
`/internal/telegram/outbox/{lease,ack}` only exist when `MCHAN_TELEGRAM_SERVICE_TOKEN` is set, because the second listener is created only then (`src/main.rs:150`, `:168-176`). Any other consumer of `projection_outbox` must reuse the Telegram token or the whole endpoint set stays off. Worth a note in the roadmap: either split the outbox routes onto their own listener/token or document that the Telegram service is the only intended consumer.

**M-15 · CI supply chain and linting.**
`.github/workflows/{rust,production}.yml` use floating refs (`actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `docker/build-push-action@v6`) — for a workflow that ships straight to production, pin by SHA (and/or require manual approval on the `production` environment). Also: no `cargo clippy` and no dependency audit/Dependabot in a codebase whose security posture rests on ~15 crates. `rust:1-alpine` is a floating base image for a release-bearing build.

**L-16 · Minor duplication and dead-ish code.** `no_store_headers()` is defined twice (`src/http/mod.rs:748`, `src/http/operations.rs:79`) with different visibility; `error_response`/`ModerateError` in `operations.rs` do double duty for metrics errors; the formatter-exclusion list is duplicated between `Makefile` and both workflows.

### 4.4 Documentation drift (matters here — the docs are the spec)

| # | Location | Claim | Reality |
| --- | --- | --- | --- |
| D-1 | `docs/MODERATION_SPEC.md:232` | "The repository currently has **58** deterministic tests" (and ":237" — "eight" in the moderation subset) | **169** test functions; the moderation subset alone has ≥21 |
| D-2 | `docs/PRIVACY.md:20` | "MChan does not currently provide user accounts, profiles, or **file uploads**" | Attachments shipped in v0.6 (`post_media`, `mchan-image`, `/images/*`); the policy also never discloses that uploads are transmitted to an external processor or that processed media is stored and served |
| D-3 | `templates/home.html:41` | "This is the **closed** testing phase" | README/CHANGELOG describe the **open** beta since v0.9 |
| D-4 | `REPO` root | no `LICENSE` file; GitHub reports `license: null` | README advertises `mchan-core` as an engine and links the repo for reuse, but reuse rights are undefined (default = all rights reserved) |
| D-5 | `docs/MODERATION_SPEC.md` (held posts) | approval semantics described precisely | correct for threads, incomplete for replies — see M-7 (approval timestamp vs original timestamp) |
| D-6 | `templates/mod_reports.html:40` | moderator queue shows raw `created_at` (UTC) | public pages render MYT through the `malaysia_post_time` filter; the staff queue does not — upstream **#35** is open for exactly this |
| D-7 | README "Current beta → Implemented" | no mention of the Telegram project or the outbox | both exist since v0.10 and are only described in `docs/CHANGELOG.md`/`OPERATIONS.md` |

---

## 5. Test suite assessment

Coverage is genuinely contractual rather than incidental: validation boundaries, status transitions, archive/lock/report behaviour, rate limits, CAPTCHA challenge namespacing and draft preservation, media cleanup and rollback, moderation scoping (admin vs assigned board moderator), ban limits, origin retention, abuse-log auditing, cache headers, Telegram idempotency (replay/conflict/rollback), outbox leasing/ack token mismatch, and migration application.

Gaps worth adding, in priority order:

1. **Timeout behaviour** for the two unbounded clients (H-1) — a loopback listener that never answers.
2. **Ordering invariants** for M-7/L-8: approve a held reply after the thread has been bumped and assert `bumped_at`; hide a bumping reply and assert board order.
3. **Rate-limiter eviction** (M-4) — assert the map does not retain idle keys (needs a small test hook or a pure-function split).
4. **`/health` uptime** semantics (L-11) — assert the value is ≥ elapsed wall time since dependency construction.
5. **Bounded thread rendering** (M-6) once paging exists.
6. A docs-consistency test: assert that numbers like the test count, or that claims such as "no file uploads", cannot silently drift again (the release script already does this pattern for the version string — extend it).

---

## 6. External context (upstream backlog, read-only)

Open upstream issues that map onto this analysis: **#52** pagination (M-6), **#50** missing OP tags / reply anchors, **#54** Markdown + link support in bodies (bodies are escaped plain text today; `>>123` quoting is inserted client-side by `post-dialog.js` but not linked on render), **#35** MYT timestamps in the mod queue (D-6), **#48** sitemap + Turnstile, **#49** mod/staff tags, **#28** legend for moderation decisions, **#13** toggles for the mascot/animation. Recently closed: **#47** "posts do not come down" → the v0.9.4 bump bounding implemented here; **#31** ban bypass (answered by the origin-clearing/retention design plus network-layer trust assumptions — see M-3 for why that assumption deserves code-level backing).

---

## 7. Recommended next steps

**Do now (hours, no design decisions):**
1. Add timeouts to the captcha and webhook clients; make the webhook non-blocking (H-1).
2. Purge `machine_idempotency` in the existing hourly task (L-10).
3. Fix `/health` uptime initialisation (L-11).
4. Rate-limiter pruning (M-4).
5. Correct `docs/PRIVACY.md` (uploads, external processor, retention) and `docs/MODERATION_SPEC.md` (test count) — these are user-facing and reviewer-facing claims (D-1, D-2).
6. Add a `LICENSE` if the fork is meant to be reusable (D-4).

**Do next (a focused change each):**
7. Edge-header hardening: trust `cf-connecting-ip`/`x-forwarded-for` only from the proxy, plus `Sec-Fetch-Site`/`Origin` checks on mutations (M-3, H-2).
8. `tracing` + `TraceLayer` and structured logs at the existing failure sites (M-13).
9. Reply pagination or a bounded thread render + separate count query (M-6).
10. `ORDER BY`-safety fix for held-reply approval and hidden-reply hiding (M-7, L-8); reuse `classify_reply_target` before screening/media in the web reply path (L-9).

**Consider (roadmap):**
11. Pin CI actions by SHA, add `cargo clippy -D warnings` and Dependabot, pin the Rust base image for releases (M-15).
12. Enable SQLite WAL for better read/write concurrency as traffic grows. **Verified against sqlx-sqlite 0.8.6 source**: `SqliteConnectOptions` already defaults `busy_timeout` to 5 s and `foreign_keys` to ON, but deliberately leaves `journal_mode` unset ("Don't set `journal_mode` unless the user requested it", referencing launchbadge/sqlx#1930), so the deployed DB runs SQLite's compiled-in default (rollback journal), *not* WAL. Apply it at connect time — `SqliteConnectOptions::from_str(&database_url)?.journal_mode(SqliteJournalMode::Wal)` (`src/main.rs:122`) — **not** as a migration: `PRAGMA journal_mode=WAL` cannot take effect inside a transaction, and SQLx runs each migration in one. See addendum item A-4.
13. Split outbox delivery from the Telegram token if a second consumer is expected (M-14).
14. Product items from upstream: pagination UI/jump-to-page, markdown/links, reply anchors + OP tags, MYT in the staff queue.

---

## 8. Verification notes (what a reviewer can re-run here)

```sh
# migrations apply cleanly, seeds intact (verified: 17 files, 13 tables)
python3 - <<'PY'
import sqlite3, glob
con = sqlite3.connect(":memory:")
for f in sorted(glob.glob("migrations/*.sql")): con.executescript(open(f).read())
print("migrations OK")
PY

# release/version consistency across Cargo.toml, Cargo.lock, changelog, home template
python3 scripts/release.py --check        # exit 0 at 0.11.0

# formatter parity with CI (needs pip; CI pins syntaqlite==0.7.1)
python3 -m pip install "syntaqlite==0.7.1"
make format-check

# the real gate, needs a Rust toolchain (not available in this sandbox)
cargo fmt --all -- --check && cargo check && cargo test

# smoke test the affected paths once built
export MCHAN_ABUSE_KEY="$(openssl rand -hex 32)"
cargo run   # http://localhost:3000/health
```

*Report generated as an analysis artifact; it is not part of the upstream project. Delete or relocate freely.*

---

## 9. Addendum — efficiency and front-end measurements (second pass)

Verified against upstream library source and the on-disk assets; still no Rust toolchain, so these are measurements and source reads, not benchmarks.

### A-1 · Request/query shape per page

| Route | SQL statements | Outbound calls | Notes |
| --- | --- | --- | --- |
| `GET /` | 1 | — | boards + nothing else |
| `GET /boards/{slug}` | 2 | — | page of 20 threads (with `COUNT` of visible replies) + one window-function query for the three newest replies of *those* thread IDs |
| `GET /boards/{slug}/archive` | 2 | — | same shape, 50/page, `created_at DESC` |
| `GET /threads/{id}` | **2** | — | thread + **all** visible replies, unpaged (`forum.rs:2758`) |
| `POST` a thread | ~4 (+1 ban check) | 1 Miya, 1 image, 1 webhook | serialized before the response |
| `POST` a reply | ~5 | 1 Miya, 1 image, 1 webhook | serialized before the response |

There is **no N+1**: reply previews are batched with `IN (…)` + `ROW_NUMBER() OVER (PARTITION BY thread_id …)` rather than one query per thread, and my earlier concern about a separate `COUNT(*)` scan for `reply_count` is unfounded — `COUNT(r.id)` rides the same `LEFT JOIN` that produces the page.

### A-2 · Where the time actually goes

- **Cold page render**: 1–2 SQLite reads + one Askama render, no image work in-process. Payloads are server-rendered HTML, so a board page with media is dominated by images, not by Core.
- **Posting is serialized chain**: ban check → rate check → origin encrypt → **Miya (≤2 s)** → **image upload (30 s timeout)** → SQLite write → **Discord webhook (no timeout)** → redirect. Every link is awaited in order, so p95 posting latency is the *sum* of the services, not the max. The webhook being last and unbounded (H-1) is the worst offender because it sits between the DB commit and the user's redirect.
- **Memory**: the rate limiter map never evicts (M-4); a thread view holds every reply in RAM at once (M-6); `machine_idempotency` never expires (L-10).

### A-3 · Static asset weight — 78% of it is dead

| Item | Size |
| --- | --- |
| `static/` total | **4.0 MB** |
| Unreferenced GIFs (`navi1/2/3.gif`, `lainburn`, `laindance`, `lainroll`, `crow`, `flyinggirl`) | **3.1 MB** — no string in `lainpet-snippet.js` or any template references them, yet `ServeDir` still publishes them |
| `lainpet-snippet.js` | 22.8 KB, unminified, loaded on 5 of 11 templates (including `404.html`) |
| `android-chrome-512x512.png` | 151 KB (referenced only from `og:image`; the webmanifest has no PNG entry) |
| `style.css` | 13.1 KB, one file, 1 `@media` block, 0 `@keyframes` |

These dead assets cost **no page bandwidth** (nothing requests them) but they are publicly fetchable, they inflate the image and every CI `docker save`, and they represent leftover mascot experiments. Deleting them is free.

### A-4 · Two sqlx defaults that matter (source-verified, v0.8.6)

1. `busy_timeout` = 5 s and `foreign_keys` = ON are already set by sqlx — so the "add these" advice is unnecessary; **WAL is the only DB-setting gap** (see step 12).
2. `bool`/`i64` decode **silently** from NULL: `impl Decode for bool { Ok(value.int64() != 0) }` and `int64()` is the raw accessor, which yields 0 for SQL `NULL`. Evidence in this repo: **all 22 seeded `threads.is_pinned` values are NULL**, and they decode into the non-optional `is_pinned: bool` fields (`forum.rs:388`, `:409`) as `false` with no error. So no page 500s — but "not pinned" has two representations (`NULL` and `0`), SQL null semantics (`NULL = 0` is false) diverge from Rust decode semantics (`NULL` → `false`), and any future `WHERE is_pinned = 0` would silently miss every legacy row. Not a live bug today (`ORDER BY` uses `COALESCE(t.is_pinned, 0)`, pin/unpin writes 0/1). Fix by normalising in a migration (`UPDATE threads SET is_pinned = COALESCE(is_pinned, 0)`) and/or `COALESCE` in the two `SELECT`s, and treat "nullable column read into a non-`Option` Rust field" as a review rule.

  This also clears the old suspicion around upstream issue #47 ("unpin does not persist"): `set_thread_pinned` binds a `bool` (0/1) and the ordering query already `COALESCE`s, so unpin persists and re-sorting works — that issue was closed by the 0.9.4 change.

### A-5 · Front-end / UX findings

1. **iOS auto-zoom on every form field.** Root font is `13px` (`static/style.css:30`) and `input, select, textarea { font: inherit }` (`:417-423`) → 13px controls, below the 16px threshold at which iOS Safari stops zooming the viewport on focus. Every reply/report/hide dialog zooms a phone layout. One-line fix.
2. **No `prefers-reduced-motion` anywhere** — not in CSS, not in `lainpet-snippet.js`. The mascot runs a perpetual `requestAnimationFrame` loop plus two `setInterval` timers on every page (including 404). `AGENTS.md` explicitly requires preserving reduced-motion behaviour, so the stated convention is currently violated by the mascot; a `matchMedia("(prefers-reduced-motion: reduce)")` guard that parks the sprite is small and puts the code back inside its own rules.
3. **No dark mode / `prefers-color-scheme`**, and no `Cache-Control` on `/static` or `/images` (tower-http's `ServeDir` only sets `Last-Modified`/`If-Modified-Since`), so browsers fall back to heuristic revalidation per asset. `/images/<image_id>/<variant>.webp` is content-addressed and can safely be `Cache-Control: public, max-age=31536000, immutable`; `/static` can take a long max-age too given the deploy model.
4. **Accessibility is otherwise solid and one AGENTS.md claim checks out**: `:focus-visible` styles exist for `a`/`button`/`input`/`select`/`textarea`, templates carry labels/`alt`/`aria-label`/`aria-expanded`, dialogs are real `<dialog>` elements, and the mascot is decorative-only markup.
5. **Look and feel is deliberate, not accidental**: cream/slate/lime palette, Arial 13px/1.35, 1100px column, default-blue links, dense information layout — a faithful 2000s imageboard aesthetic with a modern mascot. That is a *choice* consistent with the genre and the audience; I would not "modernise" it. The only things I would change are the two mechanical issues above plus contrast/spacing at small widths.
