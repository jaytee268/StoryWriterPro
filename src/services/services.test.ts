import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DesktopCommandError, isTauriRuntime, desktopInvoke } from './desktop';
import { BrowserDemoRepository } from './storyRepository';
import { SceneSaveQueue } from './sceneSaveQueue';
import type { ProjectContext, Scene } from '../types/domain';
import { LocalPrototypeBibleExtractor, changedRange, contentHash, excerptFor } from './bibleExtractor';
import { DeterministicProjectContextBuilder } from './contextBuilder';
import { answerFromProjectContext } from './providerBridge';
import { buildCodexBibleRequest, providerRouter } from './aiProviderService';
import { canonicalizeSceneForAi, contentHash as canonicalContentHash, unicodeIndexOf, unicodeSlice } from '../utils/aiText';
import { editorContentToPlainText } from '../utils/editorContent';
import { LocalPrototypeCharacterMemoryExtractor } from './characterMemoryExtractor';
import { contextHashForLongform } from './longformWorkflow';

const store = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value), removeItem: (key: string) => store.delete(key) });
vi.stubGlobal('crypto', { randomUUID: () => `browser-id-${Math.random().toString(16).slice(2)}` });

beforeEach(() => { store.clear(); vi.stubGlobal('window', {}); });

function firstScene(repository: BrowserDemoRepository): Promise<Scene> { return repository.loadWorkspace().then((workspace) => workspace.chapters[0]!.scenes[0]!); }

