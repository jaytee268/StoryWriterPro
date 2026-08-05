CREATE TABLE IF NOT EXISTS reveal_compliance_runs (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL,
    unit_id TEXT NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    contract_id TEXT NOT NULL REFERENCES reveal_contracts(id) ON DELETE RESTRICT,
    input_hash TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','running','completed','failed','stale')),
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(job_id, unit_id, contract_id, input_hash)
);

CREATE TABLE IF NOT EXISTS reveal_compliance_findings (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES reveal_compliance_runs(id) ON DELETE CASCADE,
    job_id TEXT NOT NULL,
    unit_id TEXT NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    contract_id TEXT NOT NULL REFERENCES reveal_contracts(id) ON DELETE RESTRICT,
    subject_entity_id TEXT NOT NULL REFERENCES story_entities(id) ON DELETE RESTRICT,
    character_entity_id TEXT REFERENCES story_entities(id) ON DELETE RESTRICT,
    finding_type TEXT NOT NULL CHECK(finding_type IN ('premature_revelation','impossible_character_knowledge','narrator_information_leak','forbidden_clue','reveal_plan_conflict','missing_required_foreshadowing','ambiguous_possible_leak')),
    severity TEXT NOT NULL CHECK(severity IN ('info','warning','critical')),
    evidence_excerpt TEXT NOT NULL,
    explanation TEXT NOT NULL,
    expected_knowledge_level TEXT NOT NULL CHECK(expected_knowledge_level IN ('unknown','suspects','partial','knows','false_belief')),
    actual_disclosure_level TEXT NOT NULL CHECK(actual_disclosure_level IN ('unknown','suspects','partial','knows','false_belief')),
    chapter_id TEXT,
    scene_id TEXT,
    start_offset INTEGER,
    end_offset INTEGER,
    provider_id TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    review_status TEXT NOT NULL DEFAULT 'open' CHECK(review_status IN ('open','accepted','dismissed','correction_planned','reveal_rule_changed','intentional_exception')),
    user_decision TEXT,
    source_reference_id TEXT REFERENCES project_source_references(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_reveal_compliance_runs_job ON reveal_compliance_runs(project_id, job_id, unit_id, status);
CREATE INDEX IF NOT EXISTS idx_reveal_compliance_findings_job ON reveal_compliance_findings(project_id, job_id, review_status, severity);
