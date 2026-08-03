import type { Chapter, CharacterKnowledgeState, ContextRequest, ProjectContext, StorySourceReference } from '../types/domain';
import type { StoryRepository } from './storyRepository';
import { editorContentToPlainText } from '../utils/editorContent';

const tokens = (value: string): string[] => value.toLocaleLowerCase().split(/[^\p{L}\p{N}]+/u).filter((token) => token.length > 2);
const scoreText = (value: string, needles: string[]) => needles.reduce((score, needle) => score + (tokens(value).includes(needle) ? 1 : 0), 0);

export interface ProjectContextBuilder { build(input: ContextRequest): Promise<ProjectContext>; }

/** Resolves knowledge as scene intervals: a state starts at its effective scene
 * and remains valid until (but not including) the next transition scene. */
export function resolveCharacterKnowledgeAtScene(
  currentStates: CharacterKnowledgeState[],
  historyStates: CharacterKnowledgeState[],
  sceneOrder: ReadonlyMap<string, number>,
  targetSceneId?: string,
): CharacterKnowledgeState[] {
  const targetOrder = targetSceneId ? (sceneOrder.get(targetSceneId) ?? Number.MAX_SAFE_INTEGER) : Number.MAX_SAFE_INTEGER;
  const orderOf = (sceneId: string | undefined, fallback: number): number => sceneId ? (sceneOrder.get(sceneId) ?? Number.MAX_SAFE_INTEGER) : fallback;
  const candidates = [
    ...historyStates.map((state) => ({ state, from: orderOf(state.effectiveFromSceneId ?? state.acquiredSceneId, Number.MIN_SAFE_INTEGER), until: orderOf(state.effectiveUntilSceneId ?? state.changedSceneId, Number.MAX_SAFE_INTEGER), current: false })),
    ...currentStates.map((state) => ({ state, from: orderOf(state.effectiveFromSceneId ?? state.changedSceneId ?? state.acquiredSceneId, Number.MIN_SAFE_INTEGER), until: orderOf(state.effectiveUntilSceneId, Number.MAX_SAFE_INTEGER), current: true })),
  ].filter((candidate) => candidate.from <= targetOrder && targetOrder < candidate.until)
    .sort((a, b) => a.from - b.from || Number(a.current) - Number(b.current));
  const latest = new Map<string, (typeof candidates)[number]>();
  candidates.forEach((candidate) => latest.set(candidate.state.factEntityId, candidate));
  return [...latest.values()].map(({ state }) => state);
}

export class DeterministicProjectContextBuilder implements ProjectContextBuilder {
  constructor(private readonly repository: StoryRepository) {}

