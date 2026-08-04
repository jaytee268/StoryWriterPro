-- Incremental, review-first continuity checks and plot-thread lifecycle.
CREATE TABLE IF NOT EXISTS continuity_review_settings (
    project_id TEXT PRIMARY KEY,
    word_threshold INTEGER NOT NULL DEFAULT 300 CHECK(word_threshold BETWEEN 50 AND 5000),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS continuity_review_runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    chapter_id TEXT,
    scene_id TEXT,
    source_kind TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    start_offset INTEGER,
    end_offset INTEGER,
    provider_id TEXT NOT NULL DEFAULT 'local-continuity-review',
    status TEXT NOT NULL DEFAULT 'completed' CHECK(status IN ('pending','running','completed','failed','reviewed')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    error_message TEXT,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE SET NULL,
    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_continuity_review_runs_scope
    ON continuity_review_runs(project_id, chapter_id, scene_id, content_hash);

CREATE TABLE IF NOT EXISTS continuity_review_findings (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    chapter_id TEXT,
    scene_id TEXT,
    finding_type TEXT NOT NULL CHECK(finding_type IN ('critical_contradiction','probable_contradiction','missing_explanation','character_deviation','lore_compatible_anomaly','possible_intentional_exception','insufficient_evidence')),
    severity TEXT NOT NULL CHECK(severity IN ('info','warning','critical')),
    subject_entity_id TEXT,
    related_entity_ids_json TEXT NOT NULL DEFAULT '[]',
    related_state_ids_json TEXT NOT NULL DEFAULT '[]',
    related_rule_ids_json TEXT NOT NULL DEFAULT '[]',
    objective_conflict TEXT NOT NULL,
    lore_explanations_json TEXT NOT NULL DEFAULT '[]',
    evidence_excerpt TEXT NOT NULL DEFAULT '',
    start_offset INTEGER,
    end_offset INTEGER,
    reason TEXT NOT NULL,
    review_status TEXT NOT NULL DEFAULT 'open' CHECK(review_status IN ('open','accepted','dismissed','resolved','deferred')),
    user_decision TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (run_id) REFERENCES continuity_review_runs(id) ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE SET NULL,
    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL,
    FOREIGN KEY (subject_entity_id) REFERENCES story_entities(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_continuity_findings_run_status
    ON continuity_review_findings(run_id, review_status, severity);

CREATE TABLE IF NOT EXISTS plot_thread_lifecycle (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    entity_id TEXT NOT NULL UNIQUE,
    lifecycle_status TEXT NOT NULL DEFAULT 'open' CHECK(lifecycle_status IN ('open','closure_candidate','partially_resolved','resolved','reopened','abandoned')),
    last_source_reference_id TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (entity_id) REFERENCES story_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (last_source_reference_id) REFERENCES story_source_references(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS plot_thread_lifecycle_proposals (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    proposed_status TEXT NOT NULL CHECK(proposed_status IN ('open','closure_candidate','partially_resolved','resolved','reopened','abandoned')),
    evidence_excerpt TEXT NOT NULL DEFAULT '',
    start_offset INTEGER,
    end_offset INTEGER,
    reason TEXT NOT NULL,
    review_status TEXT NOT NULL DEFAULT 'pending' CHECK(review_status IN ('pending','accepted','edited','rejected')),
    reviewed_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (run_id) REFERENCES continuity_review_runs(id) ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (entity_id) REFERENCES story_entities(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_plot_thread_lifecycle_project
    ON plot_thread_lifecycle(project_id, lifecycle_status);

CREATE INDEX IF NOT EXISTS idx_plot_thread_lifecycle_proposals_run
    ON plot_thread_lifecycle_proposals(run_id, review_status);
