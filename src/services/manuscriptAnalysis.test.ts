import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDemoRepository } from './storyRepository';
import { ManuscriptAnalysisController, createManuscriptAnalysisUnits } from './manuscriptAnalysis';
import { contentHash } from '../utils/aiText';
import type { ContinuityAnalysisResult } from '../types/domain';
import type { ContinuityAnalysisInput, StoryAiProvider } from './aiProviderService';

const values = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value), removeItem: (key: string) => values.delete(key) });

const emptyAnalysis = (): ContinuityAnalysisResult => ({ observedActions: [], proposedStateChanges: [], objectiveContradictions: [], missingExplanations: [], matchedLoreRules: [], newRuleProposals: [], plotThreadChanges: [], confidence: 0.8, evidence: [], warnings: [] });

function fakeProvider(onAnalyze: (input: ContinuityAnalysisInput) => Promise<ContinuityAnalysisResult> | ContinuityAnalysisResult, cancelActive = vi.fn(async () => undefined)): StoryAiProvider {
  return { id: 'codex-cli', analyzeContinuityPassage: vi.fn(onAnalyze), getStatus: vi.fn(), extractBiblePatch: vi.fn(async () => ({ proposals: [], warnings: [] })), extractCharacterMemoryPatch: vi.fn(async () => ({ proposals: [], warnings: [] })), analyzeProjectStyle: vi.fn(), summarize: vi.fn(async () => ({ summary: 'Fake-Synthese', importantEvents: [], openThreads: [], characterChanges: [], knowledgeChanges: [], relationshipEffects: [], warnings: [] })), answerWithProjectContext: vi.fn(), cancel: vi.fn(), cancelActive } as unknown as StoryAiProvider;
}

async function makeJob(repository: BrowserDemoRepository, importReference: string) {
  const workspace = await repository.loadWorkspace();
  const chapter = workspace.chapters[0]!;
  const scene = chapter.scenes[0]!;
  const text = 'Zettel wird fortgetragen.\n\nMalik wartet.';
  await repository.updateScene({ ...scene, content: text });
  const entity = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Zettel', type: 'object', description: '', status: 'confirmed', confidence: 1, chapterId: chapter.id, sceneId: scene.id, excerpt: 'Zettel', authorConfirmed: true, tags: [] });
  const first = 'Zettel wird fortgetragen.';
  const second = 'Malik wartet.';
  const source = await repository.createSourceReference({ projectId: workspace.project.id, entityId: entity.id, chapterId: chapter.id, sceneId: scene.id, excerpt: first, startOffset: 0, endOffset: Array.from(first).length });
  const job = await repository.createManuscriptAnalysisJob({ projectId: workspace.project.id, bookId: workspace.books[0]!.id, importReference, providerId: 'codex-cli', pageMarkers: [{ chapterId: chapter.id, pageNumber: 1, label: 'Seite 1', sourceOffset: 0, textOffset: 0 }], units: [{ id: `${importReference}-1`, chapterId: chapter.id, sceneId: scene.id, orderIndex: 0, pageNumber: 1, startOffset: 0, endOffset: Array.from(first).length, content: first, contentHash: contentHash(first) }, { id: `${importReference}-2`, chapterId: chapter.id, sceneId: scene.id, orderIndex: 1, pageNumber: 9, startOffset: Array.from(first).length + 2, endOffset: Array.from(text).length, content: second, contentHash: contentHash(second) }] });
  return { workspace: await repository.loadWorkspace(), job, entity, source };
}

