CREATE TABLE IF NOT EXISTS lore_metadata (
    entity_id TEXT PRIMARY KEY REFERENCES story_entities(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    truth_scope TEXT NOT NULL DEFAULT 'world_truth',
    truth_statement TEXT NOT NULL DEFAULT '',
    rules_text TEXT NOT NULL DEFAULT '',
    exceptions_text TEXT NOT NULL DEFAULT '',
    author_knowledge TEXT NOT NULL DEFAULT '',
    reader_knowledge TEXT NOT NULL DEFAULT '',
    reveal_plan TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_lore_metadata_project ON lore_metadata(project_id);

CREATE TABLE IF NOT EXISTS character_profiles (
    entity_id TEXT PRIMARY KEY REFERENCES story_entities(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    core_want TEXT NOT NULL DEFAULT '',
    core_need TEXT NOT NULL DEFAULT '',
    fears TEXT NOT NULL DEFAULT '',
    false_belief TEXT NOT NULL DEFAULT '',
    values_text TEXT NOT NULL DEFAULT '',
    strengths TEXT NOT NULL DEFAULT '',
    flaws TEXT NOT NULL DEFAULT '',
    pressure_behavior TEXT NOT NULL DEFAULT '',
    voice TEXT NOT NULL DEFAULT '',
    backstory TEXT NOT NULL DEFAULT '',
    arc_summary TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_character_profiles_project ON character_profiles(project_id);

CREATE TABLE IF NOT EXISTS character_scene_states (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    character_entity_id TEXT NOT NULL REFERENCES character_profiles(entity_id) ON DELETE CASCADE,
    scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
    emotional_state TEXT NOT NULL DEFAULT '',
    physical_state TEXT NOT NULL DEFAULT '',
    goal TEXT NOT NULL DEFAULT '',
    conflict TEXT NOT NULL DEFAULT '',
    knowledge TEXT NOT NULL DEFAULT '',
    relationship_state TEXT NOT NULL DEFAULT '',
    change_note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(character_entity_id, scene_id)
);
CREATE INDEX IF NOT EXISTS idx_character_scene_states_scene ON character_scene_states(scene_id);

CREATE TABLE IF NOT EXISTS project_styles (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    narrative_pov TEXT NOT NULL DEFAULT '',
    tense TEXT NOT NULL DEFAULT '',
    sentence_style TEXT NOT NULL DEFAULT '',
    dialogue_style TEXT NOT NULL DEFAULT '',
    description_density TEXT NOT NULL DEFAULT '',
    inner_monologue TEXT NOT NULL DEFAULT '',
    preferred_patterns_json TEXT NOT NULL DEFAULT '[]',
    avoided_patterns_json TEXT NOT NULL DEFAULT '[]',
    notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS style_references (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
    label TEXT NOT NULL DEFAULT '',
    excerpt TEXT NOT NULL,
    notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_style_references_project ON style_references(project_id, created_at DESC);
