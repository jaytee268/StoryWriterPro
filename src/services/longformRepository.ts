import { desktopInvoke, isTauriRuntime } from './desktop';
import type { ChapterDraftPlanResult, ChapterGenerationJob, ChapterGenerationPlan, ChapterGenerationReview, ChapterGenerationSection, CreateChapterGenerationJobInput, SaveChapterGenerationPlanInput, SaveChapterGenerationReviewInput, SaveChapterGenerationSectionInput, SaveStoryDirectionInput, SaveWritingPreferencesInput, StoryDirection, WritingPreferences } from '../types/domain';

export interface LongformRepository {
  readonly mode: 'desktop' | 'browser-demo';
  getStoryDirection(projectId: string): Promise<StoryDirection | undefined>;
  saveStoryDirection(input: SaveStoryDirectionInput): Promise<StoryDirection>;
  getWritingPreferences(projectId: string): Promise<WritingPreferences>;
  saveWritingPreferences(input: SaveWritingPreferencesInput): Promise<WritingPreferences>;
  createJob(input: CreateChapterGenerationJobInput): Promise<ChapterGenerationJob>;
  listJobs(projectId: string): Promise<ChapterGenerationJob[]>;
  updateJobStatus(jobId: string, status: ChapterGenerationJob['status'], errorMessage?: string): Promise<ChapterGenerationJob>;
  acceptContextOverride(jobId: string): Promise<ChapterGenerationJob>;
  getPlan(jobId: string): Promise<ChapterGenerationPlan | undefined>;
  savePlan(input: SaveChapterGenerationPlanInput): Promise<ChapterGenerationPlan>;
  listSections(jobId: string): Promise<ChapterGenerationSection[]>;
  saveSection(input: SaveChapterGenerationSectionInput): Promise<ChapterGenerationSection>;
  listReviews(jobId: string): Promise<ChapterGenerationReview[]>;
  saveReviews(jobId: string, reviews: SaveChapterGenerationReviewInput[]): Promise<ChapterGenerationReview[]>;
  deleteReviewsForSection(jobId: string, sectionId: string): Promise<void>;
  updateReviewStatus(id: string, status: string): Promise<ChapterGenerationReview>;
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
  acceptContextOverride(jobId: string) { return desktopInvoke<ChapterGenerationJob>('accept_chapter_generation_context_override', { jobId }); }
  getPlan(jobId: string) { return desktopInvoke<ChapterGenerationPlan | null>('get_chapter_generation_plan', { jobId }).then((value) => value ?? undefined); }
  savePlan(input: SaveChapterGenerationPlanInput) { return desktopInvoke<ChapterGenerationPlan>('save_chapter_generation_plan', input); }
  listSections(jobId: string) { return desktopInvoke<ChapterGenerationSection[]>('list_chapter_generation_sections', { jobId }); }
  saveSection(input: SaveChapterGenerationSectionInput) { return desktopInvoke<ChapterGenerationSection>('save_chapter_generation_section', input); }
  listReviews(jobId: string) { return desktopInvoke<ChapterGenerationReview[]>('list_chapter_generation_reviews', { jobId }); }
  saveReviews(jobId: string, reviews: SaveChapterGenerationReviewInput[]) { return desktopInvoke<ChapterGenerationReview[]>('save_chapter_generation_reviews', { jobId, reviews }); }
  deleteReviewsForSection(jobId: string, sectionId: string) { return desktopInvoke<void>('delete_chapter_generation_reviews_for_section', { jobId, sectionId }); }
  updateReviewStatus(id: string, status: string) { return desktopInvoke<ChapterGenerationReview>('update_chapter_generation_review_status', { id, status }); }
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
  acceptContextOverride(jobId: string) { const state = read(); const job = state.jobs.find((item) => item.id === jobId); if (!job) return Promise.reject(new Error('Schreibauftrag nicht gefunden.')); job.contextOverrideAccepted = true; job.updatedAt = now(); write(state); return Promise.resolve({ ...job }); }
  getPlan(jobId: string) { return Promise.resolve(read().plans.find((plan) => plan.jobId === jobId)); }
  savePlan(input: SaveChapterGenerationPlanInput) { const state = read(); const existing = state.plans.find((plan) => plan.jobId === input.jobId); const plan: ChapterGenerationPlan = { ...input, id: existing?.id ?? crypto.randomUUID(), createdAt: existing?.createdAt ?? now(), updatedAt: now() }; state.plans = state.plans.filter((item) => item.jobId !== input.jobId); state.plans.push(plan); const job = state.jobs.find((item) => item.id === input.jobId); if (job) Object.assign(job, { status: 'plan_ready', updatedAt: now() }); write(state); return Promise.resolve(plan); }
  listSections(jobId: string) { return Promise.resolve(read().sections.filter((section) => section.jobId === jobId).sort((a, b) => a.orderIndex - b.orderIndex)); }
  saveSection(input: SaveChapterGenerationSectionInput) { const state = read(); const existing = state.sections.find((section) => section.jobId === input.jobId && section.orderIndex === input.orderIndex); const section: ChapterGenerationSection = { ...input, id: existing?.id ?? crypto.randomUUID(), actualWords: input.content.trim().split(/\s+/).filter(Boolean).length, createdAt: existing?.createdAt ?? now(), updatedAt: now() }; state.sections = state.sections.filter((item) => !(item.jobId === input.jobId && item.orderIndex === input.orderIndex)); state.sections.push(section); write(state); return Promise.resolve(section); }
  listReviews(jobId: string) { return Promise.resolve(read().reviews.filter((review) => review.jobId === jobId)); }
  saveReviews(jobId: string, reviews: SaveChapterGenerationReviewInput[]) { const state = read(); const saved = reviews.map((review) => ({ ...review, id: crypto.randomUUID(), jobId, createdAt: now(), updatedAt: now() })); state.reviews = [...state.reviews.filter((review) => review.jobId !== jobId || !reviews.some((input) => input.sectionId === review.sectionId)), ...saved]; write(state); return Promise.resolve(saved); }
  deleteReviewsForSection(jobId: string, sectionId: string) { const state = read(); state.reviews = state.reviews.filter((review) => !(review.jobId === jobId && review.sectionId === sectionId)); write(state); return Promise.resolve(); }
  updateReviewStatus(id: string, status: string) { const state = read(); const review = state.reviews.find((item) => item.id === id); if (!review) return Promise.reject(new Error('Kapitelprüfung nicht gefunden.')); const saved = { ...review, status, updatedAt: now() }; state.reviews = state.reviews.map((item) => item.id === id ? saved : item); write(state); return Promise.resolve(saved); }
  acceptJob(jobId: string) { const state = read(); const job = state.jobs.find((item) => item.id === jobId); const plan = state.plans.find((item) => item.jobId === jobId); const sections = state.sections.filter((item) => item.jobId === jobId).sort((a, b) => a.orderIndex - b.orderIndex); if (!job || !plan || job.status !== 'draft_ready' || sections.length === 0 || sections.some((section) => !section.content.trim())) return Promise.reject(new Error('Der Entwurf ist noch nicht vollständig geprüft.')); const workspaceKey = 'storymemory-browser-demo-workspace'; try { const raw = localStorage.getItem(workspaceKey); if (!raw) return Promise.reject(new Error('Browser-Workspace konnte nicht geöffnet werden.')); const workspace = JSON.parse(raw) as { books: Array<{ id: string; projectId: string }>; chapters: Array<{ id: string; bookId: string; title: string; orderIndex: number; scenes: unknown[]; createdAt: string; updatedAt: string }> }; const book = workspace.books.find((item) => item.id === job.targetBookId && item.projectId === job.projectId); if (!book) return Promise.reject(new Error('Das Zielbuch gehört nicht zum Projekt.')); const stamp = now(); const chapterId = crypto.randomUUID(); const chapter = { id: chapterId, bookId: book.id, title: plan.chapterTitle, orderIndex: workspace.chapters.filter((item) => item.bookId === book.id).length + 1, scenes: sections.map((section, index) => ({ id: crypto.randomUUID(), chapterId, title: `Szene ${index + 1}`, orderIndex: index + 1, content: section.content, pov: plan.beats[index]?.povCharacterId ?? plan.povCharacterId ?? '', location: plan.beats[index]?.location ?? '', storyTime: '', status: 'draft', goal: plan.chapterGoal, notes: '', createdAt: stamp, updatedAt: stamp })), createdAt: stamp, updatedAt: stamp }; workspace.chapters = [...workspace.chapters, chapter]; localStorage.setItem(workspaceKey, JSON.stringify(workspace)); const saved = { ...job, status: 'accepted' as const, updatedAt: stamp, completedAt: stamp }; state.jobs = state.jobs.map((item) => item.id === jobId ? saved : item); write(state); return Promise.resolve(saved); } catch (cause) { return Promise.reject(cause instanceof Error ? cause : new Error('Der Entwurf konnte nicht atomar übernommen werden.')); } }
}

