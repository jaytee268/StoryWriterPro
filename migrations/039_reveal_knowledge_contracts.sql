CREATE TABLE IF NOT EXISTS reveal_contracts (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    subject_entity_id TEXT NOT NULL REFERENCES story_entities(id) ON DELETE RESTRICT,
    title TEXT NOT NULL,
    truth_statement TEXT NOT NULL,
    scope TEXT NOT NULL CHECK(scope IN ('series','book','arc')),
    status TEXT NOT NULL DEFAULT 'proposed' CHECK(status IN ('proposed','confirmed','rejected','retired')),
    author_confirmed INTEGER NOT NULL DEFAULT 0 CHECK(author_confirmed IN (0,1)),
    reveal_state TEXT NOT NULL DEFAULT 'author_only' CHECK(reveal_state IN ('author_only','foreshadowed','reader_revealed')),
    planned_reveal_book_id TEXT REFERENCES books(id) ON DELETE SET NULL,
    planned_reveal_chapter_id TEXT REFERENCES chapters(id) ON DELETE SET NULL,
    planned_reveal_scene_id TEXT REFERENCES scenes(id) ON DELETE SET NULL,
    planned_reveal_offset INTEGER CHECK(planned_reveal_offset IS NULL OR planned_reveal_offset >= 0),
    reveal_condition_text TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    source_reference_id TEXT REFERENCES project_source_references(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK(author_confirmed = 0 OR status = 'confirmed')
);
CREATE INDEX IF NOT EXISTS idx_reveal_contracts_project ON reveal_contracts(project_id, status, updated_at);
CREATE INDEX IF NOT EXISTS idx_reveal_contracts_subject ON reveal_contracts(project_id, subject_entity_id);

CREATE TABLE IF NOT EXISTS reveal_audience_states (
    id TEXT PRIMARY KEY,
    contract_id TEXT NOT NULL REFERENCES reveal_contracts(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    audience_kind TEXT NOT NULL CHECK(audience_kind IN ('reader','character')),
    character_entity_id TEXT REFERENCES story_entities(id) ON DELETE RESTRICT,
    knowledge_level TEXT NOT NULL CHECK(knowledge_level IN ('unknown','suspects','partial','knows','false_belief')),
    belief_text TEXT NOT NULL DEFAULT '',
    valid_from_position_json TEXT NOT NULL,
    valid_until_position_json TEXT,
    source_reference_id TEXT REFERENCES project_source_references(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'proposed' CHECK(status IN ('proposed','confirmed','rejected')),
    author_confirmed INTEGER NOT NULL DEFAULT 0 CHECK(author_confirmed IN (0,1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK((audience_kind = 'reader' AND character_entity_id IS NULL) OR (audience_kind = 'character' AND character_entity_id IS NOT NULL)),
    CHECK(author_confirmed = 0 OR status = 'confirmed')
);
CREATE INDEX IF NOT EXISTS idx_reveal_audience_contract ON reveal_audience_states(contract_id, status, updated_at);
CREATE INDEX IF NOT EXISTS idx_reveal_audience_project_character ON reveal_audience_states(project_id, character_entity_id, status);

CREATE TABLE IF NOT EXISTS reveal_clue_rules (
    id TEXT PRIMARY KEY,
    contract_id TEXT NOT NULL REFERENCES reveal_contracts(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    rule_kind TEXT NOT NULL CHECK(rule_kind IN ('allowed','forbidden','required')),
    clue_type TEXT NOT NULL,
    description TEXT NOT NULL,
    maximum_explicitness TEXT NOT NULL CHECK(maximum_explicitness IN ('subtle','suggestive','strong','direct')),
    valid_from_position_json TEXT,
    valid_until_position_json TEXT,
    source_reference_id TEXT REFERENCES project_source_references(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'proposed' CHECK(status IN ('proposed','confirmed','rejected')),
    author_confirmed INTEGER NOT NULL DEFAULT 0 CHECK(author_confirmed IN (0,1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK(author_confirmed = 0 OR status = 'confirmed')
);
CREATE INDEX IF NOT EXISTS idx_reveal_clue_rules_contract ON reveal_clue_rules(contract_id, rule_kind, status);
CREATE INDEX IF NOT EXISTS idx_reveal_clue_rules_project ON reveal_clue_rules(project_id, status);
