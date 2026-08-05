ALTER TABLE books ADD COLUMN primary_genre_id TEXT;
ALTER TABLE books ADD COLUMN secondary_genre_ids_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE books ADD COLUMN custom_genre_names_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE books ADD COLUMN genre_source TEXT;
ALTER TABLE books ADD COLUMN genre_confidence REAL;
ALTER TABLE books ADD COLUMN genre_reason TEXT;
ALTER TABLE books ADD COLUMN genre_author_confirmed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE books ADD COLUMN genre_detected_at TEXT;
