import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDemoRepository } from './storyRepository';
import { buildContinuityPrefilter, detectContinuityFindings, runContinuityReview, shouldRunContinuityReview } from './continuityReview';
import type { ContinuityAnalysisResult } from '../types/domain';
import { continuityResultSchema, normalizeContinuityResultNulls } from './aiProviderService';
import type { StoryAiProvider } from './aiProviderService';

const values = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value), removeItem: (key: string) => values.delete(key) });

const emptyAnalysis = (): ContinuityAnalysisResult => ({ observedActions: [], proposedStateChanges: [], objectiveContradictions: [], missingExplanations: [], matchedLoreRules: [], newRuleProposals: [], plotThreadChanges: [], confidence: 0, evidence: [], warnings: [] });
function fakeProvider(analyze: (input: Parameters<StoryAiProvider['analyzeContinuityPassage']>[0]) => ContinuityAnalysisResult): StoryAiProvider {
  return { id: 'codex-cli', analyzeContinuityPassage: vi.fn(async (input) => analyze(input)), getStatus: vi.fn(), extractBiblePatch: vi.fn(), extractCharacterMemoryPatch: vi.fn(), analyzeProjectStyle: vi.fn(), summarize: vi.fn(), answerWithProjectContext: vi.fn(), cancel: vi.fn(), cancelActive: vi.fn() } as unknown as StoryAiProvider;
}

