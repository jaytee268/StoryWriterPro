CREATE TABLE IF NOT EXISTS project_workflow_state (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','archived')),
    last_opened_at TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS project_onboarding_state (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    current_step TEXT NOT NULL DEFAULT 'project' CHECK(current_step IN ('project','lore','manuscript','summary','completed')),
    completed_steps_json TEXT NOT NULL DEFAULT '[]',
    skipped_steps_json TEXT NOT NULL DEFAULT '[]',
    language TEXT NOT NULL DEFAULT 'de',
    genre TEXT NOT NULL DEFAULT '',
    lore_crafter_run_id TEXT,
    import_id TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_project_workflow_status ON project_workflow_state(status, last_opened_at, updated_at);

INSERT OR IGNORE INTO project_workflow_state(project_id, status, last_opened_at, updated_at)
SELECT id, 'active', updated_at, updated_at FROM projects;

INSERT OR IGNORE INTO project_onboarding_state(project_id, current_step, completed_steps_json, skipped_steps_json, language, genre, updated_at)
SELECT id, 'completed', '["project","lore","manuscript","summary"]', '[]', 'de', '', updated_at FROM projects;