describe('Runtime und Repository', () => {
  it('erkennt Browser-Demo und Tauri getrennt', () => { expect(isTauriRuntime()).toBe(false); vi.stubGlobal('window', { __TAURI_INTERNALS__: {} }); expect(isTauriRuntime()).toBe(true); });
  it('lädt den BrowserDemoRepository isoliert', async () => { const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); expect(repository.mode).toBe('browser-demo'); expect(workspace.project.title).toBe('Zugestellt'); expect(workspace.chapters[2]?.scenes[0]?.content).toContain('Marek'); });
  it('übernimmt IDs und Beziehungen aus Repository-Ergebnissen', async () => { const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const chapter = await repository.createChapter({ bookId: workspace.books[0]!.id, title: 'Kapitel 4' }); const scene = await repository.createScene({ chapterId: chapter.id, title: 'Die Tür' }); expect(chapter.id).not.toBe('chapter-4'); expect(scene.chapterId).toBe(chapter.id); expect((await repository.loadWorkspace()).chapters.find((item) => item.id === chapter.id)?.scenes[0]?.id).toBe(scene.id); });
  it('importiert Kapitel im Browser atomar mit genau einem Kapiteltext und einer Version', async () => { const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const result = await repository.importManuscript({ projectId: workspace.project.id, bookId: workspace.books[0]!.id, chapters: [{ title: 'Import 1', content: 'Erster Text' }, { title: 'Import 2', content: 'Zweiter Text' }] }); expect(result.chapters).toHaveLength(2); expect(result.chapters.every((chapter) => chapter.scenes.length === 1 && chapter.scenes[0]?.title === 'Kapiteltext')).toBe(true); expect(result.versions).toHaveLength(2); expect(result.versions.every((version) => version.reason === 'before_import')).toBe(true); expect((await repository.loadWorkspace()).chapters).toContainEqual(result.chapters[0]); });
  it('speichert Text mit Umlauten und Zeilenumbrüchen', async () => { const repository = new BrowserDemoRepository(); const scene = await firstScene(repository); const text = 'Äpfel „sicher“\n\nZeile zwei.'; await repository.updateScene({ ...scene, content: text }); expect((await repository.loadWorkspace()).chapters[0]?.scenes[0]?.content).toBe(text); });
  it('legt bewusst gesicherte Versionen an und stellt einen älteren Stand wieder her', async () => { const repository = new BrowserDemoRepository(); const scene = await firstScene(repository); const first = await repository.updateScene({ ...scene, content: 'Erster Stand' }); await repository.createSceneVersion({ sceneId: scene.id }); const second = await repository.updateScene({ ...first, content: 'Zweiter Stand' }); await repository.createSceneVersion({ sceneId: scene.id }); const versions = await repository.listSceneVersions(scene.id); expect(versions).toHaveLength(2); const restored = await repository.restoreSceneVersion(scene.id, versions[1]!.id); expect(restored.content).toBe('Erster Stand'); expect(second.content).toBe('Zweiter Stand'); });
  it('speichert Editor-Schrift und Layout lokal', async () => { const repository = new BrowserDemoRepository(); await repository.saveEditorPreferences({ fontFamily: 'typewriter', fontSize: 22, lineHeight: 2.1 }); expect(await repository.getEditorPreferences()).toEqual({ fontFamily: 'typewriter', fontSize: 22, lineHeight: 2.1 }); });
  it('legt ein leeres Projekt mit persistentem Onboarding an und archiviert es', async () => {
    const repository = new BrowserDemoRepository();
    const project = await repository.createProject({ title: 'Neues Buch', author: 'Autorin', description: '', volumeTitle: 'Band 1', volume: 1 });
    const initial = await repository.getProjectOnboardingState(project.id);
    expect(initial.currentStep).toBe('project');
    const progressed = await repository.saveProjectOnboardingState({ ...initial, currentStep: 'manuscript', completedSteps: ['project', 'lore'], skippedSteps: [], language: 'de', genre: 'Roman' });
    expect((await repository.getProjectOnboardingState(project.id)).currentStep).toBe('manuscript');
    expect(progressed.genre).toBe('Roman');
    await repository.archiveProject(project.id);
    expect(await repository.listProjects()).toEqual([]);
  });
  it('bewahrt den unveränderten Importtext als projektweite Quelle mit Hash', async () => {
    const repository = new BrowserDemoRepository();
    const project = await repository.createProject({ title: 'Importbuch', author: 'Autorin', description: '', volumeTitle: 'Band 1', volume: 1 });
    const originalText = 'Kapitel 1\n😀 Ein Anfang.';
    const source = await repository.createProjectSourceDocument({ projectId: project.id, sourceKind: 'external_text', title: 'manuskript.txt', content: originalText, contentHash: contentHash(originalText) });
    expect((await repository.listProjectSourceDocuments(project.id))[0]).toMatchObject({ id: source.id, content: originalText, contentHash: contentHash(originalText) });
  });
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
  it('speichert Charaktergedächtnis getrennt von freien Wissensnotizen', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const character = workspace.entities.find((entity) => entity.type === 'character')!; const scene = workspace.chapters[0]!.scenes[0]!;
    const pattern = await repository.saveCharacterVoicePattern({ projectId: workspace.project.id, characterId: character.id, patternType: 'signature_phrase', patternText: 'Schon gut.', description: '', contextCondition: '', confidence: 1, status: 'confirmed', authorConfirmed: true, occurrenceCount: 2 });
    const experience = await repository.saveCharacterExperience({ projectId: workspace.project.id, characterId: character.id, sceneId: scene.id, title: 'Das Paket', objectiveSummary: 'Eine überprüfbare Beobachtung.', subjectiveInterpretation: 'Er zweifelt an sich.', emotionalImpact: 'Verunsicherung.', lastingEffect: 'Mehr Vorsicht.', significance: 'major', memoryReliability: 'reliable', status: 'confirmed', authorConfirmed: true });
    expect((await repository.listCharacterVoicePatterns(workspace.project.id, character.id))[0]).toEqual(pattern); expect((await repository.listCharacterExperiences(workspace.project.id, character.id))[0]).toEqual(experience);
  });
  it('validiert Dialogteilnehmer und normalisiert Beziehungspaare', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const chars = workspace.entities.filter((entity) => entity.type === 'character'); const scene = workspace.chapters[0]!.scenes[0]!;
    const dialogue = await repository.saveCharacterDialogueMemory({ projectId: workspace.project.id, speakerId: chars[0]!.id, sceneId: scene.id, dialogueKind: 'promise', topic: 'Paket', summary: 'Ein Versprechen.', exactExcerpt: 'Ich verspreche es.', emotionalTone: '', hiddenIntent: '', significance: 'important', truthfulness: 'unknown', status: 'confirmed', authorConfirmed: true, participants: [{ characterId: chars[0]!.id, role: 'speaker' }, { characterId: chars[1]!.id, role: 'listener' }] });
    const relationship = await repository.saveRelationshipMemory({ projectId: workspace.project.id, characterAId: chars[1]!.id, characterBId: chars[0]!.id, memoryType: 'promise', title: 'Versprechen', summary: 'Gemeinsame Geschichte.', privateMeaning: '', relationshipEffect: '', significance: 'supporting', status: 'confirmed', authorConfirmed: true });
    expect(dialogue.participants).toHaveLength(2); expect(relationship.characterAId < relationship.characterBId).toBe(true);
  });
  it('lokaler Character-Extractor erfindet keine Psychologie', async () => {
    const repository = new BrowserDemoRepository(); const workspace = await repository.loadWorkspace(); const chapter = workspace.chapters[0]!; const scene = { ...chapter.scenes[0]!, content: 'Marek sagte: Ich komme später.' }; const character = workspace.entities.find((entity) => entity.type === 'character')!;
    const result = await new LocalPrototypeCharacterMemoryExtractor().extract({ project: workspace.project, chapter, scene, characters: [character], existingEntities: workspace.entities, context: { projectId: workspace.project.id, relevantEntities: workspace.entities, relevantSources: [], openPlotThreads: [], possibleContradictions: [] } });
    expect(result.proposals[0]?.classification).toBe('observable'); expect(result.proposals[0]?.payload).toHaveProperty('hiddenIntent', '');
  });
  it('normalisiert Rich Text für AI und hält Unicode-Offsets stabil', async () => {
    expect(editorContentToPlainText('<p>Marek <strong>lief</strong>.</p>')).toBe('Marek lief.');
    expect(editorContentToPlainText('<p>Erste Zeile<br>Zweite Zeile</p><p>Äpfel 😀</p>')).toBe('Erste Zeile\nZweite Zeile\nÄpfel 😀');
    const scene = { ...(await firstScene(new BrowserDemoRepository())), content: '<p>Äpfel <strong>😀</strong> liefen.</p>' };
    const canonical = canonicalizeSceneForAi(scene);
    expect(canonical.text).toBe('Äpfel 😀 liefen.');
    expect(unicodeSlice(canonical.text, 6, 7)).toBe('😀');
    expect(unicodeIndexOf(canonical.text, '😀')).toBe(6);
    expect(canonical.hash).toBe(canonicalContentHash(canonical.text));
    expect(changedRange('Äpfel 😀 grün.', 'Äpfel 😀 blau.')).toEqual({ start: 8, end: 12 });
  });

  it('übergibt beiden Extractor-Pfaden dieselbe kanonische Szene ohne HTML', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const chapter = workspace.chapters[0]!;
    const scene = { ...chapter.scenes[0]!, content: '<p>Marek <strong>lief</strong>.</p>' };
    const input = { project: workspace.project, chapter, scene, existingEntities: workspace.entities };
    expect(buildCodexBibleRequest(input).scene.content).toBe('Marek lief.');
    const local = await new LocalPrototypeBibleExtractor().extract(input);
    expect(local.warnings[0]).toContain('gespeicherte Szenenfassung');
  });

  it('ProviderRouter verwendet im Browser ausschließlich den lokalen Provider', async () => {
    const { provider, settings } = await providerRouter.getActiveProvider();
    expect(provider.id).toBe('local-prototype');
    expect(settings.allowLocalFallback).toBe(true);
    expect((await providerRouter.getProviderStatus('local-prototype')).available).toBe(true);
  });
  it('übernimmt einen Fakt als bestätigten Kanon und verhindert eine zweite Review-Aktion', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const scene = workspace.chapters[0]!.scenes[0]!;
    const run = await repository.createBibleUpdateRun({ projectId: workspace.project.id, sceneId: scene.id, sceneUpdatedAt: scene.updatedAt ?? '', contentHash: contentHash(scene.content), extractorId: 'local-prototype-extractor', analyzedContent: scene.content });
    const [proposal] = await repository.saveBibleProposals(run.id, [{ proposalAction: 'create_entity', entityType: 'fact', candidateName: 'Augenfarbe', candidateDescription: 'Mareks Augen waren grün.', candidateStatus: 'proposed', confidence: 0.95, classification: 'observable_fact', evidenceExcerpt: 'Mareks Augen waren grün.', reason: 'Test' }], workspace.project.id, scene.id);
    const reviewed = await repository.reviewBibleProposal({ proposalId: proposal!.id, reviewStatus: 'accepted', decision: 'accept' });
    const saved = (await repository.loadWorkspace()).entities.find((entity) => entity.id === reviewed.targetEntityId);
    expect(saved).toMatchObject({ status: 'confirmed', authorConfirmed: true, origin: 'bible_update' });
    await expect(repository.reviewBibleProposal({ proposalId: proposal!.id, reviewStatus: 'accepted', decision: 'accept' })).rejects.toThrow('bereits geprüft');
    expect(await repository.listSourceReferences(workspace.project.id, saved!.id)).toHaveLength(1);
  });

  it('speichert Vermutung unbestätigt und dedupliziert Quellen', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const scene = workspace.chapters[0]!.scenes[0]!;
    const created = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Testquelle', type: 'fact', description: 'Test', status: 'proposed', confidence: 0.8, chapterId: scene.chapterId, sceneId: scene.id, excerpt: 'Marek', authorConfirmed: false, tags: [] });
    await repository.updateStoryEntity({ ...created, chapterId: scene.chapterId, sceneId: scene.id, excerpt: 'Marek' });
    await repository.updateStoryEntity({ ...created, chapterId: scene.chapterId, sceneId: scene.id, excerpt: 'Marek sah' });
    expect(await repository.listSourceReferences(workspace.project.id, created.id)).toHaveLength(2);
  });

  it('berechnet den geänderten Bereich und lässt fehlende Excerpts ohne Offsets', () => {
    expect(changedRange('Mareks Augen waren grün.', 'Mareks Augen waren blau.')).toEqual({ start: 19, end: 23 });
    expect(excerptFor('Der Text enthält das Wort nicht.', 'fehlt')).toEqual({ excerpt: 'fehlt' });
    expect(excerptFor('Marek sah Lena.', 'Lena')).toEqual({ excerpt: 'Lena', startOffset: 10, endOffset: 14 });
  });

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
  it('erkennt beobachtbare Augenfarbe und schlägt bei geändertem Wert einen Widerspruch vor', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const chapter = workspace.chapters[0]!;
    const baseScene = chapter.scenes[0]!;
    const extractor = new LocalPrototypeBibleExtractor();
    const first = await extractor.extract({ project: workspace.project, chapter, scene: { ...baseScene, content: 'Mareks Augen waren grün.' }, existingEntities: [] });
    expect(first.proposals).toContainEqual(expect.objectContaining({ candidateName: 'Mareks Augenfarbe', classification: 'observable_fact', proposalAction: 'create_entity', startOffset: 0, endOffset: 23 }));
    const entity = await repository.createStoryEntity({ projectId: workspace.project.id, name: 'Mareks Augenfarbe', type: 'fact', description: 'Mareks Augen waren grün.', status: 'confirmed', confidence: 1, chapterId: chapter.id, sceneId: baseScene.id, excerpt: 'Mareks Augen waren grün.', authorConfirmed: true, tags: [] });
    const second = await extractor.extract({ project: workspace.project, chapter, scene: { ...baseScene, content: 'Mareks Augen waren plötzlich blau.' }, existingEntities: [entity] });
    expect(second.proposals).toContainEqual(expect.objectContaining({ targetEntityId: entity.id, classification: 'possible_contradiction', proposalAction: 'mark_contradiction' }));
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

  it('legt Lore atomar an und lädt strukturierte Relationen aus beiden Richtungen', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const lore = await repository.createLoreEntry({ projectId: workspace.project.id, name: 'Die Simulation', entityType: 'secret', description: 'Eine verborgene Ursache.', status: 'confirmed', category: 'mystery', scope: 'book', revealState: 'foreshadowed', importance: 'core', truthStatement: 'Die Räume können abweichen.', rulesText: '', exceptionsText: '', authorKnowledge: 'Der Autor kennt die Ursache.', readerKnowledge: '', revealPlan: '', tags: ['Mysterium'] });
    const target = workspace.entities.find((entity) => entity.name === 'Veränderte Paketnummer')!;
    const relation = await repository.createEntityRelation({ projectId: workspace.project.id, sourceEntityId: lore.entity.id, targetEntityId: target.id, relationType: 'explains', label: 'erklärt', authorConfirmed: true });
    expect((await repository.listEntityRelations(workspace.project.id, lore.entity.id))).toContainEqual(relation);
    expect((await repository.listEntityRelations(workspace.project.id, target.id))).toContainEqual(relation);
    expect((await repository.getLoreMetadata(workspace.project.id)).find((item) => item.entityId === lore.entity.id)).toMatchObject({ category: 'mystery', revealState: 'foreshadowed', importance: 'core' });
  });

  it('speichert eine verankerte Stilreferenz mit Unicode-Offsets ohne den Szenentext zu verändern', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const scene = workspace.chapters[0]!.scenes[0]!;
    const original = scene.content;
    const excerpt = 'Karton';
    const start = Array.from(original).findIndex((_, index) => Array.from(original).slice(index, index + Array.from(excerpt).length).join('') === excerpt);
    const reference = await repository.createStyleReference({ projectId: workspace.project.id, chapterId: scene.chapterId, sceneId: scene.id, excerpt, startOffset: start, endOffset: start + Array.from(excerpt).length, category: 'description', label: 'Konkrete Beschreibung', notes: '', weight: 2 });
    expect(reference.startOffset).toBeGreaterThanOrEqual(0);
    expect((await repository.loadWorkspace()).chapters[0]!.scenes[0]!.content).toBe(original);
  });

  it('begrenzt Chat-Quellen auf die tatsächlich verwendeten Einträge', () => {
    const context: ProjectContext = {
      projectId: 'p',
      currentScene: undefined,
      currentChapter: undefined,
      relevantEntities: [
        { id: 'marek', projectId: 'p', name: 'Marek', type: 'character', description: 'Eine Figur.', status: 'confirmed', confidence: 1, source: '', chapter: '', scene: '', authorConfirmed: true, updatedAt: '', tags: [], origin: 'manual' as const },
        { id: 'package', projectId: 'p', name: 'Paketnummer', type: 'clue' as const, description: 'Die Paketnummer ist verändert.', status: 'confirmed' as const, confidence: 1, source: '', chapter: '', scene: '', authorConfirmed: true, updatedAt: '', tags: ['Paket'], origin: 'bible_update' as const },
      ],
      relevantSources: [
        { id: 'source-marek', projectId: 'p', entityId: 'marek', chapterId: 'c', sceneId: 's', excerpt: 'Marek', createdAt: '' },
        { id: 'source-package', projectId: 'p', entityId: 'package', chapterId: 'c', sceneId: 's', excerpt: 'veränderte Paketnummer', createdAt: '' },
      ],
      openPlotThreads: [],
      possibleContradictions: [],
    };
    const answer = answerFromProjectContext('Welche bestätigten Fakten gibt es zur Paketnummer?', context);
    expect(answer.sources.map((source) => source.id)).toEqual(['source-package']);
  });

  it('übernimmt Character Memory mit Teilnehmern, Evidence und idempotenter Quelle', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const characters = workspace.entities.filter((entity) => entity.type === 'character');
    const scene = workspace.chapters[0]!.scenes[0]!;
    const run = await repository.createCharacterMemoryUpdateRun({ projectId: workspace.project.id, sceneId: scene.id, contentHash: contentHash(scene.content), extractorId: 'local-prototype-extractor' });
    const [proposal] = await repository.saveCharacterMemoryProposals(run.id, [{ proposalKind: 'dialogue_memory', subjectCharacterId: characters[0]!.id, payload: { dialogueKind: 'inside_joke', topic: 'Paket', summary: 'Eine belegte Aussage.', exactExcerpt: 'Marek sagte', emotionalTone: '', hiddenIntent: '', significance: 'important', truthfulness: 'unknown', participants: [{ characterId: characters[0]!.id, role: 'speaker' }, { characterId: characters[1]!.id, role: 'listener' }] }, classification: 'observable', confidence: 0.9, evidenceExcerpt: 'Marek sagte', startOffset: undefined, endOffset: undefined, reason: 'Test' }]);
    const reviewed = await repository.reviewCharacterMemoryProposal({ proposalId: proposal!.id, reviewStatus: 'accepted' });
    expect(reviewed.acceptedMemoryKind).toBe('dialogue_memory');
    expect((await repository.listCharacterDialogueMemories(workspace.project.id))[0]!.participants).toHaveLength(2);
    expect((await repository.listCharacterMemoryEvidence(workspace.project.id, 'dialogue_memory', reviewed.acceptedMemoryId!))).toHaveLength(1);
    await expect(repository.reviewCharacterMemoryProposal({ proposalId: proposal!.id, reviewStatus: 'accepted' })).rejects.toThrow('bereits geprüft');
  });

  it('speichert Longform-Reviews und blockiert die Übernahme bis zur Ausnahme', async () => {
    const { BrowserLongformRepository } = await import('./longformRepository');
    const repository = new BrowserLongformRepository();
    const job = await repository.createJob({ projectId: 'p', targetBookId: 'b', targetWords: 800, userInstruction: 'Schreib', activeProvider: 'local-prototype', contentContextHash: 'h' });
    const reviews = await repository.saveReviews(job.id, [{ jobId: job.id, sectionId: undefined, reviewScope: 'chapter', issueType: 'canon', severity: 'blocking', title: 'Konflikt', description: 'Prüfen', relatedEntityIds: [], relatedSourceIds: [], suggestedAction: 'Neu erzeugen', status: 'open' }]);
    expect(reviews[0]!.severity).toBe('blocking');
    const exception = await repository.updateReviewStatus(reviews[0]!.id, 'exception');
    expect(exception.status).toBe('exception');
  });

  it('verwendet aktuelles Wissen ohne Historie und schließt spätere Szenenzustände aus', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const character = workspace.entities.find((entity) => entity.type === 'character')!;
    const fact = workspace.entities.find((entity) => entity.type === 'fact' || entity.type === 'clue')!;
    const firstScene = workspace.chapters[0]!.scenes[0]!;
    const lastScene = workspace.chapters.at(-1)!.scenes.at(-1)!;
    await repository.saveCharacterKnowledgeState({ projectId: workspace.project.id, characterId: character.id, factEntityId: fact.id, knowledgeState: 'knows', acquiredSceneId: firstScene.id, changedSceneId: firstScene.id, certainty: 1, notes: '', status: 'confirmed', authorConfirmed: true });
    await repository.saveCharacterSceneState({ projectId: workspace.project.id, characterEntityId: character.id, sceneId: lastScene.id, emotionalState: 'später', physicalState: '', goal: '', conflict: '', knowledgeNotes: 'spätere Notiz', relationshipState: '', changeNote: '' });
    const context = await new DeterministicProjectContextBuilder(repository).build({ projectId: workspace.project.id, currentSceneId: firstScene.id, userQuestion: character.name });
    expect(context.characterKnowledgeStates?.some((state) => state.factEntityId === fact.id && state.knowledgeState === 'knows')).toBe(true);
    expect(context.characterStates?.some((state) => state.sceneId === lastScene.id)).toBe(false);
  });

  it('verwendet historische Wissensstände nur bis zur Zielszene', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const character = workspace.entities.find((entity) => entity.type === 'character')!;
    const fact = workspace.entities.find((entity) => entity.type === 'fact' || entity.type === 'clue')!;
    const scenes = workspace.chapters.flatMap((chapter) => chapter.scenes);
    const first = await repository.saveCharacterKnowledgeState({ projectId: workspace.project.id, characterId: character.id, factEntityId: fact.id, knowledgeState: 'suspects', acquiredSceneId: scenes[0]!.id, changedSceneId: scenes[0]!.id, certainty: 0.4, notes: '', status: 'confirmed', authorConfirmed: true });
    await repository.saveCharacterKnowledgeState({ ...first, knowledgeState: 'knows', acquiredSceneId: scenes[1]!.id, changedSceneId: scenes[1]!.id });
    const context = await new DeterministicProjectContextBuilder(repository).build({ projectId: workspace.project.id, currentSceneId: scenes[0]!.id, userQuestion: character.name });
    expect(context.characterKnowledgeStates?.find((state) => state.factEntityId === fact.id)?.knowledgeState).toBe('suspects');
    const laterContext = await new DeterministicProjectContextBuilder(repository).build({ projectId: workspace.project.id, currentSceneId: scenes[1]!.id, userQuestion: character.name });
    expect(laterContext.characterKnowledgeStates?.find((state) => state.factEntityId === fact.id)?.knowledgeState).toBe('knows');
  });

  it('reicht nur bestätigte, nicht veraltete Summaries an den ProjectContext weiter', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const base = { projectId: workspace.project.id, scopeType: 'project' as const, scopeId: workspace.project.id, importantEvents: [], openThreads: [], characterChanges: [], authorConfirmed: false };
    await repository.saveNarrativeSummary({ ...base, contentHash: 'confirmed', summary: 'Bestätigt', status: 'confirmed', authorConfirmed: true });
    await repository.saveNarrativeSummary({ ...base, contentHash: 'proposed', summary: 'Vorgeschlagen', status: 'proposed' });
    await repository.saveNarrativeSummary({ ...base, contentHash: 'outdated', summary: 'Veraltet', status: 'outdated' });
    await repository.saveNarrativeSummary({ ...base, contentHash: 'rejected', summary: 'Abgelehnt', status: 'rejected' });
    const input = { projectId: workspace.project.id, currentSceneId: workspace.chapters[0]!.scenes[0]!.id, userQuestion: 'Zusammenfassung' };
    const builder = new DeterministicProjectContextBuilder(repository);
    const defaultContext = await builder.build(input);
    expect(defaultContext.narrativeSummaries?.map((item) => item.summary)).toEqual(['Bestätigt']);
    const explicitContext = await builder.build({ ...input, includeProposedSummaries: true });
    expect(explicitContext.narrativeSummaries?.map((item) => item.summary)).toEqual(expect.arrayContaining(['Bestätigt', 'Vorgeschlagen']));
    expect(explicitContext.narrativeSummaries?.some((item) => item.status === 'outdated' || item.status === 'rejected')).toBe(false);
  });

  it('verwendet nur akzeptierte Stilbeobachtungen und bestätigt aktuelle Summaries', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const run = await repository.createProjectStyleAnalysisRun({ projectId: workspace.project.id, sourceHash: 'style-hash', providerId: 'local-prototype' });
    const [accepted, rejected] = await repository.saveProjectStyleObservations(run.id, [
      { runId: run.id, projectId: workspace.project.id, observationType: 'dialogue', observationText: 'Kurze Dialogzeilen.', recommendation: 'Beibehalten', confidence: 0.9, evidence: [] },
      { runId: run.id, projectId: workspace.project.id, observationType: 'pacing', observationText: 'Zu viele Nebenwege.', recommendation: 'Kürzen', confidence: 0.8, evidence: [] },
    ]);
    await repository.reviewProjectStyleObservation(accepted!.id, 'accepted');
    await repository.reviewProjectStyleObservation(rejected!.id, 'rejected');
    await repository.saveNarrativeSummary({ projectId: workspace.project.id, scopeType: 'project', scopeId: workspace.project.id, contentHash: 'summary-1', summary: 'Bestätigte Zusammenfassung.', importantEvents: [], openThreads: [], characterChanges: [], status: 'confirmed', authorConfirmed: true });
    const context = await new DeterministicProjectContextBuilder(repository).build({ projectId: workspace.project.id, currentSceneId: workspace.chapters[0]!.scenes[0]!.id, userQuestion: 'Dialog' });
    expect(context.acceptedStyleObservations?.map((item) => item.id)).toEqual([accepted!.id]);
    expect(context.narrativeSummaries?.some((item) => item.status === 'confirmed')).toBe(true);
  });

  it('markiert eine Summary nach Contentänderung als outdated und ändert den Longform-Hash', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const scene = workspace.chapters[0]!.scenes[0]!;
    const saved = await repository.saveNarrativeSummary({ projectId: workspace.project.id, scopeType: 'scene', scopeId: scene.id, contentHash: 'old', summary: 'Alt', importantEvents: [], openThreads: [], characterChanges: [], status: 'confirmed', authorConfirmed: true });
    await repository.markNarrativeSummaryOutdated(workspace.project.id, 'scene', scene.id, 'new');
    expect((await repository.listNarrativeSummaries(workspace.project.id, 'scene', scene.id)).find((item) => item.id === saved.id)?.status).toBe('outdated');
    const acceptedObservation = { id: 'style-observation', runId: 'style-run', projectId: workspace.project.id, observationType: 'dialogue' as const, observationText: 'Kurze Dialogzeilen.', recommendation: 'Beibehalten', confidence: 0.9, evidence: [], reviewStatus: 'accepted' as const, createdAt: '2026-08-03T00:00:00Z' };
    const before = contextHashForLongform(workspace.project, workspace.chapters, undefined, { projectId: workspace.project.id, relevantEntities: [], relevantSources: [], openPlotThreads: [], possibleContradictions: [], acceptedStyleObservations: [acceptedObservation], narrativeSummaries: [saved] });
    const after = contextHashForLongform(workspace.project, workspace.chapters, undefined, { projectId: workspace.project.id, relevantEntities: [], relevantSources: [], openPlotThreads: [], possibleContradictions: [], acceptedStyleObservations: [{ ...acceptedObservation, reviewStatus: 'edited', observationText: 'Geändert.' }], narrativeSummaries: [{ ...saved, status: 'outdated' }] });
    expect(after).not.toBe(before);
  });
});
