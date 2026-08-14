ALTER TABLE threads ADD COLUMN bumped_at;

UPDATE threads SET bumped_at = created_at WHERE bumped_at IS NULL;

ALTER TABLE threads ADD COLUMN is_pinned;

CREATE INDEX idx_threads_board_listing ON threads (
  board_id,
  is_pinned DESC,
  bumped_at DESC,
  id DESC
);

CREATE TABLE board_moderators(
  board_slug TEXT NOT NULL REFERENCES boards(slug),
  email TEXT NOT NULL CHECK(email = lower(email)),
  PRIMARY KEY(board_slug, email)
);
