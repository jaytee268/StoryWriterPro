-- AI continuity evidence is additive: existing findings remain readable and
-- receive neutral defaults through the desktop migration bootstrap.
CREATE INDEX IF NOT EXISTS idx_continuity_findings_confidence
    ON continuity_review_findings(project_id, confidence, review_status);
