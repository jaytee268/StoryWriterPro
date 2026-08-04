import { z } from 'zod';
import { desktopInvoke } from './desktop';
import { providerRouter } from './aiProviderService';
import { createPlanFrame } from './longformRepository';
import type { Chapter, ChapterDraftPlanResult, ChapterGenerationDraftLedgerEntry, ChapterGenerationJob, ChapterGenerationPlan, ChapterGenerationSection, ChapterSectionDraftResult, Project, ProjectContext, SaveChapterGenerationReviewInput, StoryDirection, StoryEntity, WritingPreferences } from '../types/domain';
import { canonicalizeSceneForAi } from '../utils/aiText';

const plannedKnowledgeChangeSchema = z.object({ characterId: z.string().min(1), factEntityId: z.string().min(1), nextState: z.enum(['knows', 'suspects', 'believes_false', 'denies', 'forgot', 'unknown']), reason: z.string().max(1000) }).strict();
const plannedRelationshipChangeSchema = z.object({ characterAId: z.string().min(1), characterBId: z.string().min(1), change: z.string().max(1000), reason: z.string().max(1000) }).strict();
const continuityStateSchema = z.object({ currentLocation: z.string(), currentStoryTime: z.string(), presentCharacterIds: z.array(z.string()), characterStates: z.array(z.object({ characterId: z.string(), state: z.string(), change: z.string() }).strict()), establishedFacts: z.array(z.string()), knowledgeChanges: z.array(plannedKnowledgeChangeSchema), relationshipChanges: z.array(plannedRelationshipChangeSchema), movedObjects: z.array(z.object({ objectId: z.string(), location: z.string(), state: z.string() }).strict()), injuries: z.array(z.object({ characterId: z.string(), description: z.string(), severity: z.string() }).strict()), cluesIntroduced: z.array(z.string()), promisesCreated: z.array(z.string()), unresolvedActions: z.array(z.string()), lastParagraphSummary: z.string() }).strict();
const planResultSchema = z.object({ chapterTitle: z.string().min(1), chapterGoal: z.string(), povCharacterId: z.string().optional(), startingState: z.string(), endingState: z.string(), chapterSummary: z.string(), endingConnection: z.string(), newInformation: z.array(z.string()), withheldInformation: z.array(z.string()), assumptions: z.array(z.object({ type: z.string(), text: z.string() }).strict()), beats: z.array(z.object({ id: z.string(), orderIndex: z.number(), title: z.string(), purpose: z.string(), location: z.string().optional(), povCharacterId: z.string().optional(), participatingCharacterIds: z.array(z.string()), startingState: z.string(), event: z.string(), conflict: z.string(), newInformation: z.array(z.string()), knowledgeChanges: z.array(plannedKnowledgeChangeSchema), relationshipChanges: z.array(plannedRelationshipChangeSchema), cluesUsed: z.array(z.string()), loreEntityIds: z.array(z.string()), endingHook: z.string(), targetWords: z.number() }).strict()), warnings: z.array(z.string()) }).strict();
const sectionResultSchema = z.object({ content: z.string().min(1), continuationSummary: z.string(), continuityState: continuityStateSchema, usedEntityIds: z.array(z.string()), usedMemoryIds: z.array(z.string()), usedSourceIds: z.array(z.string()), warnings: z.array(z.string()) }).strict();
const reviewResultSchema = z.object({ issues: z.array(z.object({ reviewScope: z.enum(['section', 'chapter']), issueType: z.string().min(1), severity: z.enum(['info', 'warning', 'blocking']), title: z.string().min(1), description: z.string().min(1), relatedEntityIds: z.array(z.string()), relatedSourceIds: z.array(z.string()), suggestedAction: z.string(), status: z.string() }).strict()), warnings: z.array(z.string()) }).strict();

export interface LongformAiInput { project: Project; chapters: Chapter[]; entities: StoryEntity[]; direction?: StoryDirection; preferences: WritingPreferences; job: ChapterGenerationJob; plan?: ChapterGenerationPlan; section?: ChapterGenerationSection; previousSections?: ChapterGenerationSection[]; draftLedger?: ChapterGenerationDraftLedgerEntry[]; context?: ProjectContext; }
export interface LongformAiProvider { readonly id: 'local-prototype' | 'codex-cli'; createPlan(input: LongformAiInput): Promise<ChapterDraftPlanResult>; draftSection(input: LongformAiInput): Promise<ChapterSectionDraftResult>; reviewSection(input: LongformAiInput): Promise<SaveChapterGenerationReviewInput[]>; reviewComplete(input: LongformAiInput): Promise<SaveChapterGenerationReviewInput[]>; cancelActive(): Promise<void>; }

