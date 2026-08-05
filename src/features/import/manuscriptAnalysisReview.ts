import type { ContinuityFindingDecisionKind, ContinuityFindingDecisionStatus, ManuscriptAnalysisArtifact, ManuscriptAnalysisCompletionReport, ManuscriptAnalysisDraftLedgerEntry } from '../../types/domain';

export interface ManuscriptReviewCounts { blockingArtifacts: number; optionalGenreArtifacts: number; pendingDrafts: number; blockingOpen: number; }

export function getManuscriptReviewCounts(artifacts: ManuscriptAnalysisArtifact[], draftLedger: ManuscriptAnalysisDraftLedgerEntry[]): ManuscriptReviewCounts {
  const pending = artifacts.filter((artifact) => artifact.reviewStatus === 'pending');
  const blockingArtifacts = pending.filter((artifact) => artifact.artifactType !== 'genre_detection').length;
  const optionalGenreArtifacts = pending.filter((artifact) => artifact.artifactType === 'genre_detection').length;
  const pendingDrafts = draftLedger.filter((entry) => entry.status === 'proposed' || entry.status === 'uncertain').length;
  return { blockingArtifacts, optionalGenreArtifacts, pendingDrafts, blockingOpen: blockingArtifacts + pendingDrafts };
}

export type ManuscriptFindingAction = 'confirm_conflict' | 'canon_review' | 'lore_explanation' | 'riddle' | 'text_correction' | 'uncertain' | 'dismiss';

export function findingDecisionFor(findingId: string, action: ManuscriptFindingAction): { findingId: string; status: ContinuityFindingDecisionStatus; decisionKind: ContinuityFindingDecisionKind } {
  const decisions: Record<ManuscriptFindingAction, { status: ContinuityFindingDecisionStatus; decisionKind: ContinuityFindingDecisionKind }> = {
    confirm_conflict: { status: 'open', decisionKind: 'canon_review' },
    canon_review: { status: 'deferred_canon_review', decisionKind: 'canon_review' },
    lore_explanation: { status: 'resolved_with_confirmed_rule', decisionKind: 'confirmed_rule' },
    riddle: { status: 'deferred_open_question', decisionKind: 'open_question' },
    text_correction: { status: 'open', decisionKind: 'text_correction' },
    uncertain: { status: 'deferred_canon_review', decisionKind: 'canon_review' },
    dismiss: { status: 'dismissed', decisionKind: 'dismiss' },
  };
  return { findingId, ...decisions[action] };
}

export function reportStatusCount(report: ManuscriptAnalysisCompletionReport, status: 'confirmed' | 'rejected' | 'uncertain'): number {
  const items = [...report.payload.bibleDecisions, ...report.payload.memoryDecisions, ...report.payload.continuityFindings, ...report.payload.timelineEvents, ...report.payload.graphEdges, ...report.payload.plotThreads];
  return items.filter((item) => typeof item === 'object' && item !== null && 'reviewStatus' in item && (item as { reviewStatus?: unknown }).reviewStatus === status).length;
}