export function createLongformRepository(): LongformRepository { return isTauriRuntime() ? new TauriLongformRepository() : new BrowserLongformRepository(); }

export function createPlanFrame(input: { job: ChapterGenerationJob; chapterTitle: string; povCharacterId?: string; sceneCount: number; sectionWords: number }): ChapterDraftPlanResult {
  const beats = Array.from({ length: input.sceneCount }, (_, index) => ({ id: `beat-${index + 1}`, orderIndex: index, title: `Abschnitt ${index + 1}`, purpose: index === 0 ? 'Ausgangslage und Impuls setzen' : index === input.sceneCount - 1 ? 'Konsequenz und Kapitelhaken setzen' : 'Konflikt vertiefen und Entscheidung vorbereiten', location: undefined, povCharacterId: input.povCharacterId, participatingCharacterIds: input.povCharacterId ? [input.povCharacterId] : [], startingState: index === 0 ? 'Ausgangszustand des bisherigen Handlungsstands' : 'Zustand aus dem vorherigen Abschnitt', event: '', conflict: '', newInformation: [], knowledgeChanges: [], relationshipChanges: [], cluesUsed: [], loreEntityIds: [], endingHook: index === input.sceneCount - 1 ? 'Offener Anschluss für das nächste Kapitel' : '', targetWords: input.sectionWords }));
  return { chapterTitle: input.chapterTitle, chapterGoal: 'Noch vom Autor zu bestätigen', povCharacterId: input.povCharacterId, startingState: 'Wird im Plan festgelegt', endingState: 'Wird im Plan festgelegt', chapterSummary: '', endingConnection: '', newInformation: [], withheldInformation: [], assumptions: [{ type: 'author_decision', text: 'Die konkreten Ereignisse und das Ende dieses Kapitels müssen im Plan bestätigt werden.' }], beats, warnings: ['Der lokale Prototyp erstellt nur einen Planrahmen. Für Textgenerierung ist Codex CLI erforderlich.'] };
}