describe('sequenzielle, fortsetzbare Manuskript-Continuity', () => {
  beforeEach(() => values.clear());

  it('verarbeitet Einheiten strikt nacheinander und reicht den Draft-Ledger weiter', async () => {
    const repository = new BrowserDemoRepository();
    const { job, entity, source } = await makeJob(repository, 'sequential');
    const order: string[] = [];
    let active = 0;
    let maximum = 0;
    const seenDraftSizes: number[] = [];
    const provider = fakeProvider(async (input) => {
      order.push(input.passage.text);
      seenDraftSizes.push(input.draftLedger.length);
      active += 1; maximum = Math.max(maximum, active);
      await new Promise((resolve) => setTimeout(resolve, 1));
      active -= 1;
      return input.passage.text.startsWith('Zettel') ? { ...emptyAnalysis(), proposedStateChanges: [{ entityId: entity.id, stateKind: 'item_availability', previousState: 'vorhanden', newState: 'fortgetragen', confidence: 0.92, evidenceExcerpt: input.passage.text, sourceReferenceId: source.id, reason: 'Deterministischer Fake-Provider' }] } : emptyAnalysis();
    });
    await new ManuscriptAnalysisController(repository, job.id, provider).start();
    expect(order).toEqual(['Zettel wird fortgetragen.', 'Malik wartet.']);
    expect(maximum).toBe(1);
    expect(seenDraftSizes).toEqual([0, 1]);
    expect((await repository.getManuscriptAnalysisJob(job.id)).status).toBe('awaiting_user_review');
    expect((await repository.listManuscriptAnalysisDraftLedger(job.id))[0]).toMatchObject({ status: 'proposed', newState: 'fortgetragen' });
  });

  it('speichert Fehler als failed, führt failed hashes erneut aus und verwendet completed hashes wieder', async () => {
    const repository = new BrowserDemoRepository();
    const { job } = await makeJob(repository, 'retry');
    let calls = 0;
    const provider = fakeProvider(async () => { calls += 1; if (calls === 1) throw new Error('Provider ausgefallen'); return emptyAnalysis(); });
    await expect(new ManuscriptAnalysisController(repository, job.id, provider).start()).rejects.toThrow('Provider ausgefallen');
    expect((await repository.listManuscriptAnalysisUnits(job.id))[0]?.status).toBe('failed');
    const controller = new ManuscriptAnalysisController(repository, job.id, provider);
    await controller.retryFailed();
    expect((await repository.listManuscriptAnalysisUnits(job.id))[0]?.retryCount).toBe(1);
    expect(calls).toBe(3);
    await controller.start();
    expect(calls).toBe(3);
  });

  it('setzt einen offenen Job nach einem App-Neustart fort und überspringt die unveränderte completed Einheit', async () => {
    const repository = new BrowserDemoRepository();
    const { job } = await makeJob(repository, 'restart');
    const units = await repository.listManuscriptAnalysisUnits(job.id);
    await repository.updateManuscriptAnalysisUnit({ id: units[0]!.id, status: 'completed', content: units[0]!.content, contentHash: contentHash(units[0]!.content) });
    await repository.updateManuscriptAnalysisJob({ id: job.id, status: 'paused' });
    const calls: string[] = [];
    const provider = fakeProvider(async (input) => { calls.push(input.passage.text); return emptyAnalysis(); });
    const restartedRepository = new BrowserDemoRepository();
    await new ManuscriptAnalysisController(restartedRepository, job.id, provider).start();
    expect(calls).toEqual(['Malik wartet.']);
  });

  it('markiert Abbruch als cancelled und lässt den Lauf danach wiederaufnehmen', async () => {
    const repository = new BrowserDemoRepository();
    const { job } = await makeJob(repository, 'cancel');
    let rejectActive: ((error: Error) => void) | undefined;
    const provider = fakeProvider(() => new Promise<ContinuityAnalysisResult>((_, reject) => { rejectActive = reject; }), vi.fn(async () => { rejectActive?.(new Error('abgebrochen')); }));
    const controller = new ManuscriptAnalysisController(repository, job.id, provider);
    const running = controller.start();
    await vi.waitFor(() => expect(rejectActive).toBeDefined());
    await controller.cancel();
    await expect(running).resolves.toBeUndefined();
    expect((await repository.getManuscriptAnalysisJob(job.id)).status).toBe('cancelled');
  });

  it('speichert Seitenmarker und berechnet Positionen als Unicode-Codepoints', () => {
    const chapter = { id: 'chapter', bookId: 'book', title: 'Kapitel', orderIndex: 1, scenes: [{ id: 'scene', chapterId: 'chapter', title: 'Text', orderIndex: 1, content: '', pov: '', location: '', storyTime: '', status: 'draft' as const, goal: '', notes: '' }] };
    const units = createManuscriptAnalysisUnits([chapter], [[{ text: '😀 Zettel', startOffset: 0, endOffset: 8, page: 1 }]], [{ id: 'scene', chapterId: 'chapter' }]);
    expect(units[0]?.endOffset).toBe(8);
  });

  it('enthält im Importworkflow keine verschluckten catch(() => undefined)-Fehler', () => {
    const app = Object.values(import.meta.glob('../App.tsx', { eager: true, query: '?raw', import: 'default' }) as Record<string, string>)[0] ?? '';
    expect(app).not.toContain('catch(() => undefined)');
  });

  it('führt alle Analysephasen lokal orchestriert aus und bestätigt nichts automatisch', async () => {
    const repository = new BrowserDemoRepository();
    const { job } = await makeJob(repository, 'multipass');
    const provider = fakeProvider(async () => emptyAnalysis());
    await new ManuscriptAnalysisController(repository, job.id, provider).start();
    const awaiting = await repository.getManuscriptAnalysisJob(job.id);
    expect(awaiting.currentPhase).toBe('user_review');
    expect(awaiting.status).toBe('awaiting_user_review');
    await new ManuscriptAnalysisController(repository, job.id, provider).completeUserReview(true);
    const completed = await repository.getManuscriptAnalysisJob(job.id);
    expect(completed.currentPhase).toBe('completed');
    expect(Object.entries(completed.phaseProgress).filter(([phase]) => phase !== 'user_review').every(([, progress]) => progress.status === 'completed')).toBe(true);
    expect((await repository.listNarrativeSummaries(completed.projectId)).every((summary) => summary.status === 'proposed' && !summary.authorConfirmed)).toBe(true);
    expect(await repository.listContinuityStateLedger(completed.projectId)).toEqual([]);
    const units = await repository.listManuscriptAnalysisUnits(completed.id);
    expect(units.every((unit) => unit.actualProvider === 'codex-cli' && unit.inputHash && unit.outputHash)).toBe(true);
  });

  it('persistiert Plot-, Endzustands- und Countercheck-Phasen getrennt und als jobgebundene Vorschläge', async () => {
    const repository = new BrowserDemoRepository(); const { job, source } = await makeJob(repository, 'structured-phases'); const workspace = await repository.loadWorkspace(); const thread = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Zettelfrage', type: 'plot_thread', description: 'Wer hinterließ den Zettel?', status: 'confirmed', confidence: 1, authorConfirmed: true, tags: [], excerpt: 'Zettelfrage' }); const provider = fakeProvider(async () => emptyAnalysis()); Object.assign(provider, { synthesizePlotThreads: vi.fn(async () => ({ summary: 'Spur bleibt offen.', openQuestions: ['Wer hinterließ den Zettel?'], threadGoals: ['Herkunft klären'], developments: ['Neue Spur'], closureCandidates: [], partiallyResolved: [], reopened: [], threadProposals: [{ entityId: thread.id, proposedStatus: 'closure_candidate' as const, evidenceExcerpt: 'Zettel wird fortgetragen.', sourceReferenceId: source.id, reason: 'Vorschlag', confidence: 0.7 }], warnings: [] })), analyzeBookEndState: vi.fn(async () => ({ summary: 'Endzustand', characterEndStates: ['Malik wartet.'], knowledgeStates: [], falseBeliefs: [], relationships: [], objectOwners: ['Zettel bei Malik'], injuries: [], locations: ['Zimmer'], openActions: ['Herkunft klären'], unresolvedThreads: ['Zettelfrage'], endStateProposals: [{ category: 'object_owner', entityId: undefined, statement: 'Zettel bei Malik', confidence: 0.6, evidenceExcerpt: 'Malik wartet.', sourceReferenceId: source.id }], warnings: [] })), globalCountercheck: vi.fn(async () => ({ summary: 'Gegenprüfung', contradictoryFacts: [], prematureKnowledge: [], lostOrDestroyedObjects: [], timeAndLocationConflicts: [], contradictoryRules: [], unclearExceptions: [], uncertainSources: [], countercheckFindings: [{ severity: 'warning' as const, category: 'object_state', objectiveConflict: 'Status offen', reason: 'Prüfen', confidence: 0.5, evidenceExcerpt: 'Zettel wird fortgetragen.', sourceReferenceId: source.id }], warnings: [] })) }); await new ManuscriptAnalysisController(repository, job.id, provider).start(); const phases = await repository.listManuscriptAnalysisPhaseResults(job.id); expect(phases.map((phase) => phase.phase)).toEqual(expect.arrayContaining(['narrative_summaries', 'plot_thread_synthesis', 'book_end_state', 'global_countercheck'])); const artifacts = await repository.listManuscriptAnalysisArtifacts(job.id); expect(artifacts.map((artifact) => artifact.artifactType)).toEqual(expect.arrayContaining(['plot_thread_proposal', 'book_end_state_proposal', 'global_countercheck_finding'])); expect((await repository.listPlotThreadLifecycleProposals(workspace.project.id)).every((proposal) => proposal.reviewStatus === 'pending')).toBe(true); expect((await repository.listContinuityReviewFindings(workspace.project.id)).some((finding) => finding.objectiveConflict === 'Status offen')).toBe(true);
  });

  it('führt die Passage-Aufgaben pro Einheit chronologisch aus und übergibt keinen Zukunftstext', async () => {
    const repository = new BrowserDemoRepository();
    const { job, entity } = await makeJob(repository, 'chronological-pipeline');
    await repository.saveProvisionalEntity({ id: 'future-provisional', jobId: job.id, projectId: job.projectId, entityType: 'character', canonicalName: 'Später genannt', aliases: [], description: 'Nur auf Seite 9', confidence: 0.9, reviewStatus: 'proposed' });
    const calls: string[] = [];
    const seenContinuity: ContinuityAnalysisInput[] = [];
    const provider = fakeProvider(async (input) => { calls.push(`continuity:${input.passage.text}`); seenContinuity.push(input); return emptyAnalysis(); });
    Object.assign(provider, {
      resolveManuscriptEntityMentions: vi.fn(async (input) => { calls.push(`mentions:${input.passageText}`); expect(input.previousProvisionalEntities.some((item: { canonicalName: string }) => item.canonicalName === 'Später genannt')).toBe(false); return { entities: [], mentions: [], relations: [], events: [], mergeProposals: [], warnings: [] }; }),
      extractBiblePatch: vi.fn(async (input) => { calls.push(`bible:${input.scene.content}`); expect(input.chapter.scenes).toHaveLength(1); expect(input.scene.content).toBe(input.chapter.scenes[0]?.content); return { proposals: [], warnings: [] }; }),
      extractCharacterMemoryPatch: vi.fn(async (input) => { calls.push(`memory:${input.scene.content}`); if (input.scene.content.startsWith('Zettel')) expect(input.scene.content).not.toContain('Malik wartet.'); return { proposals: [], warnings: [] }; }),
    });
    await new ManuscriptAnalysisController(repository, job.id, provider).start();
    expect(calls.slice(0, 8)).toEqual([
      'mentions:Zettel wird fortgetragen.', 'bible:Zettel wird fortgetragen.', 'memory:Zettel wird fortgetragen.', 'continuity:Zettel wird fortgetragen.',
      'mentions:Malik wartet.', 'bible:Malik wartet.', 'memory:Malik wartet.', 'continuity:Malik wartet.',
    ]);
    expect(seenContinuity.every((input) => input.followingContext === '')).toBe(true);
    expect(seenContinuity[0]?.confirmedStoryBible.some((item) => item.id === entity.id)).toBe(true);
  });

  it('nimmt bei einer späteren Einheit nur bestätigte und frühere Zustände in den Request', async () => {
    const repository = new BrowserDemoRepository();
    const { job } = await makeJob(repository, 'temporal-context');
    const workspace = await repository.loadWorkspace();
    const future = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Spätere Figur', type: 'character', description: 'Erst auf Seite 9', status: 'confirmed', confidence: 1, authorConfirmed: true, tags: [], excerpt: 'Spätere Figur' });
    await repository.createSourceReference({ projectId: workspace.project.id, entityId: future.id, chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, excerpt: 'Malik wartet.', startOffset: 29, endOffset: 42 });
    const seen: ContinuityAnalysisInput[] = [];
    const provider = fakeProvider(async (input) => { seen.push(input); return emptyAnalysis(); });
    Object.assign(provider, { resolveManuscriptEntityMentions: vi.fn(async () => ({ entities: [], mentions: [], relations: [], events: [], mergeProposals: [], warnings: [] })) });
    await new ManuscriptAnalysisController(repository, job.id, provider).start();
    expect(seen[0]?.confirmedStoryBible.some((item) => item.id === future.id)).toBe(false);
    expect(seen[0]?.relevantSources.some((source) => source.entityId === future.id)).toBe(false);
  });

  it('setzt bei einer früheren Textänderung die aktuelle und alle späteren Einheiten zurück', async () => {
    const repository = new BrowserDemoRepository();
    const { job } = await makeJob(repository, 'invalidate-later');
    const initial: string[] = [];
    const provider = fakeProvider(async (input) => { initial.push(input.passage.text); return emptyAnalysis(); });
    const controller = new ManuscriptAnalysisController(repository, job.id, provider);
    await controller.start();
    expect(initial).toHaveLength(2);
    const workspace = await repository.loadWorkspace();
    const scene = workspace.chapters[0]!.scenes[0]!;
    await repository.updateScene({ ...scene, content: 'Zettel wird fortgetragen!\n\nMalik wartet.' });
    const rerun: string[] = [];
    const rerunProvider = fakeProvider(async (input) => { rerun.push(input.passage.text); return emptyAnalysis(); });
    await new ManuscriptAnalysisController(repository, job.id, rerunProvider).start();
    expect(rerun).toEqual(['Zettel wird fortgetragen!', 'Malik wartet.']);
    expect((await repository.listManuscriptAnalysisUnits(job.id)).every((unit) => unit.status === 'completed')).toBe(true);
  });
});
