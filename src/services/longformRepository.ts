import { desktopInvoke, isTauriRuntime } from './desktop';
import type { ChapterDraftPlanResult, ChapterGenerationJob, ChapterGenerationPlan, ChapterGenerationReview, ChapterGenerationSection, CreateChapterGenerationJobInput, SaveChapterGenerationPlanInput, SaveChapterGenerationSectionInput, SaveStoryDirectionInput, SaveWritingPreferencesInput, StoryDirection, WritingPreferences } from '../types/domain';

export interface LongformRepository {
  readonly mode: 'desktop' | 'browser-demo';
  getStoryDirection(projectId: string): Promise<StoryDirection | undefined>;
  saveStoryDirection(input: SaveStoryDirectionInput): Promise<StoryDirection>;
  getWritingPreferences(projectId: string): Promise<WritingPreferences>;
  saveWritingPreferences(input: SaveWritingPreferencesInput): Promise<WritingPreferences>;
  createJob(input: CreateChapterGenerationJobInput): Promise<ChapterGenerationJob>;
  listJobs(projectId: string): Promise<ChapterGenerationJob[]>;
  updateJobStatus(jobId: string, status: ChapterGenerationJob['status'], errorMessage?: string): Promise<ChapterGenerationJob>;
  getPlan(jobId: string): Promise<ChapterGenerationPlan | undefined>;
  savePlan(input: SaveChapterGenerationPlanInput): Promise<ChapterGenerationPlan>;
  listSections(jobId: string): Promise<ChapterGenerationSection[]>;
  saveSection(input: SaveChapterGenerationSectionInput): Promise<ChapterGenerationSection>;
  listReviews(jobId: string): Promise<ChapterGenerationReview[]>;
  acceptJob(jobId: string): Promise<ChapterGenerationJob>;
}

const defaultPreferences = (projectId: string): WritingPreferences => ({ projectId, wordsPerPage: 250, preferredSectionWords: 850, maximumSectionWords: 1200, defaultSceneCount: 4, requirePlanConfirmation: true, requireFinalConfirmation: true, createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() });
const key = 'storymemory-longform-v1';
interface BrowserLongformState { directions: Record<string, StoryDirection>; preferences: Record<string, WritingPreferences>; jobs: ChapterGenerationJob[]; plans: ChapterGenerationPlan[]; sections: ChapterGenerationSection[]; reviews: ChapterGenerationReview[]; }
const emptyState = (): BrowserLongformState => ({ directions: {}, preferences: {}, jobs: [], plans: [], sections: [], reviews: [] });
function read(): BrowserLongformState { if (typeof localStorage === 'undefined') return emptyState(); try { const value = localStorage.getItem(key); return value ? { ...emptyState(), ...JSON.parse(value) as Partial<BrowserLongformState> } : emptyState(); } catch { return emptyState(); } }
function write(state: BrowserLongformState): void { localStorage.setItem(key, JSON.stringify(state)); }
function now(): string { return new Date().toISOString(); }

export class TauriLongformRepository implements LongformRepository {
  readonly mode = 'desktop' as const;
  getStoryDirection(projectId: string) { return desktopInvoke<StoryDirection | null>('get_story_direction', { projectId }).then((value) => value ?? undefined); }
  saveStoryDirection(input: SaveStoryDirectionInput) { return desktopInvoke<StoryDirection>('save_story_direction', input); }
  getWritingPreferences(projectId: string) { return desktopInvoke<WritingPreferences>('get_writing_preferences', { projectId }); }
  saveWritingPreferences(input: SaveWritingPreferencesInput) { return desktopInvoke<WritingPreferences>('save_writing_preferences', input); }
  createJob(input: CreateChapterGenerationJobInput) { return desktopInvoke<ChapterGenerationJob>('create_chapter_generation_job', input as unknown as Record<string, unknown>); }
  listJobs(projectId: string) { return desktopInvoke<ChapterGenerationJob[]>('list_chapter_generation_jobs', { projectId }); }
  updateJobStatus(jobId: string, status: ChapterGenerationJob['status'], errorMessage?: string) { return desktopInvoke<ChapterGenerationJob>('update_chapter_generation_job_status', { jobId, status, errorMessage }); }
  getPlan(jobId: string) { return desktopInvoke<ChapterGenerationPlan | null>('get_chapter_generation_plan', { jobId }).then((value) => value ?? undefined); }
  savePlan(input: SaveChapterGenerationPlanInput) { return desktopInvoke<ChapterGenerationPlan>('save_chapter_generation_plan', input); }
  listSections(jobId: string) { return desktopInvoke<ChapterGenerationSection[]>('list_chapter_generation_sections', { jobId }); }
  saveSection(input: SaveChapterGenerationSectionInput) { return desktopInvoke<ChapterGenerationSection>('save_chapter_generation_section', input); }
  listReviews(jobId: string) { return desktopInvoke<ChapterGenerationReview[]>('list_chapter_generation_reviews', { jobId }); }
  acceptJob(jobId: string) { return desktopInvoke<ChapterGenerationJob>('accept_chapter_generation_job', { jobId }); }
}

