-- The Rust migration runner adds these columns after checking PRAGMA
-- table_info(story_entities), so a partially applied upgrade is resumable.

CREATE TABLE IF NOT EXISTS bible_update_runs (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
  scene_updated_at TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  extractor_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('pending', 'running', 'completed', 'failed', 'reviewed')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  completed_at TEXT,
  error_message TEXT
);

CREATE INDEX IF NOT EXISTS idx_bible_update_runs_scene_hash
  ON bible_update_runs(scene_id, content_hash, status);

CREATE TABLE IF NOT EXISTS bible_proposals (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES bible_update_runs(id) ON DELETE CASCADE,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
  target_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL,
  proposal_action TEXT NOT NULL CHECK(proposal_action IN ('create_entity', 'update_entity', 'add_source', 'mark_contradiction', 'create_open_question', 'create_author_note')),
  entity_type TEXT NOT NULL,
  candidate_name TEXT NOT NULL,
  candidate_description TEXT NOT NULL,
  candidate_status TEXT NOT NULL,
  confidence REAL NOT NULL,
  classification TEXT NOT NULL CHECK(classification IN ('observable_fact', 'interpretation', 'open_question', 'possible_contradiction', 'author_note')),
  evidence_excerpt TEXT NOT NULL,
  start_offset INTEGER,
  end_offset INTEGER,
  reason TEXT NOT NULL,
  review_status TEXT NOT NULL CHECK(review_status IN ('pending', 'accepted', 'edited', 'rejected')),
  reviewed_at TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_bible_proposals_run_status
  ON bible_proposals(run_id, review_status);

CREATE TABLE IF NOT EXISTS story_source_references (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL,
  proposal_id TEXT REFERENCES bible_proposals(id) ON DELETE SET NULL,
  chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
  scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
  excerpt TEXT NOT NULL DEFAULT '',
  start_offset INTEGER,
  end_offset INTEGER,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_story_sources_entity ON story_source_references(entity_id);
CREATE INDEX IF NOT EXISTS idx_story_sources_scene ON story_source_references(scene_id);
