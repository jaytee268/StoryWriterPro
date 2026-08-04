import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDemoRepository } from './storyRepository';
import { ManuscriptAnalysisController } from './manuscriptAnalysis';
import { BrowserLongformRepository } from './longformRepository';
import { contentHash } from '../utils/aiText';
import type { ContinuityAnalysisResult, DraftContinuityState, SaveChapterGenerationSectionInput } from '../types/domain';
import type { ContinuityAnalysisInput, StoryAiProvider } from './aiProviderService';

const browserValues = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => browserValues.get(key) ?? null, setItem: (key: string, value: string) => browserValues.set(key, value), removeItem: (key: string) => browserValues.delete(key), clear: () => browserValues.clear() });

const emptyContinuity = (): ContinuityAnalysisResult => ({ observedActions: [], proposedStateChanges: [], objectiveContradictions: [], missingExplanations: [], matchedLoreRules: [], newRuleProposals: [], plotThreadChanges: [], confidence: 0.9, evidence: [], warnings: [] });
const emptyDraftState: DraftContinuityState = { currentLocation: '', currentStoryTime: '', presentCharacterIds: [], characterStates: [], establishedFacts: [], knowledgeChanges: [], relationshipChanges: [], movedObjects: [], injuries: [], cluesIntroduced: [], promisesCreated: [], unresolvedActions: [], lastParagraphSummary: '' };

function fakeProvider(analyze: (input: ContinuityAnalysisInput) => Promise<ContinuityAnalysisResult> | ContinuityAnalysisResult): StoryAiProvider {
  return { id: 'fake-provider', analyzeContinuityPassage: vi.fn(analyze), getStatus: vi.fn(), extractBiblePatch: vi.fn(async () => ({ proposals: [], warnings: [] })), extractCharacterMemoryPatch: vi.fn(async () => ({ proposals: [], warnings: [] })), analyzeProjectStyle: vi.fn(async () => ({ observations: [], overallSummary: 'synthetisch', warnings: [] })), summarize: vi.fn(async () => ({ summary: 'synthetische Zusammenfassung', importantEvents: ['Zustand beobachtet'], openThreads: ['Herkunft offen'], characterChanges: [], knowledgeChanges: [], relationshipEffects: [], warnings: [] })), answerWithProjectContext: vi.fn(), cancel: vi.fn() } as unknown as StoryAiProvider;
}

