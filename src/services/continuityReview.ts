import type { Chapter, ContinuityAnalysisResult, ContinuityReviewFinding, ContinuityReviewSourceKind, ContinuityStateLedgerEntry, Project, ProjectRule, SaveContinuityFindingInput, SaveContinuityStateInput, StoryEntity, Scene, StorySourceReference, ContinuityCounterEvidence, ProvisionalEntity, ProvisionalAlias } from '../types/domain';
import { normalizeContinuityResultNulls } from './aiProviderService';
import type { ContinuityAnalysisInput, StoryAiProvider } from './aiProviderService';
import { providerRouter } from './aiProviderService';
import type { StoryRepository } from './storyRepository';
import { contentHash } from '../utils/aiText';
import { changedRange } from './bibleExtractor';
import { editorContentToPlainText } from '../utils/editorContent';
import { resolveCharacterKnowledgeAtScene } from './contextBuilder';

export interface ContinuityReviewRequest {
  project: Project;
  chapter?: Chapter;
  scene?: Scene;
  currentText: string;
  previousText?: string;
  followingText?: string;
  chronological?: boolean;
  sourceKind: ContinuityReviewSourceKind;
  startOffset?: number;
  endOffset?: number;
  draftLedger?: ContinuityStateLedgerEntry[];
  provisionalEntities?: ProvisionalEntity[];
  provisionalAliases?: ProvisionalAlias[];
  provider?: StoryAiProvider;
  persistStateProposals?: boolean;
  isCancelled?: () => boolean;
  forceAnalysis?: boolean;
}

export interface ContinuityPrefilter {
  candidateEntityIds: string[];
  confirmedStates: ContinuityStateLedgerEntry[];
  confirmedRules: ProjectRule[];
}

