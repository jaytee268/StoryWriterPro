-- Store the exact plain-text snapshot used by the local extractor. This keeps
-- changed-range calculations tied to the actual analyzed manuscript version.
ALTER TABLE bible_update_runs ADD COLUMN analyzed_content TEXT NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS idx_bible_update_runs_scene_extractor_created
  ON bible_update_runs(scene_id, extractor_id, created_at DESC);
