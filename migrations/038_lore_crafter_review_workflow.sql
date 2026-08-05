ALTER TABLE lore_crafter_runs ADD COLUMN correction_text TEXT;

CREATE TABLE IF NOT EXISTS excluded_content_decisions (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES lore_crafter_runs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    reason TEXT NOT NULL,
    suggested_target TEXT NOT NULL CHECK(suggested_target IN ('character_memory','plot_thread','continuity_state','manuscript','style')),
    selected_target TEXT CHECK(selected_target IS NULL OR selected_target IN ('character_memory','plot_thread','continuity_state','manuscript','style')),
    decision TEXT NOT NULL DEFAULT 'pending' CHECK(decision IN ('pending','routed','ignored')),
    created_artifact_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(run_id, content)
);
CREATE INDEX IF NOT EXISTS idx_excluded_content_decisions_run ON excluded_content_decisions(run_id, decision);

CREATE TABLE IF NOT EXISTS project_content_proposals (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    target_kind TEXT NOT NULL CHECK(target_kind IN ('character_memory','plot_thread','continuity_state','manuscript','style')),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    reason TEXT NOT NULL,
    source_reference_id TEXT REFERENCES project_source_references(id) ON DELETE SET NULL,
    origin TEXT NOT NULL CHECK(origin = 'lore_crafter'),
    status TEXT NOT NULL DEFAULT 'proposed' CHECK(status IN ('proposed','accepted','rejected','uncertain')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_id, target_kind, content, source_reference_id)
);
CREATE INDEX IF NOT EXISTS idx_project_content_proposals_review ON project_content_proposals(project_id, target_kind, status);

CREATE TABLE lore_sheet_items_new (
    id TEXT PRIMARY KEY,
    draft_id TEXT NOT NULL REFERENCES lore_sheet_drafts(id) ON DELETE CASCADE,
    run_id TEXT NOT NULL REFERENCES lore_crafter_runs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    item_type TEXT NOT NULL CHECK(item_type IN ('premise','world_rule','prerequisite','effect','limitation','cost','exception','term','organization','location','historical_event','known_aspect','unknown_aspect','rule_connection','open_question','warning')),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    confidence REAL NOT NULL,
    source_reference_id TEXT,
    target_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL,
    target_rule_id TEXT REFERENCES project_rules(id) ON DELETE SET NULL,
    structured_json TEXT,
    status TEXT NOT NULL CHECK(status IN ('proposed','accepted','rejected','uncertain','merged')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO lore_sheet_items_new (id,draft_id,run_id,project_id,item_type,title,content,confidence,source_reference_id,target_entity_id,target_rule_id,structured_json,status,created_at,updated_at)
SELECT id,draft_id,run_id,project_id,CASE item_type WHEN 'terminology' THEN 'term' ELSE item_type END,title,content,confidence,source_reference_id,target_entity_id,target_rule_id,structured_json,status,created_at,updated_at
FROM lore_sheet_items
WHERE item_type <> 'terminology' OR 1=1;
DROP TABLE lore_sheet_items;
ALTER TABLE lore_sheet_items_new RENAME TO lore_sheet_items;
CREATE INDEX IF NOT EXISTS idx_lore_sheet_items_review ON lore_sheet_items(draft_id, status);