export class BrowserLongformRepository implements LongformRepository {
  readonly mode = 'browser-demo' as const;
  getStoryDirection(projectId: string) { return Promise.resolve(read().directions[projectId] ?? undefined); }
  saveStoryDirection(input: SaveStoryDirectionInput) { const state = read(); const saved = { ...input, createdAt: state.directions[input.projectId]?.createdAt ?? now(), updatedAt: now() }; state.directions[input.projectId] = saved; write(state); return Promise.resolve(saved); }
  getWritingPreferences(projectId: string) { const state = read(); return Promise.resolve(state.preferences[projectId] ?? defaultPreferences(projectId)); }
  saveWritingPreferences(input: SaveWritingPreferencesInput) { const state = read(); const saved = { ...input, createdAt: state.preferences[input.projectId]?.createdAt ?? now(), updatedAt: now() }; state.preferences[input.projectId] = saved; write(state); return Promise.resolve(saved); }
  createJob(input: CreateChapterGenerationJobInput) { const state = read(); const job: ChapterGenerationJob = { ...input, id: crypto.randomUUID(), status: 'preparing', createdAt: now(), updatedAt: now() }; state.jobs.unshift(job); write(state); return Promise.resolve(job); }
  listJobs(projectId: string) { return Promise.resolve(read().jobs.filter((job) => job.projectId === projectId)); }
  updateJobStatus(jobId: string, status: ChapterGenerationJob['status'], errorMessage?: string) { const state = read(); const job = state.jobs.find((item) => item.id === jobId); if (!job) return Promise.reject(new Error('Schreibauftrag nicht gefunden.')); Object.assign(job, { status, errorMessage, updatedAt: now(), completedAt: ['accepted', 'cancelled', 'failed'].includes(status) ? job.completedAt ?? now() : job.completedAt }); write(state); return Promise.resolve({ ...job }); }
  getPlan(jobId: string) { return Promise.resolve(read().plans.find((plan) => plan.jobId === jobId)); }
  savePlan(input: SaveChapterGenerationPlanInput) { const state = read(); const existing = state.plans.find((plan) => plan.jobId === input.jobId); const plan: ChapterGenerationPlan = { ...input, id: existing?.id ?? crypto.randomUUID(), createdAt: existing?.createdAt ?? now(), updatedAt: now() }; state.plans = state.plans.filter((item) => item.jobId !== input.jobId); state.plans.push(plan); const job = state.jobs.find((item) => item.id === input.jobId); if (job) Object.assign(job, { status: 'plan_ready', updatedAt: now() }); write(state); return Promise.resolve(plan); }
  listSections(jobId: string) { return Promise.resolve(read().sections.filter((section) => section.jobId === jobId).sort((a, b) => a.orderIndex - b.orderIndex)); }
  saveSection(input: SaveChapterGenerationSectionInput) { const state = read(); const existing = state.sections.find((section) => section.jobId === input.jobId && section.orderIndex === input.orderIndex); const section: ChapterGenerationSection = { ...input, id: existing?.id ?? crypto.randomUUID(), actualWords: input.content.trim().split(/\s+/).filter(Boolean).length, createdAt: existing?.createdAt ?? now(), updatedAt: now() }; state.sections = state.sections.filter((item) => !(item.jobId === input.jobId && item.orderIndex === input.orderIndex)); state.sections.push(section); write(state); return Promise.resolve(section); }
  listReviews(jobId: string) { return Promise.resolve(read().reviews.filter((review) => review.jobId === jobId)); }
  acceptJob(jobId: string) { return this.updateJobStatus(jobId, 'accepted'); }
}

export function createLongformRepository(): LongformRepository { return isTauriRuntime() ? new TauriLongformRepository() : new BrowserLongformRepository(); }

export function createPlanFrame(input: { job: ChapterGenerationJob; chapterTitle: string; povCharacterId?: string; sceneCount: number; sectionWords: number }): ChapterDraftPlanResult {
  const beats = Array.from({ length: input.sceneCount }, (_, index) => ({ id: `beat-${index + 1}`, orderIndex: index, title: `Abschnitt ${index + 1}`, purpose: index === 0 ? 'Ausgangslage und Impuls setzen' : index === input.sceneCount - 1 ? 'Konsequenz und Kapitelhaken setzen' : 'Konflikt vertiefen und Entscheidung vorbereiten', location: undefined, povCharacterId: input.povCharacterId, participatingCharacterIds: input.povCharacterId ? [input.povCharacterId] : [], startingState: index === 0 ? 'Ausgangszustand des bisherigen Handlungsstands' : 'Zustand aus dem vorherigen Abschnitt', event: '', conflict: '', newInformation: [], knowledgeChanges: [], relationshipChanges: [], cluesUsed: [], loreEntityIds: [], endingHook: index === input.sceneCount - 1 ? 'Offener Anschluss für das nächste Kapitel' : '', targetWords: input.sectionWords }));
  return { chapterTitle: input.chapterTitle, chapterGoal: 'Noch vom Autor zu bestätigen', povCharacterId: input.povCharacterId, startingState: 'Wird im Plan festgelegt', endingState: 'Wird im Plan festgelegt', chapterSummary: '', endingConnection: '', newInformation: [], withheldInformation: [], assumptions: [{ type: 'author_decision', text: 'Die konkreten Ereignisse und das Ende dieses Kapitels müssen im Plan bestätigt werden.' }], beats, warnings: ['Der lokale Prototyp erstellt nur einen Planrahmen. Für Textgenerierung ist Codex CLI erforderlich.'] };
}
