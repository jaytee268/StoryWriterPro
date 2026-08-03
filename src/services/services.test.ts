import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DesktopCommandError, isTauriRuntime, desktopInvoke } from './desktop';
import { BrowserDemoRepository } from './storyRepository';
import { SceneSaveQueue } from './sceneSaveQueue';
import type { Scene } from '../types/domain';
import { LocalPrototypeBibleExtractor, contentHash } from './bibleExtractor';
import { DeterministicProjectContextBuilder } from './contextBuilder';
import { answerFromProjectContext } from './providerBridge';

const store = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value), removeItem: (key: string) => store.delete(key) });
vi.stubGlobal('crypto', { randomUUID: () => `browser-id-${Math.random().toString(16).slice(2)}` });

beforeEach(() => { store.clear(); vi.stubGlobal('window', {}); });

function firstScene(repository: BrowserDemoRepository): Promise<Scene> { return repository.loadWorkspace().then((workspace) => workspace.chapters[0]!.scenes[0]!); }

describe('Runtime und Repository', () => {
  it('erkennt Browser-Demo und Tauri getrennt', () => { expect(isTauriRuntime()).toBe(false); vi.stubGlobal('window', { __TAURI_INTERNALS__: {} }); expect(isTauriRuntime()).toBe(true); });
  it('lädt den BrowserDemoRepository isoliert', async () => { const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); expect(repository.mode).toBe('browser-demo'); expect(workspace.project.title).toBe('Zugestellt'); expect(workspace.chapters[2]?.scenes[0]?.content).toContain('Marek'); });
  it('übernimmt IDs und Beziehungen aus Repository-Ergebnissen', async () => { const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const chapter = await repository.createChapter({ bookId: workspace.books[0]!.id, title: 'Kapitel 4' }); const scene = await repository.createScene({ chapterId: chapter.id, title: 'Die Tür' }); expect(chapter.id).not.toBe('chapter-4'); expect(scene.chapterId).toBe(chapter.id); expect((await repository.loadWorkspace()).chapters.find((item) => item.id === chapter.id)?.scenes[0]?.id).toBe(scene.id); });
  it('speichert Text mit Umlauten und Zeilenumbrüchen', async () => { const repository = new BrowserDemoRepository(); const scene = await firstScene(repository); const text = 'Äpfel „sicher“\n\nZeile zwei.'; await repository.updateScene({ ...scene, content: text }); expect((await repository.loadWorkspace()).chapters[0]?.scenes[0]?.content).toBe(text); });
  it('legt bewusst gesicherte Versionen an und stellt einen älteren Stand wieder her', async () => { const repository = new BrowserDemoRepository(); const scene = await firstScene(repository); const first = await repository.updateScene({ ...scene, content: 'Erster Stand' }); await repository.createSceneVersion({ sceneId: scene.id }); const second = await repository.updateScene({ ...first, content: 'Zweiter Stand' }); await repository.createSceneVersion({ sceneId: scene.id }); const versions = await repository.listSceneVersions(scene.id); expect(versions).toHaveLength(2); const restored = await repository.restoreSceneVersion(scene.id, versions[1]!.id); expect(restored.content).toBe('Erster Stand'); expect(second.content).toBe('Zweiter Stand'); });
  it('speichert Editor-Schrift und Layout lokal', async () => { const repository = new BrowserDemoRepository(); await repository.saveEditorPreferences({ fontFamily: 'typewriter', fontSize: 22, lineHeight: 2.1 }); expect(await repository.getEditorPreferences()).toEqual({ fontFamily: 'typewriter', fontSize: 22, lineHeight: 2.1 }); });
});

