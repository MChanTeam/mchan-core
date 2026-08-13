# Operations guide

## Health polling

`GET /health` is an unauthenticated SQLite health check for load balancers and
frequent polling. It returns HTTP 200 when `SELECT 1` succeeds and HTTP 503
otherwise. Responses are private and have `Cache-Control: no-store, no-cache`
so a poller does not reuse stale health state.

The JSON response always contains:

- `status`: `"ok"` for a healthy check or `"unhealthy"` for a failed check.
- `service`: always `"mchan"`.
- `version`: the MChan package version compiled into the running binary
  (`env!("CARGO_PKG_VERSION")`).
- `uptime_seconds`: a nonnegative integer number of seconds since this process
  started.
- `database`: `"ok"` when the SQLite check succeeds or `"unhealthy"` when it
  fails.

A healthy response is HTTP 200:

```json
{
  "status": "ok",
  "service": "mchan",
  "version": "0.2.0",
  "uptime_seconds": 864,
  "database": "ok"
}
```

An unhealthy response is HTTP 503:

```json
{
  "status": "unhealthy",
  "service": "mchan",
  "version": "0.2.0",
  "uptime_seconds": 865,
  "database": "unhealthy"
}
```

The version shown in these examples is illustrative; deployments should
report the package version of their own binary. Uptime resets when the process
restarts and is not a timestamp or a database-duration measure.

For a one-off check:

```sh
curl -iS http://localhost:3000/health
```

For simple health polling, use the HTTP status rather than caching the JSON:

```sh
while curl -fsS http://localhost:3000/health >/dev/null; do sleep 10; done
```

## Operational metrics

`GET /internal/metrics` returns authenticated operational aggregates for the
private `mchan-ops` service. Set `MCHAN_OPS_TOKEN` to a nonempty shared secret
to enable the endpoint. If it is unset or empty, the route returns HTTP 404.
Requests without the matching bearer token return HTTP 401.

```sh
export MCHAN_OPS_TOKEN='replace-with-a-secret-token'
curl -sS http://localhost:3000/internal/metrics \
  -H "Authorization: Bearer ${MCHAN_OPS_TOKEN}"
```

The response contains process metadata; database health; counts of approved
boards, active visible or locked threads, visible replies, pending reports,
and active board/site bans; and whether Miya and the image processor are
configured. These are aggregate values only. The endpoint does not return
post or report content, origins, fingerprints, moderator identities, Discord
IDs, or secrets. Responses use `Cache-Control: no-store, private`.

`/health` remains the cheap liveness/database-health endpoint for frequent
polling. `/internal/metrics` is the authenticated operational-aggregate
endpoint and performs the additional count queries; do not substitute it for
frequent health polling.

Keep `MCHAN_OPS_TOKEN` secret: do not commit, print, log, or place it in a URL.
Expose the route only over the private service-to-Core path. Core only exposes
the measurements; `mchan-ops` owns polling, alerting, and downstream actions.

## Discord moderation bot agent

The authenticated `POST /internal/discord/moderate` endpoint applies a
moderation action to an existing report. It does not classify content, call
Miya, create reports, or accept post text. Set
`MCHAN_DISCORD_MODERATION_TOKEN` to a shared secret to enable the endpoint:

```sh
export MCHAN_DISCORD_MODERATION_TOKEN='replace-with-a-secret-token'
curl -sS -X POST http://localhost:3000/internal/discord/moderate \
  -H "Authorization: Bearer ${MCHAN_DISCORD_MODERATION_TOKEN}" \
  -H 'Content-Type: application/json' \
  --data '{"report_id":123,"action":"hide","moderator":"discord:123456789012345678"}'
```

The request JSON is `{"report_id":u64,"action":...,"days":optional
u32,"moderator":"discord:<stable user id>"}`. `moderator` is trimmed,
nonempty, no longer than 120 characters, and must begin with `discord:`.
Allowed actions are `dismiss`, `resolve`, `hide`, `remove`, `quarantine`,
`lock`, `ban-board`, and `ban-site`. Omit `days` for every action except
`ban-board` and `ban-site`; board bans require 1–30 days and site bans require
1–365 days. The action is applied atomically to the report and its target,
using the same moderation rules as the protected moderator queue.

### Statuses and retry safety

HTTP status handling is stable: 200 means applied; 401 means the bearer token
is missing or incorrect; 404 means the endpoint is disabled (no configured
secret) or the report does not exist; 409 means the report was already
handled; 422 means the action, target, days, moderator, or retained report
origin is invalid; and 500 means a database failure. Responses are JSON and
should not be cached.

A 409 is safe to treat as already complete. Do not blindly retry a 500: check
the report status before submitting the action again. Since each valid action
is applied atomically, retry only after confirming whether the original request
committed.

### Auditing and secret/network handling

Keep `MCHAN_DISCORD_MODERATION_TOKEN` secret: do not commit it, print it, log
it, or place it in a URL. Prefer exposing this route only on a private
network or localhost/restricted reverse proxy, and allow Discord bot traffic
through an isolated service-to-Core path rather than the public internet. The
`moderator` value is written to the moderation audit as the actor, so use a
stable Discord user ID and do not impersonate a human moderator.
