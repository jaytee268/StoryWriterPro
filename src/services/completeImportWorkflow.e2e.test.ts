import { beforeEach, describe, expect, it, vi } from 'vitest';
import { parseManuscriptText, splitContinuityUnits } from './manuscriptImport';
import { BrowserDemoRepository } from './storyRepository';
import { ManuscriptAnalysisController } from './manuscriptAnalysis';
import { analyzeLoreDraft, buildLoreSheet, confirmLoreCrafterRule, finishLoreCrafterReview, reviewLoreSheetItem } from './loreCrafter';
import { contentHash } from '../utils/aiText';
import { providerRouter } from './aiProviderService';
import type { ContinuityAnalysisInput, StoryAiProvider } from './aiProviderService';
import type { BookEndStateResult, BuildLoreSheetResult, CharacterMemoryExtractionInput, CharacterMemoryExtractionResult, ContinuityAnalysisResult, DetectBookGenreResult, GlobalCountercheckResult, LoreCrafterAnalysis, ManuscriptEntityResolutionInput, ManuscriptEntityResolutionResult, ManuscriptStructureInput, ManuscriptStructureResult } from '../types/domain';

const store = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value), removeItem: (key: string) => store.delete(key), clear: () => store.clear() });

const loreAnalysis: LoreCrafterAnalysis = { understandingSummary: 'Eine Regel kann den Zustand eines physischen Belegs unter einer klaren Bedingung verändern.', confirmedStatements: ['Die Regel ist nur eine mögliche Erklärung.'], proposedWorldRules: ['Ein Beleg kann unter einer Bedingung verändert erscheinen.'], prerequisites: ['Eine definierte Bedingung.'], effects: ['Der Belegzustand ist nicht eindeutig.'], limitations: ['Die Regel erklärt nicht jeden Widerspruch.'], costs: [], exceptions: ['Eine bewusste Ausnahme bleibt möglich.'], terminology: ['Beleg'], relevantOrganizations: [], relevantLocations: [], historicalBackground: [], unresolvedQuestions: ['Welche Bedingung gilt?'], contradictions: [], excludedContent: [], clarificationQuestions: ['Soll die Bedingung immer gelten?'], confidence: 0.91, warnings: [] };
const loreSheet: BuildLoreSheetResult = { title: 'Belegregeln', premise: loreAnalysis.understandingSummary, categories: ['world_rule'], worldRules: loreAnalysis.proposedWorldRules, worldRuleObjects: [{ temporaryId: 'rule-1', title: 'Veränderliche Belege', statement: loreAnalysis.proposedWorldRules[0]!, prerequisites: loreAnalysis.prerequisites, effects: loreAnalysis.effects, limitations: loreAnalysis.limitations, costs: [], exceptions: loreAnalysis.exceptions, relatedTerminology: loreAnalysis.terminology, connectedItemIds: [], sourceSpans: [{ excerpt: 'Ein Beleg kann', startOffset: 0, endOffset: 15 }], confidence: 0.91 }], prerequisites: loreAnalysis.prerequisites, effects: loreAnalysis.effects, limitations: loreAnalysis.limitations, costs: [], exceptions: loreAnalysis.exceptions, terminology: loreAnalysis.terminology, organizations: [], locations: [], historicalEvents: [], knownAspects: loreAnalysis.confirmedStatements, unknownAspects: loreAnalysis.unresolvedQuestions, ruleConnections: [], openQuestions: loreAnalysis.clarificationQuestions, warnings: [] };

const manuscript = [
  'Kapitel 1\nSeite 1\n😀 Eine Gestalt findet das Relikt und nennt den Spruch „Immer weiter“.\n\nAlex stellt es in den Schrank.\n\nDie Person neben ihm antwortet: „Immer weiter“.\n\nIn der Rückblende steht das Relikt noch im Archiv.',
  'Kapitel 2\nSeite 19\nAlex heißt vollständig Alexander und erfährt erst jetzt das Geheimnis.\n\nMira verliert das Relikt beim Ortswechsel am Hafen.\n\nZwei Personen heißen Alex; ihre Beziehung verändert sich.\n\nEin Zeitsprung führt in den nächsten Morgen.',
  'Kapitel 3\nSeite 37\nDas verlorene Relikt liegt wieder auf dem Tisch.\n\nAlexander kennt nun die Bedingung und erkennt den Fahrer.\n\nDie scheinbar geschlossene Spur bleibt wegen der Belegregel fraglich.\n\n„Immer weiter“, sagt Mira.'
].join('\n\n');

