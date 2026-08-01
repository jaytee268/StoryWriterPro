import { z } from 'zod';
import { demoChapters, demoEntities, demoProject } from './mockData';
import { desktopInvoke, isTauriRuntime } from './desktop';
import type { Book, Chapter, CreateChapterInput, CreateProjectInput, CreateSceneInput, EditorPreferences, Project, SaveStoryEntityInput, Scene, SceneVersion, StoryEntity, UpdateSceneInput, WorkspaceSnapshot } from '../types/domain';

export type RuntimeMode = 'desktop' | 'browser-demo';
export type SaveableScene = Scene;

export interface DatabaseInfo { path: string; connected: boolean; engine: 'sqlite' | 'localStorage'; detail: string; }

export interface StoryRepository {
  readonly mode: RuntimeMode;
  loadWorkspace(): Promise<WorkspaceSnapshot>;
  createProject(input: CreateProjectInput): Promise<Project>;
  createChapter(input: CreateChapterInput): Promise<Chapter>;
  createScene(input: CreateSceneInput): Promise<Scene>;
  updateScene(input: UpdateSceneInput): Promise<Scene>;
  listSceneVersions(sceneId: string): Promise<SceneVersion[]>;
  restoreSceneVersion(sceneId: string, versionId: string): Promise<Scene>;
  getEditorPreferences(): Promise<EditorPreferences>;
  saveEditorPreferences(input: EditorPreferences): Promise<EditorPreferences>;
  saveStoryEntity(input: SaveStoryEntityInput): Promise<StoryEntity>;
  listStoryEntities(projectId: string): Promise<StoryEntity[]>;
  getDatabaseInfo(): Promise<DatabaseInfo>;
}

const statusSchema = z.enum(['draft', 'revised', 'final']);
const entityStatusSchema = z.enum(['confirmed', 'proposed', 'uncertain', 'contradicted', 'retconned', 'archived']);
const entityTypeSchema = z.enum(['character', 'relationship', 'place', 'organization', 'world_rule', 'object', 'event', 'fact', 'clue', 'secret', 'plot_thread', 'retcon', 'author_note']);
const projectSchema = z.object({ id: z.string(), title: z.string(), author: z.string(), description: z.string(), updatedAt: z.string(), createdAt: z.string().optional(), wordCount: z.number(), openWarnings: z.number(), bibleProgress: z.number() });
const bookSchema = z.object({ id: z.string(), projectId: z.string(), title: z.string(), volume: z.number(), createdAt: z.string().optional(), updatedAt: z.string().optional() });
const sceneSchema = z.object({ id: z.string(), chapterId: z.string(), title: z.string(), orderIndex: z.number(), content: z.string(), pov: z.string(), location: z.string(), storyTime: z.string(), status: statusSchema, goal: z.string(), notes: z.string(), createdAt: z.string().optional(), updatedAt: z.string().optional() });
const sceneVersionSchema = z.object({ id: z.string(), sceneId: z.string(), versionNumber: z.number(), content: z.string(), createdAt: z.string(), scene: sceneSchema });
const editorPreferencesSchema = z.object({ fontFamily: z.enum(['serif', 'sans', 'typewriter']), fontSize: z.number(), lineHeight: z.number() });
const chapterSchema = z.object({ id: z.string(), bookId: z.string(), title: z.string(), orderIndex: z.number(), scenes: z.array(sceneSchema), createdAt: z.string().optional(), updatedAt: z.string().optional() });
const entitySchema = z.object({ id: z.string(), projectId: z.string().optional(), name: z.string(), type: entityTypeSchema, description: z.string(), status: entityStatusSchema, confidence: z.number(), source: z.string(), chapter: z.string(), scene: z.string(), authorConfirmed: z.boolean(), updatedAt: z.string(), createdAt: z.string().optional(), tags: z.array(z.string()) });
const workspaceSchema = z.object({ project: projectSchema, books: z.array(bookSchema), chapters: z.array(chapterSchema), entities: z.array(entitySchema) });

function parse<T>(schema: z.ZodType<T>, value: unknown, label: string): T {
  const result = schema.safeParse(value);
  if (!result.success) throw new Error(`${label} ist ungültig: ${result.error.issues.map((issue) => issue.message).join(', ')}`);
  return result.data;
}

function clone<T>(value: T): T { return JSON.parse(JSON.stringify(value)) as T; }
function now(): string { return new Date().toISOString(); }

export class TauriStoryRepository implements StoryRepository {
  readonly mode = 'desktop' as const;

