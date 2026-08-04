CREATE TABLE IF NOT EXISTS lore_crafter_runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    original_text TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending','running','awaiting_review','completed','failed','cancelled')),
    understanding_summary TEXT,
    analysis_json TEXT,
    confirmation_text TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    error_code TEXT,
    error_message TEXT
);
CREATE INDEX IF NOT EXISTS idx_lore_crafter_runs_project_hash ON lore_crafter_runs(project_id, content_hash, status);

CREATE TABLE IF NOT EXISTS lore_crafter_clarifications (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES lore_crafter_runs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    question TEXT NOT NULL,
    answer TEXT,
    status TEXT NOT NULL CHECK(status IN ('open','answered','skipped')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_lore_crafter_clarifications_run ON lore_crafter_clarifications(run_id, status);

CREATE TABLE IF NOT EXISTS lore_crafter_sources (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES lore_crafter_runs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    excerpt TEXT NOT NULL,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS lore_sheet_drafts (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES lore_crafter_runs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    sheet_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('proposed','reviewed','rejected')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_lore_sheet_drafts_project ON lore_sheet_drafts(project_id, updated_at);

CREATE TABLE IF NOT EXISTS lore_sheet_items (
    id TEXT PRIMARY KEY,
    draft_id TEXT NOT NULL REFERENCES lore_sheet_drafts(id) ON DELETE CASCADE,
    run_id TEXT NOT NULL REFERENCES lore_crafter_runs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    item_type TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    confidence REAL NOT NULL,
    source_reference_id TEXT REFERENCES lore_crafter_sources(id) ON DELETE SET NULL,
    target_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL,
    target_rule_id TEXT REFERENCES project_rules(id) ON DELETE SET NULL,
    status TEXT NOT NULL CHECK(status IN ('proposed','accepted','rejected','uncertain','merged')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_lore_sheet_items_review ON lore_sheet_items(draft_id, status);