function excerpt(text: string): { value: string; end: number } { const value = Array.from(text).slice(0, Math.min(18, Array.from(text).length)).join(''); return { value, end: Array.from(value).length }; }

function fakeProvider(keeperId: string, threadId: string): StoryAiProvider {
  let continuityCalls = 0;
  let newRuleSent = false;
  const provider: StoryAiProvider = {
    id: 'codex-cli',
    getStatus: vi.fn(),
    analyzeLoreDraft: vi.fn(async () => loreAnalysis),
    buildLoreSheet: vi.fn(async () => loreSheet),
    analyzeManuscriptStructure: vi.fn(async (input: ManuscriptStructureInput): Promise<ManuscriptStructureResult> => {
      const text = input.chapter.scenes[0]?.content ?? '';
      const characters = Array.from(text);
      const scenes = Array.from({ length: 4 }, (_, index) => {
        const startOffset = Math.floor(characters.length * index / 4);
        const endOffset = index === 3 ? characters.length : Math.floor(characters.length * (index + 1) / 4);
        return { temporaryId: `scene-${input.chapter.id}-${index}`, chapterId: input.chapter.id, startOffset, endOffset, title: `Abschnitt ${index + 1}`, povCharacterName: index % 2 ? 'Alexander' : 'Mira', povEntityId: undefined, location: index === 2 ? 'Hafen' : 'Archiv', storyTime: index === 3 ? 'Rückblende' : `Tag ${index + 1}`, participatingCharacterNames: ['Alex', 'Mira'], goal: 'Die Spur prüfen.', conflict: 'Der Zustand des Relikts ist unklar.', importantEvents: ['Eine neue Spur erscheint.'], transitionType: index === 3 ? 'flashback_start' as const : index === 2 ? 'location_change' as const : 'chapter_continuation' as const, boundaryReason: 'Synthetischer Szenenwechsel.', confidence: 0.88, evidenceExcerpt: characters.slice(startOffset, endOffset).join('') };
      });
        return { scenes, warnings: [] };
    }),
    resolveManuscriptEntityMentions: vi.fn(async (input: ManuscriptEntityResolutionInput): Promise<ManuscriptEntityResolutionResult> => {
      if (input.unit.orderIndex !== 0) return { entities: [], mentions: [], relations: [], events: [], mergeProposals: [], warnings: [] };
      const first = excerpt(input.passageText);
      return { entities: [{ temporaryId: 'alex', entityType: 'character' as const, canonicalName: 'Alex', aliases: ['Alexander'], description: 'Eine zentrale Figur.', confidence: 0.93 }, { temporaryId: 'mira', entityType: 'character' as const, canonicalName: 'Mira', aliases: ['Alex-2'], description: 'Eine zweite Figur mit verändertem Verhältnis.', confidence: 0.89 }, { temporaryId: 'relic', entityType: 'object' as const, canonicalName: 'Relikt', aliases: ['Beleg'], description: 'Ein verlorener Gegenstand.', confidence: 0.9 }], mentions: [{ mentionText: 'Gestalt', startOffset: 0, endOffset: first.end, temporaryEntityId: 'alex', alternativeTemporaryIds: [], confidence: 0.88, resolutionReason: 'Die erste Umschreibung wird als Figur vorgeschlagen.', excerpt: first.value }, { mentionText: 'Relikt', startOffset: 0, endOffset: first.end, temporaryEntityId: 'relic', alternativeTemporaryIds: [], confidence: 0.9, resolutionReason: 'Der Gegenstand wird eingeführt.', excerpt: first.value }], relations: [{ sourceTemporaryId: 'alex', targetTemporaryId: 'mira', relationType: 'trust', label: 'vertraut', confidence: 0.77 }], events: [{ title: 'Relikt gefunden', summary: 'Die Figur findet den Gegenstand.', participantTemporaryIds: ['alex', 'mira'], startOffset: 0, endOffset: first.end, confidence: 0.86, excerpt: first.value }], mergeProposals: [], warnings: [] };
    }),
    extractBiblePatch: vi.fn(async (input) => { const current = excerpt(input.scene.content); return { proposals: [{ targetEntityId: undefined, proposalAction: 'create_entity' as const, entityType: 'fact' as const, candidateName: `Beobachtung ${input.scene.id.slice(0, 6)}`, candidateDescription: 'Eine quellengebundene Beobachtung.', candidateStatus: 'proposed' as const, confidence: 0.82, classification: 'observable_fact' as const, evidenceExcerpt: current.value, startOffset: 0, endOffset: current.end, reason: 'Fake-Provider-Evidence.' }], warnings: [] }; }),
    extractCharacterMemoryPatch: vi.fn(async (input: CharacterMemoryExtractionInput): Promise<CharacterMemoryExtractionResult> => { const current = excerpt(input.scene.content); return { proposals: [{ proposalKind: 'knowledge_change' as const, subjectCharacterId: keeperId, payload: { factEntityId: threadId, knowledgeState: 'knows' as const, certainty: 0.8, notes: 'Die Figur erwirbt eine relevante Information.' }, classification: 'observable' as const, confidence: 0.8, evidenceExcerpt: current.value, startOffset: 0, endOffset: current.end, reason: 'Wissen wird in dieser Passage vorgeschlagen.' }], warnings: [] }; }),
    analyzeContinuityPassage: vi.fn(async (input: ContinuityAnalysisInput): Promise<ContinuityAnalysisResult> => {
      continuityCalls += 1;
      const current = excerpt(input.passage.text);
      const source = input.relevantSources[0];
      const entityId = input.provisionalEntities?.[0]?.id ?? input.confirmedStoryBible[0]?.id ?? threadId;
      const finding = continuityCalls > 2 ? [{ findingType: 'probable_contradiction' as const, subjectEntityId: undefined, relatedEntityIds: [], relatedStateIds: [], objectiveConflict: 'Das später wieder auftauchende Relikt widerspricht seinem dokumentierten Verlust.', evidenceExcerpt: current.value, sourceReferenceId: source?.id, counterEvidenceExcerpts: ['Das Relikt ging verloren.'], counterEvidence: source ? [{ sourceReferenceId: source.id, excerpt: source.excerpt, chapterId: source.chapterId ?? undefined, sceneId: source.sceneId ?? undefined }] : [], confidence: 0.86, startOffset: 0, endOffset: current.end, reason: 'Der objektive Konflikt bleibt sichtbar; eine Lore-Erklärung ist nur möglich.' }] : [];
      const matchedRule = input.confirmedRules[0];
      return { observedActions: [{ summary: 'Eine Handlung verändert den vorläufigen Zustand.', evidenceExcerpt: current.value, entityIds: [entityId], startOffset: 0, endOffset: current.end }], proposedStateChanges: [{ entityId, relatedEntityId: undefined, stateKind: 'item_availability' as const, previousState: 'unbekannt', newState: continuityCalls > 2 ? 'wieder aufgetaucht' : 'in der Passage vorhanden', confidence: 0.84, evidenceExcerpt: current.value, sourceReferenceId: source?.id, startOffset: 0, endOffset: current.end, reason: 'Semantische Zustandsänderung als Vorschlag.' }], objectiveContradictions: finding, missingExplanations: [], matchedLoreRules: matchedRule ? [{ ruleId: matchedRule.id, rationale: 'Die bestätigte Regel könnte den Konflikt erklären.', confidence: 0.7 }] : [], newRuleProposals: !newRuleSent ? (newRuleSent = true, [{ projectId: input.projectId, targetRuleId: undefined, title: 'Neue Belegbedingung', statement: 'Ein Beleg kann unter einer besonderen Bedingung anders erscheinen.', scope: 'project' as const, prerequisites: ['Eine besondere Bedingung.'], effects: ['Der Zustand wird unklar.'], exceptions: [], connectedLoreIds: [], sourceReferenceIds: [], evidenceExcerpt: current.value, chapterId: input.passage.chapterId ?? undefined, sceneId: input.passage.sceneId ?? undefined, startOffset: 0, endOffset: current.end, confidence: 0.54, reason: 'Unbestätigter Vorschlag für Review.' }]) : [], plotThreadChanges: input.confirmedStoryBible.some((entity) => entity.id === threadId) ? [{ entityId: threadId, proposedStatus: 'closure_candidate' as const, evidenceExcerpt: current.value, sourceReferenceId: source?.id, startOffset: 0, endOffset: current.end, reason: 'Die Spur wirkt vorläufig abgeschlossen.', confidence: 0.73 }] : [], confidence: 0.83, evidence: [{ id: `evidence-${continuityCalls}`, label: 'Passage', chapterId: input.passage.chapterId ?? undefined, sceneId: input.passage.sceneId ?? undefined, entityId, excerpt: current.value, startOffset: 0, endOffset: current.end }], warnings: [] };
    }),
    analyzeProjectStyle: vi.fn(async () => ({ observations: [], overallSummary: 'Synthetischer Stil.', warnings: [] })),
    summarize: vi.fn(async () => ({ summary: 'Kapitelzusammenfassung.', importantEvents: ['Ein Ereignis.'], openThreads: ['Die Reliktfrage.'], characterChanges: ['Eine Beziehung verändert sich.'], knowledgeChanges: ['Eine Information wird erworben.'], relationshipEffects: ['Vertrauen schwankt.'], warnings: [] })),
    analyzeNarrativeSummaries: vi.fn(async () => ({ summary: 'Buchentwicklung.', importantEvents: ['Die Spur wird verfolgt.'], openThreads: ['Die Bedingung bleibt offen.'], characterChanges: [], knowledgeChanges: [], relationshipEffects: [], warnings: [] })),
    synthesizePlotThreads: vi.fn(async () => ({ summary: 'Die Hauptspur ist nur vorläufig geschlossen.', openQuestions: ['War der Austausch echt?'], threadGoals: ['Die Herkunft des Relikts klären.'], developments: ['Ein Fahrer wird erkannt.'], closureCandidates: ['Die Spur scheint geklärt.'], partiallyResolved: [], reopened: [], threadProposals: [{ entityId: threadId, proposedStatus: 'closure_candidate' as const, evidenceExcerpt: 'Die Spur scheint geklärt.', sourceReferenceId: undefined, reason: 'Nur Nutzer darf resolved setzen.', confidence: 0.74 }], warnings: [] })),
    analyzeBookEndState: vi.fn(async (): Promise<BookEndStateResult> => ({ summary: 'Vorgeschlagener Buchendzustand.', characterEndStates: ['Alexander bleibt wachsam.'], knowledgeStates: ['Die Bedingung ist bekannt.'], falseBeliefs: [], relationships: ['Mira und Alexander vertrauen einander vorsichtiger.'], objectOwners: ['Das Relikt liegt wieder auf dem Tisch.'], injuries: [], locations: ['Hafen'], openActions: ['Lore-Bedingung prüfen.'], unresolvedThreads: ['Die Reliktfrage.'], endStateProposals: [{ category: 'object_owner', entityId: undefined, statement: 'Das Relikt liegt wieder auf dem Tisch.', confidence: 0.8, evidenceExcerpt: 'Das verlorene Relikt liegt wieder auf dem Tisch.' }], warnings: [] })),
    globalCountercheck: vi.fn(async (): Promise<GlobalCountercheckResult> => ({ summary: 'Globale Gegenprüfung bleibt sichtbar.', contradictoryFacts: ['Verlust und Wiederauftauchen widersprechen sich.'], prematureKnowledge: [], lostOrDestroyedObjects: ['Das Relikt wurde verloren.'], timeAndLocationConflicts: [], contradictoryRules: [], unclearExceptions: ['Die bestätigte Regel erklärt den Konflikt möglicherweise.'], uncertainSources: [], countercheckFindings: [{ severity: 'warning', category: 'object_state', objectiveConflict: 'Der objektive Reliktkonflikt bleibt offen.', reason: 'Eine Lore-Erklärung hebt den Konflikt nicht auf.', confidence: 0.86, evidenceExcerpt: 'Das verlorene Relikt liegt wieder auf dem Tisch.' }], warnings: [] })),
    detectBookGenre: vi.fn(async (): Promise<DetectBookGenreResult> => ({ primaryGenreId: 'mystery', customPrimaryGenre: undefined, secondaryGenreIds: ['crime'], customSecondaryGenres: [], confidence: 0.9, reasoning: 'Ermittlung, Geheimnis und wiederkehrende Spuren.', supportingSignals: ['Ermittlung'], contradictingSignals: [], alternativeGenres: [], audienceNotes: [], warnings: [] })),
    answerWithProjectContext: vi.fn(), cancel: vi.fn(async () => undefined), cancelActive: vi.fn(async () => undefined),
  };
  return provider;
}

