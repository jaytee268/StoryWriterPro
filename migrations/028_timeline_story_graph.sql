CREATE TABLE IF NOT EXISTS manuscript_timeline_events (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
    passage_unit_id TEXT REFERENCES manuscript_analysis_units(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    story_time_text TEXT NOT NULL DEFAULT '',
    normalized_time TEXT,
    temporal_order INTEGER NOT NULL,
    time_certainty TEXT NOT NULL DEFAULT 'unknown',
    location_entity_id TEXT,
    pov_character_id TEXT,
    participating_entity_ids_json TEXT NOT NULL DEFAULT '[]',
    cause_event_ids_json TEXT NOT NULL DEFAULT '[]',
    consequence_event_ids_json TEXT NOT NULL DEFAULT '[]',
    knowledge_changes_json TEXT NOT NULL DEFAULT '[]',
    state_changes_json TEXT NOT NULL DEFAULT '[]',
    related_plot_thread_ids_json TEXT NOT NULL DEFAULT '[]',
    source_reference_ids_json TEXT NOT NULL DEFAULT '[]',
    confidence REAL NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('proposed','confirmed','uncertain','rejected')),
    author_confirmed INTEGER NOT NULL DEFAULT 0,
    origin TEXT NOT NULL DEFAULT 'manuscript_analysis',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_manuscript_timeline_events_project_order ON manuscript_timeline_events(project_id, temporal_order);

CREATE TABLE IF NOT EXISTS story_graph_edges (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_entity_id TEXT NOT NULL,
    target_entity_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    valid_from_chapter_id TEXT,
    valid_from_scene_id TEXT,
    valid_from_offset INTEGER,
    valid_until_chapter_id TEXT,
    valid_until_scene_id TEXT,
    valid_until_offset INTEGER,
    source_reference_ids_json TEXT NOT NULL DEFAULT '[]',
    confidence REAL NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('proposed','confirmed','uncertain','rejected')),
    author_confirmed INTEGER NOT NULL DEFAULT 0,
    origin TEXT NOT NULL DEFAULT 'manuscript_analysis',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_story_graph_edges_project ON story_graph_edges(project_id, status);

CREATE TABLE IF NOT EXISTS mindmap_layouts (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    node_id TEXT NOT NULL,
    position_x REAL NOT NULL,
    position_y REAL NOT NULL,
    width REAL NOT NULL,
    height REAL NOT NULL,
    group_id TEXT,
    hidden INTEGER NOT NULL DEFAULT 0,
    fixed INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_id, user_id, node_id)
);
CREATE INDEX IF NOT EXISTS idx_mindmap_layouts_project_user ON mindmap_layouts(project_id, user_id);