describe('continuity import and longform fake-provider E2E', () => {
  beforeEach(() => browserValues.clear());

  it('imports pages sequentially, retries a provider failure, resumes, reviews findings and keeps the ledger proposed', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const chapter = workspace.chapters[0]!;
    const scene = chapter.scenes[0]!;
    const text = '😀 Der Zettel wird entsorgt.\n\nSpäter liegt das Papier wieder in der Tasche.';
    await repository.updateScene({ ...scene, content: text });
    const entity = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Zettel', type: 'object', description: '', status: 'confirmed', confidence: 1, chapterId: chapter.id, sceneId: scene.id, excerpt: 'Zettel', authorConfirmed: true, tags: [] });
    const first = '😀 Der Zettel wird entsorgt.';
    const second = 'Später liegt das Papier wieder in der Tasche.';
    const firstEnd = Array.from(first).length;
    const secondStart = firstEnd + 2;
    const job = await repository.createManuscriptAnalysisJob({ projectId: workspace.project.id, bookId: workspace.books[0]!.id, importReference: 'e2e-import', providerId: 'fake-provider', pageMarkers: [{ chapterId: chapter.id, pageNumber: 1, label: 'Seite 1', sourceOffset: 0, textOffset: 0 }, { chapterId: chapter.id, pageNumber: 2, label: 'Seite 2', sourceOffset: secondStart, textOffset: secondStart }], units: [{ id: 'e2e-unit-1', chapterId: chapter.id, sceneId: scene.id, orderIndex: 0, pageNumber: 1, startOffset: 0, endOffset: firstEnd, content: first, contentHash: contentHash(first) }, { id: 'e2e-unit-2', chapterId: chapter.id, sceneId: scene.id, orderIndex: 1, pageNumber: 2, startOffset: secondStart, endOffset: Array.from(text).length, content: second, contentHash: contentHash(second) }] });
    let active = 0;
    let maximum = 0;
    const provider = fakeProvider(async (input) => {
      active += 1; maximum = Math.max(maximum, active);
      await new Promise((resolve) => setTimeout(resolve, 1));
      active -= 1;
      if (input.passage.text.startsWith('😀')) return { ...emptyContinuity(), proposedStateChanges: [{ entityId: entity.id, stateKind: 'item_availability', previousState: 'vorhanden', newState: 'entsorgt', confidence: 0.96, evidenceExcerpt: input.passage.text, reason: 'Semantische Beobachtung ohne Schlüsselwortkopplung.' }] };
      return { ...emptyContinuity(), objectiveContradictions: [{ findingType: 'probable_contradiction', subjectEntityId: entity.id, relatedEntityIds: [], relatedStateIds: [], objectiveConflict: 'Ein zuvor entsorgter Gegenstand erscheint erneut.', evidenceExcerpt: input.passage.text, counterEvidenceExcerpts: ['Der Gegenstand wurde zuvor entsorgt.'], confidence: 0.88, reason: 'Papier und Zettel werden als derselbe Gegenstand verstanden.' }] };
    });
    await new ManuscriptAnalysisController(repository, job.id, provider).start();
    expect(maximum).toBe(1);
    expect((await repository.getManuscriptAnalysisJob(job.id)).status).toBe('completed');
    expect((await repository.getManuscriptAnalysisJob(job.id)).pageMarkers).toHaveLength(2);
    const draft = await repository.listManuscriptAnalysisDraftLedger(job.id);
    expect(draft).toHaveLength(1);
    expect(draft[0]).toMatchObject({ newState: 'entsorgt', status: 'proposed', unitId: 'e2e-unit-1' });
    await repository.reviewManuscriptAnalysisDraftLedger(draft[0]!.id, 'confirmed');
    expect(await repository.listContinuityStateLedger(workspace.project.id)).toEqual([]);
    expect((await repository.listContinuityReviewFindings(workspace.project.id)).some((finding) => finding.reviewStatus === 'open')).toBe(true);
  });

  it('passes draft state through longform sections, blocks critical review, regenerates and atomically accepts proposed states', async () => {
    browserValues.set('storymemory-browser-demo-workspace', JSON.stringify({ project: { id: 'p' }, books: [{ id: 'b', projectId: 'p' }], chapters: [], entities: [{ id: 'entity', projectId: 'p' }], sources: [], continuityLedger: [] }));
    const repository = new BrowserLongformRepository();
    const job = await repository.createJob({ projectId: 'p', targetBookId: 'b', targetWords: 200, userInstruction: 'synthetischer Plan', activeProvider: 'fake-provider', contentContextHash: 'context-a' });
    await repository.savePlan({ jobId: job.id, chapterTitle: 'Das Papier', chapterGoal: 'Die Spur verfolgen', povCharacterId: undefined, startingState: 'Ein Hinweis fehlt.', endingState: 'Eine neue Frage bleibt.', chapterSummary: 'Zwei Abschnitte mit Zustandsübergabe.', endingConnection: 'Fortsetzung', newInformation: [], withheldInformation: [], beats: [0, 1].map((orderIndex) => ({ id: `beat-${orderIndex}`, orderIndex, title: `Abschnitt ${orderIndex + 1}`, purpose: 'Fake-Provider-Beat', participatingCharacterIds: [], startingState: '', event: '', conflict: '', newInformation: [], knowledgeChanges: [], relationshipChanges: [], cluesUsed: [], loreEntityIds: [], endingHook: '', targetWords: 100 })), reviewStatus: 'accepted' });
    const sectionInput = (orderIndex: number, content: string, draftState: SaveChapterGenerationSectionInput['draftState'] = 'valid'): SaveChapterGenerationSectionInput => ({ jobId: job.id, planBeatId: `beat-${orderIndex}`, orderIndex, targetWords: 100, content, continuationSummary: `Nach Abschnitt ${orderIndex}`, continuityState: emptyDraftState, status: 'generated', draftState, contentHash: contentHash(content) });
    const first = await repository.saveSection(sectionInput(0, '😀 Der Zettel ist zerstört.'));
    const second = await repository.saveSection(sectionInput(1, 'Später findet Malik die Spur.'));
    await repository.replaceDraftLedger(first.id, [{ jobId: job.id, sectionId: first.id, projectId: 'p', entityId: 'entity', stateKind: 'item_existence', previousState: 'vorhanden', newState: 'zerstört', sourceExcerpt: first.content, sourceStartOffset: 0, sourceEndOffset: Array.from(first.content).length, contentHash: first.contentHash!, confidence: 0.94 }]);
    await repository.replaceDraftLedger(second.id, [{ jobId: job.id, sectionId: second.id, projectId: 'p', entityId: 'entity', stateKind: 'knowledge', previousState: 'unbekannt', newState: 'Spur erkannt', sourceExcerpt: second.content, sourceStartOffset: 0, sourceEndOffset: Array.from(second.content).length, contentHash: second.contentHash!, confidence: 0.81 }]);
    expect((await new BrowserLongformRepository().listDraftLedger(job.id)).map((entry) => entry.newState)).toEqual(['zerstört', 'Spur erkannt']);
    const review = await repository.saveReviews(job.id, [{ sectionId: second.id, reviewScope: 'section', issueType: 'continuity', severity: 'blocking', title: 'Kritischer Widerspruch', description: 'Prüfung erforderlich.', relatedEntityIds: ['entity'], relatedSourceIds: [], suggestedAction: 'Regenerieren', status: 'open' }]);
    await repository.updateJobStatus(job.id, 'draft_ready');
    await expect(repository.acceptJob({ jobId: job.id, currentContextHash: 'context-a' })).rejects.toThrow();
    await repository.updateReviewStatus(review[0]!.id, 'resolved');
    await repository.supersedeDraftLedgerFrom(job.id, 1);
    await repository.saveSection(sectionInput(1, 'Die Spur wird neu geprüft.', 'regenerate_requested'));
    await expect(repository.acceptJob({ jobId: job.id, currentContextHash: 'context-a' })).rejects.toThrow();
    const regenerated = await repository.saveSection(sectionInput(1, 'Die Spur wird neu geprüft.'));
    await repository.replaceDraftLedger(regenerated.id, [{ jobId: job.id, sectionId: regenerated.id, projectId: 'p', entityId: 'entity', stateKind: 'knowledge', previousState: 'unbekannt', newState: 'Spur erkannt', sourceExcerpt: regenerated.content, sourceStartOffset: 0, sourceEndOffset: Array.from(regenerated.content).length, contentHash: regenerated.contentHash!, confidence: 0.8 }]);
    await repository.updateJobStatus(job.id, 'draft_ready');
    const accepted = await repository.acceptJob({ jobId: job.id, currentContextHash: 'context-a' });
    expect(accepted.status).toBe('accepted');
    const workspace = JSON.parse(browserValues.get('storymemory-browser-demo-workspace')!);
    expect(workspace.chapters[0].scenes).toHaveLength(2);
    expect(workspace.continuityLedger.every((entry: { status: string; authorConfirmed: boolean }) => entry.status === 'proposed' && !entry.authorConfirmed)).toBe(true);
  });
});
