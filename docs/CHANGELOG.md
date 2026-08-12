# Changelog

All notable changes to MChan are documented here.
## [0.7] - 2026-08-12

* Added a lightweight health check for service monitoring and database availability.
* Added authenticated Discord moderation integration for handling existing reports.
* Discord moderators can dismiss or resolve reports and apply content, thread, board, and site moderation actions.
* Moderation actions are applied atomically and recorded in the moderation audit log.
* Added validation and predictable error handling for Discord moderation requests.
* Discord moderation operates independently from Miya and does not perform content classification.


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
