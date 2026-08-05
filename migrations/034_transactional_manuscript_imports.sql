-- Additive import catalog. Imported chapters carry the version id while
-- manually created chapters remain NULL and therefore untouched.
CREATE TABLE IF NOT EXISTS manuscript_import_versions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    source_document_id TEXT NOT NULL REFERENCES project_source_documents(id) ON DELETE RESTRICT,
    original_content_hash TEXT NOT NULL,
    version_number INTEGER NOT NULL CHECK(version_number > 0),
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','archived','cancelled')),
    analysis_job_id TEXT REFERENCES manuscript_analysis_jobs(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    activated_at TEXT,
    UNIQUE(project_id, book_id, original_content_hash, version_number)
);
CREATE INDEX IF NOT EXISTS idx_manuscript_import_versions_active
  ON manuscript_import_versions(project_id, book_id, status, created_at);

ALTER TABLE manuscript_analysis_jobs ADD COLUMN import_version_id TEXT REFERENCES manuscript_import_versions(id) ON DELETE SET NULL;
ALTER TABLE chapters ADD COLUMN import_version_id TEXT REFERENCES manuscript_import_versions(id) ON DELETE SET NULL;
ALTER TABLE chapters ADD COLUMN import_status TEXT NOT NULL DEFAULT 'active' CHECK(import_status IN ('active','archived'));
CREATE INDEX IF NOT EXISTS idx_chapters_import_version ON chapters(import_version_id, order_index);

-- Migrations 022/029 left older SQLite foreign-key metadata on the unit and
-- draft-ledger tables. Keep a compatibility parent for existing databases so
-- those references remain valid while new writes use the current job table.
CREATE TABLE IF NOT EXISTS manuscript_analysis_jobs_old_029 (id TEXT PRIMARY KEY);
INSERT OR IGNORE INTO manuscript_analysis_jobs_old_029(id)
  SELECT id FROM manuscript_analysis_jobs;
CREATE TRIGGER IF NOT EXISTS trg_manuscript_analysis_jobs_compat_insert
AFTER INSERT ON manuscript_analysis_jobs
BEGIN
  INSERT OR IGNORE INTO manuscript_analysis_jobs_old_029(id) VALUES (NEW.id);
END;
