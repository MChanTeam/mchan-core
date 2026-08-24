CREATE TABLE machine_idempotency(
  id INTEGER PRIMARY KEY,
  service TEXT NOT NULL CHECK(length(CAST(service AS BLOB)) BETWEEN 1 AND 128),
  operation TEXT
    NOT NULL
    CHECK(length(CAST(operation AS BLOB)) BETWEEN 1 AND 128),
  opaque_key TEXT
    NOT NULL
    CHECK(length(CAST(opaque_key AS BLOB)) BETWEEN 1 AND 512),
  request_hash BLOB NOT NULL CHECK(length(request_hash) = 32),
  result_id INTEGER CHECK(result_id IS NULL OR result_id > 0),
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
  updated_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
  UNIQUE(service, operation, opaque_key)
);

CREATE INDEX idx_machine_idempotency_created_at ON machine_idempotency (
  created_at
);

CREATE TABLE projection_outbox(
  id INTEGER PRIMARY KEY,
  kind TEXT
    NOT NULL
    CHECK(
      kind IN (
        'thread_created',
        'thread_dirty',
        'thread_removed',
        'report_created'
      )
    ),
  thread_id INTEGER,
  reply_id INTEGER,
  report_id INTEGER,
  created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
  lease_token TEXT,
  lease_expires_at TEXT,
  acknowledged_at TEXT,
  CHECK(thread_id IS NULL OR thread_id > 0),
  CHECK(reply_id IS NULL OR reply_id > 0),
  CHECK(report_id IS NULL OR report_id > 0),
  CHECK((lease_token IS NULL) = (lease_expires_at IS NULL)),
  CHECK(
    (kind IN ('thread_created', 'thread_dirty', 'thread_removed')
    AND thread_id IS NOT NULL
    AND reply_id IS NULL
    AND report_id IS NULL)
    OR (kind = 'report_created'
    AND report_id IS NOT NULL
    AND reply_id IS NULL
    AND thread_id IS NOT NULL)
    OR (kind = 'report_created'
    AND report_id IS NOT NULL
    AND reply_id IS NOT NULL
    AND thread_id IS NOT NULL)
  )
);

CREATE INDEX idx_projection_outbox_pending ON projection_outbox (
  acknowledged_at,
  lease_expires_at,
  id
);

CREATE INDEX idx_projection_outbox_lease ON projection_outbox (
  lease_token,
  lease_expires_at
);

CREATE INDEX idx_projection_outbox_created_at ON projection_outbox (created_at);
