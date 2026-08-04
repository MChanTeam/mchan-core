CREATE TABLE users(
  id INTEGER PRIMARY KEY,
  email TEXT NOT NULL UNIQUE,
  university TEXT NOT NULL,
  role TEXT NOT NULL CHECK(role IN ('user', 'moderator', 'administrator')),
  status TEXT
    NOT NULL
    CHECK(status IN ('pending', 'active', 'suspended', 'banned')),
  verified_at TEXT,
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE verification_tokens(
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id),
  token_hash TEXT NOT NULL UNIQUE,
  expires_at TEXT NOT NULL,
  used_at TEXT,
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE INDEX idx_verification_tokens_user_id ON verification_tokens (user_id);
