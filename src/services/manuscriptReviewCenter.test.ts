import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDemoRepository } from './storyRepository';
import { ManuscriptAnalysisController } from './manuscriptAnalysis';
import { contentHash } from '../utils/aiText';

const store = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value), removeItem: (key: string) => store.delete(key) });

describe('jobgebundenes Manuskript-Importreview', () => {
  beforeEach(() => store.clear());

  it('zählt ausschließlich die Artefakte des aktuellen Jobs und blockiert offene Ergebnisse', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace();
    const makeJob = (ref: string) => repository.createManuscriptAnalysisJob({ projectId: workspace.project.id, bookId: workspace.books[0]!.id, importReference: ref, providerId: 'local-prototype', units: [] });
    const first = await makeJob('review-a'); const second = await makeJob('review-b');
    await repository.saveManuscriptAnalysisArtifacts(first.id, [{ jobId: first.id, projectId: workspace.project.id, phase: 'narrative_summaries', artifactType: 'narrative_summary', artifactId: 'summary-a', reviewStatus: 'pending', explicitlySkipped: false }]);
    await repository.saveManuscriptAnalysisArtifacts(second.id, [{ jobId: second.id, projectId: workspace.project.id, phase: 'narrative_summaries', artifactType: 'narrative_summary', artifactId: 'summary-b', reviewStatus: 'pending', explicitlySkipped: false }]);
    await repository.updateManuscriptAnalysisJob({ id: first.id, status: 'awaiting_user_review', currentPhase: 'user_review' });
    await expect(new ManuscriptAnalysisController(repository, first.id).completeUserReview()).rejects.toThrow('Offene Importvorschläge');
    expect((await repository.listManuscriptAnalysisArtifacts(first.id)).map((item) => item.artifactId)).toEqual(['summary-a']);
    expect((await repository.listManuscriptAnalysisArtifacts(second.id)).map((item) => item.artifactId)).toEqual(['summary-b']);
  });

  it('erfordert beim Bulk-Skip einen Audit und behandelt Draft-Zustände ausdrücklich', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const chapter = workspace.chapters[0]!; const scene = chapter.scenes[0]!;
    const job = await repository.createManuscriptAnalysisJob({ projectId: workspace.project.id, bookId: workspace.books[0]!.id, importReference: 'review-skip', providerId: 'local-prototype', units: [{ id: 'review-unit', chapterId: chapter.id, sceneId: scene.id, orderIndex: 0, startOffset: 0, endOffset: 7, content: 'Quelle', contentHash: contentHash('Quelle') }] });
    await repository.saveManuscriptAnalysisArtifacts(job.id, [{ jobId: job.id, projectId: workspace.project.id, phase: 'global_countercheck', artifactType: 'global_countercheck_finding', artifactId: 'finding-a', reviewStatus: 'pending', explicitlySkipped: false }]);
    await repository.replaceManuscriptAnalysisDraftLedger('review-unit', [{ jobId: job.id, unitId: 'review-unit', projectId: workspace.project.id, entityId: workspace.entities[0]!.id, stateKind: 'property', previousState: '', newState: 'unsicher', chapterId: chapter.id, sceneId: scene.id, sourceExcerpt: 'Quelle', confidence: .4, status: 'proposed' }]);
    await repository.updateManuscriptAnalysisJob({ id: job.id, status: 'awaiting_user_review', currentPhase: 'user_review' });
    await new ManuscriptAnalysisController(repository, job.id).completeUserReview(true);
    expect((await repository.listManuscriptAnalysisArtifacts(job.id))[0]).toMatchObject({ reviewStatus: 'skipped', explicitlySkipped: true });
    expect((await repository.listManuscriptAnalysisReviewAudits(job.id)).map((audit) => audit.action)).toEqual(['complete_review', 'skip_open_artifacts']);
    expect((await repository.getManuscriptAnalysisJob(job.id)).status).toBe('completed');
  });

  it('schließt optionale Genre-Ergebnisse normal ab und weist sie im Bericht sichtbar aus', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace();
    const job = await repository.createManuscriptAnalysisJob({ projectId: workspace.project.id, bookId: workspace.books[0]!.id, importReference: 'review-optional-genre', providerId: 'local-prototype', units: [] });
    await repository.saveManuscriptAnalysisArtifacts(job.id, [{ jobId: job.id, projectId: workspace.project.id, phase: 'book_end_state', artifactType: 'genre_detection', artifactId: job.bookId, reviewStatus: 'pending', explicitlySkipped: false }]);
    await repository.updateManuscriptAnalysisJob({ id: job.id, status: 'awaiting_user_review', currentPhase: 'user_review' });

    await new ManuscriptAnalysisController(repository, job.id).completeUserReview();

    expect((await repository.getManuscriptAnalysisJob(job.id)).status).toBe('completed');
    expect((await repository.listManuscriptAnalysisArtifacts(job.id))[0]).toMatchObject({ artifactType: 'genre_detection', reviewStatus: 'pending', explicitlySkipped: false });
    expect((await repository.getManuscriptAnalysisCompletionReport(job.id))?.payload.openOptionalResults).toEqual([expect.objectContaining({ artifactType: 'genre_detection', reviewStatus: 'pending' })]);
  });

  it('verknüpft eine Bestätigung mit dem echten Domänenobjekt', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace();
    const job = await repository.createManuscriptAnalysisJob({ projectId: workspace.project.id, bookId: workspace.books[0]!.id, importReference: 'review-domain', providerId: 'local-prototype', units: [] });
    const summary = await repository.saveNarrativeSummary({ projectId: workspace.project.id, scopeType: 'book', scopeId: workspace.books[0]!.id, contentHash: 'hash', summary: 'Vorschlag', importantEvents: [], openThreads: [], characterChanges: [], status: 'proposed', authorConfirmed: false });
    const artifact = (await repository.saveManuscriptAnalysisArtifacts(job.id, [{ jobId: job.id, projectId: workspace.project.id, phase: 'narrative_summaries', artifactType: 'narrative_summary', artifactId: summary.id, reviewStatus: 'pending', explicitlySkipped: false }]))[0]!;
    await repository.reviewManuscriptAnalysisArtifactDecision(artifact.id, 'confirmed');
    expect((await repository.listNarrativeSummaries(workspace.project.id)).find((item) => item.id === summary.id)).toMatchObject({ status: 'confirmed', authorConfirmed: true });
    expect((await repository.listManuscriptAnalysisArtifacts(job.id)).find((item) => item.id === artifact.id)?.reviewStatus).toBe('confirmed');
  });

  it('lässt das Artefakt pending, wenn die Fachaktion fehlschlägt', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace();
    const job = await repository.createManuscriptAnalysisJob({ projectId: workspace.project.id, bookId: workspace.books[0]!.id, importReference: 'review-failure', providerId: 'local-prototype', units: [] });
    const artifact = (await repository.saveManuscriptAnalysisArtifacts(job.id, [{ jobId: job.id, projectId: workspace.project.id, phase: 'narrative_summaries', artifactType: 'narrative_summary', artifactId: 'missing-summary', reviewStatus: 'pending', explicitlySkipped: false }]))[0]!;
    await expect(repository.reviewManuscriptAnalysisArtifactDecision(artifact.id, 'confirmed')).rejects.toThrow('Zusammenfassung');
    expect((await repository.listManuscriptAnalysisArtifacts(job.id)).find((item) => item.id === artifact.id)?.reviewStatus).toBe('pending');
  });

  it('führt Timeline, Graph und provisorische Merge-Vorschläge im gleichen Jobreview', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const chapter = workspace.chapters[0]!; const scene = chapter.scenes[0]!; const book = workspace.books[0]!;
    const job = await repository.createManuscriptAnalysisJob({ projectId: workspace.project.id, bookId: book.id, importReference: 'review-graph', providerId: 'local-prototype', units: [] });
    const event = await repository.saveTimelineEvent({ projectId: workspace.project.id, bookId: book.id, chapterId: chapter.id, sceneId: scene.id, title: 'Ereignis', summary: 'Passiert', storyTimeText: '', temporalOrder: 1, timeCertainty: 'unknown', participatingEntityIds: [], causeEventIds: [], consequenceEventIds: [], knowledgeChanges: [], stateChanges: [], relatedPlotThreadIds: [], sourceReferenceIds: [], confidence: .8, status: 'proposed', authorConfirmed: false, origin: 'manuscript_analysis' });
    const edge = await repository.saveStoryGraphEdge({ projectId: workspace.project.id, sourceEntityId: workspace.entities[0]!.id, targetEntityId: workspace.entities[1]!.id, relationType: 'connected_to', label: 'Verbindung', sourceReferenceIds: [], confidence: .7, status: 'proposed', authorConfirmed: false, origin: 'manuscript_analysis' });
    const provisional = await repository.saveProvisionalEntity({ jobId: job.id, projectId: workspace.project.id, entityType: 'object', canonicalName: 'Vorläufig', aliases: [], description: 'Noch nicht kanonisch', confidence: .5, reviewStatus: 'proposed' });
    const merge = await repository.saveProvisionalMergeProposal({ jobId: job.id, projectId: workspace.project.id, leftProvisionalEntityId: provisional.id, reason: 'Zusammenführen prüfen', confidence: .5, reviewStatus: 'proposed' });
    await repository.saveManuscriptAnalysisArtifacts(job.id, [
      { jobId: job.id, projectId: workspace.project.id, phase: 'passage_continuity', artifactType: 'timeline_event', artifactId: event.id, reviewStatus: 'pending', explicitlySkipped: false },
      { jobId: job.id, projectId: workspace.project.id, phase: 'passage_continuity', artifactType: 'story_graph_edge', artifactId: edge.id, reviewStatus: 'pending', explicitlySkipped: false },
      { jobId: job.id, projectId: workspace.project.id, phase: 'passage_continuity', artifactType: 'provisional_entity', artifactId: provisional.id, reviewStatus: 'pending', explicitlySkipped: false },
      { jobId: job.id, projectId: workspace.project.id, phase: 'passage_continuity', artifactType: 'provisional_merge', artifactId: merge.id, reviewStatus: 'pending', explicitlySkipped: false },
    ]);
    expect((await repository.listManuscriptAnalysisArtifacts(job.id)).map((item) => item.artifactType)).toEqual(['timeline_event', 'story_graph_edge', 'provisional_entity', 'provisional_merge']);
  });

  it('materialisiert eine bestätigte provisorische Entität und hält einen Merge ohne Ziel pending', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const job = await repository.createManuscriptAnalysisJob({ projectId: workspace.project.id, bookId: workspace.books[0]!.id, importReference: 'review-materialize', providerId: 'local-prototype', units: [] });
    const provisional = await repository.saveProvisionalEntity({ jobId: job.id, projectId: workspace.project.id, entityType: 'object', canonicalName: 'Der Schlüssel', aliases: ['Schlüssel'], description: 'Ein vorgeschlagener Gegenstand.', confidence: .8, reviewStatus: 'proposed' });
    const entityArtifact = (await repository.saveManuscriptAnalysisArtifacts(job.id, [{ jobId: job.id, projectId: workspace.project.id, phase: 'passage_continuity', artifactType: 'provisional_entity', artifactId: provisional.id, reviewStatus: 'pending', explicitlySkipped: false }]))[0]!;
    await repository.reviewManuscriptAnalysisArtifactDecision(entityArtifact.id, 'confirmed');
    const after = await repository.loadWorkspace();
    expect(after.entities.some((entity) => entity.name === 'Der Schlüssel' && entity.authorConfirmed)).toBe(true);
    expect((await repository.listProvisionalEntities(job.id)).find((item) => item.id === provisional.id)?.reviewStatus).toBe('accepted');

    const pendingMerge = await repository.saveProvisionalEntity({ jobId: job.id, projectId: workspace.project.id, entityType: 'object', canonicalName: 'Ohne Ziel', aliases: [], description: '', confidence: .4, reviewStatus: 'proposed' });
    const merge = await repository.saveProvisionalMergeProposal({ jobId: job.id, projectId: workspace.project.id, leftProvisionalEntityId: pendingMerge.id, reason: 'Ziel fehlt', confidence: .5, reviewStatus: 'proposed' });
    const mergeArtifact = (await repository.saveManuscriptAnalysisArtifacts(job.id, [{ jobId: job.id, projectId: workspace.project.id, phase: 'passage_continuity', artifactType: 'provisional_merge', artifactId: merge.id, reviewStatus: 'pending', explicitlySkipped: false }]))[0]!;
    await expect(repository.reviewManuscriptAnalysisArtifactDecision(mergeArtifact.id, 'confirmed')).rejects.toThrow('bestehendes Ziel');
    expect((await repository.listManuscriptAnalysisArtifacts(job.id)).find((item) => item.id === mergeArtifact.id)?.reviewStatus).toBe('pending');
  });

  it('bestätigt ein Finding fachlich ohne den objektiven Konflikt als gelöst auszugeben', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const job = await repository.createManuscriptAnalysisJob({ projectId: workspace.project.id, bookId: workspace.books[0]!.id, importReference: 'review-finding', providerId: 'local-prototype', units: [] });
    const run = await repository.createContinuityReviewRun({ projectId: workspace.project.id, chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, sourceKind: 'manual', contentHash: 'finding-hash', providerId: 'local-prototype' });
    const finding = (await repository.saveContinuityReviewFindings(run.id, [{ runId: run.id, projectId: workspace.project.id, findingType: 'probable_contradiction', severity: 'warning', relatedEntityIds: [], relatedStateIds: [], relatedRuleIds: [], objectiveConflict: 'Zustand widerspricht sich.', loreExplanations: [], evidenceExcerpt: 'Beleg', counterEvidenceExcerpts: [], confidence: .7, reason: 'Prüfung', reviewStatus: 'open' }]))[0]!;
    const artifact = (await repository.saveManuscriptAnalysisArtifacts(job.id, [{ jobId: job.id, projectId: workspace.project.id, phase: 'global_countercheck', artifactType: 'global_countercheck_finding', artifactId: finding.id, reviewStatus: 'pending', explicitlySkipped: false }]))[0]!;
    await repository.applyContinuityFindingDecision({ findingId: finding.id, projectId: workspace.project.id, status: 'deferred_canon_review', decisionKind: 'canon_review' });
    expect((await repository.listContinuityReviewFindings(workspace.project.id)).find((item) => item.id === finding.id)).toMatchObject({ reviewStatus: 'deferred' });
    expect((await repository.listManuscriptAnalysisArtifacts(job.id)).find((item) => item.id === artifact.id)?.reviewStatus).toBe('uncertain');
  });

  it('überführt einen bestätigten Book-End-State als echten Continuity-Domainzustand', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const chapter = workspace.chapters[0]!; const scene = chapter.scenes[0]!; const entity = workspace.entities[0]!;
    const job = await repository.createManuscriptAnalysisJob({ projectId: workspace.project.id, bookId: workspace.books[0]!.id, importReference: 'review-end-state', providerId: 'local-prototype', units: [] });
    const source = await repository.createSourceReference({ projectId: workspace.project.id, entityId: entity.id, chapterId: chapter.id, sceneId: scene.id, excerpt: 'Endzustand', startOffset: 0, endOffset: 10 });
    const phase = await repository.saveManuscriptAnalysisPhaseResult({ jobId: job.id, projectId: workspace.project.id, phase: 'book_end_state', resultKind: 'book_end_state', payload: { endStateProposals: [{ category: 'location', entityId: entity.id, statement: 'Am Hafen', confidence: .9, evidenceExcerpt: 'Endzustand', sourceReferenceId: source.id }] }, contentHash: 'end-state', providerId: 'local-prototype', promptVersion: 'test', reviewStatus: 'pending' });
    const artifact = (await repository.saveManuscriptAnalysisArtifacts(job.id, [{ jobId: job.id, projectId: workspace.project.id, phase: 'book_end_state', artifactType: 'book_end_state_proposal', artifactId: `${phase.id}:0`, reviewStatus: 'pending', explicitlySkipped: false }]))[0]!;
    await repository.reviewManuscriptAnalysisArtifactDecision(artifact.id, 'confirmed');
    expect((await repository.listContinuityStateLedger(workspace.project.id)).some((entry) => entry.entityId === entity.id && entry.newState === 'Am Hafen' && entry.status === 'confirmed')).toBe(true);
  });
});
