-- Migration 003 introduced version_number with a zero default. Rebuild the
-- per-scene sequence from the stable creation order so newly created versions
-- continue at the correct number after an upgrade.
UPDATE scene_versions AS current_version
SET version_number = (
  SELECT COUNT(*)
  FROM scene_versions AS earlier_version
  WHERE earlier_version.scene_id = current_version.scene_id
    AND (
      earlier_version.created_at < current_version.created_at
      OR (earlier_version.created_at = current_version.created_at AND earlier_version.id <= current_version.id)
    )
);
