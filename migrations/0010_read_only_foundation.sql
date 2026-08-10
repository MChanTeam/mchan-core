ALTER TABLE threads ADD COLUMN archived_at;

CREATE INDEX idx_threads_board_archive ON threads (
  board_id,
  archived_at,
  status,
  created_at DESC,
  id DESC
);

CREATE TABLE post_media(
  id INTEGER PRIMARY KEY,
  thread_id INTEGER REFERENCES threads(id),
  reply_id INTEGER REFERENCES replies(id),
  thumbnail_path TEXT NOT NULL,
  display_path TEXT NOT NULL,
  mime_type TEXT NOT NULL,
  width INTEGER NOT NULL CHECK(width > 0),
  height INTEGER NOT NULL CHECK(height > 0),
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
  CHECK(
    (thread_id IS NOT NULL AND reply_id IS NULL)
    OR (thread_id IS NULL AND reply_id IS NOT NULL)
  )
);

CREATE UNIQUE INDEX idx_post_media_thread_id ON post_media (thread_id)
WHERE
  thread_id IS NOT NULL;

CREATE UNIQUE INDEX idx_post_media_reply_id ON post_media (reply_id)
WHERE
  reply_id IS NOT NULL;

UPDATE threads
SET
  archived_at = CURRENT_TIMESTAMP
WHERE
  id
  = (
    SELECT t.id
    FROM threads AS t
    JOIN boards AS b ON b.id = t.board_id
    WHERE
      b.slug = 'engineering'
      AND t.title = 'Study group ideas'
      AND t.archived_at IS NULL
    ORDER BY
      t.id
    LIMIT 1
  );
