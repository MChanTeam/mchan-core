# Changelog

All notable changes to MChan are documented here.

## [Unreleased]

- Updated the README and repository documentation to reflect the current MChan
  closed beta, integrations, routes, deployment model, and local development
  workflow.
- Moved supporting project documents into `docs/` and cleaned generated local
  artifacts from the repository root.
- Added the homepage team credits for Anthonny “Kuumin”, Mxrza, Chifuyu, Forg,
  and JavanMyna.
- Added a “Powered by mchan-core” link to the official
  [mchan-core repository](https://github.com/MChanTeam/mchan-core).
- Standardized public branding on `MChan` and updated community wording to
  describe Malaysian higher-education communities.
- Added the Micon mascot beside the MChan homepage title.
- Simplified the beta moderation queue to Dismiss, Resolve, Hide, Lock, Board
  ban, and Site-wide ban by removing duplicate Remove and Quarantine controls.
- Added concise root `AGENTS.md` repository guidelines covering architecture,
  commands, conventions, runtime configuration, migrations, and deterministic
  testing patterns.

## [0.7] - 2026-08-12

- Added a lightweight health check for service monitoring and database availability.
- Expanded `GET /health` with service, package version, process uptime, and
  database status metadata for safe frequent polling.
- Added authenticated Discord moderation integration for handling existing reports.
- Discord moderators can dismiss or resolve reports and apply content, thread,
  board, and site moderation actions.
- Moderation actions are applied atomically and recorded in the moderation audit log.
- Added validation and predictable error handling for Discord moderation requests.
- Discord moderation operates independently from Miya and does not perform content
  classification.
- Added Miya text screening for allow, review, and block decisions.



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
