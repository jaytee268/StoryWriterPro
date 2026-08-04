CREATE TABLE IF NOT EXISTS chapter_generation_draft_ledger (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES chapter_generation_jobs(id) ON DELETE CASCADE,
    section_id TEXT NOT NULL REFERENCES chapter_generation_sections(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    entity_id TEXT NOT NULL REFERENCES story_entities(id) ON DELETE CASCADE,
    related_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL,
    state_kind TEXT NOT NULL,
    previous_state TEXT NOT NULL DEFAULT '',
    new_state TEXT NOT NULL,
    source_excerpt TEXT NOT NULL DEFAULT '',
    source_start_offset INTEGER,
    source_end_offset INTEGER,
    content_hash TEXT NOT NULL,
    confidence REAL NOT NULL CHECK(confidence BETWEEN 0 AND 1),
    status TEXT NOT NULL DEFAULT 'proposed' CHECK(status IN ('proposed','superseded','rejected','accepted_for_manuscript_review')),
    source_reference_id TEXT REFERENCES story_source_references(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_generation_draft_ledger_job_section ON chapter_generation_draft_ledger(job_id, section_id, status);
CREATE INDEX IF NOT EXISTS idx_generation_draft_ledger_project_entity ON chapter_generation_draft_ledger(project_id, entity_id, status);
