ALTER TABLE moderation_actions RENAME TO moderation_actions_legacy;

CREATE TABLE moderation_actions(
  id INTEGER PRIMARY KEY,
  report_id INTEGER NOT NULL REFERENCES reports(id),
  moderator_email TEXT NOT NULL,
  action TEXT
    NOT NULL
    CHECK(
      action IN (
        'dismiss',
        'resolve',
        'hide',
        'remove',
        'quarantine',
        'lock',
        'ban_board',
        'ban_site'
      )
    ),
  target_kind TEXT NOT NULL CHECK(target_kind IN ('thread', 'reply')),
  target_id INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

INSERT INTO moderation_actions(
  id,
  report_id,
  moderator_email,
  action,
  target_kind,
  target_id,
  created_at
)
SELECT
  id,
  report_id,
  moderator_email,
  action,
  target_kind,
  target_id,
  created_at
FROM moderation_actions_legacy;

DROP TABLE moderation_actions_legacy;

CREATE INDEX idx_moderation_actions_report_id ON moderation_actions (report_id);

CREATE TABLE post_origins(
  id INTEGER PRIMARY KEY,
  thread_id INTEGER UNIQUE REFERENCES threads(id),
  reply_id INTEGER UNIQUE REFERENCES replies(id),
  client_fingerprint BLOB NOT NULL,
  nonce BLOB NOT NULL,
  ciphertext BLOB NOT NULL,
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
  retain_until TEXT NOT NULL DEFAULT (datetime('now', '+30 days')),
  CHECK(
    (thread_id IS NOT NULL AND reply_id IS NULL)
    OR (thread_id IS NULL AND reply_id IS NOT NULL)
  )
);

CREATE INDEX idx_post_origins_fingerprint ON post_origins (client_fingerprint);

CREATE INDEX idx_post_origins_retention ON post_origins (retain_until);

CREATE TABLE bans(
  id INTEGER PRIMARY KEY,
  client_fingerprint BLOB NOT NULL,
  scope TEXT NOT NULL CHECK(scope IN ('board', 'site')),
  board_id INTEGER REFERENCES boards(id),
  report_id INTEGER NOT NULL REFERENCES reports(id),
  moderator_email TEXT NOT NULL,
  reason TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
  revoked_at TEXT,
  CHECK(
    (scope = 'board' AND board_id IS NOT NULL)
    OR (scope = 'site' AND board_id IS NULL)
  )
);

CREATE INDEX idx_bans_active_lookup ON bans (
  client_fingerprint,
  expires_at,
  revoked_at
);

CREATE TABLE abuse_log_accesses(
  id INTEGER PRIMARY KEY,
  moderator_email TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);
