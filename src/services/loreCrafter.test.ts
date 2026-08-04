import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDemoRepository } from './storyRepository';
import type { BuildLoreSheetResult, LoreCrafterAnalysis } from '../types/domain';
import type { StoryAiProvider } from './aiProviderService';
import { analyzeLoreDraft, buildLoreSheet, reviewLoreSheetItem } from './loreCrafter';
import { contentHash } from '../utils/aiText';

const store = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value), removeItem: (key: string) => store.delete(key) });
vi.stubGlobal('crypto', { randomUUID: () => `lore-${Math.random().toString(16).slice(2)}` });

const analysis = (): LoreCrafterAnalysis => ({ understandingSummary: 'Das System hat eine Regel.', confirmedStatements: ['Das System existiert.'], proposedWorldRules: ['Das System hat eine Grenze.'], prerequisites: ['Eine Voraussetzung'], effects: ['Eine Wirkung'], limitations: ['Eine Grenze'], costs: ['Ein Preis'], exceptions: ['Eine Ausnahme'], terminology: ['System'], relevantOrganizations: [], relevantLocations: [], historicalBackground: [], unresolvedQuestions: ['Gilt die Grenze immer?'], contradictions: [], excludedContent: [{ content: 'Eine einzelne Szene.', suggestedTarget: 'manuscript', reason: 'Konkrete Handlung statt Weltregel.' }], clarificationQuestions: ['Soll die Grenze immer gelten?'], confidence: 0.82, warnings: [] });
const sheet = (): BuildLoreSheetResult => ({ title: 'System-Lore', premise: 'Eine strukturierte Weltbeschreibung.', categories: ['world_rule'], worldRules: ['Das System hat eine Grenze.'], prerequisites: ['Eine Voraussetzung'], effects: ['Eine Wirkung'], limitations: ['Eine Grenze'], costs: ['Ein Preis'], exceptions: ['Eine Ausnahme'], terminology: ['System'], organizations: [], locations: [], historicalEvents: [], knownAspects: ['Das System existiert.'], unknownAspects: ['Gilt die Grenze immer?'], ruleConnections: [], openQuestions: ['Soll die Grenze immer gelten?'], warnings: [] });
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
});
