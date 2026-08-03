import { z } from 'zod';
import { desktopInvoke } from './desktop';
import { providerRouter } from './aiProviderService';
import { createPlanFrame } from './longformRepository';
import type { Chapter, ChapterDraftPlanResult, ChapterGenerationJob, ChapterGenerationPlan, ChapterGenerationSection, ChapterSectionDraftResult, Project, StoryDirection, StoryEntity, WritingPreferences } from '../types/domain';

const planResultSchema = z.object({ chapterTitle: z.string().min(1), chapterGoal: z.string(), povCharacterId: z.string().optional(), startingState: z.string(), endingState: z.string(), chapterSummary: z.string(), endingConnection: z.string(), newInformation: z.array(z.string()), withheldInformation: z.array(z.string()), assumptions: z.array(z.object({ type: z.string(), text: z.string() })), beats: z.array(z.object({ id: z.string(), orderIndex: z.number(), title: z.string(), purpose: z.string(), location: z.string().optional(), povCharacterId: z.string().optional(), participatingCharacterIds: z.array(z.string()), startingState: z.string(), event: z.string(), conflict: z.string(), newInformation: z.array(z.string()), knowledgeChanges: z.array(z.record(z.unknown())), relationshipChanges: z.array(z.record(z.unknown())), cluesUsed: z.array(z.string()), loreEntityIds: z.array(z.string()), endingHook: z.string(), targetWords: z.number() })), warnings: z.array(z.string()) });
const sectionResultSchema = z.object({ content: z.string().min(1), continuationSummary: z.string(), continuityState: z.record(z.unknown()), usedEntityIds: z.array(z.string()), usedMemoryIds: z.array(z.string()), usedSourceIds: z.array(z.string()), warnings: z.array(z.string()) });

export interface LongformAiInput { project: Project; chapters: Chapter[]; entities: StoryEntity[]; direction?: StoryDirection; preferences: WritingPreferences; job: ChapterGenerationJob; plan?: ChapterGenerationPlan; section?: ChapterGenerationSection; }
export interface LongformAiProvider { readonly id: 'local-prototype' | 'codex-cli'; createPlan(input: LongformAiInput): Promise<ChapterDraftPlanResult>; draftSection(input: LongformAiInput): Promise<ChapterSectionDraftResult>; }

function request(input: LongformAiInput) { return { project: input.project, storyDirection: input.direction, writingPreferences: input.preferences, job: input.job, plan: input.plan, section: input.section, recentScenes: input.chapters.slice(-4).flatMap((chapter) => chapter.scenes.slice(-2)).map((scene) => ({ id: scene.id, chapterId: scene.chapterId, title: scene.title, content: scene.content, pov: scene.pov, location: scene.location, storyTime: scene.storyTime })), characters: input.entities.filter((entity) => entity.type === 'character').slice(0, 10), relevantEntities: input.entities.slice(0, 30) }; }

class LocalLongformProvider implements LongformAiProvider {
  readonly id = 'local-prototype' as const;
  constructor(private readonly preferences: WritingPreferences) {}
  async createPlan(input: LongformAiInput) { const frame = createPlanFrame({ job: input.job, chapterTitle: `Kapitel ${input.chapters.length + 1}`, sceneCount: input.job.requestedSceneCount ?? this.preferences.defaultSceneCount, sectionWords: Math.min(this.preferences.maximumSectionWords, Math.max(400, Math.round(input.job.targetWords / (input.job.requestedSceneCount ?? this.preferences.defaultSceneCount)))) }); return frame; }
  async draftSection(): Promise<ChapterSectionDraftResult> { throw new Error('Der lokale Prototyp erzeugt keinen generischen Langtext. Für Abschnitte ist Codex CLI erforderlich.'); }
}

class CodexLongformProvider implements LongformAiProvider {
  readonly id = 'codex-cli' as const;
  async createPlan(input: LongformAiInput) { const taskId = crypto.randomUUID(); const settings = await providerRouter.getSettings(); const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId, taskKind: 'planChapterDraft', requestJson: request(input), timeoutSeconds: settings.bibleUpdateTimeoutSeconds } }); return planResultSchema.parse(result.result) as unknown as ChapterDraftPlanResult; }
  async draftSection(input: LongformAiInput) { const taskId = crypto.randomUUID(); const settings = await providerRouter.getSettings(); const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId, taskKind: 'draftChapterSection', requestJson: request(input), timeoutSeconds: settings.chatTimeoutSeconds } }); return sectionResultSchema.parse(result.result) as unknown as ChapterSectionDraftResult; }
}

export function createLongformAiProvider(id: string, preferences: WritingPreferences): LongformAiProvider { return id === 'codex-cli' ? new CodexLongformProvider() : new LocalLongformProvider(preferences); }
