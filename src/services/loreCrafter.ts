import type { BuildLoreSheetResult, LoreCrafterAnalysis, LoreCrafterRun, LoreSheetDraft, LoreSheetItem, ProjectRule, SaveLoreMetadataInput, StoryEntity, LoreSheetWorldRule, LoreSheetItemType, ExcludedContentDecision, ProjectContentProposal } from '../types/domain';
import { contentHash } from '../utils/aiText';
import type { LoreCrafterAnalysisInput, LoreCrafterSheetInput, StoryAiProvider } from './aiProviderService';
import type { StoryRepository } from './storyRepository';

const PROMPT_VERSION = 'storymemory-lore-crafter-v1';
const TIMEOUT_SECONDS = 180;
export type LoreCrafterProgressStage = 'preparing' | 'provider' | 'saving' | 'review';

function confirmationText(analysis: LoreCrafterAnalysis): string {
  const section = (title: string, values: string[]) => values.length ? `${title}:\n${values.map((value) => `• ${value}`).join('\n')}` : `${title}: keine eindeutigen Angaben`;
  const excluded = analysis.excludedContent.map((item) => `${item.content} → ${item.suggestedTarget} (${item.reason})`);
  return ['So habe ich deine Lore verstanden.', analysis.understandingSummary, section('Bestätigte Aussagen', analysis.confirmedStatements), section('Wichtigste Regeln', analysis.proposedWorldRules), section('Auswirkungen', analysis.effects), section('Begriffe', analysis.terminology), section('Organisationen', analysis.relevantOrganizations), section('Orte', analysis.relevantLocations), section('Historischer Hintergrund', analysis.historicalBackground), section('Voraussetzungen', analysis.prerequisites), section('Grenzen', analysis.limitations), section('Kosten', analysis.costs), section('Ausnahmen', analysis.exceptions), section('Unsicherheiten', [...analysis.unresolvedQuestions, ...analysis.warnings]), section('Mögliche Widersprüche', analysis.contradictions), section('Ausgeschlossene Inhalte und Zielvorschläge', excluded), section('Rückfragen', analysis.clarificationQuestions)].join('\n\n');
}

function arrayItems(result: BuildLoreSheetResult): Array<{ type: LoreSheetItemType; title: string; content: string; structured?: Record<string, unknown> }> {
  const structuredRules: Array<{ type: LoreSheetItemType; title: string; content: string; structured?: Record<string, unknown> }> = (result.worldRuleObjects ?? []).map((rule: LoreSheetWorldRule) => ({ type: 'world_rule', title: rule.title, content: rule.statement, structured: rule as unknown as Record<string, unknown> }));
  const groups: Array<[LoreSheetItemType, string, string[]]> = [
    ['world_rule', 'Weltregel', result.worldRules], ['prerequisite', 'Voraussetzung', result.prerequisites], ['effect', 'Auswirkung', result.effects], ['limitation', 'Grenze', result.limitations], ['cost', 'Kosten', result.costs], ['exception', 'Ausnahme', result.exceptions], ['term', 'Begriff', result.terminology], ['organization', 'Organisation', result.organizations], ['location', 'Ort', result.locations], ['historical_event', 'Historisches Ereignis', result.historicalEvents], ['known_aspect', 'Bekannter Aspekt', result.knownAspects], ['unknown_aspect', 'Unbekannter Aspekt', result.unknownAspects], ['rule_connection', 'Regelverknüpfung', result.ruleConnections], ['open_question', 'Offene Frage', result.openQuestions],
  ];
  return [...structuredRules, ...groups.flatMap(([type, title, values]) => values.map((content) => ({ type, title, content })) )];
}

export async function analyzeLoreDraft(repository: StoryRepository, provider: StoryAiProvider, input: { projectId: string; originalText: string; correctionText?: string; onStage?: (stage: LoreCrafterProgressStage) => void }): Promise<LoreCrafterRun> {
  const hash = contentHash(input.originalText);
  const existingLore = await repository.getLoreMetadata(input.projectId);
  const existingRules = await repository.listProjectRules(input.projectId, true);
  const run = await repository.createLoreCrafterRun({ projectId: input.projectId, originalText: input.originalText, contentHash: hash, providerId: provider.id, promptVersion: PROMPT_VERSION, correctionText: input.correctionText?.trim() || undefined });
  await repository.updateLoreCrafterRun({ id: run.id, status: 'running' });
  try {
    input.onStage?.('provider');
    const request: LoreCrafterAnalysisInput = { projectId: input.projectId, originalText: input.originalText, existingLore, existingRules, correctionText: input.correctionText?.trim() || undefined };
    const analysis = await provider.analyzeLoreDraft(request, TIMEOUT_SECONDS);
    input.onStage?.('saving');
    await repository.saveLoreCrafterSource({ runId: run.id, projectId: input.projectId, excerpt: input.originalText, startOffset: 0, endOffset: Array.from(input.originalText).length });
    await repository.saveLoreCrafterClarifications(run.id, analysis.clarificationQuestions.map((question) => ({ runId: run.id, projectId: input.projectId, question, status: 'open' as const })));
    const saved = await repository.updateLoreCrafterRun({ id: run.id, status: 'awaiting_review', understandingSummary: analysis.understandingSummary, analysis, confirmationText: confirmationText(analysis), correctionText: input.correctionText?.trim() || undefined });
    input.onStage?.('review');
    return saved;
  } catch (error) {
    await repository.updateLoreCrafterRun({ id: run.id, status: 'failed', errorCode: 'LORE_CRAFTER_ANALYSIS_FAILED', errorMessage: error instanceof Error ? error.message : String(error), completedAt: new Date().toISOString() });
    throw error;
  }
}

