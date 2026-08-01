-- Existing databases may contain Story-Bible rows created before project_id
-- existed. Only the unambiguous single-project case is backfilled. Rows remain
-- NULL when several projects exist so they are not silently assigned to the
-- wrong story.
UPDATE story_entities
SET project_id = (SELECT id FROM projects LIMIT 1)
WHERE (project_id IS NULL OR trim(project_id) = '')
  AND (SELECT COUNT(*) FROM projects) = 1;

ALTER TABLE scene_versions ADD COLUMN reason TEXT NOT NULL DEFAULT 'manual';

CREATE INDEX IF NOT EXISTS idx_scene_versions_scene_reason
  ON scene_versions(scene_id, reason, created_at DESC);
