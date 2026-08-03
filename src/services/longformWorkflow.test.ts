import { describe, expect, it } from 'vitest';
import { BrowserLongformRepository } from './longformRepository';
import { buildPreflight, parseLongformIntent, targetWords } from './longformWorkflow';

describe('guided long-form workflow', () => {
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

  it('reports an open ending as a visible preparation item', () => {
    const preferences = { projectId: 'p', wordsPerPage: 250, preferredSectionWords: 850, maximumSectionWords: 1200, defaultSceneCount: 4, requirePlanConfirmation: true, requireFinalConfirmation: true, createdAt: '', updatedAt: '' };
    const preflight = buildPreflight({ id: 'p', title: 'Test', author: '', description: '', updatedAt: '', wordCount: 0, openWarnings: 0, bibleProgress: 0 }, [{ id: 'c', bookId: 'b', title: 'Kapitel 1', orderIndex: 1, scenes: [] }], [], undefined, preferences, { requested: true, instruction: 'Schreib das nächste Kapitel' });
    expect(preflight.items.some((item) => item.label === 'Story-Richtung')).toBe(true);
    expect(preflight.canPlan).toBe(true);
  });
});
