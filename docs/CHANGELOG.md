# Changelog

All notable changes to MChan are documented here.
## [0.7] - 2026-08-12

- Added `GET /health`, which checks SQLite availability and returns
  `{"status":"ok"}` with HTTP 200 when healthy or
  `{"status":"unhealthy"}` with HTTP 503 otherwise. It is unauthenticated and
  suitable for frequent polling.
- Added authenticated `POST /internal/discord/moderate` for Discord bot
  operations against existing report IDs. It applies the requested
  `dismiss`, `resolve`, `hide`, `remove`, `quarantine`, `lock`, `ban-board`, or
  `ban-site` action atomically; it does not classify content or call Miya.
  Configure `MCHAN_DISCORD_MODERATION_TOKEN` and send a bearer token plus JSON
  `{"report_id":123,"action":"hide","moderator":"discord:123456789012345678"}`.
  `moderator` must be a trimmed, nonempty `discord:` actor no longer than 120
  characters. `days` is required only for bans: 1–30 for `ban-board` and
  1–365 for `ban-site`.
- Responses use stable statuses: 200 applied, 401 missing/incorrect bearer,
  404 disabled endpoint or missing report, 409 already handled, 422 invalid
  action/target/days/moderator/origin, and 500 database failure. A 409 is
  retry-safe; callers must check report status before retrying a 500. Keep the
  shared secret out of URLs, logs, commits, and public networks. The supplied
  Discord actor is recorded as the moderation audit actor.


## [0.6] - 2026-08-11

- Added optional JPEG, PNG, and WebP thread and reply attachments through
  `mchan-image`.
- Limited uploads to 20 MiB.
- Added display and thumbnail rendering for supported attachments.
- Added atomic media persistence with compensation when a post cannot be
  completed.
- Added this public changelog.

## [0.5]

Closed-beta baseline preceding the v0.6 release.
