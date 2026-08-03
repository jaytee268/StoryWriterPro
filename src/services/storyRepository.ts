import { z } from 'zod';
import { demoChapters, demoEntities, demoProject } from './mockData';
import { desktopInvoke, isTauriRuntime } from './desktop';
import type { BibleProposal, BibleProposalDraft, BibleUpdateRun, Book, Chapter, CreateBibleUpdateRunInput, CreateChapterInput, CreateProjectInput, CreateSceneInput, CreateSceneVersionInput, CreateSourceReferenceInput, CreateStoryEntityInput, EditorPreferences, Project, ReviewBibleProposalInput, SaveStoryEntityInput, Scene, SceneVersion, StoryEntity, StorySourceReference, UpdateSceneInput, UpdateStoryEntityInput, WorkspaceSnapshot } from '../types/domain';

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
  createSceneVersion(input: CreateSceneVersionInput): Promise<SceneVersion>;
  listSceneVersions(sceneId: string): Promise<SceneVersion[]>;
  restoreSceneVersion(sceneId: string, versionId: string): Promise<Scene>;
  getEditorPreferences(): Promise<EditorPreferences>;
  saveEditorPreferences(input: EditorPreferences): Promise<EditorPreferences>;
  saveStoryEntity(input: SaveStoryEntityInput): Promise<StoryEntity>;
  listStoryEntities(projectId: string): Promise<StoryEntity[]>;
  createStoryEntity(input: CreateStoryEntityInput): Promise<StoryEntity>;
  updateStoryEntity(input: UpdateStoryEntityInput): Promise<StoryEntity>;
  archiveStoryEntity(id: string): Promise<StoryEntity>;
  getStoryEntity(id: string): Promise<StoryEntity>;
  createSourceReference(input: CreateSourceReferenceInput): Promise<StorySourceReference>;
  listSourceReferences(projectId: string, entityId?: string): Promise<StorySourceReference[]>;
  createBibleUpdateRun(input: CreateBibleUpdateRunInput): Promise<BibleUpdateRun>;
  listBibleUpdateRuns(projectId: string, sceneId?: string): Promise<BibleUpdateRun[]>;
  listBibleProposals(runId: string): Promise<BibleProposal[]>;
  saveBibleProposals(runId: string, proposals: BibleProposalDraft[], projectId: string, sceneId: string): Promise<BibleProposal[]>;
  reviewBibleProposal(input: ReviewBibleProposalInput): Promise<BibleProposal>;
  completeBibleReview(runId: string): Promise<BibleUpdateRun>;
  getDatabaseInfo(): Promise<DatabaseInfo>;
}

