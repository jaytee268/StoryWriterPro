CREATE TABLE IF NOT EXISTS manuscript_structure_runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending','running','completed','failed','reviewed')),
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_structure_runs_chapter ON manuscript_structure_runs(project_id, chapter_id, created_at);

CREATE TABLE IF NOT EXISTS manuscript_structure_proposals (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES manuscript_structure_runs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    temporary_id TEXT NOT NULL,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    title TEXT NOT NULL,
    pov_character_name TEXT,
    pov_entity_id TEXT,
    location TEXT NOT NULL DEFAULT '',
    story_time TEXT NOT NULL DEFAULT '',
    participating_character_names_json TEXT NOT NULL DEFAULT '[]',
    goal TEXT NOT NULL DEFAULT '',
    conflict TEXT NOT NULL DEFAULT '',
    important_events_json TEXT NOT NULL DEFAULT '[]',
    transition_type TEXT NOT NULL,
    boundary_reason TEXT NOT NULL,
    confidence REAL NOT NULL,
    evidence_excerpt TEXT NOT NULL,
    review_status TEXT NOT NULL CHECK (review_status IN ('proposed','accepted','edited','rejected','uncertain')),
    manual_changes_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(run_id, temporary_id)
);
CREATE INDEX IF NOT EXISTS idx_structure_proposals_run ON manuscript_structure_proposals(run_id, start_offset);