function request(input: LongformAiInput) { const previousSections = (input.previousSections ?? []).map((section) => ({ orderIndex: section.orderIndex, continuationSummary: section.continuationSummary, contentTail: Array.from(section.content).slice(-2500).join(''), continuityState: section.continuityState })); const recentScenes = input.context?.currentScene ? [canonicalizeSceneForAi(input.context.currentScene).scene] : input.chapters.slice(-4).flatMap((chapter) => chapter.scenes.slice(-2)).map((scene) => { const canonical = canonicalizeSceneForAi(scene).scene; return { id: canonical.id, chapterId: canonical.chapterId, title: canonical.title, content: canonical.content, pov: canonical.pov, location: canonical.location, storyTime: canonical.storyTime }; }); return { project: input.project, storyDirection: input.direction, writingPreferences: input.preferences, job: input.job, plan: input.plan, section: input.section, previousSections, draftLedger: input.draftLedger ?? [], projectContext: input.context, recentScenes, characters: input.context?.characterProfiles ?? input.entities.filter((entity) => entity.type === 'character'), relevantEntities: input.context?.relevantEntities ?? input.entities.filter((entity) => entity.status !== 'archived') }; }

class LocalLongformProvider implements LongformAiProvider {
  readonly id = 'local-prototype' as const;
  constructor(private readonly preferences: WritingPreferences) {}
  async createPlan(input: LongformAiInput) { const frame = createPlanFrame({ job: input.job, chapterTitle: `Kapitel ${input.chapters.length + 1}`, sceneCount: input.job.requestedSceneCount ?? this.preferences.defaultSceneCount, sectionWords: Math.min(this.preferences.maximumSectionWords, Math.max(400, Math.round(input.job.targetWords / (input.job.requestedSceneCount ?? this.preferences.defaultSceneCount)))) }); return frame; }
  async draftSection(): Promise<ChapterSectionDraftResult> { throw new Error('Der lokale Prototyp erzeugt keinen generischen Langtext. Für Abschnitte ist Codex CLI erforderlich.'); }
  async reviewSection(): Promise<SaveChapterGenerationReviewInput[]> { return []; }
  async reviewComplete(): Promise<SaveChapterGenerationReviewInput[]> { return []; }
  async cancelActive(): Promise<void> { return Promise.resolve(); }
}

export class CodexLongformProvider implements LongformAiProvider {
  readonly id = 'codex-cli' as const;
  private activeTaskId?: string;
  private async runTask(input: LongformAiInput, taskKind: 'planChapterDraft' | 'draftChapterSection' | 'reviewChapterSection' | 'reviewCompleteChapter', timeoutSeconds: number): Promise<unknown> {
    if (this.activeTaskId) throw new Error('Es läuft bereits ein Longform-Codex-Aufruf.');
    const taskId = crypto.randomUUID();
    this.activeTaskId = taskId;
    try {
      const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId, taskKind, requestJson: request(input), timeoutSeconds } });
      return result.result;
    } finally {
      if (this.activeTaskId === taskId) this.activeTaskId = undefined;
    }
  }
  async createPlan(input: LongformAiInput) { const settings = await providerRouter.getSettings(); const result = await this.runTask(input, 'planChapterDraft', settings.bibleUpdateTimeoutSeconds); return planResultSchema.parse(result) as unknown as ChapterDraftPlanResult; }
  async draftSection(input: LongformAiInput) { const settings = await providerRouter.getSettings(); const result = await this.runTask(input, 'draftChapterSection', settings.chatTimeoutSeconds); return sectionResultSchema.parse(result) as unknown as ChapterSectionDraftResult; }
  async reviewSection(input: LongformAiInput) { return this.review('reviewChapterSection', input); }
  async reviewComplete(input: LongformAiInput) { return this.review('reviewCompleteChapter', input); }
  private async review(taskKind: 'reviewChapterSection' | 'reviewCompleteChapter', input: LongformAiInput) { const settings = await providerRouter.getSettings(); const result = await this.runTask(input, taskKind, settings.chatTimeoutSeconds); const parsed = reviewResultSchema.parse(result); return parsed.issues.map((issue) => ({ ...issue, jobId: input.job.id })); }
  async cancelActive(): Promise<void> { const taskId = this.activeTaskId; if (!taskId) return; await desktopInvoke('cancel_codex_task', { taskId }); }
}

export function createLongformAiProvider(id: string, preferences: WritingPreferences): LongformAiProvider { return id === 'codex-cli' ? new CodexLongformProvider() : new LocalLongformProvider(preferences); }
