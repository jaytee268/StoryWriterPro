import type { Chapter, ChapterGenerationJob, Project, ProjectContext, Scene, StoryDirection, StoryEntity, WritingPreferences } from '../types/domain';
import { createPlanFrame, type LongformRepository } from './longformRepository';

export interface LongformIntent { requested: boolean; instruction: string; pages?: number; words?: number; sceneCount?: number; chapterNumber?: number; povName?: string; }
export interface PreflightItem { level: 'blocking' | 'recommended' | 'optional'; label: string; detail: string; }
export interface LongformPreflight { items: PreflightItem[]; canPlan: boolean; direction?: StoryDirection; preferences: WritingPreferences; targetBookId?: string; afterChapterId?: string; }

const pagePattern = /(?:ca\.?\s*)?(\d+(?:[.,]\d+)?)\s*seiten?/i;
const wordPattern = /(\d[\d.]*)\s*w(?:örter|orte|örtern)?/i;
const scenePattern = /(\d+)\s*szen(?:e|en)/i;
const chapterPattern = /kapitel\s*(\d+)/i;

export function parseLongformIntent(text: string): LongformIntent {
  const normalized = text.trim();
  const explicitVerb = /\b(schreib|schreibe|fortsetz|setz(?:e)?\s+(?:die\s+)?geschichte|mach\s+aus|erstelle|entwirf)\w*/i.test(normalized);
  const page = normalized.match(pagePattern); const words = normalized.match(wordPattern); const scenes = normalized.match(scenePattern); const chapter = normalized.match(chapterPattern); const pov = normalized.match(/aus\s+([A-ZÄÖÜ][\wÄÖÜäöüß-]*)'?s?\s+perspektive/i);
  return { requested: explicitVerb && Boolean(page || words || /\b(nächst(?:e|en)|kapitel)\b/i.test(normalized)), instruction: normalized, pages: page ? Number(page[1].replace(',', '.')) : undefined, words: words ? Number(words[1].replaceAll('.', '')) : undefined, sceneCount: scenes ? Number(scenes[1]) : undefined, chapterNumber: chapter ? Number(chapter[1]) : undefined, povName: pov?.[1] };
}

export function targetWords(intent: LongformIntent, preferences: WritingPreferences): number { return Math.max(1, Math.round(intent.words ?? (intent.pages ? intent.pages * preferences.wordsPerPage : preferences.preferredSectionWords * preferences.defaultSceneCount))); }

export function buildPreflight(project: Project, chapters: Chapter[], entities: StoryEntity[], direction: StoryDirection | undefined, preferences: WritingPreferences, intent: LongformIntent): LongformPreflight {
  void project;
  const items: PreflightItem[] = []; const book = chapters[0]?.bookId; const after = chapters.at(-1)?.id;
  if (!book) items.push({ level: 'blocking', label: 'Zielbuch', detail: 'Für dieses Projekt wurde kein Buch gefunden.' });
  if (!after) items.push({ level: 'blocking', label: 'Schreibposition', detail: 'Es gibt noch kein Kapitel, hinter dem der Entwurf eingeordnet werden kann.' });
  if (!direction) items.push({ level: 'recommended', label: 'Story-Richtung', detail: 'Prämisse, Wendepunkt und geplantes Ende sind noch nicht definiert.' });
  else if (!direction.plannedEnding.trim()) items.push({ level: 'recommended', label: 'Geplantes Ende', detail: 'Für dieses Buch ist noch kein geplantes Ende festgelegt.' });
  if (!entities.some((entity) => entity.type === 'character')) items.push({ level: 'recommended', label: 'POV-Figur', detail: 'Es ist noch keine Charakterfigur in der Story Bible vorhanden.' });
  if (!intent.pages && !intent.words) items.push({ level: 'recommended', label: 'Zielumfang', detail: 'Es wurde kein Seiten- oder Wortziel genannt. Die Projektpräferenz wird verwendet.' });
  items.push({ level: 'optional', label: 'Lokaler Prototyp', detail: 'Der lokale Provider kann Preflight und Planrahmen vorbereiten, aber keinen langen Manuskripttext erzeugen.' });
  return { items, canPlan: !items.some((item) => item.level === 'blocking'), direction, preferences, targetBookId: book, afterChapterId: after };
}

export async function createLongformJob(input: { repository: LongformRepository; project: Project; chapters: Chapter[]; entities: StoryEntity[]; intent: LongformIntent; direction?: StoryDirection; preferences: WritingPreferences; activeProvider: string; contextHash: string }): Promise<{ job: ChapterGenerationJob; preflight: LongformPreflight }> {
  const preflight = buildPreflight(input.project, input.chapters, input.entities, input.direction, input.preferences, input.intent);
  if (!preflight.canPlan || !preflight.targetBookId) throw new Error('Der Langformauftrag kann erst nach der Vorbereitung gestartet werden.');
  const job = await input.repository.createJob({ projectId: input.project.id, targetBookId: preflight.targetBookId, targetAfterChapterId: preflight.afterChapterId, requestedPages: input.intent.pages, targetWords: targetWords(input.intent, input.preferences), requestedSceneCount: input.intent.sceneCount ?? input.preferences.defaultSceneCount, userInstruction: input.intent.instruction, activeProvider: input.activeProvider, contentContextHash: input.contextHash });
  return { job, preflight };
}

export function buildLocalPlan(job: ChapterGenerationJob, chapters: Chapter[], preferences: WritingPreferences, entities: StoryEntity[], currentScene?: Scene) {
  const chapterTitle = `Kapitel ${chapters.length + 1}`; const pov = entities.find((entity) => entity.type === 'character' && (currentScene?.pov ? entity.name === currentScene.pov : true));
  const sceneCount = job.requestedSceneCount ?? preferences.defaultSceneCount; const sectionWords = Math.min(preferences.maximumSectionWords, Math.max(400, Math.round(job.targetWords / sceneCount)));
  return createPlanFrame({ job, chapterTitle, povCharacterId: pov?.id, sceneCount, sectionWords });
}

export function contextHashForLongform(project: Project, chapters: Chapter[], direction?: StoryDirection, context?: ProjectContext): string {
  const source = JSON.stringify({ projectId: project.id, updatedAt: project.updatedAt, chapters: chapters.map((chapter) => ({ id: chapter.id, updatedAt: chapter.updatedAt, scenes: chapter.scenes.map((scene) => ({ id: scene.id, updatedAt: scene.updatedAt, content: scene.content })) })), direction, entities: context?.relevantEntities.map((item) => [item.id, item.updatedAt]), lore: context?.lore?.map((item) => [item.entityId, item.updatedAt]), profiles: context?.characterProfiles?.map((item) => [item.entityId, item.updatedAt]), memories: [...(context?.characterVoicePatterns ?? []), ...(context?.characterExperiences ?? []), ...(context?.characterDialogueMemories ?? []), ...(context?.relationshipMemories ?? []), ...(context?.characterKnowledgeStates ?? [])].map((item) => [item.id, item.updatedAt]), style: context?.projectStyle?.updatedAt, styleReferences: context?.styleReferences?.map((item) => [item.id, item.updatedAt]) });
  let hash = 2166136261; for (const char of source) { hash ^= char.charCodeAt(0); hash = Math.imul(hash, 16777619); } return (hash >>> 0).toString(16);
}

export function hasBlockingReviews(items: { severity: string; status: string }[]): boolean { return items.some((item) => item.severity === 'blocking' && item.status === 'open'); }

export function contextForLongform(context: ProjectContext): Pick<ProjectContext, 'lore' | 'characterProfiles' | 'characterVoicePatterns' | 'characterExperiences' | 'characterDialogueMemories' | 'relationshipMemories' | 'characterKnowledgeStates' | 'projectStyle' | 'styleReferences'> { return { lore: context.lore?.slice(0, 20), characterProfiles: context.characterProfiles?.slice(0, 10), characterVoicePatterns: context.characterVoicePatterns?.slice(0, 20), characterExperiences: context.characterExperiences?.slice(0, 20), characterDialogueMemories: context.characterDialogueMemories?.slice(0, 20), relationshipMemories: context.relationshipMemories?.slice(0, 20), characterKnowledgeStates: context.characterKnowledgeStates?.slice(0, 30), projectStyle: context.projectStyle, styleReferences: context.styleReferences?.slice(0, 5) }; }
