CREATE TABLE reports(
  id INTEGER PRIMARY KEY,
  thread_id INTEGER REFERENCES threads(id),
  reply_id INTEGER REFERENCES replies(id),
  reason TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('pending', 'resolved', 'dismissed')),
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
  CHECK(
    (thread_id IS NOT NULL AND reply_id IS NULL)
    OR (thread_id IS NULL AND reply_id IS NOT NULL)
  )
);

CREATE INDEX idx_reports_status ON reports (status);
