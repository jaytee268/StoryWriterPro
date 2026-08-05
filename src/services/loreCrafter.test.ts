import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDemoRepository } from './storyRepository';
import type { BuildLoreSheetResult, LoreCrafterAnalysis } from '../types/domain';
import type { StoryAiProvider } from './aiProviderService';
import { analyzeLoreDraft, buildLoreSheet, confirmLoreCrafterRule, ignoreExcludedContent, reviewLoreSheetItem, routeExcludedContent } from './loreCrafter';
import { contentHash } from '../utils/aiText';

const store = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value), removeItem: (key: string) => store.delete(key) });
vi.stubGlobal('crypto', { randomUUID: () => `lore-${Math.random().toString(16).slice(2)}` });

const analysis = (): LoreCrafterAnalysis => ({ understandingSummary: 'Das System hat eine Regel.', confirmedStatements: ['Das System existiert.'], proposedWorldRules: ['Das System hat eine Grenze.'], prerequisites: ['Eine Voraussetzung'], effects: ['Eine Wirkung'], limitations: ['Eine Grenze'], costs: ['Ein Preis'], exceptions: ['Eine Ausnahme'], terminology: ['System'], relevantOrganizations: [], relevantLocations: [], historicalBackground: [], unresolvedQuestions: ['Gilt die Grenze immer?'], contradictions: [], excludedContent: [{ content: 'Eine einzelne Szene.', suggestedTarget: 'manuscript', reason: 'Konkrete Handlung statt Weltregel.' }], clarificationQuestions: ['Soll die Grenze immer gelten?'], confidence: 0.82, warnings: [] });
const sheet = (): BuildLoreSheetResult => ({ title: 'System-Lore', premise: 'Eine strukturierte Weltbeschreibung.', categories: ['world_rule'], worldRules: ['Das System hat eine Grenze.'], worldRuleObjects: [{ temporaryId: 'rule-1', title: 'Grenze des Systems', statement: 'Das System hat eine Grenze.', prerequisites: ['Eine Voraussetzung'], effects: ['Eine Wirkung'], limitations: ['Eine Grenze'], costs: ['Ein Preis'], exceptions: ['Eine Ausnahme'], relatedTerminology: ['System'], connectedItemIds: [], sourceSpans: [{ excerpt: 'Eine Regelnotiz.', startOffset: 0, endOffset: 16 }], confidence: 0.82 }], prerequisites: ['Eine Voraussetzung'], effects: ['Eine Wirkung'], limitations: ['Eine Grenze'], costs: ['Ein Preis'], exceptions: ['Eine Ausnahme'], terminology: ['System'], organizations: [], locations: [], historicalEvents: [], knownAspects: ['Das System existiert.'], unknownAspects: ['Gilt die Grenze immer?'], ruleConnections: [], openQuestions: ['Soll die Grenze immer gelten?'], warnings: [] });
function fakeProvider(): StoryAiProvider { return { id: 'codex-cli', getStatus: vi.fn(), extractBiblePatch: vi.fn(), extractCharacterMemoryPatch: vi.fn(), analyzeContinuityPassage: vi.fn(), analyzeProjectStyle: vi.fn(), summarize: vi.fn(), analyzeNarrativeSummaries: vi.fn(), synthesizePlotThreads: vi.fn(), analyzeBookEndState: vi.fn(), globalCountercheck: vi.fn(), analyzeLoreDraft: vi.fn(async () => analysis()), buildLoreSheet: vi.fn(async () => sheet()), answerWithProjectContext: vi.fn(), cancel: vi.fn(), cancelActive: vi.fn() } as unknown as StoryAiProvider; }

