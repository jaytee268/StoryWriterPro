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
});
