PRAGMA foreign_keys=OFF;

ALTER TABLE manuscript_analysis_jobs RENAME TO manuscript_analysis_jobs_old_029;
CREATE TABLE manuscript_analysis_jobs (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  import_reference TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','running','paused','awaiting_structure_review','awaiting_user_review','completed','failed','cancelled')),
  total_units INTEGER NOT NULL CHECK(total_units >= 0),
  completed_units INTEGER NOT NULL DEFAULT 0 CHECK(completed_units >= 0),
  failed_units INTEGER NOT NULL DEFAULT 0 CHECK(failed_units >= 0),
  current_unit_id TEXT,
  provider_id TEXT NOT NULL,
  page_markers_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  completed_at TEXT,
  error_message TEXT,
  current_phase TEXT NOT NULL DEFAULT 'structure',
  phase_progress_json TEXT NOT NULL DEFAULT '{}',
  phase_errors_json TEXT NOT NULL DEFAULT '{}',
  last_successful_unit_id TEXT
);
INSERT INTO manuscript_analysis_jobs SELECT id,project_id,book_id,import_reference,status,total_units,completed_units,failed_units,current_unit_id,provider_id,page_markers_json,created_at,updated_at,completed_at,error_message,current_phase,phase_progress_json,phase_errors_json,last_successful_unit_id FROM manuscript_analysis_jobs_old_029;
DROP TABLE manuscript_analysis_jobs_old_029;
CREATE UNIQUE INDEX idx_manuscript_analysis_jobs_import ON manuscript_analysis_jobs(project_id, import_reference);
CREATE INDEX idx_manuscript_analysis_jobs_status ON manuscript_analysis_jobs(project_id, status, updated_at);

PRAGMA foreign_keys=ON;
