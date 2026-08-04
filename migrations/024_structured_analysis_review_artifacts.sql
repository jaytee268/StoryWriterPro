CREATE TABLE IF NOT EXISTS manuscript_analysis_phase_results (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    phase TEXT NOT NULL,
    result_kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    review_status TEXT NOT NULL DEFAULT 'pending' CHECK(review_status IN ('pending','confirmed','rejected','uncertain','skipped')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_manuscript_phase_results_job ON manuscript_analysis_phase_results(job_id, phase, updated_at);

CREATE TABLE IF NOT EXISTS manuscript_analysis_artifacts (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    phase TEXT NOT NULL,
    unit_id TEXT REFERENCES manuscript_analysis_units(id) ON DELETE SET NULL,
    artifact_type TEXT NOT NULL,
    artifact_id TEXT NOT NULL,
    review_status TEXT NOT NULL DEFAULT 'pending' CHECK(review_status IN ('pending','confirmed','rejected','uncertain','skipped')),
    explicitly_skipped INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(job_id, artifact_type, artifact_id)
);
CREATE INDEX IF NOT EXISTS idx_manuscript_analysis_artifacts_review ON manuscript_analysis_artifacts(job_id, review_status, explicitly_skipped);

CREATE TABLE IF NOT EXISTS manuscript_analysis_review_audits (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    action TEXT NOT NULL CHECK(action IN ('skip_open_artifacts','complete_review')),
    artifact_ids_json TEXT NOT NULL DEFAULT '[]',
    artifact_types_json TEXT NOT NULL DEFAULT '[]',
    note TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS project_source_documents (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL CHECK(source_kind IN ('lore_crafter','research','author_note','external_text')),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    origin_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_project_source_documents_project ON project_source_documents(project_id, updated_at);

CREATE TABLE IF NOT EXISTS project_source_references (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_document_id TEXT REFERENCES project_source_documents(id) ON DELETE CASCADE,
    entity_id TEXT,
    proposal_id TEXT,
    chapter_id TEXT,
    scene_id TEXT,
    excerpt TEXT NOT NULL,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK((source_document_id IS NOT NULL AND chapter_id IS NULL AND scene_id IS NULL) OR (source_document_id IS NULL AND chapter_id IS NOT NULL AND scene_id IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS idx_project_source_references_project ON project_source_references(project_id, source_document_id, created_at);

ALTER TABLE lore_crafter_sources ADD COLUMN source_document_id TEXT REFERENCES project_source_documents(id) ON DELETE SET NULL;
ALTER TABLE lore_crafter_sources ADD COLUMN source_reference_id TEXT REFERENCES project_source_references(id) ON DELETE SET NULL;
ALTER TABLE lore_sheet_items ADD COLUMN structured_json TEXT;
