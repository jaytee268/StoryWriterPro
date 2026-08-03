-- Character memory graph. The migration runner applies this file only after
-- checking the version table. All tables are additive and preserve 001-009.
CREATE TABLE IF NOT EXISTS character_voice_patterns (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, character_id TEXT NOT NULL,
  related_character_id TEXT, pattern_type TEXT NOT NULL, pattern_text TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '', context_condition TEXT NOT NULL DEFAULT '',
  confidence REAL NOT NULL DEFAULT 1.0, status TEXT NOT NULL DEFAULT 'confirmed',
  author_confirmed INTEGER NOT NULL DEFAULT 1, occurrence_count INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY (character_id) REFERENCES story_entities(id) ON DELETE CASCADE,
  FOREIGN KEY (related_character_id) REFERENCES story_entities(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_character_voice_patterns_character ON character_voice_patterns(project_id, character_id);
CREATE TABLE IF NOT EXISTS character_experiences (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, character_id TEXT NOT NULL,
  event_entity_id TEXT, scene_id TEXT, title TEXT NOT NULL, objective_summary TEXT NOT NULL DEFAULT '',
  subjective_interpretation TEXT NOT NULL DEFAULT '', emotional_impact TEXT NOT NULL DEFAULT '',
  lasting_effect TEXT NOT NULL DEFAULT '', significance TEXT NOT NULL DEFAULT 'supporting',
  memory_reliability TEXT NOT NULL DEFAULT 'reliable', status TEXT NOT NULL DEFAULT 'confirmed',
  author_confirmed INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY (character_id) REFERENCES story_entities(id) ON DELETE CASCADE,
  FOREIGN KEY (event_entity_id) REFERENCES story_entities(id) ON DELETE SET NULL,
  FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_character_experiences_character ON character_experiences(project_id, character_id, scene_id);
CREATE TABLE IF NOT EXISTS character_dialogue_memories (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, speaker_id TEXT NOT NULL, scene_id TEXT NOT NULL,
  dialogue_kind TEXT NOT NULL, topic TEXT NOT NULL DEFAULT '', summary TEXT NOT NULL,
  exact_excerpt TEXT NOT NULL DEFAULT '', emotional_tone TEXT NOT NULL DEFAULT '', hidden_intent TEXT NOT NULL DEFAULT '',
  significance TEXT NOT NULL DEFAULT 'supporting', truthfulness TEXT NOT NULL DEFAULT 'unknown',
  status TEXT NOT NULL DEFAULT 'confirmed', author_confirmed INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY (speaker_id) REFERENCES story_entities(id) ON DELETE CASCADE,
  FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_character_dialogue_memories_scene ON character_dialogue_memories(project_id, scene_id);
CREATE TABLE IF NOT EXISTS dialogue_memory_participants (
  dialogue_memory_id TEXT NOT NULL, character_id TEXT NOT NULL, role TEXT NOT NULL DEFAULT 'listener',
  PRIMARY KEY (dialogue_memory_id, character_id),
  FOREIGN KEY (dialogue_memory_id) REFERENCES character_dialogue_memories(id) ON DELETE CASCADE,
  FOREIGN KEY (character_id) REFERENCES story_entities(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS relationship_memories (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, character_a_id TEXT NOT NULL, character_b_id TEXT NOT NULL,
  scene_id TEXT, memory_type TEXT NOT NULL, title TEXT NOT NULL, summary TEXT NOT NULL,
  private_meaning TEXT NOT NULL DEFAULT '', relationship_effect TEXT NOT NULL DEFAULT '',
  significance TEXT NOT NULL DEFAULT 'supporting', status TEXT NOT NULL DEFAULT 'confirmed',
  author_confirmed INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY (character_a_id) REFERENCES story_entities(id) ON DELETE CASCADE,
  FOREIGN KEY (character_b_id) REFERENCES story_entities(id) ON DELETE CASCADE,
  FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_relationship_memories_pair ON relationship_memories(project_id, character_a_id, character_b_id);
CREATE TABLE IF NOT EXISTS character_knowledge_states (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, character_id TEXT NOT NULL, fact_entity_id TEXT NOT NULL,
  knowledge_state TEXT NOT NULL, acquired_scene_id TEXT, changed_scene_id TEXT, source_character_id TEXT,
  certainty REAL NOT NULL DEFAULT 1.0, notes TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'confirmed',
  author_confirmed INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY (character_id) REFERENCES story_entities(id) ON DELETE CASCADE,
  FOREIGN KEY (fact_entity_id) REFERENCES story_entities(id) ON DELETE CASCADE,
  FOREIGN KEY (acquired_scene_id) REFERENCES scenes(id) ON DELETE SET NULL,
  FOREIGN KEY (changed_scene_id) REFERENCES scenes(id) ON DELETE SET NULL,
  FOREIGN KEY (source_character_id) REFERENCES story_entities(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_character_knowledge_character ON character_knowledge_states(project_id, character_id, updated_at);
CREATE TABLE IF NOT EXISTS character_knowledge_history (
  id TEXT PRIMARY KEY, knowledge_state_id TEXT NOT NULL, project_id TEXT NOT NULL, character_id TEXT NOT NULL,
  fact_entity_id TEXT NOT NULL, knowledge_state TEXT NOT NULL, certainty REAL NOT NULL, scene_id TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (knowledge_state_id) REFERENCES character_knowledge_states(id) ON DELETE CASCADE,
  FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY (character_id) REFERENCES story_entities(id) ON DELETE CASCADE,
  FOREIGN KEY (fact_entity_id) REFERENCES story_entities(id) ON DELETE CASCADE,
  FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL
);
CREATE TABLE IF NOT EXISTS character_memory_evidence (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, memory_kind TEXT NOT NULL, memory_id TEXT NOT NULL,
  source_reference_id TEXT NOT NULL, evidence_role TEXT NOT NULL DEFAULT 'supporting',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY (source_reference_id) REFERENCES story_source_references(id) ON DELETE CASCADE,
  UNIQUE(memory_kind, memory_id, source_reference_id)
);
CREATE INDEX IF NOT EXISTS idx_character_memory_evidence_memory ON character_memory_evidence(memory_kind, memory_id);
CREATE TABLE IF NOT EXISTS character_memory_update_runs (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, scene_id TEXT NOT NULL, content_hash TEXT NOT NULL,
  extractor_id TEXT NOT NULL, status TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  completed_at TEXT, error_message TEXT,
  FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_character_memory_runs_scene_hash ON character_memory_update_runs(scene_id, content_hash, extractor_id, status);
CREATE TABLE IF NOT EXISTS character_memory_proposals (
  id TEXT PRIMARY KEY, run_id TEXT NOT NULL, project_id TEXT NOT NULL, scene_id TEXT NOT NULL,
  proposal_kind TEXT NOT NULL, subject_character_id TEXT, related_character_id TEXT, target_entity_id TEXT,
  payload_json TEXT NOT NULL, classification TEXT NOT NULL, confidence REAL NOT NULL,
  evidence_excerpt TEXT NOT NULL, start_offset INTEGER, end_offset INTEGER, reason TEXT NOT NULL,
  review_status TEXT NOT NULL DEFAULT 'pending', reviewed_at TEXT, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (run_id) REFERENCES character_memory_update_runs(id) ON DELETE CASCADE,
  FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
  FOREIGN KEY (subject_character_id) REFERENCES story_entities(id) ON DELETE SET NULL,
  FOREIGN KEY (related_character_id) REFERENCES story_entities(id) ON DELETE SET NULL,
  FOREIGN KEY (target_entity_id) REFERENCES story_entities(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_character_memory_proposals_run ON character_memory_proposals(run_id, review_status);