  async loadWorkspace(): Promise<WorkspaceSnapshot> { return parse(workspaceSchema, await desktopInvoke('load_workspace'), 'Workspace'); }
  async createProject(input: CreateProjectInput): Promise<Project> { return parse(projectSchema, await desktopInvoke('create_project', { input }), 'Projekt'); }
  async createChapter(input: CreateChapterInput): Promise<Chapter> { return parse(chapterSchema, await desktopInvoke('create_chapter', { input }), 'Kapitel'); }
  async createScene(input: CreateSceneInput): Promise<Scene> { return parse(sceneSchema, await desktopInvoke('create_scene', { input }), 'Szene'); }
  async updateScene(input: UpdateSceneInput): Promise<Scene> { return parse(sceneSchema, await desktopInvoke('update_scene', { input }), 'Szene'); }
  async listSceneVersions(sceneId: string): Promise<SceneVersion[]> { return parse(z.array(sceneVersionSchema), await desktopInvoke('list_scene_versions', { sceneId }), 'Szenenverlauf'); }
  async restoreSceneVersion(sceneId: string, versionId: string): Promise<Scene> { return parse(sceneSchema, await desktopInvoke('restore_scene_version', { input: { sceneId, versionId } }), 'Wiederhergestellte Szene'); }
  async getEditorPreferences(): Promise<EditorPreferences> { return parse(editorPreferencesSchema, await desktopInvoke('get_editor_preferences'), 'Editor-Einstellungen'); }
  async saveEditorPreferences(input: EditorPreferences): Promise<EditorPreferences> { return parse(editorPreferencesSchema, await desktopInvoke('save_editor_preferences', { input }), 'Editor-Einstellungen'); }
  async saveStoryEntity(input: SaveStoryEntityInput): Promise<StoryEntity> { return parse(entitySchema, await desktopInvoke('save_story_entity', { input }), 'Story-Bible-Eintrag'); }
  async listStoryEntities(projectId: string): Promise<StoryEntity[]> { return parse(z.array(entitySchema), await desktopInvoke('list_story_entities', { projectId }), 'Story-Bible-Liste'); }
  async getDatabaseInfo(): Promise<DatabaseInfo> { return parse(z.object({ path: z.string(), connected: z.boolean(), engine: z.literal('sqlite'), detail: z.string() }), await desktopInvoke('database_info'), 'Datenbankstatus'); }
}

interface BrowserState { project: Project; books: Book[]; chapters: Chapter[]; entities: StoryEntity[]; versions: SceneVersion[]; editorPreferences: EditorPreferences; }
const browserKey = 'storymemory-browser-demo-workspace';
const browserPreferencesKey = 'storymemory-browser-demo-editor-preferences';
const defaultEditorPreferences: EditorPreferences = { fontFamily: 'serif', fontSize: 18, lineHeight: 1.95 };

export class BrowserDemoRepository implements StoryRepository {
  readonly mode = 'browser-demo' as const;

  private read(): BrowserState {
    try {
      const value = localStorage.getItem(browserKey);
      if (value) {
        const state = JSON.parse(value) as Partial<BrowserState>;
        return { ...state, versions: state.versions ?? [], editorPreferences: state.editorPreferences ?? defaultEditorPreferences } as BrowserState;
      }
    } catch { /* A broken demo cache is replaced with the safe example workspace. */ }
    return { project: clone(demoProject), books: [{ id: 'book-1', projectId: demoProject.id, title: demoProject.title, volume: 1 }], chapters: clone(demoChapters), entities: clone(demoEntities), versions: [], editorPreferences: defaultEditorPreferences };
  }

  private write(state: BrowserState): void { localStorage.setItem(browserKey, JSON.stringify(state)); }
  private snapshot(state = this.read()): WorkspaceSnapshot { return clone({ project: state.project, books: state.books, chapters: state.chapters, entities: state.entities }); }

