-- Migration 033: keep manuscript-import Character Memory runs attributable to
-- the resumable analysis job that produced them.
ALTER TABLE character_memory_update_runs
    ADD COLUMN manuscript_job_id TEXT REFERENCES manuscript_analysis_jobs(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_character_memory_runs_manuscript_job
    ON character_memory_update_runs(manuscript_job_id, scene_id, content_hash);
