PRAGMA foreign_keys=OFF;

ALTER TABLE manuscript_analysis_artifacts RENAME TO manuscript_analysis_artifacts_040;
CREATE TABLE manuscript_analysis_artifacts (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES manuscript_analysis_jobs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    phase TEXT NOT NULL,
    unit_id TEXT REFERENCES manuscript_analysis_units(id) ON DELETE SET NULL,
    artifact_type TEXT NOT NULL CHECK(artifact_type IN ('bible_proposal','character_memory_proposal','continuity_finding','reveal_compliance_finding','import_draft_state','project_rule_proposal','plot_thread_proposal','narrative_summary','book_end_state_proposal','global_countercheck_finding','timeline_event','story_graph_edge','provisional_entity','provisional_merge','genre_detection')),
    artifact_id TEXT NOT NULL,
    review_status TEXT NOT NULL DEFAULT 'pending' CHECK(review_status IN ('pending','confirmed','rejected','uncertain','skipped')),
    explicitly_skipped INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(job_id, artifact_type, artifact_id)
);
INSERT INTO manuscript_analysis_artifacts (id,job_id,project_id,phase,unit_id,artifact_type,artifact_id,review_status,explicitly_skipped,created_at,updated_at)
SELECT id,job_id,project_id,phase,unit_id,artifact_type,artifact_id,review_status,explicitly_skipped,created_at,updated_at
FROM manuscript_analysis_artifacts_040;
DROP TABLE manuscript_analysis_artifacts_040;
CREATE INDEX idx_manuscript_analysis_artifacts_review ON manuscript_analysis_artifacts(job_id, review_status, explicitly_skipped);

CREATE TABLE IF NOT EXISTS reveal_candidate_decisions (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES lore_crafter_runs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    candidate_temporary_id TEXT NOT NULL,
    decision TEXT NOT NULL CHECK(decision IN ('pending','reveal_draft','reveal_activated','normal_lore','uncertain','not_a_secret')),
    created_artifact_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(run_id, candidate_temporary_id)
);
CREATE INDEX IF NOT EXISTS idx_reveal_candidate_decisions_project ON reveal_candidate_decisions(project_id, run_id, decision);

PRAGMA foreign_keys=ON;