const statusSchema = z.enum(['draft', 'revised', 'final']);
const entityStatusSchema = z.enum(['confirmed', 'proposed', 'uncertain', 'contradicted', 'retconned', 'archived']);
const entityTypeSchema = z.enum(['character', 'relationship', 'place', 'organization', 'world_rule', 'object', 'event', 'fact', 'clue', 'secret', 'plot_thread', 'retcon', 'author_note']);
const projectSchema = z.object({ id: z.string(), title: z.string(), author: z.string(), description: z.string(), updatedAt: z.string(), createdAt: z.string().optional(), wordCount: z.number(), openWarnings: z.number(), bibleProgress: z.number() });
const bookSchema = z.object({ id: z.string(), projectId: z.string(), title: z.string(), volume: z.number(), createdAt: z.string().optional(), updatedAt: z.string().optional() });
const sceneSchema = z.object({ id: z.string(), chapterId: z.string(), title: z.string(), orderIndex: z.number(), content: z.string(), pov: z.string(), location: z.string(), storyTime: z.string(), status: statusSchema, goal: z.string(), notes: z.string(), createdAt: z.string().optional(), updatedAt: z.string().optional() });
const sceneVersionSchema = z.object({ id: z.string(), sceneId: z.string(), versionNumber: z.number(), content: z.string(), reason: z.enum(['manual', 'before_correction', 'before_ai_change', 'before_import', 'automatic_checkpoint']), createdAt: z.string(), scene: sceneSchema });
const editorPreferencesSchema = z.object({ fontFamily: z.enum(['serif', 'sans', 'typewriter']), fontSize: z.number(), lineHeight: z.number() });
const chapterSchema = z.object({ id: z.string(), bookId: z.string(), title: z.string(), orderIndex: z.number(), scenes: z.array(sceneSchema), createdAt: z.string().optional(), updatedAt: z.string().optional() });
const entitySchema = z.object({ id: z.string(), projectId: z.string(), name: z.string(), type: entityTypeSchema, description: z.string(), status: entityStatusSchema, confidence: z.number(), source: z.string(), chapter: z.string(), scene: z.string(), authorConfirmed: z.boolean(), updatedAt: z.string(), createdAt: z.string().optional(), tags: z.array(z.string()), origin: z.enum(['manual', 'bible_update', 'edited']) });
const sourceSchema = z.object({ id: z.string(), projectId: z.string(), entityId: z.string().optional(), proposalId: z.string().optional(), chapterId: z.string(), sceneId: z.string(), excerpt: z.string(), startOffset: z.number().optional(), endOffset: z.number().optional(), createdAt: z.string() });
const runSchema = z.object({ id: z.string(), projectId: z.string(), sceneId: z.string(), sceneUpdatedAt: z.string(), contentHash: z.string(), extractorId: z.string(), status: z.enum(['pending', 'running', 'completed', 'failed', 'reviewed']), createdAt: z.string(), completedAt: z.string().optional(), errorMessage: z.string().optional() });
const proposalSchema = z.object({ id: z.string(), runId: z.string(), projectId: z.string(), sceneId: z.string(), targetEntityId: z.string().optional(), proposalAction: z.enum(['create_entity', 'update_entity', 'add_source', 'mark_contradiction', 'create_open_question', 'create_author_note']), entityType: entityTypeSchema, candidateName: z.string(), candidateDescription: z.string(), candidateStatus: entityStatusSchema, confidence: z.number(), classification: z.enum(['observable_fact', 'interpretation', 'open_question', 'possible_contradiction', 'author_note']), evidenceExcerpt: z.string(), startOffset: z.number().optional(), endOffset: z.number().optional(), reason: z.string(), reviewStatus: z.enum(['pending', 'accepted', 'edited', 'rejected']), reviewedAt: z.string().optional(), createdAt: z.string() });
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
  async createSceneVersion(input: CreateSceneVersionInput): Promise<SceneVersion> { return parse(sceneVersionSchema, await desktopInvoke('create_scene_version', { input }), 'Szenenversion'); }
  async listSceneVersions(sceneId: string): Promise<SceneVersion[]> { return parse(z.array(sceneVersionSchema), await desktopInvoke('list_scene_versions', { sceneId }), 'Szenenverlauf'); }
  async restoreSceneVersion(sceneId: string, versionId: string): Promise<Scene> { return parse(sceneSchema, await desktopInvoke('restore_scene_version', { input: { sceneId, versionId } }), 'Wiederhergestellte Szene'); }
  async getEditorPreferences(): Promise<EditorPreferences> { return parse(editorPreferencesSchema, await desktopInvoke('get_editor_preferences'), 'Editor-Einstellungen'); }
  async saveEditorPreferences(input: EditorPreferences): Promise<EditorPreferences> { return parse(editorPreferencesSchema, await desktopInvoke('save_editor_preferences', { input }), 'Editor-Einstellungen'); }
  async saveStoryEntity(input: SaveStoryEntityInput): Promise<StoryEntity> { return parse(entitySchema, await desktopInvoke('save_story_entity', { input }), 'Story-Bible-Eintrag'); }
  async listStoryEntities(projectId: string): Promise<StoryEntity[]> { return parse(z.array(entitySchema), await desktopInvoke('list_story_entities', { projectId }), 'Story-Bible-Liste'); }
  async createStoryEntity(input: CreateStoryEntityInput): Promise<StoryEntity> { return parse(entitySchema, await desktopInvoke('create_story_entity', { input }), 'Story-Bible-Eintrag'); }
  async updateStoryEntity(input: UpdateStoryEntityInput): Promise<StoryEntity> { return parse(entitySchema, await desktopInvoke('update_story_entity', { input }), 'Story-Bible-Eintrag'); }
  async archiveStoryEntity(id: string): Promise<StoryEntity> { return parse(entitySchema, await desktopInvoke('archive_story_entity', { id }), 'Archivierter Story-Bible-Eintrag'); }
  async getStoryEntity(id: string): Promise<StoryEntity> { return parse(entitySchema, await desktopInvoke('get_story_entity', { id }), 'Story-Bible-Eintrag'); }
  async createSourceReference(input: CreateSourceReferenceInput): Promise<StorySourceReference> { return parse(sourceSchema, await desktopInvoke('create_source_reference', { input }), 'Quellenreferenz'); }
  async listSourceReferences(projectId: string, entityId?: string): Promise<StorySourceReference[]> { return parse(z.array(sourceSchema), await desktopInvoke('list_source_references', { projectId, entityId }), 'Quellenreferenzen'); }
  async createBibleUpdateRun(input: CreateBibleUpdateRunInput): Promise<BibleUpdateRun> { return parse(runSchema, await desktopInvoke('create_bible_update_run', { input }), 'Bible-Update-Lauf'); }
  async listBibleUpdateRuns(projectId: string, sceneId?: string): Promise<BibleUpdateRun[]> { return parse(z.array(runSchema), await desktopInvoke('list_bible_update_runs', { projectId, sceneId }), 'Bible-Update-Läufe'); }
  async listBibleProposals(runId: string): Promise<BibleProposal[]> { return parse(z.array(proposalSchema), await desktopInvoke('list_bible_proposals', { runId }), 'Bible-Vorschläge'); }
  async saveBibleProposals(runId: string, proposals: BibleProposalDraft[], projectId: string, sceneId: string): Promise<BibleProposal[]> { return parse(z.array(proposalSchema), await desktopInvoke('save_bible_proposals', { runId, proposals: proposals.map((proposal) => ({ ...proposal, runId, projectId, sceneId })) }), 'Bible-Vorschläge'); }
  async reviewBibleProposal(input: ReviewBibleProposalInput): Promise<BibleProposal> { return parse(proposalSchema, await desktopInvoke('review_bible_proposal', { input }), 'Review-Vorschlag'); }
  async completeBibleReview(runId: string): Promise<BibleUpdateRun> { return parse(runSchema, await desktopInvoke('complete_bible_review', { runId }), 'Bible-Review'); }
  async getDatabaseInfo(): Promise<DatabaseInfo> { return parse(z.object({ path: z.string(), connected: z.boolean(), engine: z.literal('sqlite'), detail: z.string() }), await desktopInvoke('database_info'), 'Datenbankstatus'); }
}