async function runFullBrowserWorkflow(providerOverride?: StoryAiProvider) {
  const repository = new BrowserDemoRepository();
  const created = await repository.createProject({ title: 'E2E ohne Genre', author: '', volumeTitle: 'E2E ohne Genre', description: '' });
  const workspace = await repository.loadWorkspace(created.id);
  const keeper = await repository.createStoryEntity({ projectId: created.id, name: 'Keeper', type: 'character', description: 'Eine bereits bestätigte Nebenfigur.', status: 'confirmed', confidence: 1, excerpt: '', authorConfirmed: true, tags: [] });
  const thread = await repository.createStoryEntity({ projectId: created.id, name: 'Reliktfrage', type: 'plot_thread', description: 'Wer hat das Relikt ausgetauscht?', status: 'confirmed', confidence: 1, excerpt: '', authorConfirmed: true, tags: ['Austausch'] });
  const provider = providerOverride ?? fakeProvider(keeper.id, thread.id);
  const loreRun = await analyzeLoreDraft(repository, provider, { projectId: created.id, originalText: 'Eine bestätigte Regel kann einen physischen Beleg unter einer Bedingung verändert erscheinen lassen.' });
  const clarification = (await repository.listLoreCrafterClarifications(loreRun.id))[0]!;
  await repository.saveLoreCrafterClarifications(loreRun.id, [{ id: clarification.id, runId: loreRun.id, projectId: created.id, question: clarification.question, answer: 'Nur wenn die definierte Bedingung erfüllt ist.', status: 'answered' }]);
  const loreSheet = await buildLoreSheet(repository, provider, loreRun.id, true);
  const ruleItem = loreSheet.items.find((item) => item.itemType === 'world_rule')!;
  const acceptedRuleItem = await reviewLoreSheetItem(repository, ruleItem, 'accepted');
  await confirmLoreCrafterRule(repository, acceptedRuleItem);
  await finishLoreCrafterReview(repository, await repository.getLoreCrafterRun(loreRun.id), [acceptedRuleItem]);

  const preview = parseManuscriptText(manuscript, 'paste.txt', 'txt');
  expect(preview.chapters).toHaveLength(3);
  expect(preview.pageMarkersFound).toBe(3);
  const imported = await repository.importManuscript({ projectId: created.id, bookId: workspace.books[0]!.id, chapters: preview.chapters.map((chapter) => ({ title: chapter.title, content: chapter.content })) });
  const pageMarkers = preview.chapters.flatMap((chapter, index) => chapter.pageMarkers.map((marker) => ({ chapterId: imported.chapters[index]!.id, pageNumber: marker.page, label: marker.label, sourceOffset: marker.sourceOffset, textOffset: marker.textOffset })));
  const units = imported.chapters.map((chapter, index) => ({ chapterId: chapter.id, sceneId: chapter.scenes[0]!.id, orderIndex: index, pageNumber: index * 18 + 1, startOffset: 0, endOffset: Array.from(chapter.scenes[0]!.content).length, content: chapter.scenes[0]!.content, contentHash: contentHash(chapter.scenes[0]!.content) }));
  const job = await repository.createManuscriptAnalysisJob({ projectId: created.id, bookId: workspace.books[0]!.id, importReference: 'complete-import-e2e', providerId: provider.id, pageMarkers, units });
  const controller = new ManuscriptAnalysisController(repository, job.id, provider);
  await controller.start();
  expect((await repository.getManuscriptAnalysisJob(job.id)).status).toBe('awaiting_structure_review');
  const structureRuns = await repository.listManuscriptStructureRuns(created.id);
  expect(structureRuns).toHaveLength(3);
  for (const run of structureRuns) for (const proposal of await repository.listManuscriptStructureProposals(run.id)) await repository.reviewManuscriptStructureProposal(proposal.id, 'accepted');
  await repository.applyReviewedManuscriptStructure(job.id);
  await controller.start();
  await repository.getManuscriptAnalysisJob(job.id);
  expect(await repository.listManuscriptAnalysisUnits(job.id)).toHaveLength(12);
  expect(await repository.listManuscriptAnalysisPhaseResults(job.id)).not.toHaveLength(0);
  expect(await repository.listProvisionalEntities(job.id)).not.toHaveLength(0);
  expect(await repository.listContinuityReviewFindings(created.id)).not.toHaveLength(0);
  expect(await repository.listProjectRules(created.id, true)).toEqual(expect.arrayContaining([expect.objectContaining({ status: 'confirmed', authorConfirmed: true })]));
  expect(await repository.getManuscriptAnalysisJob(job.id)).toMatchObject({ status: 'awaiting_user_review', currentPhase: 'user_review' });
  const artifacts = await repository.listManuscriptAnalysisArtifacts(job.id);
  expect(artifacts.some((artifact) => artifact.artifactType === 'genre_detection')).toBe(true);
  const reviewOrder = (type: string) => type === 'provisional_entity' ? 0 : type === 'provisional_merge' ? 1 : type === 'bible_proposal' || type === 'character_memory_proposal' ? 2 : type === 'import_draft_state' ? 9 : 5;
  for (const artifact of [...artifacts].sort((left, right) => reviewOrder(left.artifactType) - reviewOrder(right.artifactType))) {
    const status = artifact.artifactType === 'project_rule_proposal' ? 'rejected' : 'confirmed';
    await repository.reviewManuscriptAnalysisArtifactDecision(artifact.id, status);
  }
  const drafts = await repository.listManuscriptAnalysisDraftLedger(job.id);
  for (const draft of drafts) await repository.reviewManuscriptAnalysisDraftLedger(draft.id, 'confirmed');
  const finalController = new ManuscriptAnalysisController(repository, job.id, provider);
  await finalController.completeUserReview(false);
  const report = await repository.getManuscriptAnalysisCompletionReport(job.id);
  const finalWorkspace = await repository.loadWorkspace(created.id);
  const finalUnits = await repository.listManuscriptAnalysisUnits(job.id);
  const finalProvisional = await repository.listProvisionalEntities(job.id);
  return { repository, job, report, finalWorkspace, finalUnits, finalProvisional };
}

