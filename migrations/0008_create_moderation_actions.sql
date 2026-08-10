CREATE TABLE moderation_actions(
  id INTEGER PRIMARY KEY,
  report_id INTEGER NOT NULL REFERENCES reports(id),
  moderator_email TEXT NOT NULL,
  action TEXT NOT NULL CHECK(action IN ('dismiss', 'resolve', 'hide', 'lock')),
  target_kind TEXT NOT NULL CHECK(target_kind IN ('thread', 'reply')),
  target_id INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE INDEX idx_moderation_actions_report_id ON moderation_actions (report_id);
