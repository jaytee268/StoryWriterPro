-- Additive integrity migration. Rebuilds the three import tables only so their
-- status checks can represent review and invalidation without changing data.
PRAGMA foreign_keys=OFF;

ALTER TABLE manuscript_analysis_jobs RENAME TO manuscript_analysis_jobs_old_022;
CREATE TABLE manuscript_analysis_jobs (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  import_reference TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','running','paused','awaiting_user_review','completed','failed','cancelled')),
  total_units INTEGER NOT NULL CHECK(total_units >= 0), completed_units INTEGER NOT NULL DEFAULT 0 CHECK(completed_units >= 0), failed_units INTEGER NOT NULL DEFAULT 0 CHECK(failed_units >= 0),
  current_unit_id TEXT, provider_id TEXT NOT NULL, page_markers_json TEXT NOT NULL DEFAULT '[]', created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, completed_at TEXT, error_message TEXT,
  current_phase TEXT NOT NULL DEFAULT 'structure', phase_progress_json TEXT NOT NULL DEFAULT '{}', phase_errors_json TEXT NOT NULL DEFAULT '{}', last_successful_unit_id TEXT
);
INSERT INTO manuscript_analysis_jobs SELECT id,project_id,book_id,import_reference,status,total_units,completed_units,failed_units,current_unit_id,provider_id,page_markers_json,created_at,updated_at,completed_at,error_message,current_phase,phase_progress_json,phase_errors_json,last_successful_unit_id FROM manuscript_analysis_jobs_old_022;
DROP TABLE manuscript_analysis_jobs_old_022;
CREATE UNIQUE INDEX idx_manuscript_analysis_jobs_import ON manuscript_analysis_jobs(project_id, import_reference);
CREATE INDEX idx_manuscript_analysis_jobs_status ON manuscript_analysis_jobs(project_id, status, updated_at);

ALTER TABLE manuscript_analysis_units RENAME TO manuscript_analysis_units_old_022;
CREATE TABLE manuscript_analysis_units (
  id TEXT PRIMARY KEY, job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE, chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE, scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE, order_index INTEGER NOT NULL, page_number INTEGER, start_offset INTEGER NOT NULL CHECK(start_offset >= 0), end_offset INTEGER NOT NULL CHECK(end_offset >= start_offset), content TEXT NOT NULL, content_hash TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','running','completed','failed','stale','skipped')), retry_count INTEGER NOT NULL DEFAULT 0 CHECK(retry_count >= 0), continuity_run_id TEXT REFERENCES continuity_review_runs(id) ON DELETE SET NULL, error_message TEXT, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, completed_at TEXT, requested_provider TEXT, actual_provider TEXT, prompt_version TEXT, input_hash TEXT, output_hash TEXT, error_code TEXT, UNIQUE(job_id, order_index)
);
INSERT INTO manuscript_analysis_units SELECT id,job_id,project_id,chapter_id,scene_id,order_index,page_number,start_offset,end_offset,content,content_hash,status,retry_count,continuity_run_id,error_message,created_at,updated_at,completed_at,requested_provider,actual_provider,prompt_version,input_hash,output_hash,error_code FROM manuscript_analysis_units_old_022;
DROP TABLE manuscript_analysis_units_old_022;
CREATE INDEX idx_manuscript_analysis_units_job_status ON manuscript_analysis_units(job_id, status, order_index);
CREATE INDEX idx_manuscript_analysis_units_scene_position ON manuscript_analysis_units(scene_id, start_offset, end_offset);
CREATE INDEX idx_manuscript_analysis_units_provider_hash ON manuscript_analysis_units(job_id, content_hash, actual_provider, status);

ALTER TABLE manuscript_analysis_draft_ledger RENAME TO manuscript_analysis_draft_ledger_old_022;
CREATE TABLE manuscript_analysis_draft_ledger (
  id TEXT PRIMARY KEY, job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE, unit_id TEXT NOT NULL REFERENCES manuscript_analysis_units(id) ON DELETE CASCADE, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE, entity_id TEXT NOT NULL REFERENCES story_entities(id) ON DELETE CASCADE, related_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL, state_kind TEXT NOT NULL, previous_state TEXT NOT NULL DEFAULT '', new_state TEXT NOT NULL, chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE, scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE, start_offset INTEGER, end_offset INTEGER, confidence REAL NOT NULL CHECK(confidence BETWEEN 0 AND 1), status TEXT NOT NULL DEFAULT 'proposed' CHECK(status IN ('proposed','confirmed','rejected','uncertain','superseded')), created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, source_excerpt TEXT NOT NULL DEFAULT '', source_reference_id TEXT REFERENCES story_source_references(id) ON DELETE SET NULL
);
INSERT INTO manuscript_analysis_draft_ledger SELECT id,job_id,unit_id,project_id,entity_id,related_entity_id,state_kind,previous_state,new_state,chapter_id,scene_id,start_offset,end_offset,confidence,status,created_at,updated_at,source_excerpt,source_reference_id FROM manuscript_analysis_draft_ledger_old_022;
DROP TABLE manuscript_analysis_draft_ledger_old_022;
CREATE INDEX idx_manuscript_analysis_draft_ledger_job ON manuscript_analysis_draft_ledger(job_id, unit_id, status);
PRAGMA foreign_keys=ON;
