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

describe('Projekt-Onboarding', () => {
  afterEach(() => document.body.replaceChildren());

  it('legt ein Projekt ohne Genre an und ruft saveBookGenres nicht auf', async () => {
    const saveBookGenres = vi.fn();
    const created = { ...project, id: 'created-1', title: 'Mein Buch' };
    const repository = { createProject: vi.fn(async () => created), loadWorkspace: vi.fn(async () => workspace()), saveBookGenres, saveProjectOnboardingState: vi.fn(async (input) => ({ ...input, updatedAt: '' })) } as unknown as StoryRepository;
    const container = document.createElement('div');
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(createElement(ProjectOnboarding, { repository, onCreated: vi.fn(), onContinue: vi.fn(), onOpenLore: vi.fn(), onOpenImport: vi.fn() })));
    await act(async () => { inputValue(container.querySelector('#project-title') as HTMLInputElement, 'Mein Buch'); container.querySelector('form')?.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); });
    expect(repository.createProject).toHaveBeenCalled();
    expect(saveBookGenres).not.toHaveBeenCalled();
    act(() => root.unmount());
  });
});
