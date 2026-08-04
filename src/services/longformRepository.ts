import { desktopInvoke, isTauriRuntime } from './desktop';
import { contentHash } from '../utils/aiText';
import type { AcceptChapterGenerationJobInput, ChapterDraftPlanResult, ChapterGenerationDraftLedgerEntry, ChapterGenerationJob, ChapterGenerationPlan, ChapterGenerationReview, ChapterGenerationSection, CreateChapterGenerationJobInput, SaveChapterGenerationDraftLedgerInput, SaveChapterGenerationPlanInput, SaveChapterGenerationReviewInput, SaveChapterGenerationSectionInput, SaveStoryDirectionInput, SaveWritingPreferencesInput, StoryDirection, StorySourceReference, ContinuityStateLedgerEntry, WritingPreferences } from '../types/domain';

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
  listDraftLedger(jobId: string): Promise<ChapterGenerationDraftLedgerEntry[]>;
  replaceDraftLedger(sectionId: string, entries: SaveChapterGenerationDraftLedgerInput[]): Promise<ChapterGenerationDraftLedgerEntry[]>;
  supersedeDraftLedgerFrom(jobId: string, orderIndex: number): Promise<void>;
  listReviews(jobId: string): Promise<ChapterGenerationReview[]>;
  saveReviews(jobId: string, reviews: SaveChapterGenerationReviewInput[]): Promise<ChapterGenerationReview[]>;
  deleteReviewsForSection(jobId: string, sectionId: string): Promise<void>;
  updateReviewStatus(id: string, status: string): Promise<ChapterGenerationReview>;
  acceptJob(input: AcceptChapterGenerationJobInput): Promise<ChapterGenerationJob>;
}

const defaultPreferences = (projectId: string): WritingPreferences => ({ projectId, wordsPerPage: 250, preferredSectionWords: 850, maximumSectionWords: 1200, defaultSceneCount: 4, requirePlanConfirmation: true, requireFinalConfirmation: true, createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() });
const key = 'storymemory-longform-v1';
interface BrowserLongformState { directions: Record<string, StoryDirection>; preferences: Record<string, WritingPreferences>; jobs: ChapterGenerationJob[]; plans: ChapterGenerationPlan[]; sections: ChapterGenerationSection[]; draftLedger: ChapterGenerationDraftLedgerEntry[]; reviews: ChapterGenerationReview[]; }
const emptyState = (): BrowserLongformState => ({ directions: {}, preferences: {}, jobs: [], plans: [], sections: [], draftLedger: [], reviews: [] });
function read(): BrowserLongformState { if (typeof localStorage === 'undefined') return emptyState(); try { const value = localStorage.getItem(key); return value ? { ...emptyState(), ...JSON.parse(value) as Partial<BrowserLongformState>, draftLedger: (JSON.parse(value) as Partial<BrowserLongformState>).draftLedger ?? [] } : emptyState(); } catch { return emptyState(); } }
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
  listDraftLedger(jobId: string) { return desktopInvoke<ChapterGenerationDraftLedgerEntry[]>('list_chapter_generation_draft_ledger', { jobId }); }
  replaceDraftLedger(sectionId: string, entries: SaveChapterGenerationDraftLedgerInput[]) { return desktopInvoke<ChapterGenerationDraftLedgerEntry[]>('replace_chapter_generation_draft_ledger', { sectionId, entries }); }
  supersedeDraftLedgerFrom(jobId: string, orderIndex: number) { return desktopInvoke<void>('supersede_chapter_generation_draft_ledger_from', { jobId, orderIndex }); }
  listReviews(jobId: string) { return desktopInvoke<ChapterGenerationReview[]>('list_chapter_generation_reviews', { jobId }); }
  saveReviews(jobId: string, reviews: SaveChapterGenerationReviewInput[]) { return desktopInvoke<ChapterGenerationReview[]>('save_chapter_generation_reviews', { jobId, reviews }); }
  deleteReviewsForSection(jobId: string, sectionId: string) { return desktopInvoke<void>('delete_chapter_generation_reviews_for_section', { jobId, sectionId }); }
  updateReviewStatus(id: string, status: string) { return desktopInvoke<ChapterGenerationReview>('update_chapter_generation_review_status', { id, status }); }
  acceptJob(input: AcceptChapterGenerationJobInput) { return desktopInvoke<ChapterGenerationJob>('accept_chapter_generation_job', input as unknown as Record<string, unknown>); }
}

