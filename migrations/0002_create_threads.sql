CREATE TABLE threads (
	id INTEGER PRIMARY KEY,
	board_id INTEGER NOT NULL REFERENCES boards(id),
	title TEXT NOT NULL,
	body TEXT NOT NULL,
	status TEXT NOT NULL 
		CHECK (status IN ('visible', 'hidden', 'locked')),
	created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_threads_board_id
	ON threads (board_id);

INSERT INTO threads (board_id, title, body, status)
VALUES (
	(SELECT id FROM boards WHERE slug = 'engineering'),
	'Welcome to Engineering',
	'Introduce yourself and share useful resources.',
	'visible'
	);

INSERT INTO threads (board_id, title, body, status)
VALUES (
	(SELECT id FROM boards WHERE slug = 'engineering'),
	'Study group ideas',
	'What subject should we study for',
	'visible'
	)