  async loadWorkspace(): Promise<WorkspaceSnapshot> { return this.snapshot(); }
  async createProject(input: CreateProjectInput): Promise<Project> {
    const state = this.read();
    const timestamp = now();
    const project: Project = { id: crypto.randomUUID(), title: input.title, author: input.author, description: input.description ?? 'Neues lokales StoryMemory-Projekt', updatedAt: timestamp, createdAt: timestamp, wordCount: 0, openWarnings: 0, bibleProgress: 0 };
    state.project = project;
    state.books = [{ id: crypto.randomUUID(), projectId: project.id, title: input.volumeTitle ?? input.title, volume: input.volume ?? 1, createdAt: timestamp, updatedAt: timestamp }];
    state.chapters = [];
    state.entities = [];
    state.versions = [];
    this.write(state);
    return clone(project);
  }
  async createChapter(input: CreateChapterInput): Promise<Chapter> {
    const state = this.read();
    if (!state.books.some((book) => book.id === input.bookId)) throw new Error('Das ausgewählte Buch wurde nicht gefunden.');
    const timestamp = now();
    const chapter: Chapter = { id: crypto.randomUUID(), bookId: input.bookId, title: input.title, orderIndex: state.chapters.filter((item) => item.bookId === input.bookId).length + 1, scenes: [], createdAt: timestamp, updatedAt: timestamp };
    state.chapters.push(chapter); this.write(state); return clone(chapter);
  }
  async createScene(input: CreateSceneInput): Promise<Scene> {
    const state = this.read();
    const chapter = state.chapters.find((item) => item.id === input.chapterId);
    if (!chapter) throw new Error('Das ausgewählte Kapitel wurde nicht gefunden.');
    const timestamp = now();
    const scene: Scene = { id: crypto.randomUUID(), chapterId: input.chapterId, title: input.title, orderIndex: chapter.scenes.length + 1, content: '', pov: '', location: '', storyTime: '', status: 'draft', goal: '', notes: '', createdAt: timestamp, updatedAt: timestamp };
    chapter.scenes.push(scene); chapter.updatedAt = timestamp; this.write(state); return clone(scene);
  }
  async updateScene(input: UpdateSceneInput): Promise<Scene> {
    const state = this.read();
    const chapter = state.chapters.find((item) => item.id === input.chapterId);
    if (!chapter) throw new Error('Das Kapitel der Szene wurde nicht gefunden.');
    const index = chapter.scenes.findIndex((item) => item.id === input.id);
    if (index < 0) throw new Error('Die Szene wurde nicht gefunden.');
    const saved = { ...input, updatedAt: now() };
    const version: SceneVersion = { id: crypto.randomUUID(), sceneId: saved.id, versionNumber: state.versions.filter((item) => item.sceneId === saved.id).length + 1, content: saved.content, createdAt: saved.updatedAt ?? now(), scene: clone(saved) };
    state.versions = [version, ...state.versions];
    chapter.scenes[index] = saved; chapter.updatedAt = saved.updatedAt ?? now(); state.project.updatedAt = saved.updatedAt ?? now(); this.write(state); return clone(saved);
  }
  async listSceneVersions(sceneId: string): Promise<SceneVersion[]> { return this.read().versions.filter((version) => version.sceneId === sceneId).map(clone); }
  async restoreSceneVersion(sceneId: string, versionId: string): Promise<Scene> { const version = this.read().versions.find((item) => item.sceneId === sceneId && item.id === versionId); if (!version) throw new Error('Die Version wurde nicht gefunden.'); return this.updateScene({ ...version.scene, id: sceneId }); }
  async getEditorPreferences(): Promise<EditorPreferences> { const value = localStorage.getItem(browserPreferencesKey); return value ? editorPreferencesSchema.parse(JSON.parse(value)) : clone(defaultEditorPreferences); }
  async saveEditorPreferences(input: EditorPreferences): Promise<EditorPreferences> { const saved = editorPreferencesSchema.parse({ ...input, fontSize: Math.max(14, Math.min(28, input.fontSize)), lineHeight: Math.max(1.3, Math.min(2.5, input.lineHeight)) }); localStorage.setItem(browserPreferencesKey, JSON.stringify(saved)); return saved; }
  async saveStoryEntity(input: SaveStoryEntityInput): Promise<StoryEntity> { const state = this.read(); const saved = { ...input, updatedAt: now() }; state.entities = [saved, ...state.entities.filter((item) => item.id !== saved.id)]; this.write(state); return clone(saved); }
  async listStoryEntities(projectId: string): Promise<StoryEntity[]> { return this.read().entities.filter((entity) => !entity.projectId || entity.projectId === projectId).map(clone); }
  async getDatabaseInfo(): Promise<DatabaseInfo> { return { path: 'Browser-Demo: localStorage', connected: true, engine: 'localStorage', detail: 'Nur Vorschau-Daten im Browser. Die Desktop-App verwendet SQLite.' }; }
}

export function createStoryRepository(): StoryRepository { return isTauriRuntime() ? new TauriStoryRepository() : new BrowserDemoRepository(); }
