ALTER TABLE manuscript_analysis_jobs ADD COLUMN current_phase TEXT NOT NULL DEFAULT 'structure';
ALTER TABLE manuscript_analysis_jobs ADD COLUMN phase_progress_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE manuscript_analysis_jobs ADD COLUMN phase_errors_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE manuscript_analysis_jobs ADD COLUMN last_successful_unit_id TEXT;

ALTER TABLE manuscript_analysis_units ADD COLUMN requested_provider TEXT;
ALTER TABLE manuscript_analysis_units ADD COLUMN actual_provider TEXT;
ALTER TABLE manuscript_analysis_units ADD COLUMN prompt_version TEXT;
ALTER TABLE manuscript_analysis_units ADD COLUMN input_hash TEXT;
ALTER TABLE manuscript_analysis_units ADD COLUMN output_hash TEXT;
ALTER TABLE manuscript_analysis_units ADD COLUMN error_code TEXT;

CREATE INDEX IF NOT EXISTS idx_manuscript_analysis_units_provider_hash
    ON manuscript_analysis_units(job_id, content_hash, actual_provider, status);

ALTER TABLE scenes ADD COLUMN is_implicit INTEGER NOT NULL DEFAULT 0;