describe('vollständiger Erststart- und Manuskriptimport-Workflow', () => {
  beforeEach(() => store.clear());

  it('führt den gesamten Fake-Provider-Workflow ohne Bulk-Skip aus', async () => {
    const result = await runFullBrowserWorkflow();
    expect(result.report).toBeTruthy();
    expect(result.report?.payload.recognizedScenes).toHaveLength(12);
    expect(result.finalUnits.every((unit) => unit.status === 'completed')).toBe(true);
    expect(result.finalWorkspace.chapters).toHaveLength(3);
    expect(result.finalWorkspace.chapters.every((chapter) => chapter.scenes.length === 4)).toBe(true);
    expect(result.finalWorkspace.books[0]?.primaryGenreId).toBe('mystery');
    expect(result.finalProvisional.every((entity) => entity.reviewStatus === 'accepted')).toBe(true);
    const confirmedIds = new Set(result.finalWorkspace.entities.map((entity) => entity.id));
    expect(result.finalWorkspace.entities.some((entity) => entity.name === 'Alex')).toBe(true);
    expect(result.finalWorkspace.entities.some((entity) => entity.name === 'Relikt')).toBe(true);
    expect(result.finalWorkspace.entities.some((entity) => entity.id === 'provisional-alex')).toBe(false);
    const findings = await result.repository.listContinuityReviewFindings(result.job.projectId);
    const ledger = await result.repository.listContinuityStateLedger(result.job.projectId);
    expect(findings.length).toBeGreaterThan(0);
    expect(ledger.every((entry) => entry.authorConfirmed && confirmedIds.has(entry.entityId))).toBe(true);
    expect(result.report?.payload.providers).toEqual(expect.arrayContaining(['codex-cli']));
  });

  it('Lasttest hält 54 Seiten, 3 Kapitel, Unicode und 12 strukturierte Prüfeinheiten ohne Zeichenverlust', async () => {
    let page = 1;
    const chapterTexts = Array.from({ length: 3 }, (_, chapterIndex) => `Kapitel ${chapterIndex + 1}\n${Array.from({ length: 18 }, (_, sceneIndex) => { const text = `Seite ${page}\n😀 Figur ${chapterIndex + 1}-${sceneIndex + 1} e\u0301 verfolgt die Spur.`; page += 1; return text; }).join('\n\n')}`);
    const lastText = chapterTexts.join('\n\n');
    const preview = parseManuscriptText(lastText, '54-seiten.txt', 'txt');
    expect(preview.chapters).toHaveLength(3);
    expect(preview.pageMarkersFound).toBe(54);
    expect(preview.chapters.reduce((sum, chapter) => sum + Array.from(chapter.content).length, 0)).toBeGreaterThan(54 * 10);
    const units = preview.chapters.flatMap((chapter) => splitContinuityUnits(chapter.content, chapter.pageMarkers, 300));
    expect(units.every((unit) => unit.startOffset >= 0 && unit.endOffset >= unit.startOffset)).toBe(true);
    expect(units.reduce((sum, unit) => sum + Array.from(unit.text).length, 0)).toBe(preview.chapters.reduce((sum, chapter) => sum + Array.from(chapter.content).length, 0));
    expect(units.length).toBe(54);
    const structuralScenes = preview.chapters.flatMap((chapter) => { const length = Array.from(chapter.content).length; return Array.from({ length: 4 }, (_, index) => ({ startOffset: Math.floor(length * index / 4), endOffset: index === 3 ? length : Math.floor(length * (index + 1) / 4) })); });
    expect(structuralScenes).toHaveLength(12);
    expect(structuralScenes.every((scene) => scene.endOffset > scene.startOffset)).toBe(true);
    expect(Array.from(lastText).length).toBeLessThan(100_000);
  });

  it('führt den vollständigen Live-Codex-Workflow nur opt-in aus', async (context) => {
    const env = (globalThis as typeof globalThis & { process?: { env?: Record<string, string | undefined> } }).process?.env;
    if (!env?.STORYMEMORY_RUN_COMPLETE_IMPORT_E2E) {
      context.skip();
      return;
    }
    const active = await providerRouter.getActiveProvider();
    const status = await active.provider.getStatus();
    if (active.provider.id !== 'codex-cli' || !status.available || status.capabilities?.authentication !== 'authenticated') {
      context.skip();
      return;
    }
    const result = await runFullBrowserWorkflow(active.provider);
    expect(result.report?.payload.recognizedScenes).toHaveLength(12);
  });
});
