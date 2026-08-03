-- Migration 009 is additive. SQLite does not support a portable
-- ALTER TABLE ... ADD COLUMN IF NOT EXISTS, so the Rust migration runner
-- checks columns before applying these additions. This file contains the
-- new relation schema and documents the additive columns it applies.
--
-- lore_metadata: category, scope, reveal_state, importance
-- style_references: chapter_id, start_offset, end_offset, category, weight

CREATE TABLE IF NOT EXISTS story_entity_relations (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_entity_id TEXT NOT NULL REFERENCES story_entities(id) ON DELETE CASCADE,
    target_entity_id TEXT NOT NULL REFERENCES story_entities(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    author_confirmed INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(source_entity_id, target_entity_id, relation_type)
);

CREATE INDEX IF NOT EXISTS idx_story_entity_relations_project ON story_entity_relations(project_id);
CREATE INDEX IF NOT EXISTS idx_story_entity_relations_source ON story_entity_relations(source_entity_id);
CREATE INDEX IF NOT EXISTS idx_story_entity_relations_target ON story_entity_relations(target_entity_id);