  async build(input: ContextRequest): Promise<ProjectContext> {
    const workspace = await this.repository.loadWorkspace();
    const currentChapter = workspace.chapters.find((chapter) => chapter.id === input.currentChapterId) ?? workspace.chapters.find((chapter) => chapter.scenes.some((scene) => scene.id === input.currentSceneId));
    const currentScene = currentChapter?.scenes.find((scene) => scene.id === input.currentSceneId) ?? currentChapter?.scenes[0];
    const entities = workspace.entities.filter((entity) => entity.projectId === input.projectId && entity.status !== 'archived');
    const [sources, lore, styleReferences, projectStyle, relations, allRules, allLedger, allVoicePatterns, allExperiences, allDialogueMemories, allRelationshipMemories, allKnowledgeStates, allKnowledgeHistory, styleObservations, narrativeSummaries] = await Promise.all([this.repository.listSourceReferences(input.projectId), this.repository.getLoreMetadata(input.projectId), this.repository.listStyleReferences(input.projectId), this.repository.getProjectStyle(input.projectId), this.repository.listEntityRelations(input.projectId), this.repository.listProjectRules(input.projectId, true), this.repository.listContinuityStateLedger(input.projectId), this.repository.listCharacterVoicePatterns(input.projectId), this.repository.listCharacterExperiences(input.projectId), this.repository.listCharacterDialogueMemories(input.projectId), this.repository.listRelationshipMemories(input.projectId), this.repository.listCharacterKnowledgeStates(input.projectId), this.repository.listCharacterKnowledgeHistory(input.projectId), this.repository.listProjectStyleObservations(input.projectId), this.repository.listNarrativeSummaries(input.projectId)]);
    const questionTokens = tokens(input.userQuestion);
    const sceneText = editorContentToPlainText(currentScene?.content ?? '');
    const sceneTokens = tokens(sceneText);
    const sourceEntityIds = new Set(sources.filter((source) => source.sceneId === currentScene?.id).map((source) => source.entityId).filter((id): id is string => Boolean(id)));
    const directEntities = entities.filter((entity) => {
      const searchable = `${entity.name} ${entity.description} ${entity.tags.join(' ')}`;
      return sourceEntityIds.has(entity.id) || sceneTokens.some((token) => tokens(entity.name).includes(token)) || scoreText(searchable, questionTokens) > 0 || (currentChapter && entity.chapter === currentChapter.title) || entity.type === 'plot_thread' || entity.status === 'contradicted';
    });
    const relevantIds = new Set(directEntities.map((entity) => entity.id));
    relations.forEach((relation) => { if (relevantIds.has(relation.sourceEntityId)) relevantIds.add(relation.targetEntityId); if (relevantIds.has(relation.targetEntityId)) relevantIds.add(relation.sourceEntityId); });
    const relevantEntities = entities.filter((entity) => relevantIds.has(entity.id)).sort((a, b) => Number(b.status === 'confirmed') - Number(a.status === 'confirmed')).slice(0, 30);
    const relevantEntityIds = new Set(relevantEntities.map((entity) => entity.id));
    const relevantSources = sources.filter((source) => source.entityId ? relevantEntityIds.has(source.entityId) : source.sceneId === currentScene?.id).slice(0, 30);
    const relevantLore = lore.filter((item) => relevantEntityIds.has(item.entityId)).map((item) => ({ item, score: (item.importance === 'core' ? 8 : item.importance === 'supporting' ? 4 : 1) + (relations.some((relation) => relation.sourceEntityId === item.entityId || relation.targetEntityId === item.entityId) ? 6 : 0) + (relevantSources.some((source) => source.entityId === item.entityId && source.sceneId === currentScene?.id) ? 6 : 0) + scoreText(`${item.truthStatement} ${item.rulesText} ${item.revealPlan}`, questionTokens) })).sort((a, b) => b.score - a.score).slice(0, 20).map(({ item }) => item);
    const characterIds = relevantEntities.filter((entity) => entity.type === 'character').sort((a, b) => {
      const score = (entity: typeof a) => Number(entity.id === currentScene?.pov || entity.name.toLocaleLowerCase().split(/\s+/u).some((name) => sceneTokens.includes(name))) * 10 + Number(sourceEntityIds.has(entity.id)) * 8 + scoreText(`${entity.name} ${entity.description}`, questionTokens);
      return score(b) - score(a);
    }).map((entity) => entity.id).slice(0, 10);
    const profiles = await Promise.all(characterIds.map((id) => this.repository.getCharacterProfile(id)));
    const characterProfiles = profiles.filter((profile): profile is NonNullable<typeof profile> => Boolean(profile));
    const allStates = await this.repository.listCharacterSceneStates(input.projectId);
    const sceneOrder = new Map(workspace.chapters.flatMap((chapter) => chapter.scenes.map((scene, index) => [scene.id, chapter.orderIndex * 10000 + index] as const)));
    const currentOrder = currentScene ? sceneOrder.get(currentScene.id) ?? Number.MAX_SAFE_INTEGER : Number.MAX_SAFE_INTEGER;
    const characterStates = allStates.filter((state) => characterIds.includes(state.characterEntityId) && (sceneOrder.get(state.sceneId) ?? Number.MAX_SAFE_INTEGER) <= currentOrder).sort((a, b) => { const aCurrent = a.sceneId === currentScene?.id; const bCurrent = b.sceneId === currentScene?.id; if (aCurrent !== bCurrent) return Number(bCurrent) - Number(aCurrent); return (currentOrder - (sceneOrder.get(a.sceneId) ?? 0)) - (currentOrder - (sceneOrder.get(b.sceneId) ?? 0)); }).slice(0, 10);
    const relevantRelations = relations.filter((relation) => relevantEntityIds.has(relation.sourceEntityId) || relevantEntityIds.has(relation.targetEntityId)).slice(0, 20);
    const relevantRules = allRules.filter((rule) => rule.connectedLoreIds.some((id) => relevantEntityIds.has(id)) || scoreText(`${rule.title} ${rule.statement} ${rule.effects.join(' ')}`, questionTokens) > 0).slice(0, 20);
    const relevantLedger = allLedger.filter((entry) => entry.status === 'confirmed' && entry.authorConfirmed && relevantEntityIds.has(entry.entityId) && (!currentScene || (() => { const chapter = workspace.chapters.find((item) => item.id === entry.chapterId); const scene = chapter?.scenes.find((item) => item.id === entry.sceneId); if (!chapter || !scene) return false; return chapter.orderIndex < (currentChapter?.orderIndex ?? Number.MAX_SAFE_INTEGER) || (chapter.orderIndex === (currentChapter?.orderIndex ?? Number.MAX_SAFE_INTEGER) && scene.orderIndex <= (currentScene.orderIndex ?? Number.MAX_SAFE_INTEGER)); })())).slice(0, 30);
    const relevantVoicePatterns = allVoicePatterns.filter((pattern) => {
      const firstOrder: number = pattern.firstObservedSceneId ? (sceneOrder.get(pattern.firstObservedSceneId) ?? 0) : 0;
      const retiredOrder: number = pattern.retiredSceneId ? (sceneOrder.get(pattern.retiredSceneId) ?? Number.MAX_SAFE_INTEGER) : Number.MAX_SAFE_INTEGER;
      return (characterIds.includes(pattern.characterId) || (pattern.relatedCharacterId ? characterIds.includes(pattern.relatedCharacterId) : false)) && firstOrder <= currentOrder && currentOrder < retiredOrder;
    }).sort((a, b) => b.occurrenceCount - a.occurrenceCount).slice(0, 20);
    const relevantExperiences = allExperiences.filter((experience) => characterIds.includes(experience.characterId) && (!experience.sceneId || (sceneOrder.get(experience.sceneId) ?? 0) <= currentOrder)).slice(0, 20);
    const relevantDialogueMemories = allDialogueMemories.filter((memory) => memory.participants.some((participant) => characterIds.includes(participant.characterId)) && (sceneOrder.get(memory.sceneId) ?? 0) <= currentOrder).slice(0, 20);
    const relevantRelationshipMemories = allRelationshipMemories.filter((memory) => characterIds.includes(memory.characterAId) && characterIds.includes(memory.characterBId) && (!memory.sceneId || (sceneOrder.get(memory.sceneId) ?? 0) <= currentOrder)).slice(0, 20);
    const relevantKnowledgeStates = characterIds.flatMap((characterId) => resolveCharacterKnowledgeAtScene(allKnowledgeStates.filter((state) => state.characterId === characterId), allKnowledgeHistory.filter((state) => state.characterId === characterId), sceneOrder, currentScene?.id)).slice(0, 30);
    const categoryHint = questionTokens.includes('dialog') ? ['dialogue', 'general', 'humor'] : questionTokens.some((token) => ['spannung', 'konflikt', 'tension'].includes(token)) ? ['tension', 'general', 'description'] : ['general', 'dialogue', 'inner_monologue', 'description'];
    const relevantStyleReferences = [...styleReferences].sort((a, b) => (b.weight - a.weight) || (Number(categoryHint.includes(b.category)) - Number(categoryHint.includes(a.category))) || (Number(b.sceneId === currentScene?.id) - Number(a.sceneId === currentScene?.id)) || Number(a.startOffset === undefined) - Number(b.startOffset === undefined)).slice(0, 5);
    const usableSummaries = narrativeSummaries.filter((item) => item.status === 'confirmed' || (input.includeProposedSummaries === true && item.status === 'proposed'));
    return { projectId: input.projectId, currentScene, currentChapter, relevantEntities, relevantSources, openPlotThreads: relevantEntities.filter((entity) => entity.type === 'plot_thread' && entity.status !== 'confirmed'), possibleContradictions: relevantEntities.filter((entity) => entity.status === 'contradicted'), lore: relevantLore, projectRules: relevantRules, entityRelations: relevantRelations, continuityStates: relevantLedger, characterProfiles, characterStates, characterVoicePatterns: relevantVoicePatterns, characterExperiences: relevantExperiences, characterDialogueMemories: relevantDialogueMemories, relationshipMemories: relevantRelationshipMemories, characterKnowledgeStates: relevantKnowledgeStates, projectStyle, styleReferences: relevantStyleReferences, acceptedStyleObservations: styleObservations.filter((item) => item.reviewStatus === 'accepted' || item.reviewStatus === 'edited'), narrativeSummaries: usableSummaries };
  }
}

export function sourceToChatLabel(source: StorySourceReference, chapters: Chapter[]): string { const chapter = chapters.find((item) => item.id === source.chapterId); const scene = chapter?.scenes.find((item) => item.id === source.sceneId); return `${chapter?.title ?? 'Kapitel'} · ${scene?.title ?? 'Szene'}`; }
