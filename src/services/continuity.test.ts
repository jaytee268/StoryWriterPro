import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDemoRepository } from './storyRepository';

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
});