interface BrowserScene { id: string; chapterId: string; title: string; orderIndex: number; content: string; pov: string; location: string; storyTime: string; status: string; goal: string; notes: string; createdAt: string; updatedAt: string; }
interface BrowserChapter { id: string; bookId: string; title: string; orderIndex: number; scenes: BrowserScene[]; createdAt: string; updatedAt: string; }
interface BrowserWorkspace { project?: { id: string }; books: Array<{ id: string; projectId: string }>; chapters: BrowserChapter[]; entities?: Array<{ id: string; projectId: string }>; sources?: StorySourceReference[]; continuityLedger?: ContinuityStateLedgerEntry[]; }
const browserWorkspaceKey = 'storymemory-browser-demo-workspace';

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
  listSections(jobId: string) { return Promise.resolve(read().sections.filter((section) => section.jobId === jobId).sort((a, b) => a.orderIndex - b.orderIndex).map((section) => ({ ...section, draftState: section.draftState ?? 'valid' }))); }
  saveSection(input: SaveChapterGenerationSectionInput) { const state = read(); const existing = state.sections.find((section) => section.jobId === input.jobId && section.orderIndex === input.orderIndex); const section: ChapterGenerationSection = { ...input, draftState: input.draftState ?? existing?.draftState ?? 'valid', contentHash: input.contentHash ?? existing?.contentHash ?? contentHash(input.content), id: existing?.id ?? crypto.randomUUID(), actualWords: input.content.trim().split(/\s+/).filter(Boolean).length, createdAt: existing?.createdAt ?? now(), updatedAt: now() }; state.sections = state.sections.filter((item) => !(item.jobId === input.jobId && input.orderIndex === item.orderIndex)); state.sections.push(section); write(state); return Promise.resolve(section); }
  listDraftLedger(jobId: string) { return Promise.resolve(read().draftLedger.filter((entry) => entry.jobId === jobId).sort((a, b) => a.createdAt.localeCompare(b.createdAt))); }
  replaceDraftLedger(sectionId: string, entries: SaveChapterGenerationDraftLedgerInput[]) { const state = read(); const section = state.sections.find((item) => item.id === sectionId); const job = section && state.jobs.find((item) => item.id === section.jobId); if (!section || !job) return Promise.reject(new Error('Abschnitt oder Schreibauftrag nicht gefunden.')); const stamp = now(); const saved = entries.map((entry) => ({ ...entry, id: entry.id ?? crypto.randomUUID(), jobId: job.id, sectionId, projectId: job.projectId, status: entry.status ?? 'proposed', createdAt: stamp, updatedAt: stamp })); state.draftLedger = [...state.draftLedger.filter((entry) => entry.sectionId !== sectionId), ...saved]; write(state); return Promise.resolve(saved); }
  supersedeDraftLedgerFrom(jobId: string, orderIndex: number) { const state = read(); const sectionIds = new Set(state.sections.filter((section) => section.jobId === jobId && section.orderIndex >= orderIndex).map((section) => section.id)); const stamp = now(); state.draftLedger = state.draftLedger.map((entry) => sectionIds.has(entry.sectionId) && entry.status === 'proposed' ? { ...entry, status: 'superseded' as const, updatedAt: stamp } : entry); write(state); return Promise.resolve(); }
  listReviews(jobId: string) { return Promise.resolve(read().reviews.filter((review) => review.jobId === jobId)); }
  saveReviews(jobId: string, reviews: SaveChapterGenerationReviewInput[]) { const state = read(); const saved = reviews.map((review) => ({ ...review, id: crypto.randomUUID(), jobId, createdAt: now(), updatedAt: now() })); state.reviews = [...state.reviews.filter((review) => review.jobId !== jobId || !reviews.some((input) => input.sectionId === review.sectionId)), ...saved]; write(state); return Promise.resolve(saved); }
  deleteReviewsForSection(jobId: string, sectionId: string) { const state = read(); state.reviews = state.reviews.filter((review) => !(review.jobId === jobId && review.sectionId === sectionId)); write(state); return Promise.resolve(); }
  updateReviewStatus(id: string, status: string) { const state = read(); const review = state.reviews.find((item) => item.id === id); if (!review) return Promise.reject(new Error('Kapitelprüfung nicht gefunden.')); const saved = { ...review, status, updatedAt: now() }; state.reviews = state.reviews.map((item) => item.id === id ? saved : item); write(state); return Promise.resolve(saved); }
  acceptJob(input: AcceptChapterGenerationJobInput) {
    const state = read();
    const job = state.jobs.find((item) => item.id === input.jobId);
    const plan = state.plans.find((item) => item.jobId === input.jobId);
    const sections = state.sections.filter((item) => item.jobId === input.jobId).sort((a, b) => a.orderIndex - b.orderIndex);
    const ledger = state.draftLedger.filter((entry) => entry.jobId === input.jobId);
    const invalid = !job || !plan || job.status !== 'draft_ready' || (input.currentContextHash !== job.contentContextHash && !job.contextOverrideAccepted) || sections.length !== plan.beats.length || sections.some((section) => !section.content.trim() || ['pending', 'regenerate_requested', 'failed'].includes(section.status) || ['stale', 'regenerate_requested'].includes(section.draftState ?? 'valid') || section.contentHash !== contentHash(section.content) || ledger.some((entry) => entry.sectionId === section.id && entry.status === 'proposed' && entry.contentHash !== section.contentHash)) || state.reviews.some((review) => review.jobId === input.jobId && review.status === 'open' && review.severity === 'blocking');
    if (invalid) return Promise.reject(new Error('Der Entwurf ist noch nicht vollständig geprüft.'));
    try {
      const raw = localStorage.getItem(browserWorkspaceKey);
      if (!raw) return Promise.reject(new Error('Browser-Workspace konnte nicht geöffnet werden.'));
      const workspace = JSON.parse(raw) as BrowserWorkspace;
      const book = workspace.books.find((item) => item.id === job!.targetBookId && item.projectId === job!.projectId);
      if (!book) return Promise.reject(new Error('Das Zielbuch gehört nicht zum Projekt.'));
      if (workspace.entities && ledger.some((entry) => entry.status === 'proposed' && !workspace.entities!.some((entity) => entity.id === entry.entityId && entity.projectId === job!.projectId))) return Promise.reject(new Error('Ein Draft-Zustand verweist auf eine fremde Entität.'));
      const stamp = now(); const chapterId = crypto.randomUUID();
      const chapter: BrowserChapter = { id: chapterId, bookId: book.id, title: plan!.chapterTitle, orderIndex: workspace.chapters.filter((item) => item.bookId === book.id).length + 1, scenes: sections.map((section, index) => ({ id: crypto.randomUUID(), chapterId, title: plan!.beats[index]?.title ?? `Szene ${index + 1}`, orderIndex: index + 1, content: section.content, pov: plan!.beats[index]?.povCharacterId ?? plan!.povCharacterId ?? '', location: plan!.beats[index]?.location ?? '', storyTime: '', status: 'draft', goal: plan!.chapterGoal, notes: '', createdAt: stamp, updatedAt: stamp })), createdAt: stamp, updatedAt: stamp };
      workspace.sources = workspace.sources ?? []; workspace.continuityLedger = workspace.continuityLedger ?? [];
      chapter.scenes.forEach((scene, index) => { const section = sections[index]!; ledger.filter((entry) => entry.sectionId === section.id && entry.status === 'proposed').forEach((entry) => { const sourceId = crypto.randomUUID(); workspace.sources!.push({ id: sourceId, projectId: job!.projectId, entityId: entry.entityId, chapterId, sceneId: scene.id, excerpt: entry.sourceExcerpt, startOffset: entry.sourceStartOffset, endOffset: entry.sourceEndOffset, createdAt: stamp }); workspace.continuityLedger!.push({ id: crypto.randomUUID(), projectId: job!.projectId, entityId: entry.entityId, relatedEntityId: entry.relatedEntityId, stateKind: entry.stateKind, previousState: entry.previousState, newState: entry.newState, evidenceExcerpt: entry.sourceExcerpt, chapterId, sceneId: scene.id, startOffset: entry.sourceStartOffset, endOffset: entry.sourceEndOffset, sourceReferenceId: sourceId, status: 'proposed', confidence: entry.confidence, authorConfirmed: false, createdAt: stamp, updatedAt: stamp }); entry.status = 'accepted_for_manuscript_review'; entry.sourceReferenceId = sourceId; entry.updatedAt = stamp; }); });
      workspace.chapters = [...workspace.chapters, chapter]; localStorage.setItem(browserWorkspaceKey, JSON.stringify(workspace)); const saved = { ...job!, status: 'accepted' as const, updatedAt: stamp, completedAt: stamp }; state.jobs = state.jobs.map((item) => item.id === input.jobId ? saved : item); state.draftLedger = state.draftLedger.map((entry) => ledger.find((item) => item.id === entry.id) ?? entry); write(state); return Promise.resolve(saved);
    } catch (cause) { return Promise.reject(cause instanceof Error ? cause : new Error('Der Entwurf konnte nicht atomar übernommen werden.')); }
  }
}

