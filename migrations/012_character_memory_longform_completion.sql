ALTER TABLE character_voice_patterns ADD COLUMN first_observed_scene_id TEXT REFERENCES scenes(id) ON DELETE SET NULL;
ALTER TABLE character_voice_patterns ADD COLUMN last_observed_scene_id TEXT REFERENCES scenes(id) ON DELETE SET NULL;
ALTER TABLE character_voice_patterns ADD COLUMN retired_scene_id TEXT REFERENCES scenes(id) ON DELETE SET NULL;
ALTER TABLE character_memory_proposals ADD COLUMN analyzed_content_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE character_memory_proposals ADD COLUMN accepted_memory_id TEXT;
ALTER TABLE character_memory_proposals ADD COLUMN accepted_memory_kind TEXT;
ALTER TABLE character_memory_update_runs ADD COLUMN analyzed_content TEXT NOT NULL DEFAULT '';
ALTER TABLE chapter_generation_jobs ADD COLUMN context_override_accepted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chapter_generation_jobs ADD COLUMN last_resumed_at TEXT;
