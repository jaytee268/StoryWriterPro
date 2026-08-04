import { describe, expect, it, vi } from 'vitest';
import { BrowserLongformRepository } from './longformRepository';
import { buildPreflight, cancelLongformContinuityAnalysis, parseLongformIntent, targetWords } from './longformWorkflow';
import { contentHash } from '../utils/aiText';

describe('guided long-form workflow', () => {
  it('cancels the active continuity provider before accepting a newer edit', async () => {
    const cancelActive = vi.fn(async () => undefined);
    const token = { cancelled: false };
    await cancelLongformContinuityAnalysis({ cancelActive }, token);
    expect(token.cancelled).toBe(true);
    expect(cancelActive).toHaveBeenCalledOnce();
  });
  it('recognizes explicit chapter requests but not normal questions', () => {
    expect(parseLongformIntent('Schreib mir das nächste Kapitel mit ungefähr 17 Seiten.')).toMatchObject({ requested: true, pages: 17 });
    expect(parseLongformIntent('Welche Figuren kommen in Kapitel 3 vor?').requested).toBe(false);
  });

  it('converts pages using project preferences', () => {
    expect(targetWords({ requested: true, instruction: '17 Seiten', pages: 17 }, { projectId: 'p', wordsPerPage: 250, preferredSectionWords: 850, maximumSectionWords: 1200, defaultSceneCount: 4, requirePlanConfirmation: true, requireFinalConfirmation: true, createdAt: '', updatedAt: '' })).toBe(4250);
  });

  it('persists direction, jobs, plans and sections in browser demo isolation', async () => {
    const values = new Map<string, string>();
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: (name: string) => values.get(name) ?? null, setItem: (name: string, value: string) => values.set(name, value), clear: () => values.clear() } });
    const repository = new BrowserLongformRepository();
    const direction = await repository.saveStoryDirection({ projectId: 'p', premise: 'Mystery', currentStoryPhase: '', bookGoal: '', plannedEnding: '', endingStatus: 'open', centralTwist: '', thematicGoal: '', mustHappen: [], mustNotHappen: [], nextTurningPoint: '', revealConstraints: [], authorNotes: '' });
    expect(direction.premise).toBe('Mystery');
    const job = await repository.createJob({ projectId: 'p', targetBookId: 'b', targetWords: 850, userInstruction: 'Schreib eine Szene', activeProvider: 'local-prototype', contentContextHash: 'hash' });
    expect((await repository.listJobs('p'))[0].id).toBe(job.id);
    await repository.savePlan({ jobId: job.id, chapterTitle: 'Kapitel 4', chapterGoal: 'Ziel', povCharacterId: undefined, startingState: '', endingState: '', chapterSummary: '', endingConnection: '', newInformation: [], withheldInformation: [], beats: [{ id: 'beat-1', orderIndex: 0, title: 'Impuls', purpose: 'Start', participatingCharacterIds: [], startingState: '', event: '', conflict: '', newInformation: [], knowledgeChanges: [], relationshipChanges: [], cluesUsed: [], loreEntityIds: [], endingHook: '', targetWords: 850 }], reviewStatus: 'pending' });
    expect((await repository.getPlan(job.id))?.chapterTitle).toBe('Kapitel 4');
    await repository.saveSection({ jobId: job.id, planBeatId: 'beat-1', orderIndex: 0, targetWords: 850, content: 'Manueller Entwurf.', continuationSummary: '', continuityState: { currentLocation: '', currentStoryTime: '', presentCharacterIds: [], characterStates: [], establishedFacts: [], knowledgeChanges: [], relationshipChanges: [], movedObjects: [], injuries: [], cluesIntroduced: [], promisesCreated: [], unresolvedActions: [], lastParagraphSummary: '' }, status: 'generated' });
    expect((await repository.listSections(job.id))[0].actualWords).toBe(2);
  });

  it('rekonstruiert den Longform-Draft-Ledger nach Neustart und invalidiert Folgeabschnitte', async () => {
    const values = new Map<string, string>();
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: (name: string) => values.get(name) ?? null, setItem: (name: string, value: string) => values.set(name, value), clear: () => values.clear() } });
    values.set('storymemory-browser-demo-workspace', JSON.stringify({ project: { id: 'p' }, books: [{ id: 'b', projectId: 'p' }], chapters: [], entities: [{ id: 'entity', projectId: 'p' }], sources: [], continuityLedger: [] }));
    const repository = new BrowserLongformRepository();
    const job = await repository.createJob({ projectId: 'p', targetBookId: 'b', targetWords: 1200, userInstruction: 'Schreib', activeProvider: 'codex-cli', contentContextHash: 'ctx' });
    await repository.savePlan({ jobId: job.id, chapterTitle: 'Kapitel', chapterGoal: 'Ziel', povCharacterId: undefined, startingState: '', endingState: '', chapterSummary: '', endingConnection: '', newInformation: [], withheldInformation: [], beats: [0, 1, 2].map((orderIndex) => ({ id: `beat-${orderIndex}`, orderIndex, title: `Abschnitt ${orderIndex}`, purpose: '', participatingCharacterIds: [], startingState: '', event: '', conflict: '', newInformation: [], knowledgeChanges: [], relationshipChanges: [], cluesUsed: [], loreEntityIds: [], endingHook: '', targetWords: 400 })), reviewStatus: 'accepted' });
    const sections = [];
    for (const orderIndex of [0, 1, 2]) sections.push(await repository.saveSection({ jobId: job.id, planBeatId: `beat-${orderIndex}`, orderIndex, targetWords: 400, content: `Text ${orderIndex}`, continuationSummary: '', continuityState: { currentLocation: '', currentStoryTime: '', presentCharacterIds: [], characterStates: [], establishedFacts: [], knowledgeChanges: [], relationshipChanges: [], movedObjects: [], injuries: [], cluesIntroduced: [], promisesCreated: [], unresolvedActions: [], lastParagraphSummary: '' }, status: 'generated', draftState: 'valid' }));
    await repository.replaceDraftLedger(sections[0]!.id, [{ jobId: job.id, sectionId: sections[0]!.id, projectId: 'p', entityId: 'entity', stateKind: 'item_availability', previousState: 'vorhanden', newState: 'zerstört', sourceExcerpt: 'Text 0', sourceStartOffset: 0, sourceEndOffset: 6, contentHash: contentHash('Text 0'), confidence: 0.9 }]);
    await repository.replaceDraftLedger(sections[1]!.id, [{ jobId: job.id, sectionId: sections[1]!.id, projectId: 'p', entityId: 'entity', stateKind: 'item_availability', previousState: 'zerstört', newState: 'gefunden', sourceExcerpt: 'Text 1', sourceStartOffset: 0, sourceEndOffset: 6, contentHash: contentHash('Text 1'), confidence: 0.8 }]);
    const restarted = new BrowserLongformRepository();
    expect((await restarted.listDraftLedger(job.id)).map((entry) => entry.newState)).toEqual(['zerstört', 'gefunden']);
    await restarted.supersedeDraftLedgerFrom(job.id, 1);
    expect((await restarted.listDraftLedger(job.id)).map((entry) => entry.status)).toEqual(['proposed', 'superseded']);
    const stale = await restarted.saveSection({ ...sections[1]!, draftState: 'stale' });
    expect(stale.draftState).toBe('stale');
  });

  it('blockiert die Übernahme bei stale oder geändertem Abschnitt und übernimmt Vorschläge erst atomar', async () => {
    const values = new Map<string, string>();
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: (name: string) => values.get(name) ?? null, setItem: (name: string, value: string) => values.set(name, value), clear: () => values.clear() } });
    values.set('storymemory-browser-demo-workspace', JSON.stringify({ project: { id: 'p' }, books: [{ id: 'b', projectId: 'p' }], chapters: [], entities: [{ id: 'entity', projectId: 'p' }], sources: [], continuityLedger: [] }));
    const repository = new BrowserLongformRepository();
    const job = await repository.createJob({ projectId: 'p', targetBookId: 'b', targetWords: 400, userInstruction: 'Schreib', activeProvider: 'codex-cli', contentContextHash: 'ctx' });
    await repository.savePlan({ jobId: job.id, chapterTitle: 'Kapitel', chapterGoal: 'Ziel', povCharacterId: undefined, startingState: '', endingState: '', chapterSummary: '', endingConnection: '', newInformation: [], withheldInformation: [], beats: [{ id: 'beat', orderIndex: 0, title: 'Abschnitt', purpose: '', participatingCharacterIds: [], startingState: '', event: '', conflict: '', newInformation: [], knowledgeChanges: [], relationshipChanges: [], cluesUsed: [], loreEntityIds: [], endingHook: '', targetWords: 400 }], reviewStatus: 'accepted' });
    const section = await repository.saveSection({ jobId: job.id, planBeatId: 'beat', orderIndex: 0, targetWords: 400, content: 'Text', continuationSummary: '', continuityState: { currentLocation: '', currentStoryTime: '', presentCharacterIds: [], characterStates: [], establishedFacts: [], knowledgeChanges: [], relationshipChanges: [], movedObjects: [], injuries: [], cluesIntroduced: [], promisesCreated: [], unresolvedActions: [], lastParagraphSummary: '' }, status: 'generated', draftState: 'stale' });
    await repository.updateJobStatus(job.id, 'draft_ready');
    await expect(repository.acceptJob({ jobId: job.id, currentContextHash: 'ctx' })).rejects.toThrow();
    await repository.saveSection({ ...section, draftState: 'valid', contentHash: 'invalid' });
    await expect(repository.acceptJob({ jobId: job.id, currentContextHash: 'ctx' })).rejects.toThrow();
    const valid = await repository.saveSection({ ...section, draftState: 'valid', contentHash: contentHash('Text') });
    await repository.replaceDraftLedger(valid.id, [{ jobId: job.id, sectionId: valid.id, projectId: 'p', entityId: 'entity', stateKind: 'item_existence', previousState: '', newState: 'vorhanden', sourceExcerpt: 'Text', sourceStartOffset: 0, sourceEndOffset: 4, contentHash: contentHash('Text'), confidence: 1 }]);
    const accepted = await repository.acceptJob({ jobId: job.id, currentContextHash: 'ctx' });
    expect(accepted.status).toBe('accepted');
    const workspace = JSON.parse(values.get('storymemory-browser-demo-workspace')!);
    expect(workspace.continuityLedger[0].status).toBe('proposed');
  });

  it('reports an open ending as a visible preparation item', () => {
    const preferences = { projectId: 'p', wordsPerPage: 250, preferredSectionWords: 850, maximumSectionWords: 1200, defaultSceneCount: 4, requirePlanConfirmation: true, requireFinalConfirmation: true, createdAt: '', updatedAt: '' };
    const preflight = buildPreflight({ id: 'p', title: 'Test', author: '', description: '', updatedAt: '', wordCount: 0, openWarnings: 0, bibleProgress: 0 }, [{ id: 'c', bookId: 'b', title: 'Kapitel 1', orderIndex: 1, scenes: [] }], [], undefined, preferences, { requested: true, instruction: 'Schreib das nächste Kapitel' });
    expect(preflight.items.some((item) => item.label === 'Story-Richtung')).toBe(true);
    expect(preflight.canPlan).toBe(true);
  });
});
