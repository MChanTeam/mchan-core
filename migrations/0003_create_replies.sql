CREATE TABLE replies (
	id INTEGER PRIMARY KEY,
	thread_id INTEGER NOT NULL REFERENCES threads(id),
	body TEXT NOT NULL,
	status TEXT NOT NULL
		CHECK (status IN ('visible', 'hidden')),
	created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_replies_thread_id
	ON replies (thread_id);

INSERT INTO replies (thread_id, body, status)
VALUES (
	(
	SELECT id
		FROM threads
		WHERE title = 'Welcome to Engineering'
	),
	'Glad to be here.',
	'visible'
	);

INSERT INTO replies (thread_id, body, status)
   VALUES (
       (
           SELECT id
           FROM threads
           WHERE title = 'Welcome to Engineering'
       ),
       'Looking forward to meeting everyone.',
       'visible'
   );
