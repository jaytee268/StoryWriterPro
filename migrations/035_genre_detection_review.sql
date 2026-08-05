CREATE TABLE IF NOT EXISTS book_genre_detection_runs (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    primary_genre_id TEXT,
    custom_primary_genre TEXT,
    secondary_genre_ids_json TEXT NOT NULL DEFAULT '[]',
    custom_secondary_genres_json TEXT NOT NULL DEFAULT '[]',
    confidence REAL NOT NULL,
    reasoning TEXT NOT NULL,
    supporting_signals_json TEXT NOT NULL DEFAULT '[]',
    contradicting_signals_json TEXT NOT NULL DEFAULT '[]',
    alternative_genres_json TEXT NOT NULL DEFAULT '[]',
    audience_notes_json TEXT NOT NULL DEFAULT '[]',
    warnings_json TEXT NOT NULL DEFAULT '[]',
    provider_id TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    review_status TEXT NOT NULL DEFAULT 'pending' CHECK(review_status IN ('pending','confirmed','rejected','uncertain','skipped')),
    detected_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_book_genre_detection_runs_job ON book_genre_detection_runs(job_id, review_status);

ALTER TABLE books ADD COLUMN genre_supporting_signals_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE books ADD COLUMN genre_contradicting_signals_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE books ADD COLUMN genre_alternatives_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE books ADD COLUMN genre_audience_notes_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE books ADD COLUMN genre_warnings_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE books ADD COLUMN genre_prompt_version TEXT;
