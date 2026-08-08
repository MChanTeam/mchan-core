ALTER TABLE threads
ADD COLUMN poster_id TEXT NOT NULL DEFAULT 'Anonymous';

ALTER TABLE replies
ADD COLUMN poster_id TEXT NOT NULL DEFAULT 'Anonymous';