export async function buildLoreSheet(repository: StoryRepository, provider: StoryAiProvider, runId: string, understandingConfirmed: boolean, onStage?: (stage: LoreCrafterProgressStage) => void): Promise<{ draft: LoreSheetDraft; items: LoreSheetItem[] }> {
  if (!understandingConfirmed) throw new Error('Bestätige zuerst, dass das Verständnis korrekt ist.');
  const run = await repository.getLoreCrafterRun(runId);
  if (run.status !== 'awaiting_review' || !run.analysis) throw new Error('Der Lore-Crafter-Lauf ist noch nicht zur Sheet-Erstellung bereit.');
  if (contentHash(run.originalText) !== run.contentHash) throw new Error('Die Lore-Notizen wurden verändert. Führe die Analyse erneut aus.');
  const existingDraft = await repository.getLoreSheetDraft(runId);
  if (existingDraft) {
    const existingItems = await repository.listLoreSheetItems(existingDraft.id);
    if (existingItems.length > 0) {
      onStage?.('review');
      return { draft: existingDraft, items: existingItems };
    }
  }
  const clarifications = await repository.listLoreCrafterClarifications(runId);
  const input: LoreCrafterSheetInput = { projectId: run.projectId, originalText: run.originalText, analysis: run.analysis, clarifications: clarifications.map(({ question, answer, status }) => ({ question, answer, status })) };
  onStage?.('provider');
  const result = await provider.buildLoreSheet(input, TIMEOUT_SECONDS);
  onStage?.('saving');
  const draftInput = { id: existingDraft?.id, runId, projectId: run.projectId, contentHash: run.contentHash, title: result.title, premise: result.premise, categories: result.categories, worldRules: result.worldRules, prerequisites: result.prerequisites, effects: result.effects, limitations: result.limitations, costs: result.costs, exceptions: result.exceptions, terminology: result.terminology, organizations: result.organizations, locations: result.locations, historicalEvents: result.historicalEvents, knownAspects: result.knownAspects, unknownAspects: result.unknownAspects, ruleConnections: result.ruleConnections, openQuestions: result.openQuestions, status: 'proposed' as const };
  const source = (await repository.listLoreCrafterSources(runId))[0];
  const atomic = await repository.saveLoreSheetWithItems(draftInput, arrayItems(result).map((item) => ({ draftId: '', runId, projectId: run.projectId, itemType: item.type, title: item.title, content: item.content, structured: item.structured, confidence: run.analysis?.confidence ?? 0, sourceReferenceId: source?.sourceReferenceId ?? source?.id })));
  onStage?.('review');
  return atomic;
}

function metadataFor(entity: StoryEntity, content: string, itemType: string): SaveLoreMetadataInput {
  const category = itemType === 'world_rule' ? 'world_rule' : itemType === 'historical_event' ? 'history' : itemType === 'term' ? 'terminology' : 'objective_truth';
  return { entityId: entity.id, projectId: entity.projectId, category, scope: 'book', revealState: 'author_only', importance: itemType === 'world_rule' ? 'core' : 'supporting', truthStatement: content, rulesText: itemType === 'world_rule' ? content : '', exceptionsText: itemType === 'exception' ? content : '', authorKnowledge: content, readerKnowledge: '', revealPlan: '' };
}