describe('AI-gestützte semantische Continuity', () => {
  beforeEach(() => values.clear());

  it('akzeptiert den gemeinsamen Nullable-Fixture-Datensatz', () => {
    const fixture = { observedActions: [{ summary: 'Beobachtung', evidenceExcerpt: '', entityIds: [], startOffset: null, endOffset: null }], proposedStateChanges: [{ entityId: 'entity', relatedEntityId: null, stateKind: 'location', previousState: '', newState: 'unbekannt', confidence: 0.5, evidenceExcerpt: '', sourceReferenceId: null, startOffset: null, endOffset: null, reason: '' }], objectiveContradictions: [{ findingType: 'missing_explanation', subjectEntityId: null, relatedEntityIds: [], relatedStateIds: [], objectiveConflict: '', evidenceExcerpt: '', sourceReferenceId: null, counterEvidenceExcerpts: [], counterEvidence: null, confidence: 0.5, startOffset: null, endOffset: null, reason: '' }], missingExplanations: [], matchedLoreRules: [], newRuleProposals: [{ projectId: 'project', targetRuleId: null, title: '', statement: '', scope: 'project', prerequisites: [], effects: [], exceptions: [], connectedLoreIds: [], sourceReferenceIds: [], evidenceExcerpt: '', chapterId: null, sceneId: null, startOffset: null, endOffset: null, confidence: 0.5, reason: '' }], plotThreadChanges: [{ entityId: 'entity', proposedStatus: 'open', evidenceExcerpt: '', sourceReferenceId: null, startOffset: null, endOffset: null, reason: '', confidence: 0.5 }], confidence: 0.5, evidence: [{ id: 'evidence', label: '', chapterId: null, sceneId: null, entityId: null, excerpt: null, sourceReferenceId: null, startOffset: null, endOffset: null }], warnings: [] };
    expect(normalizeContinuityResultNulls(continuityResultSchema.parse(fixture) as ContinuityAnalysisResult).objectiveContradictions[0]?.subjectEntityId).toBeUndefined();
  });

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
    const state = await repository.saveContinuityStateEntry({ projectId: workspace.project.id, entityId: entity.id, stateKind: 'item_availability', previousState: 'verfügbar', newState: 'weggeworfen', chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, status: 'confirmed', confidence: 1, authorConfirmed: true });
    const provider = fakeProvider((input) => input.passage.text.includes('Jackentasche') ? { ...emptyAnalysis(), objectiveContradictions: [{ findingType: 'probable_contradiction', subjectEntityId: entity.id, relatedEntityIds: [entity.id], relatedStateIds: [state.id], objectiveConflict: 'Ein zuvor entsorgter Gegenstand taucht wieder auf.', evidenceExcerpt: input.passage.text, counterEvidenceExcerpts: ['Der Zettel wurde zuvor entsorgt.'], confidence: 0.91, reason: 'Die AI erkennt die Zustandsbeziehung trotz anderer Formulierungen.' }] } : emptyAnalysis());
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

  it('speichert passage-relative Provider-Offsets als absolute Unicode-Positionen und legt eine Quelle an', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const entity = workspace.entities.find((item) => item.type === 'object') ?? workspace.entities[0]!;
    const provider = fakeProvider(() => ({ ...emptyAnalysis(), proposedStateChanges: [{ entityId: entity.id, stateKind: 'location', previousState: '', newState: 'Archiv', confidence: 0.9, evidenceExcerpt: 'Notiz', startOffset: 3, endOffset: 8, reason: 'Belegter Ortswechsel.' }] }));
    const result = await runContinuityReview(repository, { project: workspace.project, chapter: workspace.chapters[0], scene: workspace.chapters[0]!.scenes[0]!, currentText: '🧩̈ Notiz liegt hier.', startOffset: 40, endOffset: 61, sourceKind: 'manual', provider });
    expect(result.stateProposals[0]).toMatchObject({ startOffset: 43, endOffset: 48, evidenceExcerpt: 'Notiz' });
    expect(result.stateProposals[0]?.sourceReferenceId).toBeTruthy();
    expect((await repository.listSourceReferences(workspace.project.id)).some((source) => source.startOffset === 43 && source.endOffset === 48)).toBe(true);
  });

  it('übernimmt historisches Figurenwissen nur bis zur Zielszene', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const character = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Mira', type: 'character', description: '', status: 'confirmed', confidence: 1, chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, excerpt: '', authorConfirmed: true, tags: [] });
    const fact = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Die Mappe', type: 'fact', description: '', status: 'confirmed', confidence: 1, chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, excerpt: '', authorConfirmed: true, tags: [] });
    await repository.saveCharacterKnowledgeState({ projectId: workspace.project.id, characterId: character.id, factEntityId: fact.id, knowledgeState: 'knows', acquiredSceneId: workspace.chapters[1]!.scenes[0]!.id, changedSceneId: workspace.chapters[1]!.scenes[0]!.id, certainty: 1, notes: '', status: 'confirmed', authorConfirmed: true });
    const provider = fakeProvider((input) => { expect(input.characterKnowledge.some((state) => state.factEntityId === fact.id)).toBe(false); return emptyAnalysis(); });
    await runContinuityReview(repository, { project: workspace.project, chapter: workspace.chapters[0], scene: workspace.chapters[0]!.scenes[0]!, currentText: 'Mira blickt zur Tür.', sourceKind: 'manual', provider });
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

  it('markiert Textkorrektur erst nach einer erfolgreichen erneuten Analyse als gelöst', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const entity = workspace.entities[0]!;
    let calls = 0;
    const provider = fakeProvider(() => calls++ === 0 ? { ...emptyAnalysis(), objectiveContradictions: [{ findingType: 'probable_contradiction', subjectEntityId: entity.id, relatedEntityIds: [entity.id], relatedStateIds: [], objectiveConflict: 'Konflikt', evidenceExcerpt: 'Beleg', startOffset: 0, endOffset: 5, counterEvidenceExcerpts: [], confidence: 0.8, reason: 'Providerbeleg' }] } : emptyAnalysis());
    const first = await runContinuityReview(repository, { project: workspace.project, chapter: workspace.chapters[0], scene: workspace.chapters[0]!.scenes[0], currentText: 'Beleg bleibt.', sourceKind: 'manual', provider });
    const finding = first.findings[0]!;
    await repository.applyContinuityFindingDecision({ findingId: finding.id, projectId: workspace.project.id, status: 'open', decisionKind: 'text_correction', sourceReferenceId: finding.sourceReferenceId });
    expect((await repository.listContinuityFindingDecisions(workspace.project.id))[0]?.status).toBe('open');
    await runContinuityReview(repository, { project: workspace.project, chapter: workspace.chapters[0], scene: workspace.chapters[0]!.scenes[0], currentText: 'Korrigiert.', sourceKind: 'manual', provider });
    expect((await repository.listContinuityFindingDecisions(workspace.project.id))[0]?.status).toBe('resolved_after_text_change');
  });

  it('verlangt bestätigte Regeln, begründete Ausnahmen und hält neue Regeln pending', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const finding = (await runContinuityReview(repository, { project: workspace.project, chapter: workspace.chapters[0], scene: workspace.chapters[0]!.scenes[0], currentText: 'Konflikt.', sourceKind: 'manual', provider: fakeProvider(() => ({ ...emptyAnalysis(), objectiveContradictions: [{ findingType: 'missing_explanation', subjectEntityId: workspace.entities[0]!.id, relatedEntityIds: [], relatedStateIds: [], objectiveConflict: 'Konflikt', evidenceExcerpt: 'Konflikt.', startOffset: 0, endOffset: 9, counterEvidenceExcerpts: [], confidence: 0.7, reason: 'Erklärung fehlt.' }] })) })).findings[0]!;
    const proposedRule = await repository.saveProjectRule({ projectId: workspace.project.id, title: 'Unbestätigt', statement: 'Noch nicht aktiv', scope: 'project', prerequisites: [], effects: [], exceptions: [], connectedLoreIds: [], sourceReferenceIds: [], status: 'proposed', confidence: 0.5, authorConfirmed: false, origin: 'manual' });
    await expect(repository.applyContinuityFindingDecision({ findingId: finding.id, projectId: workspace.project.id, status: 'resolved_with_confirmed_rule', decisionKind: 'confirmed_rule', ruleId: proposedRule.id })).rejects.toThrow();
    const confirmedRule = await repository.saveProjectRule({ ...proposedRule, status: 'confirmed', authorConfirmed: true });
    await repository.applyContinuityFindingDecision({ findingId: finding.id, projectId: workspace.project.id, status: 'resolved_with_confirmed_rule', decisionKind: 'confirmed_rule', ruleId: confirmedRule.id, sourceReferenceId: finding.sourceReferenceId });
    expect((await repository.listContinuityFindingDecisions(workspace.project.id))[0]?.status).toBe('resolved_with_confirmed_rule');
    const pendingProposal = await repository.saveProjectRuleProposal({ projectId: workspace.project.id, title: 'Neue Regelprüfung', statement: 'Nur ein unbestätigter Entwurf', scope: 'project', prerequisites: [], effects: [], exceptions: [], connectedLoreIds: [], sourceReferenceIds: finding.sourceReferenceId ? [finding.sourceReferenceId] : [], evidenceExcerpt: finding.evidenceExcerpt, chapterId: finding.chapterId, sceneId: finding.sceneId, startOffset: finding.startOffset, endOffset: finding.endOffset, confidence: finding.confidence, reason: finding.reason });
    expect(pendingProposal.reviewStatus).toBe('pending');
  });

  it('persistiert Ausnahmebegründung, Open-Question-Verknüpfung und Kanon-Audit ohne Überschreiben', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const finding = (await runContinuityReview(repository, { project: workspace.project, chapter: workspace.chapters[0], scene: workspace.chapters[0]!.scenes[0], currentText: 'Konflikt.', sourceKind: 'manual', provider: fakeProvider(() => ({ ...emptyAnalysis(), objectiveContradictions: [{ findingType: 'possible_intentional_exception', subjectEntityId: workspace.entities[0]!.id, relatedEntityIds: [], relatedStateIds: [], objectiveConflict: 'Konflikt', evidenceExcerpt: 'Konflikt.', startOffset: 0, endOffset: 9, counterEvidenceExcerpts: [], confidence: 0.7, reason: 'Ausnahme möglich.' }] })) })).findings[0]!;
    await expect(repository.applyContinuityFindingDecision({ findingId: finding.id, projectId: workspace.project.id, status: 'accepted_exception', decisionKind: 'intentional_exception', sourceReferenceId: finding.sourceReferenceId })).rejects.toThrow();
    await repository.applyContinuityFindingDecision({ findingId: finding.id, projectId: workspace.project.id, status: 'deferred_canon_review', decisionKind: 'canon_review', sourceReferenceId: finding.sourceReferenceId, canonAction: 'retcon', canonReason: 'Explizite Autorentscheidung', canonSourceReferenceIds: finding.sourceReferenceId ? [finding.sourceReferenceId] : [] });
    expect((await repository.listContinuityCanonChangeAudits(workspace.project.id, finding.id))[0]?.action).toBe('retcon');
    expect((await repository.listContinuityStateLedger(workspace.project.id)).length).toBe(0);
  });

  it('bestätigt Zustände nur mit Source Reference und speichert offene Fragen als Bible-Entity', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const scene = workspace.chapters[0]!.scenes[0]!;
    const source = await repository.createSourceReference({ projectId: workspace.project.id, entityId: workspace.entities[0]!.id, chapterId: workspace.chapters[0]!.id, sceneId: scene.id, excerpt: 'Zustand', startOffset: 0, endOffset: 7 });
    const proposed = await repository.saveContinuityStateEntry({ projectId: workspace.project.id, entityId: workspace.entities[0]!.id, stateKind: 'location', previousState: '', newState: 'Archiv', chapterId: workspace.chapters[0]!.id, sceneId: scene.id, sourceReferenceId: source.id, status: 'proposed', confidence: 0.9, authorConfirmed: false });
    await expect(repository.saveContinuityStateEntry({ ...proposed, status: 'confirmed', sourceReferenceId: undefined, authorConfirmed: true })).rejects.toThrow();
    const confirmed = await repository.saveContinuityStateEntry({ ...proposed, status: 'confirmed', sourceReferenceId: source.id, authorConfirmed: true });
    expect(confirmed.status).toBe('confirmed');
    const question = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Offene Frage', type: 'open_question', description: 'Warum?', status: 'proposed', confidence: 0.5, chapterId: workspace.chapters[0]!.id, sceneId: scene.id, excerpt: 'Warum?', authorConfirmed: false, tags: ['open_question'] });
    expect(question.type).toBe('open_question');
  });

  it('weist einen AI-resolved-Plot-Thread zurück und lässt die Nutzerentscheidung mit Quelle speichern', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const thread = workspace.entities.find((entity) => entity.type === 'plot_thread') ?? workspace.entities[0]!;
    const run = await repository.createContinuityReviewRun({ projectId: workspace.project.id, chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, sourceKind: 'manual', contentHash: 'thread-review', providerId: 'codex-cli' });
    await expect(repository.savePlotThreadLifecycleProposal({ runId: run.id, projectId: workspace.project.id, entityId: thread.id, proposedStatus: 'resolved' as never, evidenceExcerpt: 'Beleg', sourceReferenceId: undefined, startOffset: 0, endOffset: 5, reason: 'AI darf dies nicht setzen', confidence: 0.8 })).rejects.toThrow();
  });
});
