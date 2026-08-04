import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserDemoRepository } from './storyRepository';
import { contentHash } from '../utils/aiText';

const store = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value), removeItem: (key: string) => store.delete(key) });

describe('persistente Timeline und Story-Graph-Daten', () => {
  beforeEach(() => store.clear());

  it('startet ohne Demo-Ereignisse und speichert ein Ereignis aus einer Seite mit Unicode-Quelle', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const chapter = workspace.chapters[0]!;
    const scene = chapter.scenes[0]!;
    const text = '😀 Ein Ereignis im Manuskript.';
    await repository.updateScene({ ...scene, content: text });
    expect(await repository.listTimelineEvents(workspace.project.id)).toEqual([]);
    const source = await repository.createSourceReference({ projectId: workspace.project.id, chapterId: chapter.id, sceneId: scene.id, excerpt: '😀 Ein Ereignis', startOffset: 0, endOffset: Array.from('😀 Ein Ereignis').length });
    const event = await repository.saveTimelineEvent({ projectId: workspace.project.id, bookId: workspace.books[0]!.id, chapterId: chapter.id, sceneId: scene.id, passageUnitId: 'page-3', title: 'Ereignis', summary: 'Ein wichtiges Ereignis.', storyTimeText: 'später', temporalOrder: 300, timeCertainty: 'relative', participatingEntityIds: [], causeEventIds: [], consequenceEventIds: [], knowledgeChanges: ['Eine Figur erfährt einen Fakt.'], stateChanges: ['Der Zustand ändert sich.'], relatedPlotThreadIds: [], sourceReferenceIds: [source.id], confidence: .8, status: 'proposed', authorConfirmed: false, origin: 'manuscript_analysis' });
    expect(event.sourceReferenceIds).toEqual([source.id]);
    expect((await repository.listTimelineEvents(workspace.project.id))[0]?.passageUnitId).toBe('page-3');
  });

  it('hält unbekannte Zeit, Flashback-Ordnung und Ursache/Folge als Daten statt als UI-Annahme', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const chapter = workspace.chapters[0]!; const scene = chapter.scenes[0]!;
    const base = { projectId: workspace.project.id, bookId: workspace.books[0]!.id, chapterId: chapter.id, sceneId: scene.id, participatingEntityIds: [], knowledgeChanges: [], stateChanges: [], relatedPlotThreadIds: [], sourceReferenceIds: [], confidence: .7, status: 'proposed' as const, authorConfirmed: false, origin: 'manuscript_analysis' as const };
    const flashback = await repository.saveTimelineEvent({ ...base, id: 'flashback', title: 'Rückblick', summary: 'Frühere Ursache.', storyTimeText: 'vorher', temporalOrder: 1, timeCertainty: 'relative', causeEventIds: [], consequenceEventIds: ['later'] });
    const later = await repository.saveTimelineEvent({ ...base, id: 'later', title: 'Folge', summary: 'Spätere Folge.', storyTimeText: '', temporalOrder: 2, timeCertainty: 'unknown', causeEventIds: ['flashback'], consequenceEventIds: [] });
    expect(flashback.timeCertainty).toBe('relative'); expect(later.timeCertainty).toBe('unknown'); expect(later.causeEventIds).toEqual(['flashback']);
  });

  it('speichert vorgeschlagene und bestätigte Graph-Kanten getrennt und erhält Layouts nach Neustart', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const [source, target] = workspace.entities.filter((entity) => entity.type === 'character').slice(0, 2);
    const edge = await repository.saveStoryGraphEdge({ projectId: workspace.project.id, sourceEntityId: source!.id, targetEntityId: target!.id, relationType: 'connected_to', label: 'Fake-Provider-Vorschlag', sourceReferenceIds: [], confidence: .6, status: 'proposed', authorConfirmed: false, origin: 'manuscript_analysis' });
    expect(edge.status).toBe('proposed');
    await repository.reviewStoryGraphEdge(edge.id, 'confirmed');
    expect((await repository.listStoryGraphEdges(workspace.project.id))[0]).toMatchObject({ status: 'confirmed', authorConfirmed: true });
    const layout = await repository.saveMindmapLayout({ projectId: workspace.project.id, userId: 'author', nodeId: source!.id, positionX: 42, positionY: 84, width: 190, height: 78, hidden: false, fixed: true });
    expect((await new BrowserDemoRepository().listMindmapLayouts(workspace.project.id, 'author'))[0]).toMatchObject({ id: layout.id, positionX: 42, positionY: 84 });
    expect(contentHash('😀')).not.toBe(contentHash('x'));
  });
});
