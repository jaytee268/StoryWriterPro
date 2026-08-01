ALTER TABLE scene_versions ADD COLUMN version_number INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scene_versions ADD COLUMN snapshot_json TEXT NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS idx_scene_versions_scene_created
  ON scene_versions(scene_id, created_at DESC);
