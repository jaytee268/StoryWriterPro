-- Additive continuity data-quality fields. Existing rows remain readable.
ALTER TABLE continuity_state_ledger ADD COLUMN reason TEXT NOT NULL DEFAULT '';
ALTER TABLE continuity_state_ledger ADD COLUMN evidence_excerpt TEXT NOT NULL DEFAULT '';
ALTER TABLE continuity_review_findings ADD COLUMN source_reference_id TEXT REFERENCES story_source_references(id) ON DELETE SET NULL;
ALTER TABLE continuity_review_findings ADD COLUMN counter_evidence_structured_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE plot_thread_lifecycle_proposals ADD COLUMN source_reference_id TEXT REFERENCES story_source_references(id) ON DELETE SET NULL;

CREATE INDEX idx_continuity_findings_source_reference
    ON continuity_review_findings(project_id, source_reference_id);
