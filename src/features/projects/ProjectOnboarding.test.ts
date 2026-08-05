// @vitest-environment jsdom
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ProjectOnboarding } from './ProjectOnboarding';
import type { Project, WorkspaceSnapshot } from '../../types/domain';
import type { StoryRepository } from '../../services/storyRepository';

const testGlobal = globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean };
testGlobal.IS_REACT_ACT_ENVIRONMENT = true;

const project: Project = { id: 'project-1', title: 'Test', author: '', description: '', status: 'active', createdAt: '', updatedAt: '', wordCount: 0, openWarnings: 0, bibleProgress: 0 };

function workspace(): WorkspaceSnapshot {
  return { project, books: [{ id: 'book-1', projectId: project.id, title: 'Test', volume: 1, secondaryGenreIds: [], customGenreNames: [], genreAuthorConfirmed: false }], chapters: [], entities: [], versions: [], editorPreferences: { fontFamily: 'sans', fontSize: 16, lineHeight: 1.7 }, sources: [], sourceDocuments: [], manuscriptImportVersions: [], runs: [], proposals: [], lore: [], characterProfiles: [], characterStates: [], styleReferences: [], styleRuns: [], styleObservations: [], summaries: [], relations: [], rules: [], ruleProposals: [], continuityLedger: [], continuitySettings: { projectId: project.id, wordThreshold: 300, updatedAt: '' }, continuityRuns: [], continuityFindings: [], continuityDecisions: [], continuityCanonAudits: [], manuscriptAnalysisJobs: [], manuscriptAnalysisUnits: [], manuscriptAnalysisDraftLedger: [], manuscriptAnalysisPhaseResults: [], manuscriptAnalysisArtifacts: [], manuscriptAnalysisReviewAudits: [], manuscriptStructureRuns: [], manuscriptStructureProposals: [], provisionalEntities: [], provisionalMentions: [], provisionalMergeProposals: [], plotThreadLifecycles: [], plotThreadProposals: [], voicePatterns: [], experiences: [], dialogueMemories: [], relationshipMemories: [], knowledgeStates: [], knowledgeHistory: [], memoryEvidence: [], memoryRuns: [], memoryProposals: [] } as WorkspaceSnapshot;
}