export async function reviewLoreSheetItem(repository: StoryRepository, item: LoreSheetItem, status: 'accepted' | 'rejected' | 'uncertain' | 'merged', editedContent?: string, mergeTargetId?: string): Promise<LoreSheetItem> {
  const content = editedContent?.trim() || item.content;
  if (status !== 'accepted' && status !== 'merged') return repository.reviewLoreSheetItem(item.projectId, item.id, status);
  if (item.targetEntityId || item.targetRuleId) return repository.reviewLoreSheetItem(item.projectId, item.id, status);
  if (item.itemType === 'world_rule') {
    const structured = item.structured as Partial<LoreSheetWorldRule> | undefined;
    const rule: ProjectRule = await repository.saveProjectRule({ id: mergeTargetId, projectId: item.projectId, title: item.title, statement: content, scope: 'project', prerequisites: structured?.prerequisites ?? [], effects: structured?.effects ?? [], exceptions: structured?.exceptions ?? [...(structured?.limitations ?? []), ...(structured?.costs ?? [])], connectedLoreIds: [], sourceReferenceIds: item.sourceReferenceId ? [item.sourceReferenceId] : [], status: 'proposed', confidence: item.confidence, authorConfirmed: false, origin: 'lore_crafter' });
    const saved = await repository.reviewLoreSheetItem(item.projectId, item.id, status);
    return (await repository.saveLoreSheetItems(item.draftId, [{ ...saved, content, targetRuleId: rule.id }])).find((next) => next.id === saved.id) ?? saved;
  }
  const entityType = item.itemType === 'organization' ? 'organization' : item.itemType === 'location' ? 'place' : item.itemType === 'historical_event' ? 'event' : 'fact';
  const entity = mergeTargetId
    ? await repository.updateStoryEntity({ ...(await repository.getStoryEntity(mergeTargetId)), projectId: item.projectId, description: content, excerpt: content, status: 'proposed', confidence: item.confidence, authorConfirmed: false, origin: 'lore_crafter' })
    : await repository.createStoryEntity({ projectId: item.projectId, name: item.title, type: entityType, description: content, status: 'proposed', confidence: item.confidence, excerpt: content, authorConfirmed: false, tags: ['lore_crafter'], origin: 'lore_crafter' });
  await repository.saveLoreMetadata(metadataFor(entity, content, item.itemType));
  const saved = await repository.reviewLoreSheetItem(item.projectId, item.id, status);
  return (await repository.saveLoreSheetItems(item.draftId, [{ ...saved, content, targetEntityId: entity.id }])).find((next) => next.id === saved.id) ?? saved;
}

export async function routeExcludedContent(repository: StoryRepository, run: LoreCrafterRun, content: string, reason: string, suggestedTarget: 'character_memory' | 'plot_thread' | 'continuity_state' | 'manuscript' | 'style', selectedTarget = suggestedTarget): Promise<{ decision: ExcludedContentDecision; proposal: ProjectContentProposal }> {
  const source = (await repository.listLoreCrafterSources(run.id))[0];
  if (!source?.sourceReferenceId) throw new Error('Für die Weiterleitung fehlt die Quelle der ursprünglichen Lore-Notizen.');
  const proposal = await repository.saveProjectContentProposal({ projectId: run.projectId, targetKind: selectedTarget, title: 'Aus Lore-Notizen vorgeschlagen', content, reason, sourceReferenceId: source.sourceReferenceId, origin: 'lore_crafter', status: 'proposed' });
  const decision = await repository.saveExcludedContentDecision({ runId: run.id, projectId: run.projectId, content, reason, suggestedTarget, selectedTarget, decision: 'routed', createdArtifactId: proposal.id });
  return { decision, proposal };
}

export async function ignoreExcludedContent(repository: StoryRepository, run: LoreCrafterRun, content: string, reason: string, suggestedTarget: 'character_memory' | 'plot_thread' | 'continuity_state' | 'manuscript' | 'style'): Promise<ExcludedContentDecision> {
  return repository.saveExcludedContentDecision({ runId: run.id, projectId: run.projectId, content, reason, suggestedTarget, selectedTarget: suggestedTarget, decision: 'ignored' });
}

export async function confirmLoreCrafterRule(repository: StoryRepository, item: LoreSheetItem): Promise<ProjectRule> {
  if (!item.targetRuleId || !item.sourceReferenceId) throw new Error('Die Regel benötigt zuerst eine übernommene Quelle.');
  const current = (await repository.listProjectRules(item.projectId)).find((rule) => rule.id === item.targetRuleId);
  if (!current) throw new Error('Die vorgeschlagene Projektregel wurde nicht gefunden.');
  return repository.saveProjectRule({ ...current, status: 'confirmed', authorConfirmed: true, sourceReferenceIds: Array.from(new Set([...current.sourceReferenceIds, item.sourceReferenceId])), origin: 'lore_crafter' });
}

export async function finishLoreCrafterReview(repository: StoryRepository, run: LoreCrafterRun, items: LoreSheetItem[]): Promise<LoreCrafterRun> {
  if (!items.length || items.some((item) => item.status === 'proposed')) throw new Error('Alle Lore-Sheet-Einträge müssen entschieden oder ausdrücklich unsicher gespeichert werden.');
  return repository.updateLoreCrafterRun({ id: run.id, status: 'completed', completedAt: new Date().toISOString() });
}