const words = (text: string): string[] => text.match(/[\p{L}\p{N}]+(?:['’_-][\p{L}\p{N}]+)*/gu) ?? [];
export const countContinuityWords = (text: string): number => words(text).length;

export function incrementalWordRange(previousText: string | undefined, currentText: string): { start: number; end: number; addedWords: number } {
  const range = changedRange(previousText, currentText) ?? { start: 0, end: Array.from(currentText).length };
  return { ...range, addedWords: countContinuityWords(Array.from(currentText).slice(range.start, range.end).join('')) };
}

export function shouldRunContinuityReview(previousText: string | undefined, currentText: string, threshold: number, sourceKind: ContinuityReviewSourceKind): boolean {
  if (sourceKind !== 'word_threshold' || !previousText) return true;
  return incrementalWordRange(previousText, currentText).addedWords >= threshold;
}

const lower = (value: string): string => value.toLocaleLowerCase('de-DE');
const codepoints = (value: string): string[] => Array.from(value);
const mentions = (text: string, entity: StoryEntity): boolean => {
  const searchable = lower(text);
  return [entity.name, ...entity.tags].some((needle) => needle.trim().length > 1 && searchable.includes(lower(needle)));
};

function positionFor(entry: ContinuityStateLedgerEntry, chapters: Chapter[]): [number, number, number] {
  const chapter = chapters.find((item) => item.id === entry.chapterId);
  const scene = chapter?.scenes.find((item) => item.id === entry.sceneId);
  return [chapter?.orderIndex ?? Number.MAX_SAFE_INTEGER, scene?.orderIndex ?? Number.MAX_SAFE_INTEGER, entry.startOffset ?? 0];
}

function isFuture(entry: ContinuityStateLedgerEntry, chapters: Chapter[], chapter?: Chapter, scene?: Scene, offset?: number): boolean {
  if (!chapter || !scene || !entry.chapterId || !entry.sceneId) return false;
  const entryChapter = chapters.find((item) => item.id === entry.chapterId);
  const entryScene = entryChapter?.scenes.find((item) => item.id === entry.sceneId);
  if (!entryChapter || !entryScene) return true;
  return entryChapter.orderIndex > chapter.orderIndex || (entryChapter.orderIndex === chapter.orderIndex && (entryScene.orderIndex > scene.orderIndex || (entryScene.orderIndex === scene.orderIndex && offset !== undefined && (entry.startOffset ?? 0) > offset)));
}

function sourceIsAtOrBefore(source: StorySourceReference, chapters: Chapter[], chapter?: Chapter, scene?: Scene, offset?: number): boolean {
  if (!chapter || !scene || !source.chapterId) return true;
  const sourceChapter = chapters.find((item) => item.id === source.chapterId);
  if (!sourceChapter) return false;
  if (sourceChapter.orderIndex < chapter.orderIndex) return true;
  if (sourceChapter.orderIndex > chapter.orderIndex) return false;
  if (source.sceneId !== scene.id) {
    if (!source.sceneId) return true;
    const sourceScene = sourceChapter.scenes.find((item) => item.id === source.sceneId);
    const targetScene = chapter.scenes.find((item) => item.id === scene.id);
    return !sourceScene || !targetScene || sourceScene.orderIndex <= targetScene.orderIndex;
  }
  return offset === undefined || source.startOffset === undefined || source.startOffset <= offset;
}

export function buildContinuityPrefilter(input: ContinuityReviewRequest & { chapters: Chapter[]; entities: StoryEntity[]; ledger: ContinuityStateLedgerEntry[]; rules: ProjectRule[]; sources?: StorySourceReference[] }): ContinuityPrefilter {
  const searchableText = `${input.previousText ?? ''}\n${input.currentText}${input.chronological ? '' : `\n${input.followingText ?? ''}`}`;
  const passageStart = input.startOffset ?? 0;
  const passageEnd = input.endOffset ?? codepoints(input.currentText).length;
  const sourceIdsInPassage = new Set(input.sources?.filter((source) => source.chapterId === input.chapter?.id && source.sceneId === input.scene?.id && (source.startOffset === undefined || source.endOffset === undefined || (source.startOffset <= passageEnd && source.endOffset >= passageStart))).flatMap((source) => source.entityId ? [source.entityId] : []) ?? []);
  const activeEntityIds = new Set(input.ledger.filter((entry) => !isFuture(entry, input.chapters, input.chapter, input.scene, passageStart)).flatMap((entry) => [entry.entityId, ...(entry.relatedEntityId ? [entry.relatedEntityId] : [])]));
  const pov = lower(input.scene?.pov ?? '');
  const candidateEntityIds = input.entities.filter((entity) => mentions(searchableText, entity) || sourceIdsInPassage.has(entity.id) || activeEntityIds.has(entity.id) || entity.id === input.scene?.pov || (pov.length > 0 && [entity.name, ...entity.tags].some((value) => lower(value) === pov)) || entity.type === 'plot_thread' || entity.status === 'contradicted').map((entity) => entity.id);
  const candidateSet = new Set(candidateEntityIds);
  const confirmedStates = [...input.ledger, ...(input.draftLedger ?? [])]
    .filter((entry) => (entry.status === 'confirmed' && entry.authorConfirmed || (input.draftLedger ?? []).some((draft) => draft.id === entry.id)) && candidateSet.has(entry.entityId) && !isFuture(entry, input.chapters, input.chapter, input.scene, passageStart))
    .sort((a, b) => { const pa = positionFor(a, input.chapters); const pb = positionFor(b, input.chapters); return pb[0] - pa[0] || pb[1] - pa[1] || pb[2] - pa[2]; });
  const confirmedRules = input.rules.filter((rule) => rule.status === 'confirmed' && rule.authorConfirmed);
  return { candidateEntityIds: candidateEntityIds.slice(0, 160), confirmedStates: confirmedStates.slice(0, 240), confirmedRules };
}

/** @deprecated Semantic findings are provider-owned. This compatibility function is intentionally non-decisive. */
export function detectContinuityFindings(_input: ContinuityReviewRequest & { chapters: Chapter[]; entities: StoryEntity[]; ledger: ContinuityStateLedgerEntry[]; rules: ProjectRule[] }): SaveContinuityFindingInput[] {
  void _input;
  return [];
}

function sourceEvidence(result: ContinuityAnalysisResult, currentText: string, startOffset?: number, endOffset?: number, passageStartOffset = 0): string {
  const relativeStart = startOffset === undefined ? 0 : Math.max(0, startOffset - passageStartOffset);
  const relativeEnd = endOffset === undefined ? undefined : Math.max(0, endOffset - passageStartOffset);
  return result.evidence[0]?.excerpt?.trim() || codepoints(currentText).slice(relativeStart, relativeEnd).join('').trim() || codepoints(currentText).slice(0, 240).join('').trim();
}

function normalizeRelativeOffsets<T extends { startOffset?: number; endOffset?: number; evidenceExcerpt?: string }>(item: T, text: string, passageStartOffset: number): T {
  const start = item.startOffset;
  const end = item.endOffset;
  if (start === undefined && end === undefined) return item;
  if (start === undefined || end === undefined || start < 0 || end < start || end > codepoints(text).length) throw new Error('Der AI-Provider lieferte ungültige passage-relative Unicode-Offsets.');
  const excerpt = codepoints(text).slice(start, end).join('');
  if (item.evidenceExcerpt?.trim() && excerpt.trim() !== item.evidenceExcerpt.trim()) throw new Error('Die AI-Belegstelle stimmt nicht mit ihren Unicode-Offsets überein.');
  return { ...item, startOffset: passageStartOffset + start, endOffset: passageStartOffset + end };
}

function normalizeContinuityOffsets(result: ContinuityAnalysisResult, text: string, passageStartOffset: number): ContinuityAnalysisResult {
  const normalized = normalizeContinuityResultNulls(result);
  const counter = (item: ContinuityCounterEvidence): ContinuityCounterEvidence => {
    if (item.sourceReferenceId) return item;
    const normalizedCounter = normalizeRelativeOffsets({ ...item, evidenceExcerpt: item.excerpt }, text, passageStartOffset);
    return { ...item, startOffset: normalizedCounter.startOffset, endOffset: normalizedCounter.endOffset };
  };
  return {
    ...normalized,
    observedActions: normalized.observedActions.map((item) => normalizeRelativeOffsets(item, text, passageStartOffset)),
    proposedStateChanges: normalized.proposedStateChanges.map((item) => normalizeRelativeOffsets(item, text, passageStartOffset)),
    objectiveContradictions: normalized.objectiveContradictions.map((item) => ({ ...normalizeRelativeOffsets(item, text, passageStartOffset), counterEvidence: item.counterEvidence?.map(counter) })),
    missingExplanations: normalized.missingExplanations.map((item) => ({ ...normalizeRelativeOffsets(item, text, passageStartOffset), counterEvidence: item.counterEvidence?.map(counter) })),
    newRuleProposals: normalized.newRuleProposals.map((item) => normalizeRelativeOffsets(item, text, passageStartOffset)),
    plotThreadChanges: normalized.plotThreadChanges.map((item) => normalizeRelativeOffsets(item, text, passageStartOffset)),
    evidence: normalized.evidence.map((item) => ('sourceReferenceId' in item && item.sourceReferenceId) ? item : normalizeRelativeOffsets({ ...item, evidenceExcerpt: item.excerpt }, text, passageStartOffset) as typeof item),
  };
}

async function createEvidenceSource(repository: StoryRepository, projectId: string, chapter: Chapter | undefined, scene: Scene | undefined, excerpt: string, startOffset?: number, endOffset?: number, entityId?: string): Promise<string | undefined> {
  if (!chapter || !scene || !excerpt.trim() || startOffset === undefined || endOffset === undefined) return undefined;
  const source = await repository.createSourceReference({ projectId, entityId, chapterId: chapter.id, sceneId: scene.id, excerpt: excerpt.trim(), startOffset, endOffset });
  return source.id;
}

async function saveFinding(repository: StoryRepository, runId: string, project: Project, chapter: Chapter | undefined, scene: Scene | undefined, chapters: Chapter[], item: ContinuityAnalysisResult['objectiveContradictions'][number], rules: ProjectRule[], sources: StorySourceReference[], result: ContinuityAnalysisResult, currentText: string, startOffset?: number, endOffset?: number, passageStartOffset = 0): Promise<SaveContinuityFindingInput> {
  const explanations = result.matchedLoreRules.map((match) => {
    const rule = rules.find((candidate) => candidate.id === match.ruleId);
    return rule ? `${rule.title}: ${rule.statement} — ${match.rationale}` : match.rationale;
  });
  const excerpt = item.evidenceExcerpt || sourceEvidence(result, currentText, startOffset, endOffset, passageStartOffset);
  const absoluteStart = item.startOffset ?? startOffset;
  const absoluteEnd = item.endOffset ?? endOffset;
  if (item.sourceReferenceId && !sources.some((source) => source.id === item.sourceReferenceId)) throw new Error('Der AI-Finding verweist auf eine nicht übergebene Quelle.');
  const sourceReferenceId = item.sourceReferenceId || await createEvidenceSource(repository, project.id, chapter, scene, excerpt, absoluteStart, absoluteEnd, item.subjectEntityId);
  const counterEvidence: ContinuityCounterEvidence[] = [];
  for (const counter of item.counterEvidence ?? []) {
    if (counter.sourceReferenceId && !sources.some((source) => source.id === counter.sourceReferenceId)) throw new Error('Der AI-Gegenbeleg verweist auf eine nicht übergebene Quelle.');
    const counterChapter = counter.chapterId ? chapters.find((candidate) => candidate.id === counter.chapterId) : undefined;
    const counterScene = counterChapter && counter.sceneId ? counterChapter.scenes.find((candidate) => candidate.id === counter.sceneId) : undefined;
    if (!counter.sourceReferenceId && (!counterChapter || !counterScene || counter.startOffset === undefined || counter.endOffset === undefined)) throw new Error('Ein Gegenbeleg benötigt eine gültige Source Reference oder Kapitel, Szene und absolute Unicode-Offsets.');
    const sourceReferenceId = counter.sourceReferenceId || await createEvidenceSource(repository, project.id, counterChapter, counterScene, counter.excerpt, counter.startOffset, counter.endOffset, item.subjectEntityId);
    counterEvidence.push({ ...counter, sourceReferenceId });
  }
  return { runId, projectId: project.id, chapterId: chapter?.id, sceneId: scene?.id, findingType: item.findingType, severity: item.findingType === 'critical_contradiction' ? 'critical' : 'warning', subjectEntityId: item.subjectEntityId, relatedEntityIds: item.relatedEntityIds, relatedStateIds: item.relatedStateIds, relatedRuleIds: result.matchedLoreRules.map((match) => match.ruleId), objectiveConflict: item.objectiveConflict, loreExplanations: explanations, evidenceExcerpt: excerpt, sourceReferenceId, counterEvidenceExcerpts: item.counterEvidenceExcerpts, counterEvidence, confidence: item.confidence, startOffset: absoluteStart, endOffset: absoluteEnd, reason: item.reason, reviewStatus: 'open' };
}

async function proposedStateInput(repository: StoryRepository, projectId: string, chapter: Chapter | undefined, scene: Scene | undefined, item: ContinuityAnalysisResult['proposedStateChanges'][number]): Promise<SaveContinuityStateInput> {
  const sourceReferenceId = item.sourceReferenceId || await createEvidenceSource(repository, projectId, chapter, scene, item.evidenceExcerpt, item.startOffset, item.endOffset, item.entityId);
  return { projectId, entityId: item.entityId, relatedEntityId: item.relatedEntityId, stateKind: item.stateKind, previousState: item.previousState, newState: item.newState, reason: item.reason, evidenceExcerpt: item.evidenceExcerpt, chapterId: chapter?.id, sceneId: scene?.id, startOffset: item.startOffset, endOffset: item.endOffset, sourceReferenceId, status: 'proposed', confidence: item.confidence, authorConfirmed: false };
}

export async function runContinuityReview(repository: StoryRepository, input: ContinuityReviewRequest): Promise<{ runId: string; findings: ContinuityReviewFinding[]; stateProposals: ContinuityStateLedgerEntry[]; draftStateChanges: ContinuityAnalysisResult['proposedStateChanges']; analysis: ContinuityAnalysisResult }> {
  const workspace = await repository.loadWorkspace();
  const settings = await repository.getContinuityReviewSettings(input.project.id);
  const currentText = editorContentToPlainText(input.currentText);
  const previousText = input.previousText ? editorContentToPlainText(input.previousText) : undefined;
  const followingText = input.chronological ? undefined : input.followingText ? editorContentToPlainText(input.followingText) : undefined;
  const correctionFindings = input.scene?.id ? await repository.listContinuityReviewFindings(input.project.id) : [];
  const correctionDecisions = input.scene?.id ? await repository.listContinuityFindingDecisions(input.project.id) : [];
  const pendingTextCorrection = Boolean(input.scene?.id && correctionDecisions.some((decision) => decision.decisionKind === 'text_correction' && decision.status === 'open' && correctionFindings.some((finding) => finding.id === decision.findingId && finding.sceneId === input.scene?.id)));
  if (!input.forceAnalysis && !pendingTextCorrection && !shouldRunContinuityReview(previousText, currentText, settings.wordThreshold, input.sourceKind)) return { runId: '', findings: [], stateProposals: [], draftStateChanges: [], analysis: { observedActions: [], proposedStateChanges: [], objectiveContradictions: [], missingExplanations: [], matchedLoreRules: [], newRuleProposals: [], plotThreadChanges: [], confidence: 0, evidence: [], warnings: ['Die konfigurierte Prüfschwelle wurde noch nicht erreicht.'] } };
  const hash = contentHash(`${currentText}\n${previousText ?? ''}\n${followingText ?? ''}`);
  const previousRun = input.chronological ? undefined : (await repository.listContinuityReviewRuns(input.project.id, input.chapter?.id, input.scene?.id)).find((run) => run.sourceKind === input.sourceKind && run.contentHash === hash && run.status === 'completed');
  if (previousRun) return { runId: previousRun.id, findings: await repository.listContinuityReviewFindings(input.project.id, previousRun.id), stateProposals: [], draftStateChanges: [], analysis: { observedActions: [], proposedStateChanges: [], objectiveContradictions: [], missingExplanations: [], matchedLoreRules: [], newRuleProposals: [], plotThreadChanges: [], confidence: 0, evidence: [], warnings: ['Dieser Abschnitt wurde bereits mit demselben Inhalt geprüft.'] } };

  const entities = await repository.listStoryEntities(input.project.id);
  const [ledger, rules, sources, lore, knowledgeStates, knowledgeHistory, voicePatterns, experiences, dialogueMemories, relationshipMemories, openFindings, profiles] = await Promise.all([
    repository.listContinuityStateLedger(input.project.id),
    repository.listProjectRules(input.project.id, true),
    repository.listSourceReferences(input.project.id),
    repository.getLoreMetadata(input.project.id),
    repository.listCharacterKnowledgeStates(input.project.id),
    repository.listCharacterKnowledgeHistory(input.project.id),
    repository.listCharacterVoicePatterns(input.project.id),
    repository.listCharacterExperiences(input.project.id),
    repository.listCharacterDialogueMemories(input.project.id),
    repository.listRelationshipMemories(input.project.id),
    repository.listContinuityReviewFindings(input.project.id),
    Promise.all(entities.filter((entity) => entity.type === 'character').map((entity) => repository.getCharacterProfile(entity.id))),
  ]);
  const prefilter = buildContinuityPrefilter({ ...input, currentText, previousText, followingText, chapters: workspace.chapters, entities, ledger, rules, sources });
  const candidateIds = new Set(prefilter.candidateEntityIds);
  dialogueMemories.forEach((memory) => memory.participants.forEach((participant) => { if (memory.status === 'confirmed' && memory.authorConfirmed) candidateIds.add(participant.characterId); }));
  relationshipMemories.forEach((memory) => { if (memory.status === 'confirmed' && memory.authorConfirmed) { candidateIds.add(memory.characterAId); candidateIds.add(memory.characterBId); } });
  const confirmedEntities = entities.filter((entity) => entity.status === 'confirmed' && entity.authorConfirmed && (candidateIds.has(entity.id) || entity.type === 'plot_thread')).slice(0, 160);
  const passageStartOffset = input.startOffset ?? 0;
  const passageEndOffset = input.endOffset ?? passageStartOffset + codepoints(currentText).length;
  const relevantSources = sources.filter((source) => sourceIsAtOrBefore(source, workspace.chapters, input.chapter, input.scene, passageEndOffset) && (!source.entityId || candidateIds.has(source.entityId) || (source.chapterId === input.chapter?.id && source.sceneId === input.scene?.id))).slice(0, 120);
  const confirmedRules = rules.filter((rule) => rule.status === 'confirmed' && rule.authorConfirmed);
  const relevantStates = [...prefilter.confirmedStates, ...(input.draftLedger ?? []).filter((entry) => candidateIds.has(entry.entityId))].filter((entry, index, all) => all.findIndex((candidate) => candidate.id === entry.id) === index);
  const characterIds = new Set(confirmedEntities.filter((entity) => entity.type === 'character').map((entity) => entity.id));
  const sceneOrder = new Map(workspace.chapters.flatMap((chapter) => chapter.scenes.map((scene, index) => [scene.id, chapter.orderIndex * 10000 + index] as const)));
  const relevantKnowledge = [...characterIds].flatMap((characterId) => resolveCharacterKnowledgeAtScene(knowledgeStates.filter((state) => state.characterId === characterId && state.status === 'confirmed' && state.authorConfirmed), knowledgeHistory.filter((state) => state.characterId === characterId && state.status === 'confirmed' && state.authorConfirmed), sceneOrder, input.scene?.id)).filter((state, index, all) => all.findIndex((candidate) => candidate.factEntityId === state.factEntityId) === index).slice(0, 120);
  const targetOrder = input.scene ? sceneOrder.get(input.scene.id) ?? Number.MAX_SAFE_INTEGER : Number.MAX_SAFE_INTEGER;
  const beforeTarget = (sceneId?: string) => sceneId === undefined || (sceneOrder.get(sceneId) ?? Number.MAX_SAFE_INTEGER) <= targetOrder;
  const activeAtTarget = (from?: string, until?: string) => beforeTarget(from) && (until === undefined || (sceneOrder.get(until) ?? Number.MAX_SAFE_INTEGER) > targetOrder);
  const relevantVoicePatterns = voicePatterns.filter((memory) => characterIds.has(memory.characterId) && memory.status === 'confirmed' && memory.authorConfirmed && activeAtTarget(memory.firstObservedSceneId, memory.retiredSceneId)).slice(0, 40);
  const relevantExperiences = experiences.filter((memory) => characterIds.has(memory.characterId) && memory.status === 'confirmed' && memory.authorConfirmed && beforeTarget(memory.sceneId)).slice(0, 60);
  const relevantDialogueMemories = dialogueMemories.filter((memory) => memory.status === 'confirmed' && memory.authorConfirmed && beforeTarget(memory.sceneId) && memory.participants.some((participant) => characterIds.has(participant.characterId))).slice(0, 60);
  const relevantRelationshipMemories = relationshipMemories.filter((memory) => memory.status === 'confirmed' && memory.authorConfirmed && beforeTarget(memory.sceneId) && (characterIds.has(memory.characterAId) || characterIds.has(memory.characterBId))).slice(0, 60);
  const attachMemorySources = async <T extends { id: string; sourceReferenceIds?: string[] }>(kind: string, memories: T[]): Promise<T[]> => Promise.all(memories.map(async (memory) => ({ ...memory, sourceReferenceIds: (await repository.listCharacterMemoryEvidence(input.project.id, kind, memory.id)).slice(0, 8).map((evidence) => evidence.sourceReferenceId) })));
  const characterMemories = [...await attachMemorySources('voice_pattern', relevantVoicePatterns), ...await attachMemorySources('experience', relevantExperiences), ...await attachMemorySources('dialogue_memory', relevantDialogueMemories), ...await attachMemorySources('relationship_memory', relevantRelationshipMemories)];
  const providerInput: ContinuityAnalysisInput = { projectId: input.project.id, passage: { text: currentText, changedText: codepoints(currentText).slice(Math.max(0, (input.startOffset ?? incrementalWordRange(previousText, currentText).start) - passageStartOffset), Math.max(0, (input.endOffset ?? incrementalWordRange(previousText, currentText).end) - passageStartOffset)).join(''), chapterId: input.chapter?.id, sceneId: input.scene?.id, startOffset: input.startOffset, endOffset: input.endOffset, passageStartOffset, passageEndOffset, coordinateSystem: 'unicode_codepoints' }, previousContext: previousText ?? '', followingContext: followingText ?? '', confirmedStoryBible: confirmedEntities, provisionalEntities: input.provisionalEntities?.slice(0, 160), provisionalAliases: input.provisionalAliases?.slice(0, 320), confirmedLore: lore.filter((item) => candidateIds.has(item.entityId)).slice(0, 120), confirmedRules, continuityStatesBeforePosition: relevantStates, draftLedger: input.draftLedger ?? [], characterKnowledge: relevantKnowledge, characterProfiles: profiles.filter((profile): profile is NonNullable<typeof profile> => Boolean(profile)), characterMemories, activePlotThreads: confirmedEntities.filter((entity) => entity.type === 'plot_thread'), relevantSources, openFindings: openFindings.filter((finding) => finding.reviewStatus === 'open').slice(0, 100).map(({ id, objectiveConflict, reviewStatus }) => ({ id, objectiveConflict, reviewStatus })), continuityDecisions: correctionDecisions.filter((decision) => decision.status !== 'open').slice(0, 100).map(({ id, findingId, status, decisionKind, ruleId, sourceReferenceId, exceptionReason }) => ({ id, findingId, status, decisionKind, ruleId, sourceReferenceId, exceptionReason })) };
  const run = await repository.createContinuityReviewRun({ projectId: input.project.id, chapterId: input.chapter?.id, sceneId: input.scene?.id, sourceKind: input.sourceKind, contentHash: hash, startOffset: input.startOffset, endOffset: input.endOffset, providerId: input.provider?.id });
  try {
    await repository.updateContinuityReviewRunStatus({ id: run.id, status: 'running' });
    const active = input.provider ? { provider: input.provider, settings: await providerRouter.getSettings() } : await providerRouter.getActiveProvider();
    const rawResult = await active.provider.analyzeContinuityPassage(providerInput, active.settings.bibleUpdateTimeoutSeconds);
    const result = normalizeContinuityOffsets(rawResult, currentText, passageStartOffset);
    if (input.isCancelled?.()) throw new Error('Die Kontinuitätsprüfung wurde abgebrochen.');
    const allContradictions = [...result.objectiveContradictions, ...result.missingExplanations];
    const findings: SaveContinuityFindingInput[] = [];
    for (const item of allContradictions) findings.push(await saveFinding(repository, run.id, input.project, input.chapter, input.scene, workspace.chapters, item, confirmedRules, relevantSources, result, currentText, item.startOffset ?? passageStartOffset, item.endOffset ?? passageEndOffset, passageStartOffset));
    const savedFindings = findings.length ? await repository.saveContinuityReviewFindings(run.id, findings) : [];
    const stateProposals: ContinuityStateLedgerEntry[] = [];
    const provisionalIds = new Set((input.provisionalEntities ?? []).map((entity) => entity.id));
    const draftStateChanges = result.proposedStateChanges.filter((item) => entities.some((entity) => entity.id === item.entityId && entity.projectId === input.project.id) || provisionalIds.has(item.entityId));
    if (input.persistStateProposals !== false) {
      for (const item of draftStateChanges.filter((candidate) => entities.some((entity) => entity.id === candidate.entityId && entity.projectId === input.project.id))) stateProposals.push(await repository.saveContinuityStateEntry(await proposedStateInput(repository, input.project.id, input.chapter, input.scene, item)));
    }
    for (const proposal of result.newRuleProposals) {
      const sourceReferenceId = proposal.sourceReferenceIds.find((id) => relevantSources.some((source) => source.id === id)) || await createEvidenceSource(repository, input.project.id, input.chapter, input.scene, proposal.evidenceExcerpt, proposal.startOffset, proposal.endOffset);
      await repository.saveProjectRuleProposal({ ...proposal, id: undefined, projectId: input.project.id, sourceReferenceIds: sourceReferenceId ? [sourceReferenceId] : [], connectedLoreIds: proposal.connectedLoreIds.filter((id) => confirmedEntities.some((entity) => entity.id === id) || lore.some((item) => item.entityId === id)), reviewStatus: 'pending', chapterId: input.chapter?.id, sceneId: input.scene?.id });
    }
    for (const change of result.plotThreadChanges) {
      if (!entities.some((entity) => entity.id === change.entityId && entity.type === 'plot_thread')) continue;
      const sourceReferenceId = change.sourceReferenceId && relevantSources.some((source) => source.id === change.sourceReferenceId) ? change.sourceReferenceId : await createEvidenceSource(repository, input.project.id, input.chapter, input.scene, change.evidenceExcerpt, change.startOffset, change.endOffset, change.entityId);
      await repository.savePlotThreadLifecycleProposal({ runId: run.id, projectId: input.project.id, entityId: change.entityId, proposedStatus: change.proposedStatus, evidenceExcerpt: change.evidenceExcerpt, sourceReferenceId, startOffset: change.startOffset ?? passageStartOffset, endOffset: change.endOffset ?? passageEndOffset, reason: change.reason, confidence: change.confidence });
    }
    await repository.updateContinuityReviewRunStatus({ id: run.id, status: 'completed' });
    if (input.scene?.id) await repository.reconcileContinuityTextCorrection({ projectId: input.project.id, sceneId: input.scene.id, runId: run.id, contentHash: hash, findings: savedFindings.map((finding) => ({ findingType: finding.findingType, subjectEntityId: finding.subjectEntityId, objectiveConflict: finding.objectiveConflict })) });
    return { runId: run.id, findings: savedFindings, stateProposals, draftStateChanges, analysis: result };
  } catch (error) {
    const cancelled = input.isCancelled?.() ?? false;
    await repository.updateContinuityReviewRunStatus({ id: run.id, status: cancelled ? 'cancelled' : 'failed', errorMessage: error instanceof Error ? error.message : String(error) });
    throw error;
  }
}

export function canonicalText(scene: Scene): string { return editorContentToPlainText(scene.content); }

export function findingsToLongformReviews(findings: ContinuityReviewFinding[], jobId: string, sectionId?: string, continuityRunId?: string): Array<{ jobId: string; sectionId?: string; continuityRunId?: string; reviewScope: 'section'; issueType: 'canon' | 'lore' | 'knowledge' | 'character'; severity: 'info' | 'warning' | 'blocking'; title: string; description: string; relatedEntityIds: string[]; relatedSourceIds: string[]; suggestedAction: string; status: string }> {
  return findings.map((finding) => ({ jobId, sectionId, continuityRunId, reviewScope: 'section', issueType: finding.findingType === 'missing_explanation' ? 'character' : finding.relatedRuleIds.length ? 'lore' : 'canon', severity: finding.severity === 'critical' ? 'blocking' : finding.severity, title: finding.objectiveConflict, description: `${finding.reason}${finding.loreExplanations.length ? ` Bestätigte mögliche Erklärung: ${finding.loreExplanations.join(' · ')}` : ''} Sicherheit: ${Math.round(finding.confidence * 100)}%.`, relatedEntityIds: finding.relatedEntityIds, relatedSourceIds: [], suggestedAction: 'Autorentscheidung erforderlich', status: 'open' }));
}