export function createLongformRepository(): LongformRepository { return isTauriRuntime() ? new TauriLongformRepository() : new BrowserLongformRepository(); }

export function createPlanFrame(input: { job: ChapterGenerationJob; chapterTitle: string; povCharacterId?: string; sceneCount: number; sectionWords: number }): ChapterDraftPlanResult {
  const beats = Array.from({ length: input.sceneCount }, (_, index) => ({ id: `beat-${index + 1}`, orderIndex: index, title: `Abschnitt ${index + 1}`, purpose: index === 0 ? 'Ausgangslage und Impuls setzen' : index === input.sceneCount - 1 ? 'Konsequenz und Kapitelhaken setzen' : 'Konflikt vertiefen und Entscheidung vorbereiten', location: undefined, povCharacterId: input.povCharacterId, participatingCharacterIds: input.povCharacterId ? [input.povCharacterId] : [], startingState: index === 0 ? 'Ausgangszustand des bisherigen Handlungsstands' : 'Zustand aus dem vorherigen Abschnitt', event: '', conflict: '', newInformation: [], knowledgeChanges: [], relationshipChanges: [], cluesUsed: [], loreEntityIds: [], endingHook: index === input.sceneCount - 1 ? 'Offener Anschluss für das nächste Kapitel' : '', targetWords: input.sectionWords }));
  return { chapterTitle: input.chapterTitle, chapterGoal: 'Noch vom Autor zu bestätigen', povCharacterId: input.povCharacterId, startingState: 'Wird im Plan festgelegt', endingState: 'Wird im Plan festgelegt', chapterSummary: '', endingConnection: '', newInformation: [], withheldInformation: [], assumptions: [{ type: 'author_decision', text: 'Die konkreten Ereignisse und das Ende dieses Kapitels müssen im Plan bestätigt werden.' }], beats, warnings: ['Der lokale Prototyp erstellt nur einen Planrahmen. Für Textgenerierung ist Codex CLI erforderlich.'] };
}
