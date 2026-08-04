-- Resumable, sequential manuscript continuity analysis.
-- Additive status overlay for continuity runs. Existing run rows and checks stay intact.
CREATE TABLE continuity_review_run_statuses (
    run_id TEXT PRIMARY KEY REFERENCES continuity_review_runs(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK(status IN ('pending','running','completed','failed','cancelled','reviewed')),
    completed_at TEXT,
    error_message TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO continuity_review_run_statuses (run_id, status, completed_at, error_message)
SELECT id, status, completed_at, error_message FROM continuity_review_runs;
CREATE INDEX idx_continuity_review_run_statuses_status
    ON continuity_review_run_statuses(status, updated_at);

CREATE TABLE manuscript_analysis_jobs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    import_reference TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','running','paused','completed','failed','cancelled')),
    total_units INTEGER NOT NULL CHECK(total_units >= 0),
    completed_units INTEGER NOT NULL DEFAULT 0 CHECK(completed_units >= 0),
    failed_units INTEGER NOT NULL DEFAULT 0 CHECK(failed_units >= 0),
    current_unit_id TEXT,
    provider_id TEXT NOT NULL,
    page_markers_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    error_message TEXT
);
CREATE UNIQUE INDEX idx_manuscript_analysis_jobs_import ON manuscript_analysis_jobs(project_id, import_reference);
CREATE INDEX idx_manuscript_analysis_jobs_status ON manuscript_analysis_jobs(project_id, status, updated_at);

CREATE TABLE manuscript_analysis_units (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
    order_index INTEGER NOT NULL,
    page_number INTEGER,
    start_offset INTEGER NOT NULL CHECK(start_offset >= 0),
    end_offset INTEGER NOT NULL CHECK(end_offset >= start_offset),
    content TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','running','completed','failed','skipped')),
    retry_count INTEGER NOT NULL DEFAULT 0 CHECK(retry_count >= 0),
    continuity_run_id TEXT REFERENCES continuity_review_runs(id) ON DELETE SET NULL,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    UNIQUE(job_id, order_index)
);
CREATE INDEX idx_manuscript_analysis_units_job_status ON manuscript_analysis_units(job_id, status, order_index);
CREATE INDEX idx_manuscript_analysis_units_scene_position ON manuscript_analysis_units(scene_id, start_offset, end_offset);

CREATE TABLE manuscript_analysis_draft_ledger (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE,
    unit_id TEXT NOT NULL REFERENCES manuscript_analysis_units(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    entity_id TEXT NOT NULL REFERENCES story_entities(id) ON DELETE CASCADE,
    related_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL,
    state_kind TEXT NOT NULL,
    previous_state TEXT NOT NULL DEFAULT '',
    new_state TEXT NOT NULL,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
    start_offset INTEGER,
    end_offset INTEGER,
    confidence REAL NOT NULL CHECK(confidence BETWEEN 0 AND 1),
    status TEXT NOT NULL DEFAULT 'proposed' CHECK(status IN ('proposed','confirmed','rejected')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_manuscript_analysis_draft_ledger_job ON manuscript_analysis_draft_ledger(job_id, unit_id, status);
