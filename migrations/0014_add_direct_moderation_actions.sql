CREATE TABLE direct_moderation_actions(
  id INTEGER PRIMARY KEY,
  moderator_email TEXT NOT NULL,
  target_kind TEXT NOT NULL CHECK(target_kind IN ('thread', 'reply')),
  target_id INTEGER NOT NULL,
  reason TEXT
    NOT NULL
    CHECK(
      reason IN (
        'spam',
        'harassment',
        'doxxing',
        'threat',
        'sexual-content',
        'illegal-content',
        'off-topic',
        'board-rule',
        'other'
      )
    ),
  note TEXT,
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE INDEX idx_direct_moderation_actions_target ON direct_moderation_actions (
  target_kind,
  target_id
);
