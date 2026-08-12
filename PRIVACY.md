# MChan Privacy Policy

**Effective date: 2026-08-10**

MChan is a text-first open beta for anonymous public discussion. This policy
explains what the current beta stores and why. It describes the current
implementation; it does not promise features that have not shipped.

## What MChan stores

When you publish a thread or reply, MChan stores the public content, including
its title or body, board or thread association, creation time, and the
thread-scoped poster ID shown with it. Published content can be read, copied,
quoted, and archived by other people. Threads may become read-only public
archives with no automatic deletion date; treat published content as potentially
permanent. There is no author edit or self-deletion control. Moderators can hide
content, but hiding does not mean that the underlying record is physically
deleted.

MChan does not currently provide user accounts, profiles, or file uploads. Do
not put information in a public post that you need to keep private.

## Anonymous identifiers and operational data

MChan derives the public poster ID shown within each thread from a keyed
fingerprint of the client/network value supplied by the request or its trusted
proxy. The identifier is not an account and does not display your raw network
address, name, or email address.

Anonymous-to-public does **not** mean untraceable. To operate the service,
limit abuse, enforce bans, investigate serious incidents, and protect MChan,
the server processes a client/network value supplied by the request or its
trusted proxy. For each post, MChan stores a protected origin record: the
client/network value is encrypted, and MChan stores a separate keyed
fingerprint. The fingerprint is used to derive thread-scoped poster IDs, for
rate limiting, and for matching active board or site bans.

The encryption and fingerprinting key is kept in the deployment runtime. It is
required to decrypt retained origin records and to produce matching
fingerprints. The key must remain secret and stable while those records are
retained. If it is lost or changed, old protected records may no longer be
readable or matchable. No internet service can guarantee absolute security.

## Reports, moderation, and bans

MChan stores reports submitted for threads and replies, including the selected
reason, target, status, and time. It stores moderation actions and audit
information, which may include the target, action, time, and the authenticated
moderator identity. It also stores board and site bans, their protected keyed
fingerprint, scope, reason, moderator identity, and expiry. Board bans can be
set for 1–30 days and site bans for 1–365 days.

These records are used to review rule violations, keep moderation accountable,
prevent repeat abuse, enforce bans, and protect the service. Reports,
moderation records, and ban records do not currently share the automatic
30-day deletion period for post-origin records. They may remain longer where
needed for moderation, abuse investigation, audit, or service protection.

## Retention and access

Protected post-origin records are retained for up to **30 days**. MChan purges
expired origin records at startup and hourly while the process is running.
Public posts and archives, reports, moderation actions, and bans have no
automatic 30-day deletion period and may be retained longer; the beta does not
promise a general deletion date for them.

Ordinary users cannot view protected network information. Decrypted abuse
records are available only to an authenticated, allowlisted moderator, and
access to that view is recorded. The protected abuse-log response is marked
`Cache-Control: no-store, private` and `Pragma: no-cache` so it is not intended
to be stored by browser or shared caches. Public content is different: other
people and their software may copy or cache it.

## Cloudflare and Turnstile

The deployment uses Cloudflare Tunnel and Cloudflare Access for routing and
restricted moderator access. Cloudflare may therefore process request,
network, and authentication information as part of those services.

Turnstile is optional. When enabled, MChan may present a challenge when posting
behaviour appears suspicious or rate limits are reached. MChan sends the
Turnstile response and the relevant client/network value to the configured
Turnstile verification service, normally Cloudflare, to verify the challenge.
When Turnstile is not configured, this extra challenge and verification are
not used.

## Disclosure

MChan may share protected information with service providers involved in
hosting, routing, access control, or security, and may disclose information
when reasonably necessary to investigate abuse or safety incidents or to
comply with applicable law or valid legal process. MChan does not make public
the protected network value or decrypted abuse records as part of ordinary
site use.

## Reporting and operator channels

Use the **Report** control on the relevant thread or reply to flag spam,
harassment, doxxing, threats, illegal content, or another rule concern. For a
safety, privacy, legal, or operational issue that cannot be reported there,
use the published operator channel for the current MChan deployment. This
policy intentionally does not invent or publish an email address or other
operator contact detail.

## Changes

MChan may update this policy as the beta changes. The effective date at the top
will be updated when a new version takes effect. The current policy is the
version published at this path.
