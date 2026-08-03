import { z } from 'zod';
import { desktopInvoke } from './desktop';
import { providerRouter } from './aiProviderService';
import { createPlanFrame } from './longformRepository';
import type { Chapter, ChapterDraftPlanResult, ChapterGenerationJob, ChapterGenerationPlan, ChapterGenerationSection, ChapterSectionDraftResult, Project, ProjectContext, SaveChapterGenerationReviewInput, StoryDirection, StoryEntity, WritingPreferences } from '../types/domain';

const planResultSchema = z.object({ chapterTitle: z.string().min(1), chapterGoal: z.string(), povCharacterId: z.string().optional(), startingState: z.string(), endingState: z.string(), chapterSummary: z.string(), endingConnection: z.string(), newInformation: z.array(z.string()), withheldInformation: z.array(z.string()), assumptions: z.array(z.object({ type: z.string(), text: z.string() })), beats: z.array(z.object({ id: z.string(), orderIndex: z.number(), title: z.string(), purpose: z.string(), location: z.string().optional(), povCharacterId: z.string().optional(), participatingCharacterIds: z.array(z.string()), startingState: z.string(), event: z.string(), conflict: z.string(), newInformation: z.array(z.string()), knowledgeChanges: z.array(z.record(z.unknown())), relationshipChanges: z.array(z.record(z.unknown())), cluesUsed: z.array(z.string()), loreEntityIds: z.array(z.string()), endingHook: z.string(), targetWords: z.number() })), warnings: z.array(z.string()) });
const sectionResultSchema = z.object({ content: z.string().min(1), continuationSummary: z.string(), continuityState: z.record(z.unknown()), usedEntityIds: z.array(z.string()), usedMemoryIds: z.array(z.string()), usedSourceIds: z.array(z.string()), warnings: z.array(z.string()) }).strict();
const reviewResultSchema = z.object({ issues: z.array(z.object({ reviewScope: z.enum(['section', 'chapter']), issueType: z.string().min(1), severity: z.enum(['info', 'warning', 'blocking']), title: z.string().min(1), description: z.string().min(1), relatedEntityIds: z.array(z.string()), relatedSourceIds: z.array(z.string()), suggestedAction: z.string(), status: z.string() }).strict()), warnings: z.array(z.string()) }).strict();

export interface LongformAiInput { project: Project; chapters: Chapter[]; entities: StoryEntity[]; direction?: StoryDirection; preferences: WritingPreferences; job: ChapterGenerationJob; plan?: ChapterGenerationPlan; section?: ChapterGenerationSection; previousSections?: ChapterGenerationSection[]; context?: ProjectContext; }
export interface LongformAiProvider { readonly id: 'local-prototype' | 'codex-cli'; createPlan(input: LongformAiInput): Promise<ChapterDraftPlanResult>; draftSection(input: LongformAiInput): Promise<ChapterSectionDraftResult>; reviewSection(input: LongformAiInput): Promise<SaveChapterGenerationReviewInput[]>; reviewComplete(input: LongformAiInput): Promise<SaveChapterGenerationReviewInput[]>; cancelActive(): Promise<void>; }

function request(input: LongformAiInput) { const previousSections = (input.previousSections ?? []).map((section) => ({ orderIndex: section.orderIndex, continuationSummary: section.continuationSummary, contentTail: Array.from(section.content).slice(-2500).join(''), continuityState: section.continuityState })); return { project: input.project, storyDirection: input.direction, writingPreferences: input.preferences, job: input.job, plan: input.plan, section: input.section, previousSections, projectContext: input.context, recentScenes: input.context?.currentScene ? [input.context.currentScene] : input.chapters.slice(-4).flatMap((chapter) => chapter.scenes.slice(-2)).map((scene) => ({ id: scene.id, chapterId: scene.chapterId, title: scene.title, content: scene.content, pov: scene.pov, location: scene.location, storyTime: scene.storyTime })), characters: input.context?.characterProfiles ?? input.entities.filter((entity) => entity.type === 'character'), relevantEntities: input.context?.relevantEntities ?? input.entities.filter((entity) => entity.status !== 'archived') }; }

class LocalLongformProvider implements LongformAiProvider {
  readonly id = 'local-prototype' as const;
  constructor(private readonly preferences: WritingPreferences) {}
  async createPlan(input: LongformAiInput) { const frame = createPlanFrame({ job: input.job, chapterTitle: `Kapitel ${input.chapters.length + 1}`, sceneCount: input.job.requestedSceneCount ?? this.preferences.defaultSceneCount, sectionWords: Math.min(this.preferences.maximumSectionWords, Math.max(400, Math.round(input.job.targetWords / (input.job.requestedSceneCount ?? this.preferences.defaultSceneCount)))) }); return frame; }
  async draftSection(): Promise<ChapterSectionDraftResult> { throw new Error('Der lokale Prototyp erzeugt keinen generischen Langtext. Für Abschnitte ist Codex CLI erforderlich.'); }
  async reviewSection(): Promise<SaveChapterGenerationReviewInput[]> { return []; }
  async reviewComplete(): Promise<SaveChapterGenerationReviewInput[]> { return []; }
  async cancelActive(): Promise<void> { return Promise.resolve(); }
}

class CodexLongformProvider implements LongformAiProvider {
  readonly id = 'codex-cli' as const;
  async createPlan(input: LongformAiInput) { const taskId = crypto.randomUUID(); const settings = await providerRouter.getSettings(); const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId, taskKind: 'planChapterDraft', requestJson: request(input), timeoutSeconds: settings.bibleUpdateTimeoutSeconds } }); return planResultSchema.parse(result.result) as unknown as ChapterDraftPlanResult; }
  async draftSection(input: LongformAiInput) { const taskId = crypto.randomUUID(); const settings = await providerRouter.getSettings(); const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId, taskKind: 'draftChapterSection', requestJson: request(input), timeoutSeconds: settings.chatTimeoutSeconds } }); return sectionResultSchema.parse(result.result) as unknown as ChapterSectionDraftResult; }
  async reviewSection(input: LongformAiInput) { return this.review('reviewChapterSection', input); }
  async reviewComplete(input: LongformAiInput) { return this.review('reviewCompleteChapter', input); }
  private async review(taskKind: 'reviewChapterSection' | 'reviewCompleteChapter', input: LongformAiInput) { const taskId = crypto.randomUUID(); const settings = await providerRouter.getSettings(); const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId, taskKind, requestJson: request(input), timeoutSeconds: settings.chatTimeoutSeconds } }); const parsed = reviewResultSchema.parse(result.result); return parsed.issues.map((issue) => ({ ...issue, jobId: input.job.id })); }
  async cancelActive(): Promise<void> { return Promise.resolve(); }
}

export function createLongformAiProvider(id: string, preferences: WritingPreferences): LongformAiProvider { return id === 'codex-cli' ? new CodexLongformProvider() : new LocalLongformProvider(preferences); }
