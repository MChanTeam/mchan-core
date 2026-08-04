CREATE TABLE boards(
  id INTEGER PRIMARY KEY,
  slug TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  description TEXT NOT NULL,
  status TEXT
    NOT NULL
    CHECK(status IN ('pending', 'approved', 'rejected', 'archived')),
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

INSERT INTO boards(slug, name, description, status)
VALUES
  (
    'engineering',
    'Engineering',
    'Discussion for the Engineering Faculty.',
    'approved'
  );