describe('Lore Crafter Workflow', () => {
  beforeEach(() => store.clear());

  it('strukturiert freie Notizen, zeigt Verständnis und verändert noch keinen Kanon', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const beforeEntities = workspace.entities.length; const beforeRules = (await repository.listProjectRules(workspace.project.id)).length;
    const run = await analyzeLoreDraft(repository, fakeProvider(), { projectId: workspace.project.id, originalText: '😀 Das System hat eine Grenze.' });
    expect(run.status).toBe('awaiting_review'); expect(run.confirmationText).toContain('So habe ich deine Lore verstanden'); expect(run.analysis?.excludedContent[0]?.suggestedTarget).toBe('manuscript'); expect((await repository.loadWorkspace()).entities).toHaveLength(beforeEntities); expect(await repository.listProjectRules(workspace.project.id)).toHaveLength(beforeRules);
  });

  it('verankert die Source Reference mit Unicode-Positionen und verlangt die Verständnisbestätigung', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const provider = fakeProvider(); const text = '😀 Notiz mit kombinierendem Zeichen e\u0301.'; const run = await analyzeLoreDraft(repository, provider, { projectId: workspace.project.id, originalText: text }); const source = (await repository.listLoreCrafterSources(run.id))[0]!;
    expect(source.startOffset).toBe(0); expect(source.endOffset).toBe(Array.from(text).length); expect(source.excerpt).toBe(text); await expect(buildLoreSheet(repository, provider, run.id, false)).rejects.toThrow('Verständnis');
  });

  it('erstellt ein proposed Lore Sheet, lässt Korrekturen einfließen und übernimmt einzelne Einträge als inaktiven Entwurf', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const provider = fakeProvider(); const run = await analyzeLoreDraft(repository, provider, { projectId: workspace.project.id, originalText: 'Eine Regelnotiz.' }); const built = await buildLoreSheet(repository, provider, run.id, true);
    expect(built.draft.status).toBe('proposed'); expect(built.items.every((item) => item.status === 'proposed')).toBe(true); const accepted = await reviewLoreSheetItem(repository, built.items.find((item) => item.itemType === 'world_rule')!, 'accepted', 'Die bearbeitete Regel.'); expect(accepted.status).toBe('accepted'); expect((await repository.listProjectRules(workspace.project.id, true)).filter((rule) => rule.origin === 'lore_crafter')).toHaveLength(0); expect((await repository.listProjectRules(workspace.project.id)).find((rule) => rule.origin === 'lore_crafter')).toMatchObject({ status: 'proposed', authorConfirmed: false, statement: 'Die bearbeitete Regel.' });
  });

  it('setzt bei verändertem Inhalt einen neuen hashgebundenen Lauf an und kann den alten Lauf nach Neustart wieder laden', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const provider = fakeProvider(); const first = await analyzeLoreDraft(repository, provider, { projectId: workspace.project.id, originalText: 'Erste Notiz.' }); const second = await analyzeLoreDraft(repository, provider, { projectId: workspace.project.id, originalText: 'Geänderte Notiz.' });
    expect(first.id).not.toBe(second.id); expect(second.contentHash).toBe(contentHash('Geänderte Notiz.')); const resumed = new BrowserDemoRepository(); expect((await resumed.listLoreCrafterRuns(workspace.project.id)).map((item) => item.id)).toEqual(expect.arrayContaining([first.id, second.id]));
  });

  it('speichert Lore-Quellen als projektweite Quelldokumente und aktiviert Regeln erst nach Autorbestätigung', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const provider = fakeProvider(); const run = await analyzeLoreDraft(repository, provider, { projectId: workspace.project.id, originalText: 'Eine Regelnotiz.' }); const built = await buildLoreSheet(repository, provider, run.id, true); const source = (await repository.listLoreCrafterSources(run.id))[0]!; const documents = await repository.listProjectSourceDocuments(workspace.project.id);
    expect(source.sourceDocumentId).toBeTruthy(); expect(source.sourceReferenceId).toBeTruthy(); expect(documents[0]?.content).toBe('Eine Regelnotiz.');
    const ruleItem = built.items.find((item) => item.itemType === 'world_rule' && item.structured)!; const accepted = await reviewLoreSheetItem(repository, ruleItem, 'accepted'); const rules = await repository.listProjectRules(workspace.project.id);
    expect(rules.find((rule) => rule.id === accepted.targetRuleId)).toMatchObject({ status: 'proposed', authorConfirmed: false, prerequisites: ['Eine Voraussetzung'], effects: ['Eine Wirkung'], exceptions: ['Eine Ausnahme'] }); await confirmLoreCrafterRule(repository, accepted); expect((await repository.listProjectRules(workspace.project.id)).find((rule) => rule.id === accepted.targetRuleId)).toMatchObject({ status: 'confirmed', authorConfirmed: true });
  });

  it('führt Merge und Excluded-Content-Routing als echte vorgeschlagene Workflows aus', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const provider = fakeProvider(); const existing = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'System', type: 'fact', description: 'Alt', status: 'proposed', confidence: 0.5, excerpt: 'Alt', authorConfirmed: false, tags: [] }); const run = await analyzeLoreDraft(repository, provider, { projectId: workspace.project.id, originalText: 'Eine Regelnotiz.' }); const built = await buildLoreSheet(repository, provider, run.id, true); const item = built.items.find((candidate) => candidate.itemType === 'term')!; const merged = await reviewLoreSheetItem(repository, item, 'merged', 'Zusammengeführt', existing.id); expect(merged.targetEntityId).toBe(existing.id); expect((await repository.getStoryEntity(existing.id)).description).toBe('Zusammengeführt'); const routed = await routeExcludedContent(repository, run, 'Eine einzelne Szene.', 'Konkrete Handlung.', 'manuscript'); expect(routed.proposal.status).toBe('proposed'); expect(routed.decision.decision).toBe('routed'); expect((await repository.listProjectContentProposals(workspace.project.id)).some((candidate) => candidate.id === routed.proposal.id && candidate.targetKind === 'manuscript')).toBe(true); expect((await repository.listLoreSheetItems(built.draft.id)).every((candidate) => !candidate.itemType.startsWith('routed_'))).toBe(true);
  });

  it('speichert terminology als kanonischen term und persistiert Ignorieren ohne Routing', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const provider = fakeProvider(); const run = await analyzeLoreDraft(repository, provider, { projectId: workspace.project.id, originalText: 'Eine Regelnotiz.' }); const built = await buildLoreSheet(repository, provider, run.id, true);
    expect(built.items.some((item) => item.itemType === 'term')).toBe(true);
    const excluded = analysis().excludedContent[0]!; await expect(ignoreExcludedContent(repository, run, excluded.content, excluded.reason, excluded.suggestedTarget)).resolves.toMatchObject({ decision: 'ignored' });
    const reloaded = new BrowserDemoRepository(); expect(await reloaded.listExcludedContentDecisions(run.id)).toMatchObject([{ content: excluded.content, decision: 'ignored' }]); expect((await reloaded.listProjectContentProposals(workspace.project.id)).length).toBe(0);
  });

  it('lehnt einen unbekannten Lore-Sheet-Typ vor dem Speichern ab', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const provider = fakeProvider(); const run = await analyzeLoreDraft(repository, provider, { projectId: workspace.project.id, originalText: 'Eine Regelnotiz.' });
    await expect(repository.saveLoreSheetWithItems({ runId: run.id, projectId: workspace.project.id, contentHash: run.contentHash, title: 'Ungültig', premise: '', categories: [], worldRules: [], prerequisites: [], effects: [], limitations: [], costs: [], exceptions: [], terminology: [], organizations: [], locations: [], historicalEvents: [], knownAspects: [], unknownAspects: [], ruleConnections: [], openQuestions: [], status: 'proposed' }, [{ draftId: '', runId: run.id, projectId: workspace.project.id, itemType: 'routed_manuscript' as never, title: 'Ungültig', content: 'x', confidence: 0 }])).rejects.toThrow('ungültigen Eintrag');
    expect(await repository.getLoreSheetDraft(run.id)).toBeUndefined();
  });

  it('erstellt bei erneutem Laden eines vollständigen Lore Sheets keine Duplikate', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const provider = fakeProvider(); const run = await analyzeLoreDraft(repository, provider, { projectId: workspace.project.id, originalText: 'Eine Regelnotiz.' }); const first = await buildLoreSheet(repository, provider, run.id, true); const second = await buildLoreSheet(repository, provider, run.id, true);
    expect(second.draft.id).toBe(first.draft.id); expect(second.items.map((item) => item.id)).toEqual(first.items.map((item) => item.id));
  });
});