interface BrowserState { project: Project; books: Book[]; chapters: Chapter[]; entities: StoryEntity[]; versions: SceneVersion[]; editorPreferences: EditorPreferences; sources: StorySourceReference[]; runs: BibleUpdateRun[]; proposals: BibleProposal[]; }
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
        return { ...state, versions: state.versions ?? [], editorPreferences: state.editorPreferences ?? defaultEditorPreferences, sources: state.sources ?? [], runs: state.runs ?? [], proposals: state.proposals ?? [] } as BrowserState;
      }
    } catch { /* A broken demo cache is replaced with the safe example workspace. */ }
    return { project: clone(demoProject), books: [{ id: 'book-1', projectId: demoProject.id, title: demoProject.title, volume: 1 }], chapters: clone(demoChapters), entities: clone(demoEntities), versions: [], editorPreferences: defaultEditorPreferences, sources: [], runs: [], proposals: [] };
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
    state.versions = []; state.sources = []; state.runs = []; state.proposals = [];
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
    chapter.scenes[index] = saved; chapter.updatedAt = saved.updatedAt ?? now(); state.project.updatedAt = saved.updatedAt ?? now(); this.write(state); return clone(saved);
  }
  async createSceneVersion(input: CreateSceneVersionInput): Promise<SceneVersion> {
    const state = this.read();
    const scene = state.chapters.flatMap((chapter) => chapter.scenes).find((item) => item.id === input.sceneId);
    if (!scene) throw new Error('Die Szene wurde nicht gefunden.');
    const reason = input.reason ?? 'manual';
    const version: SceneVersion = { id: crypto.randomUUID(), sceneId: scene.id, versionNumber: state.versions.filter((item) => item.sceneId === scene.id).length + 1, content: scene.content, reason, createdAt: now(), scene: clone(scene) };
    state.versions = [version, ...state.versions]; this.write(state); return clone(version);
  }
  async listSceneVersions(sceneId: string): Promise<SceneVersion[]> { return this.read().versions.filter((version) => version.sceneId === sceneId).map(clone); }
  async restoreSceneVersion(sceneId: string, versionId: string): Promise<Scene> { const version = this.read().versions.find((item) => item.sceneId === sceneId && item.id === versionId); if (!version) throw new Error('Die Version wurde nicht gefunden.'); return this.updateScene({ ...version.scene, id: sceneId }); }
  async getEditorPreferences(): Promise<EditorPreferences> { const value = localStorage.getItem(browserPreferencesKey); return value ? editorPreferencesSchema.parse(JSON.parse(value)) : clone(defaultEditorPreferences); }
  async saveEditorPreferences(input: EditorPreferences): Promise<EditorPreferences> { const saved = editorPreferencesSchema.parse({ ...input, fontSize: Math.max(14, Math.min(28, input.fontSize)), lineHeight: Math.max(1.3, Math.min(2.5, input.lineHeight)) }); localStorage.setItem(browserPreferencesKey, JSON.stringify(saved)); return saved; }
  async saveStoryEntity(input: SaveStoryEntityInput): Promise<StoryEntity> { const state = this.read(); const saved = { ...input, updatedAt: now(), origin: input.origin ?? 'manual' }; state.entities = [saved, ...state.entities.filter((item) => item.id !== saved.id)]; this.write(state); return clone(saved); }
  async listStoryEntities(projectId: string): Promise<StoryEntity[]> { return this.read().entities.filter((entity) => !entity.projectId || entity.projectId === projectId).map(clone); }
  async createStoryEntity(input: CreateStoryEntityInput): Promise<StoryEntity> { const state = this.read(); const stamp = now(); const chapter = state.chapters.find((item) => item.id === input.chapterId); const scene = chapter?.scenes.find((item) => item.id === input.sceneId); const saved: StoryEntity = { id: crypto.randomUUID(), projectId: input.projectId, name: input.name, type: input.type, description: input.description, status: input.status, confidence: input.confidence, source: input.excerpt, chapter: chapter?.title ?? '', scene: scene?.title ?? '', authorConfirmed: input.authorConfirmed, tags: input.tags, updatedAt: stamp, createdAt: stamp, origin: 'manual' }; state.entities = [saved, ...state.entities]; if (chapter && scene && input.sceneId) state.sources.push({ id: crypto.randomUUID(), projectId: input.projectId, entityId: saved.id, chapterId: chapter.id, sceneId: input.sceneId, excerpt: input.excerpt, createdAt: stamp }); this.write(state); return clone(saved); }
  async updateStoryEntity(input: UpdateStoryEntityInput): Promise<StoryEntity> { const state = this.read(); const current = state.entities.find((item) => item.id === input.id); if (!current) throw new Error('Der Story-Bible-Eintrag wurde nicht gefunden.'); const stamp = now(); const chapter = state.chapters.find((item) => item.id === input.chapterId); const scene = chapter?.scenes.find((item) => item.id === input.sceneId); const saved: StoryEntity = { ...current, ...input, chapter: chapter?.title ?? '', scene: scene?.title ?? '', source: input.excerpt, updatedAt: stamp, origin: current.origin === 'manual' ? 'edited' : current.origin }; state.entities = state.entities.map((item) => item.id === saved.id ? saved : item); this.write(state); return clone(saved); }
  async archiveStoryEntity(id: string): Promise<StoryEntity> { const state = this.read(); const current = state.entities.find((item) => item.id === id); if (!current) throw new Error('Der Story-Bible-Eintrag wurde nicht gefunden.'); const saved = { ...current, status: 'archived' as const, updatedAt: now() }; state.entities = state.entities.map((item) => item.id === id ? saved : item); this.write(state); return clone(saved); }
  async getStoryEntity(id: string): Promise<StoryEntity> { const item = this.read().entities.find((entity) => entity.id === id); if (!item) throw new Error('Der Story-Bible-Eintrag wurde nicht gefunden.'); return clone(item); }
  async createSourceReference(input: CreateSourceReferenceInput): Promise<StorySourceReference> { const state = this.read(); const saved: StorySourceReference = { id: crypto.randomUUID(), ...input, createdAt: now() }; state.sources.push(saved); this.write(state); return clone(saved); }
  async listSourceReferences(projectId: string, entityId?: string): Promise<StorySourceReference[]> { return this.read().sources.filter((source) => source.projectId === projectId && (!entityId || source.entityId === entityId)).map(clone); }
  async createBibleUpdateRun(input: CreateBibleUpdateRunInput): Promise<BibleUpdateRun> { const state = this.read(); const existing = !input.force ? state.runs.find((run) => run.sceneId === input.sceneId && run.contentHash === input.contentHash && ['completed', 'reviewed'].includes(run.status)) : undefined; if (existing) return clone(existing); const saved: BibleUpdateRun = { id: crypto.randomUUID(), ...input, extractorId: input.extractorId, status: 'pending', createdAt: now() }; state.runs.unshift(saved); this.write(state); return clone(saved); }
  async listBibleUpdateRuns(projectId: string, sceneId?: string): Promise<BibleUpdateRun[]> { return this.read().runs.filter((run) => run.projectId === projectId && (!sceneId || run.sceneId === sceneId)).map(clone); }
  async listBibleProposals(runId: string): Promise<BibleProposal[]> { return this.read().proposals.filter((proposal) => proposal.runId === runId).map(clone); }
  async saveBibleProposals(runId: string, proposals: BibleProposalDraft[], projectId: string, sceneId: string): Promise<BibleProposal[]> { const state = this.read(); const saved = proposals.map((proposal) => ({ ...proposal, id: crypto.randomUUID(), runId, projectId, sceneId, reviewStatus: 'pending' as const, createdAt: now() })); state.proposals = [...saved, ...state.proposals.filter((item) => item.runId !== runId)]; state.runs = state.runs.map((run) => run.id === runId ? { ...run, status: 'completed' as const, completedAt: now() } : run); this.write(state); return clone(saved); }
  async reviewBibleProposal(input: ReviewBibleProposalInput): Promise<BibleProposal> { const state = this.read(); const proposal = state.proposals.find((item) => item.id === input.proposalId); if (!proposal) throw new Error('Der Vorschlag wurde nicht gefunden.'); const saved = { ...proposal, ...input, reviewStatus: input.reviewStatus, candidateName: input.candidateName ?? proposal.candidateName, candidateDescription: input.candidateDescription ?? proposal.candidateDescription, candidateStatus: input.candidateStatus ?? proposal.candidateStatus, classification: input.classification ?? proposal.classification, reviewedAt: now() }; let reviewed = saved; if (input.reviewStatus !== 'rejected' && !proposal.targetEntityId) { const chapter = state.chapters.find((item) => item.scenes.some((scene) => scene.id === saved.sceneId)); const scene = chapter?.scenes.find((item) => item.id === saved.sceneId); const stamp = now(); const entity: StoryEntity = { id: crypto.randomUUID(), projectId: proposal.projectId, name: saved.candidateName, type: saved.entityType, description: saved.candidateDescription, status: saved.candidateStatus, confidence: saved.confidence, source: saved.evidenceExcerpt, chapter: chapter?.title ?? '', scene: scene?.title ?? '', authorConfirmed: false, updatedAt: stamp, createdAt: stamp, tags: [], origin: 'bible_update' }; state.entities = [entity, ...state.entities]; if (chapter && scene) state.sources.push({ id: crypto.randomUUID(), projectId: proposal.projectId, entityId: entity.id, proposalId: proposal.id, chapterId: chapter.id, sceneId: scene.id, excerpt: saved.evidenceExcerpt, createdAt: stamp }); reviewed = { ...saved, targetEntityId: entity.id }; } state.proposals = state.proposals.map((item) => item.id === proposal.id ? reviewed : item); this.write(state); return clone(reviewed); }
  async completeBibleReview(runId: string): Promise<BibleUpdateRun> { const state = this.read(); if (state.proposals.some((proposal) => proposal.runId === runId && proposal.reviewStatus === 'pending')) throw new Error('Bitte prüfe zuerst alle offenen Vorschläge.'); state.runs = state.runs.map((run) => run.id === runId ? { ...run, status: 'reviewed' as const } : run); this.write(state); return clone(state.runs.find((run) => run.id === runId)!); }
  async getDatabaseInfo(): Promise<DatabaseInfo> { return { path: 'Browser-Demo: localStorage', connected: true, engine: 'localStorage', detail: 'Nur Vorschau-Daten im Browser. Die Desktop-App verwendet SQLite.' }; }
}

export function createStoryRepository(): StoryRepository { return isTauriRuntime() ? new TauriStoryRepository() : new BrowserDemoRepository(); }
