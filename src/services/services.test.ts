import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createChapter, createProject, createScene, getLocalState, saveScene, saveStoryEntity } from './localStore';
import { MockCorrectionService, chunkText } from './correctionService';
import { MockProvider } from './providerBridge';
import { demoEntities } from './mockData';

const store = new Map<string, string>();
vi.stubGlobal('localStorage', { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value), removeItem: (key: string) => store.delete(key) });

beforeEach(() => { store.clear(); });

describe('lokales Projektmodell', () => {
  it('legt ein Projekt an', async () => { const project = await createProject('Neue Geschichte', 'Ada Autor'); expect(project.title).toBe('Neue Geschichte'); expect(getLocalState().project.author).toBe('Ada Autor'); });
  it('legt ein Kapitel und eine Szene an', async () => { const chapter = await createChapter('Kapitel 4 – Rückkehr'); const scene = await createScene(chapter.id, 'Die Tür'); expect(scene.chapterId).toBe(chapter.id); expect(getLocalState().chapters.some((item) => item.id === chapter.id && item.scenes[0]?.title === 'Die Tür')).toBe(true); });
  it('speichert eine Szene lokal', async () => { const chapter = await createChapter('Kapitel 4'); const scene = await createScene(chapter.id, 'Die Tür'); await saveScene({ ...scene, content: 'Lokaler Text.' }); expect(getLocalState().chapters.find((item) => item.id === chapter.id)?.scenes[0]?.content).toBe('Lokaler Text.'); });
  it('speichert einen Story-Bible-Eintrag', async () => { const entity = { ...demoEntities[0], id: 'local-entity', name: 'Neue Figur' }; await saveStoryEntity(entity); expect(getLocalState().entities[0]?.name).toBe('Neue Figur'); });
});

describe('Provider und Korrektur', () => {
  it('liefert eine Mock-Provider-Antwort mit Quellen', async () => { const result = await new MockProvider().runTask({ id: 'task', type: 'chat', prompt: 'Prüfe', context: [] }); expect(result.text).toContain('Band 1'); expect(result.sources).toContain('Kapitel 3'); });
  it('erkennt Korrekturen als Diff und kann sie anwenden', async () => { const service = new MockCorrectionService(); const result = await service.check('Die Szene wiederspiegelt  den Konflikt.'); expect(result.corrections).toHaveLength(2); const fixed = service.apply(result.sourceText, result.corrections[0]!); expect(fixed).toContain('widerspiegelt'); });
  it('chunked Text an Absatzgrenzen mit Überlappung', () => { const text = Array.from({ length: 40 }, (_, index) => `Absatz ${index} mit ausreichend Inhalt für den lokalen Import.`).join('\n\n'); const chunks = chunkText(text, 40, 8); expect(chunks.length).toBeGreaterThan(1); expect(chunks[0]).toContain('Absatz 0'); expect(chunks.at(-1)).toContain('Absatz 39'); });
});
