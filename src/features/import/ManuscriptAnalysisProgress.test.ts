import { describe, expect, it } from 'vitest';
import { findingDecisionFor, getManuscriptReviewCounts } from './manuscriptAnalysisReview';

describe('sichtbarer Manuskript-Reviewworkflow', () => {
  it('zählt Genre nur optional und erlaubt den normalen Abschluss ohne Pflichtentscheidungen', () => {
    const counts = getManuscriptReviewCounts([
      { artifactType: 'genre_detection', reviewStatus: 'pending' },
    ] as never, []);
    expect(counts).toEqual({ blockingArtifacts: 0, optionalGenreArtifacts: 1, pendingDrafts: 0, blockingOpen: 0 });
  });

  it('zählt verpflichtende Artefakte und Draft-Zustände als blockierend', () => {
    const counts = getManuscriptReviewCounts([
      { artifactType: 'bible_proposal', reviewStatus: 'pending' },
      { artifactType: 'genre_detection', reviewStatus: 'pending' },
    ] as never, [{ status: 'proposed' }] as never);
    expect(counts.blockingOpen).toBe(2);
    expect(counts.optionalGenreArtifacts).toBe(1);
  });

  it('ordnet alle zentralen Finding-Aktionen einem fachlichen DecisionKind und Status zu', () => {
    expect(findingDecisionFor('finding', 'confirm_conflict')).toMatchObject({ status: 'open', decisionKind: 'canon_review' });
    expect(findingDecisionFor('finding', 'canon_review')).toMatchObject({ status: 'deferred_canon_review', decisionKind: 'canon_review' });
    expect(findingDecisionFor('finding', 'lore_explanation')).toMatchObject({ status: 'resolved_with_confirmed_rule', decisionKind: 'confirmed_rule' });
    expect(findingDecisionFor('finding', 'riddle')).toMatchObject({ status: 'deferred_open_question', decisionKind: 'open_question' });
    expect(findingDecisionFor('finding', 'text_correction')).toMatchObject({ status: 'open', decisionKind: 'text_correction' });
    expect(findingDecisionFor('finding', 'uncertain')).toMatchObject({ status: 'deferred_canon_review', decisionKind: 'canon_review' });
    expect(findingDecisionFor('finding', 'dismiss')).toMatchObject({ status: 'dismissed', decisionKind: 'dismiss' });
  });

  it('enthält Normalabschluss, getrennten Bulk-Skip und Abschlussbericht in der sichtbaren Komponente', () => {
    const source = Object.values(import.meta.glob('./ManuscriptAnalysisProgress.tsx', { eager: true, query: '?raw', import: 'default' }) as Record<string, string>)[0] ?? '';
    expect(source).toContain('onCompleteReview(false)');
    expect(source).toContain('onCompleteReview(true)');
    expect(source).toContain('Optional · noch nicht bestätigt');
    expect(source).toContain('Abschlussbericht');
  });
});
