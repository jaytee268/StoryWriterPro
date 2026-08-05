import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDemoRepository } from './storyRepository';
import { buildRevealContext, compareRevealPositions, formatRevealContextForAi, validateRevealComplianceInput, validateRevealComplianceResultReferences } from './revealKnowledge';
import type { RevealComplianceInput, RevealComplianceResult } from '../types/domain';

const values = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value), removeItem: (key: string) => values.delete(key) });

describe('structured reveal knowledge engine', () => {
  beforeEach(() => values.clear());

  it('verarbeitet den Daniel-Contract zeitlich und trennt Autorwahrheit von Leser- und Figurenwissen', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const book = workspace.books[0]!;
    const chapters = [...workspace.chapters];
    while (chapters.length < 10) { const chapter = await repository.createChapter({ bookId: book.id, title: `Kapitel ${chapters.length + 1}` }); const createdScene = await repository.createScene({ chapterId: chapter.id, title: 'Text' }); chapters.push({ ...chapter, scenes: [createdScene] }); }
    const finalBase = await repository.createChapter({ bookId: book.id, title: 'Finale' });
    const finalChapter = { ...finalBase, scenes: [await repository.createScene({ chapterId: finalBase.id, title: 'Reveal' })] };
    const scene = (chapter: typeof chapters[number]) => chapter.scenes[0]!;
    const create = (name: string, type: 'secret' | 'character') => repository.createStoryEntity({ projectId: workspace.project.id, name, type, description: '', status: 'confirmed', confidence: 1, chapterId: chapters[0]!.id, sceneId: scene(chapters[0]!).id, excerpt: '', authorConfirmed: true, tags: [] });
    const subject = await create('Daniel-Branch-Wahrheit', 'secret');
    const daniel = await create('Daniel', 'character');
    const sora = await create('Sora', 'character');
    const m = await create('M', 'character');
    const contract = await repository.saveRevealContract({ projectId: workspace.project.id, subjectEntityId: subject.id, title: 'Branch-Identität', truthStatement: 'Kaelen, Sora, Vance und M sind verschiedene Branch-Versionen von Daniel.', scope: 'book', status: 'confirmed', authorConfirmed: true, revealState: 'author_only', revealConditionText: 'Erst im Finale.', notes: '' });
    const position = (chapter: typeof chapters[number]) => ({ bookId: book.id, chapterId: chapter.id, sceneId: scene(chapter).id, offset: 0 });
    await repository.saveRevealAudienceState({ projectId: workspace.project.id, contractId: contract.id, audienceKind: 'reader', knowledgeLevel: 'unknown', beliefText: '', validFromPosition: position(chapters[0]!), status: 'confirmed', authorConfirmed: true });
    await repository.saveRevealAudienceState({ projectId: workspace.project.id, contractId: contract.id, audienceKind: 'character', characterEntityId: daniel.id, knowledgeLevel: 'unknown', beliefText: '', validFromPosition: position(chapters[0]!), status: 'confirmed', authorConfirmed: true });
    await repository.saveRevealAudienceState({ projectId: workspace.project.id, contractId: contract.id, audienceKind: 'character', characterEntityId: m.id, knowledgeLevel: 'knows', beliefText: 'M kennt die Wahrheit.', validFromPosition: position(chapters[0]!), status: 'confirmed', authorConfirmed: true });
    await repository.saveRevealAudienceState({ projectId: workspace.project.id, contractId: contract.id, audienceKind: 'character', characterEntityId: sora.id, knowledgeLevel: 'suspects', beliefText: 'Sora vermutet einen Zusammenhang.', validFromPosition: position(chapters[0]!), status: 'confirmed', authorConfirmed: true });
    await repository.saveRevealAudienceState({ projectId: workspace.project.id, contractId: contract.id, audienceKind: 'reader', knowledgeLevel: 'knows', beliefText: '', validFromPosition: position(finalChapter), status: 'confirmed', authorConfirmed: true });
    await repository.saveRevealAudienceState({ projectId: workspace.project.id, contractId: contract.id, audienceKind: 'character', characterEntityId: daniel.id, knowledgeLevel: 'knows', beliefText: 'Daniel erfährt die Wahrheit.', validFromPosition: position(finalChapter), status: 'confirmed', authorConfirmed: true });
    await repository.saveRevealClueRule({ projectId: workspace.project.id, contractId: contract.id, ruleKind: 'allowed', clueType: 'similarity', description: 'Ähnliche Formulierungen und Humor.', maximumExplicitness: 'suggestive', validFromPosition: position(chapters[0]!), validUntilPosition: position(finalChapter), status: 'confirmed', authorConfirmed: true });
    await repository.saveRevealClueRule({ projectId: workspace.project.id, contractId: contract.id, ruleKind: 'forbidden', clueType: 'direct_identity', description: 'Direkte Identitätsaussage vor dem Finale.', maximumExplicitness: 'direct', validFromPosition: position(chapters[0]!), validUntilPosition: position(finalChapter), status: 'confirmed', authorConfirmed: true });
    await repository.saveRevealClueRule({ projectId: workspace.project.id, contractId: contract.id, ruleKind: 'required', clueType: 'foreshadowing', description: 'Mehrere einzeln erklärbare Gemeinsamkeiten.', maximumExplicitness: 'subtle', validFromPosition: position(chapters[0]!), validUntilPosition: position(finalChapter), status: 'confirmed', authorConfirmed: true });

    const early = await buildRevealContext(repository, { projectId: workspace.project.id, position: position(chapters[0]!), povCharacterId: daniel.id, participatingCharacterIds: [m.id, sora.id] });
    expect(early.confirmedAuthorTruths[0]?.truthStatement).toContain('Branch-Versionen');
    expect(early.readerKnowledgeAtPosition[0]?.knowledgeLevel).toBe('unknown');
    expect(early.povCharacterKnowledgeAtPosition[0]?.knowledgeLevel).toBe('unknown');
    expect(early.participantKnowledgeAtPosition.map((item) => item.knowledgeLevel)).toEqual(expect.arrayContaining(['knows', 'suspects']));
    expect(early.forbiddenClues).toHaveLength(1);
    expect(formatRevealContextForAi(early)).toContain('AUTHOR TRUTH — NEVER COPY AUTOMATICALLY');
    const atFinale = await buildRevealContext(repository, { projectId: workspace.project.id, position: position(finalChapter), povCharacterId: daniel.id });
    expect(atFinale.readerKnowledgeAtPosition[0]?.knowledgeLevel).toBe('knows');
    expect(atFinale.povCharacterKnowledgeAtPosition[0]?.knowledgeLevel).toBe('knows');
    const fakeProvider = {
      validateRevealCompliance: async (input: RevealComplianceInput): Promise<RevealComplianceResult> => {
        validateRevealComplianceInput(input);
        const start = Array.from(input.text).findIndex(() => true);
        if (input.text === 'Kaelen war eine andere Version von Daniel.' && input.revealContext.readerKnowledgeAtPosition[0]?.knowledgeLevel === 'unknown') return { findings: [{ findingType: 'premature_revelation', severity: 'critical', contractId: contract.id, subjectEntityId: subject.id, evidenceExcerpt: input.text, explanation: 'Direkte Identitätsaussage vor dem geplanten Reveal.', expectedKnowledgeLevel: 'unknown', actualDisclosureLevel: 'knows', confidence: 1, startOffset: start, endOffset: Array.from(input.text).length }], warnings: [] };
        if (input.text === 'Daniel wusste noch nicht, dass alle Nutzer Versionen seiner selbst waren.') return { findings: [{ findingType: 'narrator_information_leak', severity: 'critical', contractId: contract.id, subjectEntityId: subject.id, evidenceExcerpt: input.text, explanation: 'Der Erzähler nennt die objektive Wahrheit vor dem Reveal.', expectedKnowledgeLevel: 'unknown', actualDisclosureLevel: 'knows', confidence: 1, startOffset: 0, endOffset: Array.from(input.text).length }], warnings: [] };
        return { findings: [], warnings: [] };
      },
    };
    const beforeInput = { projectId: workspace.project.id, manuscriptPosition: position(chapters[0]!), textKind: 'manuscript' as const, participatingCharacterIds: [], revealContext: early };
    const premature = await fakeProvider.validateRevealCompliance({ ...beforeInput, text: 'Kaelen war eine andere Version von Daniel.' });
    expect(premature.findings[0]?.findingType).toBe('premature_revelation');
    expect(premature.findings[0]?.severity).toBe('critical');
    expect((await fakeProvider.validateRevealCompliance({ ...beforeInput, text: 'Kaelen benutzte dieselbe ungewöhnliche Formulierung wie Daniel.' })).findings).toHaveLength(0);
    expect((await fakeProvider.validateRevealCompliance({ ...beforeInput, text: 'Daniel wusste noch nicht, dass alle Nutzer Versionen seiner selbst waren.' })).findings[0]?.findingType).toBe('narrator_information_leak');
    expect((await fakeProvider.validateRevealCompliance({ projectId: workspace.project.id, manuscriptPosition: position(finalChapter), text: 'Kaelen war eine andere Version von Daniel.', textKind: 'manuscript', participatingCharacterIds: [], revealContext: atFinale })).findings).toHaveLength(0);
    expect(compareRevealPositions({ chapterOrderIndex: 1, sceneOrderIndex: 1, offset: 0 }, { chapterOrderIndex: 11, sceneOrderIndex: 1, offset: 0 })).toBe(-1);
  });

  it('validiert semantische Providerbefunde nur gegen angeforderten Kontext und Unicode-Text', () => {
    const context = { confirmedAuthorTruths: [{ id: 'contract', subjectEntityId: 'secret' } as never], readerKnowledgeAtPosition: [], povCharacterKnowledgeAtPosition: [], participantKnowledgeAtPosition: [], allowedClues: [], forbiddenClues: [], requiredClues: [], plannedReveals: [], warnings: [] };
    const input: RevealComplianceInput = { projectId: 'project', manuscriptPosition: {}, text: 'Kaelen war Daniel.', textKind: 'manuscript', participatingCharacterIds: [], revealContext: context };
    const result: RevealComplianceResult = { findings: [{ findingType: 'premature_revelation', severity: 'critical', contractId: 'contract', subjectEntityId: 'secret', evidenceExcerpt: 'Kaelen war Daniel.', explanation: 'Direkte Identitätsaussage vor dem Reveal.', expectedKnowledgeLevel: 'unknown', actualDisclosureLevel: 'knows', confidence: 1, startOffset: 0, endOffset: 18 }], warnings: [] };
    expect(validateRevealComplianceResultReferences(input, result)).toEqual(result);
    expect(() => validateRevealComplianceResultReferences(input, { ...result, findings: [{ ...result.findings[0]!, contractId: 'future' }] })).toThrow('CODEX_INVALID_REFERENCE');
  });

  it('wählt Audience States in Browser und Tauri-kompatibler Reihenfolge und lehnt ungültige Intervalle ab', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const book = workspace.books[0]!;
    const chapter = await repository.createChapter({ bookId: book.id, title: 'Auswahl' });
    const scene = await repository.createScene({ chapterId: chapter.id, title: 'Position' });
    const subject = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Auswahl-Wahrheit', type: 'secret', description: '', status: 'confirmed', confidence: 1, chapterId: chapter.id, sceneId: scene.id, excerpt: '', authorConfirmed: true, tags: [] });
    const contract = await repository.saveRevealContract({ projectId: workspace.project.id, subjectEntityId: subject.id, title: 'Auswahl', truthStatement: 'Eine Testwahrheit.', scope: 'book', status: 'confirmed', authorConfirmed: true, revealState: 'author_only', revealConditionText: '', notes: '' });
    const current = { bookId: book.id, chapterId: chapter.id, sceneId: scene.id, offset: 0 };
    const broad = await repository.saveRevealAudienceState({ projectId: workspace.project.id, contractId: contract.id, audienceKind: 'reader', knowledgeLevel: 'unknown', beliefText: '', validFromPosition: { chapterId: chapter.id }, status: 'confirmed', authorConfirmed: true });
    const precise = await repository.saveRevealAudienceState({ projectId: workspace.project.id, contractId: contract.id, audienceKind: 'reader', knowledgeLevel: 'knows', beliefText: '', validFromPosition: current, status: 'confirmed', authorConfirmed: true });
    expect((await buildRevealContext(repository, { projectId: workspace.project.id, position: current })).readerKnowledgeAtPosition[0]?.id).toBe(precise.id);
    const equalA = await repository.saveRevealAudienceState({ projectId: workspace.project.id, contractId: contract.id, audienceKind: 'reader', knowledgeLevel: 'partial', beliefText: '', validFromPosition: current, status: 'confirmed', authorConfirmed: true });
    const equalB = await repository.saveRevealAudienceState({ projectId: workspace.project.id, contractId: contract.id, audienceKind: 'reader', knowledgeLevel: 'suspects', beliefText: '', validFromPosition: current, status: 'confirmed', authorConfirmed: true });
    const expected = [precise, equalA, equalB].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt) || right.id.localeCompare(left.id))[0]!;
    const context = await buildRevealContext(repository, { projectId: workspace.project.id, position: current });
    expect(context.readerKnowledgeAtPosition[0]?.id).toBe(expected.id);
    expect(context.warnings).toContain(`Widersprüchliche gleichrangige Wissensstände für ${contract.id}:reader:reader.`);
    expect(broad.id).not.toBe(context.readerKnowledgeAtPosition[0]?.id);
    await expect(repository.saveRevealAudienceState({ projectId: workspace.project.id, contractId: contract.id, audienceKind: 'reader', knowledgeLevel: 'unknown', beliefText: '', validFromPosition: current, validUntilPosition: current, status: 'confirmed', authorConfirmed: true })).rejects.toThrow('validUntil strikt nach validFrom');
    await expect(repository.saveRevealClueRule({ projectId: workspace.project.id, contractId: contract.id, ruleKind: 'forbidden', clueType: 'direct', description: 'Test', maximumExplicitness: 'direct', validFromPosition: {}, validUntilPosition: {}, status: 'confirmed', authorConfirmed: true })).rejects.toThrow('validUntil strikt nach validFrom');
  });
});
