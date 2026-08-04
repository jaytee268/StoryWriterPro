import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDemoRepository } from './storyRepository';
import { buildContinuityPrefilter, detectContinuityFindings, runContinuityReview, shouldRunContinuityReview } from './continuityReview';
import type { ContinuityAnalysisResult } from '../types/domain';
import type { StoryAiProvider } from './aiProviderService';

const values = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value), removeItem: (key: string) => values.delete(key) });

const emptyAnalysis = (): ContinuityAnalysisResult => ({ observedActions: [], proposedStateChanges: [], objectiveContradictions: [], missingExplanations: [], matchedLoreRules: [], newRuleProposals: [], plotThreadChanges: [], confidence: 0, evidence: [], warnings: [] });
function fakeProvider(analyze: (input: Parameters<StoryAiProvider['analyzeContinuityPassage']>[0]) => ContinuityAnalysisResult): StoryAiProvider {
  return { id: 'codex-cli', analyzeContinuityPassage: vi.fn(async (input) => analyze(input)), getStatus: vi.fn(), extractBiblePatch: vi.fn(), extractCharacterMemoryPatch: vi.fn(), analyzeProjectStyle: vi.fn(), summarize: vi.fn(), answerWithProjectContext: vi.fn(), cancel: vi.fn(), cancelActive: vi.fn() } as unknown as StoryAiProvider;
}

