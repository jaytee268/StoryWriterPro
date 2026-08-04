import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDemoRepository } from './storyRepository';
import { detectContinuityFindings, runContinuityReview, shouldRunContinuityReview } from './continuityReview';

const values = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value), removeItem: (key: string) => values.delete(key) });

describe('dynamischer Continuity-Unterbau', () => {
  beforeEach(() => values.clear());

  it('löst Verfügbarkeit und Besitzerwechsel an Manuskriptpositionen auf', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const entity = workspace.entities.find((item) => item.type === 'clue')!;
    const owner = workspace.entities.find((item) => item.type === 'character')!;
    const otherScene = workspace.chapters[1]!.scenes[0]!;
    await repository.saveContinuityStateEntry({ projectId: workspace.project.id, entityId: entity.id, stateKind: 'item_availability', previousState: '', newState: 'verfügbar', chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id, status: 'confirmed', confidence: 1, authorConfirmed: true });
    await repository.saveContinuityStateEntry({ projectId: workspace.project.id, entityId: entity.id, stateKind: 'item_availability', previousState: 'verfügbar', newState: 'nicht verfügbar', chapterId: workspace.chapters[1]!.id, sceneId: otherScene.id, status: 'confirmed', confidence: 1, authorConfirmed: true });
    await repository.saveContinuityStateEntry({ projectId: workspace.project.id, entityId: entity.id, relatedEntityId: owner.id, stateKind: 'ownership', previousState: '', newState: owner.name, chapterId: workspace.chapters[1]!.id, sceneId: otherScene.id, status: 'confirmed', confidence: 1, authorConfirmed: true });
    await expect(repository.getStateAtPosition(workspace.project.id, entity.id, 'item_availability', { chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id })).resolves.toMatchObject({ newState: 'verfügbar' });
    await expect(repository.getStateAtPosition(workspace.project.id, entity.id, 'item_availability', { chapterId: workspace.chapters[1]!.id, sceneId: otherScene.id })).resolves.toMatchObject({ newState: 'nicht verfügbar' });
    await expect(repository.getStateAtPosition(workspace.project.id, entity.id, 'ownership', { chapterId: workspace.chapters[0]!.id, sceneId: workspace.chapters[0]!.scenes[0]!.id })).resolves.toBeUndefined();
  });

  it('führt eine bestätigte Verletzung in spätere Szenen fort und schließt Zukunft aus', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const character = workspace.entities.find((item) => item.type === 'character')!;
    const future = workspace.chapters[2]!.scenes[0]!;
    await repository.saveContinuityStateEntry({ projectId: workspace.project.id, entityId: character.id, stateKind: 'injury', previousState: '', newState: 'verletzte Hand', chapterId: workspace.chapters[2]!.id, sceneId: future.id, status: 'confirmed', confidence: 1, authorConfirmed: true });
    await expect(repository.getStateAtPosition(workspace.project.id, character.id, 'injury', { chapterId: workspace.chapters[1]!.id, sceneId: workspace.chapters[1]!.scenes[0]!.id })).resolves.toBeUndefined();
    await expect(repository.getStateAtPosition(workspace.project.id, character.id, 'injury', { chapterId: workspace.chapters[2]!.id, sceneId: future.id })).resolves.toMatchObject({ newState: 'verletzte Hand' });
  });

  it('hält unbestätigte Projektregeln aus dem aktiven Regelkontext heraus', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const pending = await repository.saveProjectRule({ projectId: workspace.project.id, title: 'Offene Regel', statement: 'Noch nicht bestätigt.', scope: 'project', prerequisites: [], effects: [], exceptions: [], connectedLoreIds: [], sourceReferenceIds: [], status: 'proposed', confidence: 0.8, authorConfirmed: false, origin: 'bible_update' });
    expect(await repository.listProjectRules(workspace.project.id, true)).not.toContainEqual(pending);
    const confirmed = await repository.saveProjectRule({ ...pending, status: 'confirmed', authorConfirmed: true });
    expect(await repository.listProjectRules(workspace.project.id, true)).toContainEqual(confirmed);
  });

  it('meldet einen weggeworfenen Gegenstand objektiv, ohne selbst eine Lore-Erklärung zu erfinden', () => {
    const project = { id: 'p', title: 'Test', author: '', description: '', updatedAt: '', wordCount: 0, openWarnings: 0, bibleProgress: 0 };
    const chapter = { id: 'c', bookId: 'b', title: 'Kapitel 1', orderIndex: 1, scenes: [{ id: 's', chapterId: 'c', title: 'Szene', orderIndex: 1, content: '', pov: '', location: '', storyTime: '', status: 'draft' as const, goal: '', notes: '' }] };
    const entity = { id: 'zettel', projectId: 'p', name: 'Zettel', type: 'object' as const, description: '', status: 'confirmed' as const, confidence: 1, source: '', chapter: '', scene: '', authorConfirmed: true, updatedAt: '', tags: [], origin: 'manual' as const };
    const state = { id: 'state', projectId: 'p', entityId: 'zettel', stateKind: 'item_availability' as const, previousState: 'verfügbar', newState: 'weggeworfen', chapterId: 'c', sceneId: 's', status: 'confirmed' as const, confidence: 1, authorConfirmed: true, createdAt: '', updatedAt: '' };
    const findings = detectContinuityFindings({ project, chapter, scene: chapter.scenes[0], chapters: [chapter], entities: [entity], ledger: [state], rules: [], currentText: 'Malik zeigt denselben Zettel einer anderen Figur.', sourceKind: 'bible_update' });
    expect(findings[0]).toMatchObject({ findingType: 'critical_contradiction', severity: 'critical' });
    expect(findings[0]?.loreExplanations).toEqual([]);
  });

  it('stuft eine mögliche Ausnahme bei einer körperlichen Eigenschaft als Erklärungslücke ein', () => {
    const project = { id: 'p', title: 'Test', author: '', description: '', updatedAt: '', wordCount: 0, openWarnings: 0, bibleProgress: 0 };
    const chapter = { id: 'c', bookId: 'b', title: 'Kapitel 1', orderIndex: 1, scenes: [{ id: 's', chapterId: 'c', title: 'Szene', orderIndex: 1, content: '', pov: '', location: '', storyTime: '', status: 'draft' as const, goal: '', notes: '' }] };
    const entity = { id: 'malik', projectId: 'p', name: 'Malik', type: 'character' as const, description: '', status: 'confirmed' as const, confidence: 1, source: '', chapter: '', scene: '', authorConfirmed: true, updatedAt: '', tags: [], origin: 'manual' as const };
    const state = { id: 'state', projectId: 'p', entityId: 'malik', stateKind: 'physical_condition' as const, previousState: '', newState: 'laktoseintolerant', chapterId: 'c', sceneId: 's', status: 'confirmed' as const, confidence: 1, authorConfirmed: true, createdAt: '', updatedAt: '' };
    const findings = detectContinuityFindings({ project, chapter, scene: chapter.scenes[0], chapters: [chapter], entities: [entity], ledger: [state], rules: [], currentText: 'Malik trinkt Milch.', sourceKind: 'bible_update' });
    expect(findings[0]).toMatchObject({ findingType: 'missing_explanation', severity: 'warning' });
  });

  it('startet die Wortschwellenprüfung erst nach der konfigurierten Menge neuer Wörter', () => {
    expect(shouldRunContinuityReview('Ein kurzer Anfang.', `${'neues '.repeat(20)}Ende.`, 300, 'word_threshold')).toBe(false);
    expect(shouldRunContinuityReview('Ein kurzer Anfang.', `${'neues '.repeat(301)}Ende.`, 300, 'word_threshold')).toBe(true);
    expect(shouldRunContinuityReview('Ein kurzer Anfang.', 'Seitenmarker-Prüfung.', 300, 'page_marker')).toBe(true);
  });

  it('trennt einen objektiven Konflikt von einer bestätigten möglichen Lore-Erklärung', () => {
    const project = { id: 'p', title: 'Test', author: '', description: '', updatedAt: '', wordCount: 0, openWarnings: 0, bibleProgress: 0 };
    const chapter = { id: 'c', bookId: 'b', title: 'Kapitel 1', orderIndex: 1, scenes: [{ id: 's', chapterId: 'c', title: 'Szene', orderIndex: 1, content: '', pov: '', location: '', storyTime: '', status: 'draft' as const, goal: '', notes: '' }] };
    const entity = { id: 'zettel', projectId: 'p', name: 'Zettel', type: 'object' as const, description: '', status: 'confirmed' as const, confidence: 1, source: '', chapter: '', scene: '', authorConfirmed: true, updatedAt: '', tags: [], origin: 'manual' as const };
    const state = { id: 'state', projectId: 'p', entityId: 'zettel', stateKind: 'item_availability' as const, previousState: 'verfügbar', newState: 'weggeworfen', chapterId: 'c', sceneId: 's', status: 'confirmed' as const, confidence: 1, authorConfirmed: true, createdAt: '', updatedAt: '' };
    const rule = { id: 'rule', projectId: 'p', title: 'Rückkehr', statement: 'Ein bestätigter Zustand kann unter einer ausdrücklich genannten Bedingung zurückkehren.', scope: 'project' as const, prerequisites: [], effects: [], exceptions: [], connectedLoreIds: ['zettel'], sourceReferenceIds: [], status: 'confirmed' as const, confidence: 1, authorConfirmed: true, origin: 'manual' as const, createdAt: '', updatedAt: '' };
    const findings = detectContinuityFindings({ project, chapter, scene: chapter.scenes[0], chapters: [chapter], entities: [entity], ledger: [state], rules: [rule], currentText: 'Eine Bedingung erfüllt sich: Er zeigt denselben Zettel.', sourceKind: 'bible_update' });
    expect(findings[0]).toMatchObject({ findingType: 'lore_compatible_anomaly', severity: 'warning', objectiveConflict: expect.stringContaining('weggeworfen') });
    expect(findings[0]?.loreExplanations).toHaveLength(1);
  });

  it('erzeugt einen überprüfbaren Abschlussvorschlag für einen Handlungsstrang, ohne ihn selbst abzuschließen', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const chapter = workspace.chapters[0]!;
    const scene = chapter.scenes[0]!;
    const thread = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Offene Frage', type: 'plot_thread', description: 'Eine offene Spur', status: 'confirmed', confidence: 1, chapterId: chapter.id, sceneId: scene.id, excerpt: '', authorConfirmed: true, tags: [] });
    await runContinuityReview(repository, { project: workspace.project, chapter, scene, currentText: 'Die offene Frage ist geklärt.', sourceKind: 'manual' });
    const proposals = await repository.listPlotThreadLifecycleProposals(workspace.project.id);
    expect(proposals).toHaveLength(1);
    expect(proposals[0]).toMatchObject({ entityId: thread.id, proposedStatus: 'closure_candidate', reviewStatus: 'pending' });
    expect(await repository.listPlotThreadLifecycles(workspace.project.id)).not.toContainEqual(expect.objectContaining({ entityId: thread.id, lifecycleStatus: 'resolved' }));
  });
});
