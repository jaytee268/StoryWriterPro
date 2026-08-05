import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDemoRepository } from './storyRepository';

const store = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value), removeItem: (key: string) => store.delete(key) });

describe('Onboarding und Importversionen', () => {
  beforeEach(() => store.clear());

  it('legt bei identischem Originalhash eine getrennte Importversion an', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const chapter = workspace.chapters[0]!; const scene = chapter.scenes[0]!;
    const input = { projectId: workspace.project.id, bookId: workspace.books[0]!.id, importReference: 'same-original-hash', providerId: 'local-prototype', units: [{ chapterId: chapter.id, sceneId: scene.id, orderIndex: 0, startOffset: 0, endOffset: 5, content: 'Text', contentHash: 'hash' }] };
    const first = await repository.createManuscriptAnalysisJob(input);
    expect(await repository.createManuscriptAnalysisJob(input)).toMatchObject({ id: first.id });
    const second = await repository.createManuscriptAnalysisJob({ ...input, newVersion: true });
    expect(second.id).not.toBe(first.id);
    expect(second.importReference).toBe('same-original-hash:version-2');
    expect(await repository.listManuscriptAnalysisJobs(workspace.project.id)).toHaveLength(2);
  });

  it('speichert bewusste Lore-Entscheidungen und verknüpft den Lauf für Resume', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace();
    const state = await repository.getProjectOnboardingState(workspace.project.id);
    const saved = await repository.saveProjectOnboardingState({ ...state, currentStep: 'manuscript', completedSteps: ['project', 'lore'], skippedSteps: [], loreCrafterRunId: 'lore-run-resume' });
    expect(saved).toMatchObject({ currentStep: 'manuscript', loreCrafterRunId: 'lore-run-resume' });
  });
});
