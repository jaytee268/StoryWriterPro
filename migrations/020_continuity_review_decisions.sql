-- Fachliche Continuity-Entscheidungen bleiben auditierbar und ergänzen die
-- historischen review_status-Werte, deren CHECK-Constraint unverändert bleibt.
ALTER TABLE plot_thread_lifecycle_proposals
    ADD COLUMN confidence REAL NOT NULL DEFAULT 0;

ALTER TABLE manuscript_analysis_draft_ledger
    ADD COLUMN source_excerpt TEXT NOT NULL DEFAULT '';

ALTER TABLE manuscript_analysis_draft_ledger
    ADD COLUMN source_reference_id TEXT REFERENCES story_source_references(id) ON DELETE SET NULL;

CREATE TABLE IF NOT EXISTS continuity_review_decisions (
    id TEXT PRIMARY KEY,
    finding_id TEXT NOT NULL UNIQUE REFERENCES continuity_review_findings(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK(status IN ('open','resolved_after_text_change','resolved_with_confirmed_rule','accepted_exception','deferred_rule_review','deferred_canon_review','deferred_open_question','dismissed')),
    decision_kind TEXT NOT NULL,
    rule_id TEXT REFERENCES project_rules(id) ON DELETE SET NULL,
    rule_proposal_id TEXT REFERENCES project_rule_proposals(id) ON DELETE SET NULL,
    open_question_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL,
    source_reference_id TEXT REFERENCES story_source_references(id) ON DELETE SET NULL,
    exception_reason TEXT,
    content_hash TEXT,
    payload_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_continuity_decisions_project_status ON continuity_review_decisions(project_id, status);

CREATE TABLE IF NOT EXISTS continuity_canon_change_audits (
    id TEXT PRIMARY KEY,
    finding_id TEXT NOT NULL REFERENCES continuity_review_findings(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    target_entity_id TEXT,
    target_state_id TEXT,
    action TEXT NOT NULL CHECK(action IN ('previous_incomplete','retcon','new_information','unreliable_perspective','cancelled')),
    reason TEXT NOT NULL,
    previous_source_reference_id TEXT REFERENCES story_source_references(id) ON DELETE SET NULL,
    new_source_reference_id TEXT REFERENCES story_source_references(id) ON DELETE SET NULL,
    source_reference_ids_json TEXT NOT NULL DEFAULT '[]',
    payload_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_continuity_canon_audits_finding ON continuity_canon_change_audits(finding_id, created_at);