describe('Desktop-Fehler und Autosave', () => {
  it('verschluckt Tauri-Fehler nicht', async () => { await expect(desktopInvoke('load_workspace')).rejects.toBeInstanceOf(DesktopCommandError); await expect(desktopInvoke('load_workspace')).rejects.toMatchObject({ command: 'load_workspace' }); });
  it('wechselt sauber zwischen dirty, saving und saved', async () => { const statuses: string[] = []; const scene = { ...(await firstScene(new BrowserDemoRepository())), content: 'Neu' }; const queue = new SceneSaveQueue(async (value) => value, { onStatus: (status) => statuses.push(status), onSaved: vi.fn(), onError: vi.fn() }, 1); queue.schedule(scene); await new Promise((resolve) => setTimeout(resolve, 5)); await queue.flush(); expect(statuses).toEqual(['dirty', 'saving', 'saved']); });
  it('lässt eine alte Antwort keinen neueren Text überschreiben', async () => { let resolveFirst: (() => void) | undefined; let calls = 0; const saved: string[] = []; const scene = await firstScene(new BrowserDemoRepository()); const queue = new SceneSaveQueue((value) => new Promise<Scene>((resolve) => { calls += 1; if (calls === 1) resolveFirst = () => resolve(value); else resolve(value); }), { onStatus: () => undefined, onSaved: (value) => saved.push(value.content), onError: () => undefined }, 1); queue.schedule({ ...scene, content: 'alt' }); const first = queue.flush(); queue.schedule({ ...scene, content: 'neu' }); resolveFirst?.(); await first; expect(saved).toEqual(['neu']); });
  it('speichert nach dem Debounce nur die letzte Fassung', async () => { const scene = await firstScene(new BrowserDemoRepository()); const saved: string[] = []; const queue = new SceneSaveQueue(async (value) => { saved.push(value.content); return value; }, { onStatus: () => undefined, onSaved: vi.fn(), onError: vi.fn() }, 5); queue.schedule({ ...scene, content: 'eins' }); queue.schedule({ ...scene, content: 'zwei' }); await new Promise((resolve) => setTimeout(resolve, 15)); expect(saved).toEqual(['zwei']); });
  it('führt zwei schnelle Flush-Aufrufe ohne parallelen Konflikt aus', async () => { const scene = await firstScene(new BrowserDemoRepository()); let calls = 0; let release: (() => void) | undefined; const queue = new SceneSaveQueue((value) => new Promise<Scene>((resolve) => { calls += 1; release = () => resolve(value); }), { onStatus: () => undefined, onSaved: vi.fn(), onError: vi.fn() }); queue.schedule({ ...scene, content: 'einmal' }); const first = queue.flush(); const second = queue.flush(); expect(calls).toBe(1); release?.(); await Promise.all([first, second]); expect(calls).toBe(1); });
  it('behält einen fehlgeschlagenen Snapshot für den Retry', async () => { const scene = await firstScene(new BrowserDemoRepository()); let attempts = 0; const saved: string[] = []; const queue = new SceneSaveQueue(async (value) => { attempts += 1; if (attempts === 1) throw new Error('SQLite nicht erreichbar'); saved.push(value.content); return value; }, { onStatus: () => undefined, onSaved: vi.fn(), onError: vi.fn() }, 1); queue.schedule({ ...scene, content: 'Retry-Inhalt' }); await queue.flush(); expect(queue.hasPendingChanges()).toBe(true); expect(queue.getStatus()).toBe('error'); await queue.flush(); expect(saved).toEqual(['Retry-Inhalt']); expect(queue.hasPendingChanges()).toBe(false); expect(queue.getStatus()).toBe('saved'); });
  it('meldet pending, saving, saved und error korrekt', async () => { const scene = await firstScene(new BrowserDemoRepository()); let release: (() => void) | undefined; const queue = new SceneSaveQueue((value) => new Promise<Scene>((resolve) => { release = () => resolve(value); }), { onStatus: () => undefined, onSaved: vi.fn(), onError: vi.fn() }); expect(queue.hasPendingChanges()).toBe(false); queue.schedule(scene); expect(queue.hasPendingChanges()).toBe(true); expect(queue.getStatus()).toBe('dirty'); const flush = queue.flush(); expect(queue.getStatus()).toBe('saving'); expect(queue.hasPendingChanges()).toBe(true); release?.(); await flush; expect(queue.getStatus()).toBe('saved'); expect(queue.hasPendingChanges()).toBe(false); });
});

describe('Story-Bible-Review und grounded context', () => {
  it('legt, bearbeitet und archiviert einen Story-Bible-Eintrag', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const created = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Neue Spur', type: 'clue', description: 'Eine überprüfbare Beobachtung.', status: 'proposed', confidence: 0.7, excerpt: 'Eine Spur', authorConfirmed: false, tags: ['Test'] });
    const edited = await repository.updateStoryEntity({ ...created, projectId: workspace.project.id, name: 'Bearbeitete Spur', excerpt: 'Neue Passage' });
    expect(edited.name).toBe('Bearbeitete Spur');
    expect((await repository.archiveStoryEntity(edited.id)).status).toBe('archived');
    expect((await repository.listStoryEntities(workspace.project.id)).find((entity) => entity.id === edited.id)?.status).toBe('archived');
  });
  it('trennt beobachtbare Fakten und Vermutungen im Prototype-Extractor', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const scene = workspace.chapters[2]!.scenes[0]!;
    const result = await new LocalPrototypeBibleExtractor().extract({ project: workspace.project, chapter: workspace.chapters[2]!, scene, existingEntities: workspace.entities });
    expect(result.proposals.some((proposal) => proposal.classification === 'observable_fact')).toBe(true);
    expect(result.proposals.every((proposal) => proposal.candidateDescription !== 'Marek hasst Lena.')).toBe(true);
  });
  it('verwendet bei identischem Content Hash denselben abgeschlossenen Run', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const scene = workspace.chapters[0]!.scenes[0]!;
    const input = { projectId: workspace.project.id, sceneId: scene.id, sceneUpdatedAt: scene.updatedAt ?? '', contentHash: contentHash(scene.content), extractorId: 'local-prototype-extractor' };
    const first = await repository.createBibleUpdateRun(input);
    await repository.saveBibleProposals(first.id, [{ proposalAction: 'create_entity', entityType: 'fact', candidateName: 'Testfakt', candidateDescription: 'Beobachtet.', candidateStatus: 'proposed', confidence: 0.5, classification: 'observable_fact', evidenceExcerpt: 'Text', reason: 'Test' }], input.projectId, input.sceneId);
    const second = await repository.createBibleUpdateRun(input);
    expect(second.id).toBe(first.id);
  });
  it('baut Chat-Antworten nur aus dem aktuellen Kontext', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const builder = new DeterministicProjectContextBuilder(repository);
    const context = await builder.build({ projectId: workspace.project.id, currentChapterId: workspace.chapters[2]!.id, currentSceneId: workspace.chapters[2]!.scenes[0]!.id, userQuestion: 'Welche Figuren kommen vor?' });
    const answer = answerFromProjectContext('Welche Figuren kommen vor?', context);
    expect(answer.text).toContain('Marek');
    expect(answer.sources.every((source) => source.id)).toBe(true);
  });
});
