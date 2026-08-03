-- Dynamic continuity foundations. All values remain project-local and reviewable.
CREATE TABLE IF NOT EXISTS project_rules (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    title TEXT NOT NULL,
    statement TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT 'project',
    prerequisites_json TEXT NOT NULL DEFAULT '[]',
    effects_json TEXT NOT NULL DEFAULT '[]',
    exceptions_json TEXT NOT NULL DEFAULT '[]',
    connected_lore_ids_json TEXT NOT NULL DEFAULT '[]',
    source_reference_ids_json TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'proposed',
    confidence REAL NOT NULL DEFAULT 0,
    author_confirmed INTEGER NOT NULL DEFAULT 0,
    origin TEXT NOT NULL DEFAULT 'manual',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_project_rules_project_status
    ON project_rules(project_id, status, author_confirmed);

CREATE TABLE IF NOT EXISTS project_rule_proposals (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    target_rule_id TEXT,
    title TEXT NOT NULL,
    statement TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT 'project',
    prerequisites_json TEXT NOT NULL DEFAULT '[]',
    effects_json TEXT NOT NULL DEFAULT '[]',
    exceptions_json TEXT NOT NULL DEFAULT '[]',
    connected_lore_ids_json TEXT NOT NULL DEFAULT '[]',
    source_reference_ids_json TEXT NOT NULL DEFAULT '[]',
    evidence_excerpt TEXT NOT NULL DEFAULT '',
    chapter_id TEXT,
    scene_id TEXT,
    start_offset INTEGER,
    end_offset INTEGER,
    confidence REAL NOT NULL DEFAULT 0,
    reason TEXT NOT NULL DEFAULT '',
    review_status TEXT NOT NULL DEFAULT 'pending',
    reviewed_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (target_rule_id) REFERENCES project_rules(id) ON DELETE SET NULL,
    FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE SET NULL,
    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_project_rule_proposals_project_status
    ON project_rule_proposals(project_id, review_status);

CREATE TABLE IF NOT EXISTS continuity_state_ledger (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    related_entity_id TEXT,
    state_kind TEXT NOT NULL,
    previous_state TEXT NOT NULL DEFAULT '',
    new_state TEXT NOT NULL,
    chapter_id TEXT,
    scene_id TEXT,
    start_offset INTEGER,
    end_offset INTEGER,
    source_reference_id TEXT,
    status TEXT NOT NULL DEFAULT 'proposed',
    confidence REAL NOT NULL DEFAULT 0,
    author_confirmed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (entity_id) REFERENCES story_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (related_entity_id) REFERENCES story_entities(id) ON DELETE SET NULL,
    FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE SET NULL,
    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL,
    FOREIGN KEY (source_reference_id) REFERENCES story_source_references(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_continuity_ledger_entity_kind
    ON continuity_state_ledger(project_id, entity_id, state_kind, scene_id);

CREATE INDEX IF NOT EXISTS idx_continuity_ledger_scene
    ON continuity_state_ledger(project_id, chapter_id, scene_id);