describe('AI-gestützte semantische Continuity', () => {
  beforeEach(() => values.clear());

  it('löst bestätigte Zustände an Manuskriptpositionen auf und schließt Zukunft aus', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const entity = workspace.entities.find((item) => item.type === 'clue')!;
    const earlyChapter = workspace.chapters[0]!;
    const futureChapter = workspace.chapters[2]!;
    await repository.saveContinuityStateEntry({ projectId: workspace.project.id, entityId: entity.id, stateKind: 'item_availability', previousState: '', newState: 'verfügbar', chapterId: earlyChapter.id, sceneId: earlyChapter.scenes[0]!.id, status: 'confirmed', confidence: 1, authorConfirmed: true });
    await repository.saveContinuityStateEntry({ projectId: workspace.project.id, entityId: entity.id, stateKind: 'item_availability', previousState: 'verfügbar', newState: 'später verändert', chapterId: futureChapter.id, sceneId: futureChapter.scenes[0]!.id, status: 'confirmed', confidence: 1, authorConfirmed: true });
    const provider = fakeProvider((input) => { expect(input.continuityStatesBeforePosition.some((state) => state.newState === 'später verändert')).toBe(false); return emptyAnalysis(); });
    await runContinuityReview(repository, { project: workspace.project, chapter: earlyChapter, scene: earlyChapter.scenes[0], currentText: `${entity.name} erscheint in einer indirekten Formulierung.`, sourceKind: 'manual', provider });
  });

  it('behandelt semantische Paraphrasen des weggeworfenen Zettels durch den Provider', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const entity = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Zettel', type: 'object', description: '', status: 'confirmed', confidence: 1, chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, excerpt: '', authorConfirmed: true, tags: [] });
    await repository.saveContinuityStateEntry({ projectId: workspace.project.id, entityId: entity.id, stateKind: 'item_availability', previousState: 'verfügbar', newState: 'weggeworfen', chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, status: 'confirmed', confidence: 1, authorConfirmed: true });
    const provider = fakeProvider((input) => input.passage.text.includes('Jackentasche') ? { ...emptyAnalysis(), objectiveContradictions: [{ findingType: 'probable_contradiction', subjectEntityId: entity.id, relatedEntityIds: [entity.id], relatedStateIds: ['missing-state'], objectiveConflict: 'Ein zuvor entsorgter Gegenstand taucht wieder auf.', evidenceExcerpt: input.passage.text, counterEvidenceExcerpts: ['Der Zettel wurde zuvor entsorgt.'], confidence: 0.91, reason: 'Die AI erkennt die Zustandsbeziehung trotz anderer Formulierungen.' }] } : emptyAnalysis());
    const result = await runContinuityReview(repository, { project: workspace.project, chapter: workspace.chapters[0], scene: workspace.chapters[0]!.scenes[0], currentText: 'Den Zettel zog er später wieder aus der Jackentasche.', sourceKind: 'manual', provider });
    expect(result.findings[0]).toMatchObject({ findingType: 'probable_contradiction', confidence: 0.91 });
    expect(result.findings[0]?.counterEvidenceExcerpts).toEqual(['Der Zettel wurde zuvor entsorgt.']);
  });

  it('ordnet bestätigte Lore-Regeln semantisch zu, ohne den objektiven Konflikt zu entfernen', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const entity = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Beweis', type: 'object', description: '', status: 'confirmed', confidence: 1, chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, excerpt: '', authorConfirmed: true, tags: [] });
    await repository.saveContinuityStateEntry({ projectId: workspace.project.id, entityId: entity.id, stateKind: 'item_existence', previousState: '', newState: 'nicht vorhanden', chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, status: 'confirmed', confidence: 1, authorConfirmed: true });
    const rule = await repository.saveProjectRule({ projectId: workspace.project.id, title: 'Veränderliche Beweise', statement: 'Physische Beweise können sich unter einer bestätigten Bedingung verändern.', scope: 'project', prerequisites: [], effects: [], exceptions: [], connectedLoreIds: [], sourceReferenceIds: [], status: 'confirmed', confidence: 1, authorConfirmed: true, origin: 'manual' });
    const provider = fakeProvider(() => ({ ...emptyAnalysis(), objectiveContradictions: [{ findingType: 'lore_compatible_anomaly', subjectEntityId: entity.id, relatedEntityIds: [entity.id], relatedStateIds: [], objectiveConflict: 'Der frühere Zustand widerspricht der aktuellen Erscheinung.', evidenceExcerpt: 'Der Beweis liegt wieder vor.', counterEvidenceExcerpts: ['Der Beweis war nicht vorhanden.'], confidence: 0.8, reason: 'Die Regel ist eine mögliche, aber noch zu bestätigende Erklärung.' }], matchedLoreRules: [{ ruleId: rule.id, rationale: 'Die Regel beschreibt genau eine mögliche Zustandsveränderung.', confidence: 0.78 }] }));
    const result = await runContinuityReview(repository, { project: workspace.project, chapter: workspace.chapters[0], scene: workspace.chapters[0]!.scenes[0], currentText: 'Der Beweis lag wieder auf dem Tisch.', sourceKind: 'manual', provider });
    expect(result.findings[0]?.loreExplanations[0]).toContain('Veränderliche Beweise');
    expect(result.findings[0]?.severity).toBe('warning');
  });

  it('behandelt indirekte Milchformulierungen als Autorentscheidung statt als lokalen harten Fehler', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const malik = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Malik', type: 'character', description: '', status: 'confirmed', confidence: 1, chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, excerpt: '', authorConfirmed: true, tags: [] });
    await repository.saveContinuityStateEntry({ projectId: workspace.project.id, entityId: malik.id, stateKind: 'physical_condition', previousState: '', newState: 'laktoseintolerant', chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, status: 'confirmed', confidence: 1, authorConfirmed: true });
    const provider = fakeProvider((input) => ({ ...emptyAnalysis(), missingExplanations: [{ findingType: 'missing_explanation', subjectEntityId: malik.id, relatedEntityIds: [malik.id], relatedStateIds: [], objectiveConflict: 'Eine bestätigte körperliche Eigenschaft steht neben einer möglichen Milchaufnahme.', evidenceExcerpt: input.passage.text, counterEvidenceExcerpts: [], confidence: 0.55, reason: 'Medizinische oder produktspezifische Ausnahme möglich; Autorentscheidung erforderlich.' }] }));
    const texts = ['Malik trinkt Milch.', 'Malik leert seinen Milchkaffee.', 'Er nimmt einen großen Schluck aus dem Glas.', 'Malik bestellt einen Cappuccino.'];
    for (const text of texts) { const result = await runContinuityReview(repository, { project: workspace.project, chapter: workspace.chapters[0], scene: workspace.chapters[0]!.scenes[0], currentText: text, sourceKind: 'manual', provider }); expect(result.findings[0]?.findingType).toBe('missing_explanation'); }
  });

  it('speichert Zustandsänderungen automatisch als unbestätigte Ledger-Vorschläge', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const entity = workspace.entities.find((item) => item.type === 'object') ?? workspace.entities[0]!;
    const provider = fakeProvider(() => ({ ...emptyAnalysis(), proposedStateChanges: [{ entityId: entity.id, stateKind: 'location', previousState: '', newState: 'Archiv', confidence: 0.93, evidenceExcerpt: 'Der Gegenstand liegt im Archiv.', reason: 'AI beobachtet einen Ortswechsel.' }] }));
    const result = await runContinuityReview(repository, { project: workspace.project, chapter: workspace.chapters[0], scene: workspace.chapters[0]!.scenes[0], currentText: 'Der Gegenstand liegt im Archiv.', sourceKind: 'manual', provider });
    expect(result.stateProposals[0]).toMatchObject({ newState: 'Archiv', status: 'proposed', authorConfirmed: false });
    expect(await repository.getStateAtPosition(workspace.project.id, entity.id, 'location', { chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id })).toBeUndefined();
  });

  it('erzeugt einen Plot-Thread-Kandidaten ohne resolved automatisch zu setzen', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const thread = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Austausch des Pakets', type: 'plot_thread', description: 'Wer hat das Paket ausgetauscht?', status: 'confirmed', confidence: 1, chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, excerpt: '', authorConfirmed: true, tags: [] });
    const provider = fakeProvider(() => ({ ...emptyAnalysis(), plotThreadChanges: [{ entityId: thread.id, proposedStatus: 'closure_candidate', evidenceExcerpt: 'Daniel erkannte den Fahrer auf der Aufnahme.', reason: 'Die zentrale Frage könnte teilweise beantwortet sein.', confidence: 0.86 }] }));
    await runContinuityReview(repository, { project: workspace.project, chapter: workspace.chapters[0], scene: workspace.chapters[0]!.scenes[0], currentText: 'Daniel erkannte den Fahrer auf der Aufnahme.', sourceKind: 'manual', provider });
    expect(await repository.listPlotThreadLifecycleProposals(workspace.project.id)).toEqual(expect.arrayContaining([expect.objectContaining({ entityId: thread.id, proposedStatus: 'closure_candidate', reviewStatus: 'pending' })]));
    expect(await repository.listPlotThreadLifecycles(workspace.project.id)).not.toContainEqual(expect.objectContaining({ entityId: thread.id, lifecycleStatus: 'resolved' }));
  });

  it('lässt lokale Heuristiken allein keine semantische Entscheidung treffen', () => {
    const entity = { id: 'zettel', projectId: 'p', name: 'Zettel', type: 'object' as const, description: '', status: 'confirmed' as const, confidence: 1, source: '', chapter: '', scene: '', authorConfirmed: true, updatedAt: '', tags: [], origin: 'manual' as const };
    const chapter = { id: 'c', bookId: 'b', title: 'Kapitel', orderIndex: 1, scenes: [{ id: 's', chapterId: 'c', title: 'Szene', orderIndex: 1, content: '', pov: '', location: '', storyTime: '', status: 'draft' as const, goal: '', notes: '' }] };
    const state = { id: 'state', projectId: 'p', entityId: entity.id, stateKind: 'item_availability' as const, previousState: '', newState: 'weggeworfen', chapterId: 'c', sceneId: 's', status: 'confirmed' as const, confidence: 1, authorConfirmed: true, createdAt: '', updatedAt: '' };
    expect(detectContinuityFindings({ project: { id: 'p', title: 'Test', author: '', description: '', updatedAt: '', wordCount: 0, openWarnings: 0, bibleProgress: 0 }, chapter, scene: chapter.scenes[0], chapters: [chapter], entities: [entity], ledger: [state], rules: [], currentText: 'Das Papier landete im Müll und taucht später wieder auf.', sourceKind: 'manual' })).toEqual([]);
    expect(buildContinuityPrefilter({ project: { id: 'p', title: 'Test', author: '', description: '', updatedAt: '', wordCount: 0, openWarnings: 0, bibleProgress: 0 }, chapter, scene: chapter.scenes[0], chapters: [chapter], entities: [entity], ledger: [state], rules: [], currentText: 'Zettel', sourceKind: 'manual' }).candidateEntityIds).toEqual(['zettel']);
  });

  it('startet Wortschwellen nur nach der konfigurierten Menge und Seitenprüfungen sofort', () => {
    expect(shouldRunContinuityReview('Ein kurzer Anfang.', `${'neues '.repeat(20)}Ende.`, 300, 'word_threshold')).toBe(false);
    expect(shouldRunContinuityReview('Ein kurzer Anfang.', `${'neues '.repeat(301)}Ende.`, 300, 'word_threshold')).toBe(true);
    expect(shouldRunContinuityReview('Ein kurzer Anfang.', 'Seitenmarker-Prüfung.', 300, 'page_marker')).toBe(true);
  });
});
