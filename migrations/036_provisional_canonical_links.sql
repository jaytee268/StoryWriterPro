ALTER TABLE provisional_entity_mentions ADD COLUMN canonical_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL;
ALTER TABLE provisional_relations ADD COLUMN canonical_source_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL;
ALTER TABLE provisional_relations ADD COLUMN canonical_target_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL;
ALTER TABLE provisional_events ADD COLUMN canonical_participant_entity_ids_json TEXT NOT NULL DEFAULT '[]';

CREATE INDEX IF NOT EXISTS idx_provisional_mentions_canonical ON provisional_entity_mentions(canonical_entity_id);
CREATE INDEX IF NOT EXISTS idx_provisional_relations_canonical_source ON provisional_relations(canonical_source_entity_id);
CREATE INDEX IF NOT EXISTS idx_provisional_relations_canonical_target ON provisional_relations(canonical_target_entity_id);
