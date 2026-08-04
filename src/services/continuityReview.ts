import type { Chapter, ContinuityAnalysisResult, ContinuityReviewFinding, ContinuityReviewSourceKind, ContinuityStateLedgerEntry, Project, ProjectRule, SaveContinuityFindingInput, SaveContinuityStateInput, StoryEntity, Scene } from '../types/domain';
import type { ContinuityAnalysisInput, StoryAiProvider } from './aiProviderService';
import { providerRouter } from './aiProviderService';
import type { StoryRepository } from './storyRepository';
import { contentHash } from '../utils/aiText';
import { changedRange } from './bibleExtractor';
import { editorContentToPlainText } from '../utils/editorContent';

export interface ContinuityReviewRequest {
  project: Project;
  chapter?: Chapter;
  scene?: Scene;
  currentText: string;
  previousText?: string;
  followingText?: string;
  sourceKind: ContinuityReviewSourceKind;
  startOffset?: number;
  endOffset?: number;
  draftLedger?: ContinuityStateLedgerEntry[];
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
const mentions = (text: string, entity: StoryEntity): boolean => lower(text).includes(lower(entity.name));

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

export function buildContinuityPrefilter(input: ContinuityReviewRequest & { chapters: Chapter[]; entities: StoryEntity[]; ledger: ContinuityStateLedgerEntry[]; rules: ProjectRule[] }): ContinuityPrefilter {
  const searchableText = `${input.previousText ?? ''}\n${input.currentText}\n${input.followingText ?? ''}`;
  const candidateEntityIds = input.entities.filter((entity) => mentions(searchableText, entity)).map((entity) => entity.id);
  const candidateSet = new Set(candidateEntityIds);
  const confirmedStates = [...input.ledger, ...(input.draftLedger ?? [])]
    .filter((entry) => entry.status === 'confirmed' && entry.authorConfirmed && candidateSet.has(entry.entityId) && !isFuture(entry, input.chapters, input.chapter, input.scene, input.startOffset))
    .sort((a, b) => { const pa = positionFor(a, input.chapters); const pb = positionFor(b, input.chapters); return pb[0] - pa[0] || pb[1] - pa[1] || pb[2] - pa[2]; });
  const confirmedRules = input.rules.filter((rule) => rule.status === 'confirmed' && rule.authorConfirmed);
  return { candidateEntityIds, confirmedStates, confirmedRules };
}

/** @deprecated Semantic findings are provider-owned. This compatibility function is intentionally non-decisive. */
export function detectContinuityFindings(_input: ContinuityReviewRequest & { chapters: Chapter[]; entities: StoryEntity[]; ledger: ContinuityStateLedgerEntry[]; rules: ProjectRule[] }): SaveContinuityFindingInput[] {
  void _input;
  return [];
}

function sourceEvidence(result: ContinuityAnalysisResult, currentText: string, startOffset?: number, endOffset?: number): string {
  return result.evidence[0]?.excerpt?.trim() || Array.from(currentText).slice(startOffset ?? 0, endOffset).join('').trim() || currentText.slice(0, 240);
}

function saveFinding(runId: string, project: Project, chapter: Chapter | undefined, scene: Scene | undefined, item: ContinuityAnalysisResult['objectiveContradictions'][number], rules: ProjectRule[], result: ContinuityAnalysisResult, currentText: string, startOffset?: number, endOffset?: number): SaveContinuityFindingInput {
  const explanations = result.matchedLoreRules.map((match) => {
    const rule = rules.find((candidate) => candidate.id === match.ruleId);
    return rule ? `${rule.title}: ${rule.statement} — ${match.rationale}` : match.rationale;
  });
  return { runId, projectId: project.id, chapterId: chapter?.id, sceneId: scene?.id, findingType: item.findingType, severity: item.findingType === 'critical_contradiction' ? 'critical' : 'warning', subjectEntityId: item.subjectEntityId, relatedEntityIds: item.relatedEntityIds, relatedStateIds: item.relatedStateIds, relatedRuleIds: result.matchedLoreRules.map((match) => match.ruleId), objectiveConflict: item.objectiveConflict, loreExplanations: explanations, evidenceExcerpt: item.evidenceExcerpt || sourceEvidence(result, currentText, startOffset, endOffset), counterEvidenceExcerpts: item.counterEvidenceExcerpts, confidence: item.confidence, startOffset: item.startOffset ?? startOffset, endOffset: item.endOffset ?? endOffset, reason: item.reason, reviewStatus: 'open' };
}

function proposedStateInput(projectId: string, chapter: Chapter | undefined, scene: Scene | undefined, item: ContinuityAnalysisResult['proposedStateChanges'][number]): SaveContinuityStateInput {
  return { projectId, entityId: item.entityId, relatedEntityId: item.relatedEntityId, stateKind: item.stateKind, previousState: item.previousState, newState: item.newState, chapterId: chapter?.id, sceneId: scene?.id, startOffset: item.startOffset, endOffset: item.endOffset, status: 'proposed', confidence: item.confidence, authorConfirmed: false };
}

export async function runContinuityReview(repository: StoryRepository, input: ContinuityReviewRequest): Promise<{ runId: string; findings: ContinuityReviewFinding[]; stateProposals: ContinuityStateLedgerEntry[]; draftStateChanges: ContinuityAnalysisResult['proposedStateChanges']; analysis: ContinuityAnalysisResult }> {
  const workspace = await repository.loadWorkspace();
  const settings = await repository.getContinuityReviewSettings(input.project.id);
  const currentText = editorContentToPlainText(input.currentText);
  const previousText = input.previousText ? editorContentToPlainText(input.previousText) : undefined;
  const followingText = input.followingText ? editorContentToPlainText(input.followingText) : undefined;
  if (!input.forceAnalysis && !shouldRunContinuityReview(previousText, currentText, settings.wordThreshold, input.sourceKind)) return { runId: '', findings: [], stateProposals: [], draftStateChanges: [], analysis: { observedActions: [], proposedStateChanges: [], objectiveContradictions: [], missingExplanations: [], matchedLoreRules: [], newRuleProposals: [], plotThreadChanges: [], confidence: 0, evidence: [], warnings: ['Die konfigurierte Prüfschwelle wurde noch nicht erreicht.'] } };
  const hash = contentHash(`${currentText}\n${previousText ?? ''}\n${followingText ?? ''}`);
  const previousRun = (await repository.listContinuityReviewRuns(input.project.id, input.chapter?.id, input.scene?.id)).find((run) => run.sourceKind === input.sourceKind && run.contentHash === hash && run.status === 'completed');
  if (previousRun) return { runId: previousRun.id, findings: await repository.listContinuityReviewFindings(input.project.id, previousRun.id), stateProposals: [], draftStateChanges: [], analysis: { observedActions: [], proposedStateChanges: [], objectiveContradictions: [], missingExplanations: [], matchedLoreRules: [], newRuleProposals: [], plotThreadChanges: [], confidence: 0, evidence: [], warnings: ['Dieser Abschnitt wurde bereits mit demselben Inhalt geprüft.'] } };

  const entities = await repository.listStoryEntities(input.project.id);
  const [ledger, rules, sources, lore, knowledgeStates, experiences, dialogueMemories, relationshipMemories, openFindings, profiles] = await Promise.all([
    repository.listContinuityStateLedger(input.project.id),
    repository.listProjectRules(input.project.id, true),
    repository.listSourceReferences(input.project.id),
    repository.getLoreMetadata(input.project.id),
    repository.listCharacterKnowledgeStates(input.project.id),
    repository.listCharacterExperiences(input.project.id),
    repository.listCharacterDialogueMemories(input.project.id),
    repository.listRelationshipMemories(input.project.id),
    repository.listContinuityReviewFindings(input.project.id),
    Promise.all(entities.filter((entity) => entity.type === 'character').map((entity) => repository.getCharacterProfile(entity.id))),
  ]);
  const prefilter = buildContinuityPrefilter({ ...input, currentText, previousText, followingText, chapters: workspace.chapters, entities, ledger, rules });
  const candidateIds = new Set(prefilter.candidateEntityIds);
  const confirmedEntities = entities.filter((entity) => entity.status === 'confirmed' && entity.authorConfirmed && (candidateIds.has(entity.id) || entity.type === 'plot_thread'));
  const relevantSources = sources.filter((source) => !source.entityId || candidateIds.has(source.entityId));
  const confirmedRules = rules.filter((rule) => rule.status === 'confirmed' && rule.authorConfirmed);
  const relevantStates = [...prefilter.confirmedStates, ...(input.draftLedger ?? []).filter((entry) => candidateIds.has(entry.entityId))];
  const providerInput: ContinuityAnalysisInput = { projectId: input.project.id, passage: { text: currentText, changedText: Array.from(currentText).slice(input.startOffset ?? incrementalWordRange(previousText, currentText).start, input.endOffset ?? incrementalWordRange(previousText, currentText).end).join(''), chapterId: input.chapter?.id, sceneId: input.scene?.id, startOffset: input.startOffset, endOffset: input.endOffset }, previousContext: previousText ?? '', followingContext: followingText ?? '', confirmedStoryBible: confirmedEntities, confirmedLore: lore.filter((item) => candidateIds.has(item.entityId)), confirmedRules, continuityStatesBeforePosition: relevantStates, draftLedger: input.draftLedger ?? [], characterKnowledge: knowledgeStates.filter((state) => candidateIds.has(state.characterId)), characterProfiles: profiles.filter((profile): profile is NonNullable<typeof profile> => Boolean(profile)), characterMemories: [...experiences, ...dialogueMemories, ...relationshipMemories], activePlotThreads: confirmedEntities.filter((entity) => entity.type === 'plot_thread'), relevantSources, openFindings: openFindings.filter((finding) => finding.reviewStatus === 'open').map(({ id, objectiveConflict, reviewStatus }) => ({ id, objectiveConflict, reviewStatus })) };
  const run = await repository.createContinuityReviewRun({ projectId: input.project.id, chapterId: input.chapter?.id, sceneId: input.scene?.id, sourceKind: input.sourceKind, contentHash: hash, startOffset: input.startOffset, endOffset: input.endOffset, providerId: input.provider?.id });
  try {
    await repository.updateContinuityReviewRunStatus({ id: run.id, status: 'running' });
    const active = input.provider ? { provider: input.provider, settings: await providerRouter.getSettings() } : await providerRouter.getActiveProvider();
    const result = await active.provider.analyzeContinuityPassage(providerInput, active.settings.bibleUpdateTimeoutSeconds);
    if (input.isCancelled?.()) throw new Error('Die Kontinuitätsprüfung wurde abgebrochen.');
    const allContradictions = [...result.objectiveContradictions, ...result.missingExplanations];
    const findings = allContradictions.map((item) => saveFinding(run.id, input.project, input.chapter, input.scene, item, confirmedRules, result, currentText, input.startOffset, input.endOffset));
    const savedFindings = findings.length ? await repository.saveContinuityReviewFindings(run.id, findings) : [];
    const stateProposals: ContinuityStateLedgerEntry[] = [];
    const draftStateChanges = result.proposedStateChanges.filter((item) => entities.some((entity) => entity.id === item.entityId && entity.projectId === input.project.id));
    if (input.persistStateProposals !== false) {
      for (const item of draftStateChanges) stateProposals.push(await repository.saveContinuityStateEntry(proposedStateInput(input.project.id, input.chapter, input.scene, item)));
    }
    for (const proposal of result.newRuleProposals) await repository.saveProjectRuleProposal({ ...proposal, id: undefined, reviewStatus: 'pending', chapterId: proposal.chapterId ?? input.chapter?.id, sceneId: proposal.sceneId ?? input.scene?.id });
    for (const change of result.plotThreadChanges) {
      if (!entities.some((entity) => entity.id === change.entityId && entity.type === 'plot_thread')) continue;
      await repository.savePlotThreadLifecycleProposal({ runId: run.id, projectId: input.project.id, entityId: change.entityId, proposedStatus: change.proposedStatus, evidenceExcerpt: change.evidenceExcerpt, startOffset: change.startOffset ?? input.startOffset, endOffset: change.endOffset ?? input.endOffset, reason: change.reason });
    }
    await repository.updateContinuityReviewRunStatus({ id: run.id, status: 'completed' });
    return { runId: run.id, findings: savedFindings, stateProposals, draftStateChanges, analysis: result };
  } catch (error) {
    const cancelled = input.isCancelled?.() ?? false;
    await repository.updateContinuityReviewRunStatus({ id: run.id, status: cancelled ? 'cancelled' : 'failed', errorMessage: error instanceof Error ? error.message : String(error) });
    throw error;
  }
}

export function canonicalText(scene: Scene): string { return editorContentToPlainText(scene.content); }

export function findingsToLongformReviews(findings: ContinuityReviewFinding[], jobId: string, sectionId?: string): Array<{ jobId: string; sectionId?: string; reviewScope: 'section'; issueType: 'canon' | 'lore' | 'knowledge' | 'character'; severity: 'info' | 'warning' | 'blocking'; title: string; description: string; relatedEntityIds: string[]; relatedSourceIds: string[]; suggestedAction: string; status: string }> {
  return findings.map((finding) => ({ jobId, sectionId, reviewScope: 'section', issueType: finding.findingType === 'missing_explanation' ? 'character' : finding.relatedRuleIds.length ? 'lore' : 'canon', severity: finding.severity === 'critical' ? 'blocking' : finding.severity, title: finding.objectiveConflict, description: `${finding.reason}${finding.loreExplanations.length ? ` Bestätigte mögliche Erklärung: ${finding.loreExplanations.join(' · ')}` : ''} Sicherheit: ${Math.round(finding.confidence * 100)}%.`, relatedEntityIds: finding.relatedEntityIds, relatedSourceIds: [], suggestedAction: 'Autorentscheidung erforderlich', status: 'open' }));
}