function inputValue(input: HTMLInputElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
  setter?.call(input, value);
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

function textareaValue(textarea: HTMLTextAreaElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set;
  setter?.call(textarea, value);
  textarea.dispatchEvent(new Event('input', { bubbles: true }));
}

describe('Projekt-Onboarding', () => {
  afterEach(() => { window.localStorage.clear(); document.body.replaceChildren(); });

  it('legt vor dem letzten Schritt kein Projekt an und speichert am Ende ohne Genre', async () => {
    const saveBookGenres = vi.fn();
    const created = { ...project, id: 'created-1', title: 'Mein Buch' };
    const repository = { createProject: vi.fn(async () => created), loadWorkspace: vi.fn(async () => workspace()), saveBookGenres, saveProjectOnboardingState: vi.fn(async (input) => ({ ...input, updatedAt: '' })) } as unknown as StoryRepository;
    const container = document.createElement('div');
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(createElement(ProjectOnboarding, { repository, onCreated: vi.fn(), onContinue: vi.fn(), onOpenLore: vi.fn(), onOpenImport: vi.fn() })));
    await act(async () => { inputValue(container.querySelector('#project-title') as HTMLInputElement, 'Mein Buch'); container.querySelector('form')?.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); });
    expect(repository.createProject).not.toHaveBeenCalled();
    expect(container.textContent).toContain('Möchtest du deine Welt vorbereiten?');
    await act(async () => (container.querySelector('button.primary-button') as HTMLButtonElement).click());
    expect(container.textContent).toContain('Gibt es schon Manuskripttext?');
    await act(async () => (container.querySelector('button.primary-button') as HTMLButtonElement).click());
    expect(container.textContent).toContain('Alles bereit?');
    await act(async () => [...container.querySelectorAll('button')].find((button) => button.textContent?.includes('Projekt anlegen'))?.click());
    expect(repository.createProject).toHaveBeenCalled();
    expect(saveBookGenres).not.toHaveBeenCalled();
    act(() => root.unmount());
  });

  it('stellt den lokalen Fortschritt nach einem Neustart wieder her und verwirft ihn beim Abbrechen', async () => {
    const repository = { createProject: vi.fn(), loadWorkspace: vi.fn(), saveBookGenres: vi.fn(), saveProjectOnboardingState: vi.fn() } as unknown as StoryRepository;
    const firstContainer = document.createElement('div');
    document.body.append(firstContainer);
    const firstRoot = createRoot(firstContainer);
    await act(async () => firstRoot.render(createElement(ProjectOnboarding, { repository, onCreated: vi.fn(), onContinue: vi.fn(), onOpenLore: vi.fn(), onOpenImport: vi.fn() })));
    await act(async () => { inputValue(firstContainer.querySelector('#project-title') as HTMLInputElement, 'Gespeicherter Entwurf'); firstContainer.querySelector('form')?.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); });
    act(() => firstRoot.unmount());
    firstContainer.remove();
    const secondContainer = document.createElement('div');
    document.body.append(secondContainer);
    const secondRoot = createRoot(secondContainer);
    await act(async () => secondRoot.render(createElement(ProjectOnboarding, { repository, onCreated: vi.fn(), onContinue: vi.fn(), onOpenLore: vi.fn(), onOpenImport: vi.fn() })));
    expect(secondContainer.textContent).toContain('Möchtest du deine Welt vorbereiten?');
    await act(async () => (secondContainer.querySelector('button.primary-button') as HTMLButtonElement).click());
    await act(async () => (secondContainer.querySelector('button.primary-button') as HTMLButtonElement).click());
    expect(secondContainer.textContent).toContain('Gespeicherter Entwurf');
    await act(async () => (secondContainer.querySelector('.onboarding-flow-top .text-button') as HTMLButtonElement).click());
    expect(window.localStorage.getItem('storymemory.new-project-onboarding.v1')).toBeNull();
    expect(repository.createProject).not.toHaveBeenCalled();
    act(() => secondRoot.unmount());
  });

  it('übernimmt Lore-Notizen erst nach dem finalen Anlegen in den Lore-Crafter-Kontext', async () => {
    const created = { ...project, id: 'created-lore-1', title: 'Lore-Buch' };
    const onCreated = vi.fn();
    const repository = { createProject: vi.fn(async () => created), loadWorkspace: vi.fn(async () => workspace()), saveBookGenres: vi.fn(), saveProjectOnboardingState: vi.fn(async (input) => ({ ...input, updatedAt: '' })) } as unknown as StoryRepository;
    const container = document.createElement('div');
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(createElement(ProjectOnboarding, { repository, onCreated, onContinue: vi.fn(), onOpenLore: vi.fn(), onOpenImport: vi.fn() })));
    await act(async () => { inputValue(container.querySelector('#project-title') as HTMLInputElement, 'Lore-Buch'); container.querySelector('form')?.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); });
    await act(async () => { textareaValue(container.querySelector('#onboarding-lore-notes') as HTMLTextAreaElement, 'Die Stadt schützt ihre Archive durch ein jährliches Ritual.'); (container.querySelector('button.primary-button') as HTMLButtonElement).click(); });
    await act(async () => textareaValue(container.querySelector('#onboarding-manuscript-text') as HTMLTextAreaElement, 'Kapitel 1\n\nMarek öffnete die Tür.'));
    expect(repository.createProject).not.toHaveBeenCalled();
    await act(async () => (container.querySelector('button.primary-button') as HTMLButtonElement).click());
    await act(async () => [...container.querySelectorAll('button')].find((button) => button.textContent?.includes('Projekt anlegen'))?.click());
    expect(onCreated).toHaveBeenCalledWith(created, expect.objectContaining({ currentStep: 'completed' }), 'Die Stadt schützt ihre Archive durch ein jährliches Ritual.', 'Kapitel 1\n\nMarek öffnete die Tür.');
    act(() => root.unmount());
  });
});
