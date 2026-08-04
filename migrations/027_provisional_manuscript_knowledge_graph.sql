CREATE TABLE IF NOT EXISTS provisional_entities (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    canonical_name TEXT NOT NULL,
    aliases_json TEXT NOT NULL DEFAULT '[]',
    description TEXT NOT NULL DEFAULT '',
    first_source_reference_id TEXT,
    last_source_reference_id TEXT,
    confidence REAL NOT NULL,
    review_status TEXT NOT NULL CHECK(review_status IN ('proposed','accepted','rejected','uncertain','merged')),
    existing_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_provisional_entities_job_name ON provisional_entities(job_id, canonical_name);

CREATE TABLE IF NOT EXISTS provisional_entity_mentions (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    passage_unit_id TEXT NOT NULL REFERENCES manuscript_analysis_units(id) ON DELETE CASCADE,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    excerpt TEXT NOT NULL,
    mention_text TEXT NOT NULL,
    resolved_provisional_entity_id TEXT REFERENCES provisional_entities(id) ON DELETE SET NULL,
    alternative_entity_ids_json TEXT NOT NULL DEFAULT '[]',
    confidence REAL NOT NULL,
    resolution_reason TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_provisional_mentions_unit ON provisional_entity_mentions(passage_unit_id, start_offset);

CREATE TABLE IF NOT EXISTS provisional_aliases (
    id TEXT PRIMARY KEY,
    provisional_entity_id TEXT NOT NULL REFERENCES provisional_entities(id) ON DELETE CASCADE,
    alias TEXT NOT NULL,
    source_reference_id TEXT,
    confidence REAL NOT NULL,
    review_status TEXT NOT NULL CHECK(review_status IN ('proposed','accepted','rejected','uncertain')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(provisional_entity_id, alias)
);

CREATE TABLE IF NOT EXISTS provisional_relations (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_provisional_entity_id TEXT NOT NULL REFERENCES provisional_entities(id) ON DELETE CASCADE,
    target_provisional_entity_id TEXT NOT NULL REFERENCES provisional_entities(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    confidence REAL NOT NULL,
    review_status TEXT NOT NULL CHECK(review_status IN ('proposed','accepted','rejected','uncertain')),
    source_reference_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS provisional_merge_proposals (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    left_provisional_entity_id TEXT NOT NULL REFERENCES provisional_entities(id) ON DELETE CASCADE,
    right_provisional_entity_id TEXT,
    existing_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL,
    reason TEXT NOT NULL,
    confidence REAL NOT NULL,
    review_status TEXT NOT NULL CHECK(review_status IN ('proposed','accepted','rejected','uncertain')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS provisional_events (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    passage_unit_id TEXT NOT NULL REFERENCES manuscript_analysis_units(id) ON DELETE CASCADE,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    participant_entity_ids_json TEXT NOT NULL DEFAULT '[]',
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    confidence REAL NOT NULL,
    review_status TEXT NOT NULL CHECK(review_status IN ('proposed','accepted','rejected','uncertain')),
    source_reference_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
