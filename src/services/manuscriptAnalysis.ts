import type { Chapter, ContinuityStateLedgerEntry, CreateManuscriptAnalysisJobInput, ManuscriptAnalysisDraftLedgerEntry, ManuscriptAnalysisJob, ManuscriptAnalysisPhase, ManuscriptAnalysisPhaseProgress, ManuscriptAnalysisUnit, ManuscriptSynthesisResult, NarrativeSummaryAnalysisResult, ManuscriptPhaseInput, PlotThreadSynthesisResult, BookEndStateResult, GlobalCountercheckResult, ManuscriptAnalysisArtifactType, SaveContinuityFindingInput, StoryEntity, ProvisionalEntity } from '../types/domain';
import type { StoryRepository } from './storyRepository';
import { contentHash } from '../utils/aiText';
import { editorContentToPlainText } from '../utils/editorContent';
import { providerRouter, type StoryAiProvider as Provider } from './aiProviderService';
import { runContinuityReview } from './continuityReview';
import { DeterministicProjectContextBuilder } from './contextBuilder';
import { validateManuscriptStructure, localStructureHints } from './manuscriptStructure';
import { matchPriorProvisionalEntity, provisionalEntityId } from './provisionalGraph';
import { buildHierarchicalPhaseContext, truncateUnicode } from './analysisBudget';

const activeJobs = new Map<string, Promise<void>>();
const PHASES: ManuscriptAnalysisPhase[] = ['structure', 'passage_continuity', 'bible_extraction', 'character_memory', 'scene_or_chapter_synthesis', 'narrative_summaries', 'plot_thread_synthesis', 'book_end_state', 'global_countercheck', 'user_review', 'completed'];
const PROMPT_VERSION = 'manuscript-analysis-v2-multipass';

function draftEntryToLedger(entry: ManuscriptAnalysisDraftLedgerEntry): ContinuityStateLedgerEntry {
  return { id: entry.id, projectId: entry.projectId, entityId: entry.entityId, relatedEntityId: entry.relatedEntityId, stateKind: entry.stateKind, previousState: entry.previousState, newState: entry.newState, reason: 'Vorgeschlagene Änderung aus dem Manuskript-Analysejob', evidenceExcerpt: entry.sourceExcerpt, chapterId: entry.chapterId, sceneId: entry.sceneId, startOffset: entry.startOffset, endOffset: entry.endOffset, sourceReferenceId: entry.sourceReferenceId, status: 'proposed', confidence: entry.confidence, authorConfirmed: false, createdAt: entry.createdAt, updatedAt: entry.updatedAt };
}

function errorText(error: unknown): string { return error instanceof Error ? error.message : String(error); }
function errorCode(error: unknown): string { return error instanceof Error && 'code' in error && typeof error.code === 'string' ? error.code : 'ANALYSIS_PHASE_FAILED'; }
function phaseIndex(phase: ManuscriptAnalysisPhase): number { return PHASES.indexOf(phase); }
function chapterText(chapter: Chapter): string { return chapter.scenes.map((scene) => editorContentToPlainText(scene.content)).join('\n\n'); }
function passageScene(scene: Chapter['scenes'][number], text: string): Chapter['scenes'][number] { return { ...scene, content: text }; }
function passageChapter(chapter: Chapter, scene: Chapter['scenes'][number], text: string): Chapter { return { ...chapter, scenes: [passageScene(scene, text)] }; }
function sourceIsAtOrBefore(source: { chapterId?: string; sceneId?: string; endOffset?: number }, chapters: Chapter[], unit: ManuscriptAnalysisUnit): boolean {
  if (!source.chapterId) return true;
  const sourceChapter = chapters.find((chapter) => chapter.id === source.chapterId);
  const targetChapter = chapters.find((chapter) => chapter.id === unit.chapterId);
  if (!sourceChapter || !targetChapter) return false;
  if (sourceChapter.orderIndex < targetChapter.orderIndex) return true;
  if (sourceChapter.orderIndex > targetChapter.orderIndex) return false;
  if (!source.sceneId || source.sceneId !== unit.sceneId) {
    if (!source.sceneId) return true;
    const sourceScene = sourceChapter.scenes.find((scene) => scene.id === source.sceneId);
    const targetScene = targetChapter.scenes.find((scene) => scene.id === unit.sceneId);
    return !sourceScene || !targetScene || sourceScene.orderIndex <= targetScene.orderIndex;
  }
  return source.endOffset === undefined || source.endOffset <= unit.endOffset;
}
function entitiesAtOrBefore(entities: StoryEntity[], sources: Awaited<ReturnType<StoryRepository['listSourceReferences']>>, chapters: Chapter[], unit: ManuscriptAnalysisUnit): StoryEntity[] {
  return entities.filter((entity) => {
    const entitySources = sources.filter((source) => source.entityId === entity.id);
    if (entitySources.length > 0) return entitySources.some((source) => sourceIsAtOrBefore(source, chapters, unit));
    const chapter = chapters.find((candidate) => candidate.title === entity.chapter);
    if (chapter) return chapter.orderIndex <= (chapters.find((candidate) => candidate.id === unit.chapterId)?.orderIndex ?? Number.MAX_SAFE_INTEGER);
    return !entity.chapter && !entity.scene;
  });
}
function synthesisToSummary(result: ManuscriptSynthesisResult | NarrativeSummaryAnalysisResult, projectId: string, scopeType: 'chapter' | 'book', scopeId: string, sourceText: string) {
  const extended = result as ManuscriptSynthesisResult;
  const characterChanges = 'characterChanges' in result ? result.characterChanges : [];
  return { projectId, scopeType, scopeId, contentHash: contentHash(sourceText), summary: result.summary, importantEvents: result.importantEvents, openThreads: result.openThreads, characterChanges: [...characterChanges, ...(extended.knowledgeChanges ?? []), ...(extended.relationshipChanges ?? []), ...(extended.characterEndStates ?? [])], status: 'proposed' as const, authorConfirmed: false };
}

export interface ManuscriptAnalysisProgress { job: ManuscriptAnalysisJob; units: ManuscriptAnalysisUnit[]; draftLedger: ManuscriptAnalysisDraftLedgerEntry[]; phaseResults: Awaited<ReturnType<StoryRepository['listManuscriptAnalysisPhaseResults']>>; artifacts: Awaited<ReturnType<StoryRepository['listManuscriptAnalysisArtifacts']>>; completionReport?: Awaited<ReturnType<StoryRepository['getManuscriptAnalysisCompletionReport']>>; }
export interface ManuscriptReviewArtifactDetail { artifactId: string; artifactType: ManuscriptAnalysisArtifactType; title: string; body: string; chapterId?: string; sceneId?: string; excerpt?: string; confidence?: number; reason?: string; sourceReferenceId?: string; startOffset?: number; endOffset?: number; }

export async function loadManuscriptAnalysisReviewDetails(repository: StoryRepository, progress: ManuscriptAnalysisProgress): Promise<ManuscriptReviewArtifactDetail[]> {
  const projectId = progress.job.projectId;
  const [runs, memoryRuns, bible, memories, findings, rules, threads, summaries, events, edges, provisional, merges, sources] = await Promise.all([
    repository.listBibleUpdateRuns(projectId), repository.listCharacterMemoryUpdateRuns(projectId), repository.listBibleUpdateRuns(projectId).then(async (items) => (await Promise.all(items.map((run) => repository.listBibleProposals(run.id)))).flat()), repository.listCharacterMemoryUpdateRuns(projectId).then(async (items) => (await Promise.all(items.map((run) => repository.listCharacterMemoryProposals(run.id)))).flat()), repository.listContinuityReviewFindings(projectId), repository.listProjectRuleProposals(projectId), repository.listPlotThreadLifecycleProposals(projectId), repository.listNarrativeSummaries(projectId), repository.listTimelineEvents(projectId), repository.listStoryGraphEdges(projectId), repository.listProvisionalEntities(progress.job.id), repository.listProvisionalMergeProposals(progress.job.id), repository.listSourceReferences(projectId),
  ]);
  void runs; void memoryRuns;
  const details: ManuscriptReviewArtifactDetail[] = [];
  const add = (detail: ManuscriptReviewArtifactDetail) => { if (progress.artifacts.some((artifact) => artifact.artifactId === detail.artifactId && artifact.artifactType === detail.artifactType)) details.push(detail); };
  const sourceFor = (id?: string) => sources.find((source) => source.id === id);
  for (const proposal of bible) add({ artifactId: proposal.id, artifactType: 'bible_proposal', title: proposal.candidateName, body: proposal.candidateDescription, chapterId: progress.units.find((unit) => unit.sceneId === proposal.sceneId)?.chapterId, sceneId: proposal.sceneId, excerpt: proposal.evidenceExcerpt, confidence: proposal.confidence, reason: proposal.reason, sourceReferenceId: sources.find((source) => source.proposalId === proposal.id)?.id, startOffset: proposal.startOffset, endOffset: proposal.endOffset });
  for (const proposal of memories) add({ artifactId: proposal.id, artifactType: 'character_memory_proposal', title: proposal.proposalKind, body: JSON.stringify(proposal.payload), sceneId: proposal.sceneId, excerpt: proposal.evidenceExcerpt, confidence: proposal.confidence, reason: proposal.reason, sourceReferenceId: sources.find((source) => source.proposalId === proposal.id)?.id, startOffset: proposal.startOffset, endOffset: proposal.endOffset });
  for (const finding of findings) add({ artifactId: finding.id, artifactType: 'continuity_finding', title: finding.findingType, body: finding.objectiveConflict, chapterId: finding.chapterId, sceneId: finding.sceneId, excerpt: finding.evidenceExcerpt, confidence: finding.confidence, reason: finding.reason, sourceReferenceId: finding.sourceReferenceId, startOffset: finding.startOffset, endOffset: finding.endOffset });
  for (const proposal of rules) add({ artifactId: proposal.id, artifactType: 'project_rule_proposal', title: proposal.title, body: proposal.statement, chapterId: proposal.chapterId, sceneId: proposal.sceneId, excerpt: proposal.evidenceExcerpt, confidence: proposal.confidence, reason: proposal.reason, sourceReferenceId: proposal.sourceReferenceIds[0], startOffset: proposal.startOffset, endOffset: proposal.endOffset });
  for (const proposal of threads) add({ artifactId: proposal.id, artifactType: 'plot_thread_proposal', title: proposal.proposedStatus, body: proposal.evidenceExcerpt, excerpt: proposal.evidenceExcerpt, confidence: proposal.confidence, reason: proposal.reason, sourceReferenceId: proposal.sourceReferenceId, startOffset: proposal.startOffset, endOffset: proposal.endOffset });
  for (const summary of summaries) add({ artifactId: summary.id, artifactType: 'narrative_summary', title: `${summary.scopeType}-Zusammenfassung`, body: summary.summary, confidence: 1 });
  for (const event of events) add({ artifactId: event.id, artifactType: 'timeline_event', title: event.title, body: event.summary, chapterId: event.chapterId, sceneId: event.sceneId, confidence: event.confidence, sourceReferenceId: event.sourceReferenceIds[0] });
  for (const edge of edges) add({ artifactId: edge.id, artifactType: 'story_graph_edge', title: edge.label, body: `${edge.sourceEntityId} → ${edge.targetEntityId} (${edge.relationType})`, chapterId: edge.validFromChapterId, sceneId: edge.validFromSceneId, confidence: edge.confidence, sourceReferenceId: edge.sourceReferenceIds[0], startOffset: edge.validFromOffset });
  for (const entity of provisional) add({ artifactId: entity.id, artifactType: 'provisional_entity', title: entity.canonicalName, body: entity.description, confidence: entity.confidence, sourceReferenceId: entity.firstSourceReferenceId });
  for (const merge of merges) add({ artifactId: merge.id, artifactType: 'provisional_merge', title: 'Merge-Vorschlag', body: merge.reason, confidence: merge.confidence });
  for (const entry of progress.draftLedger) add({ artifactId: entry.id, artifactType: 'import_draft_state', title: entry.stateKind, body: `${entry.previousState} → ${entry.newState}`, chapterId: entry.chapterId, sceneId: entry.sceneId, excerpt: entry.sourceExcerpt, confidence: entry.confidence, reason: 'Vorgeschlagene Zustandsänderung', sourceReferenceId: entry.sourceReferenceId, startOffset: entry.startOffset, endOffset: entry.endOffset });
  for (const result of progress.phaseResults) {
    for (const artifact of progress.artifacts.filter((item) => item.phase === result.phase && (item.artifactType === 'book_end_state_proposal' || item.artifactType === 'global_countercheck_finding'))) {
      const phaseItems = artifact.artifactType === 'book_end_state_proposal' ? result.payload.endStateProposals : result.payload.countercheckFindings;
      const item = artifact.artifactId.startsWith(`${result.id}:`) && Array.isArray(phaseItems) ? phaseItems[Number(artifact.artifactId.split(':').pop())] : result.payload;
      add({ artifactId: artifact.artifactId, artifactType: artifact.artifactType, title: artifact.artifactType, body: JSON.stringify(item ?? result.payload), confidence: typeof (item as { confidence?: unknown } | undefined)?.confidence === 'number' ? (item as { confidence: number }).confidence : undefined });
    }
  }
  return details.map((detail) => { const source = sourceFor(detail.sourceReferenceId); return source ? { ...detail, chapterId: source.chapterId ?? detail.chapterId, sceneId: source.sceneId ?? detail.sceneId, excerpt: source.excerpt || detail.excerpt, startOffset: source.startOffset ?? detail.startOffset, endOffset: source.endOffset ?? detail.endOffset } : detail; });
}

export class ManuscriptAnalysisController {
  private paused = false;
  private cancelled = false;
  private currentProvider?: Provider;
  private runPromise?: Promise<void>;

  constructor(private readonly repository: StoryRepository, public readonly jobId: string, private readonly providerOverride?: Provider) {}

  start(): Promise<void> {
    const existing = activeJobs.get(this.jobId);
    if (existing) return existing;
    if (activeJobs.size > 0) throw new Error('Eine andere Manuskriptanalyse läuft bereits.');
    this.paused = false;
    this.cancelled = false;
    this.runPromise = this.execute();
    activeJobs.set(this.jobId, this.runPromise);
    void this.runPromise.then(() => { if (activeJobs.get(this.jobId) === this.runPromise) activeJobs.delete(this.jobId); }, () => { if (activeJobs.get(this.jobId) === this.runPromise) activeJobs.delete(this.jobId); });
    return this.runPromise;
  }

  async pause(): Promise<void> {
    this.paused = true;
    await this.currentProvider?.cancelActive();
    if (!this.runPromise) await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'paused' });
  }

  async cancel(): Promise<void> {
    this.cancelled = true;
    await this.currentProvider?.cancelActive();
    if (!this.runPromise) await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'cancelled', errorMessage: 'Analyse wurde abgebrochen.' });
  }

  async completeUserReview(explicitlySkipOpen = false): Promise<void> {
    const job = await this.repository.getManuscriptAnalysisJob(this.jobId);
    if (job.status !== 'awaiting_user_review' || job.currentPhase !== 'user_review') throw new Error('Der Importjob wartet derzeit nicht auf ein Nutzerreview.');
    const [draft, artifacts] = await Promise.all([this.repository.listManuscriptAnalysisDraftLedger(this.jobId), this.repository.listManuscriptAnalysisArtifacts(this.jobId)]);
    const openArtifacts = artifacts.filter((artifact) => artifact.reviewStatus === 'pending');
    const open = openArtifacts.length || draft.filter((entry) => entry.status === 'proposed' || entry.status === 'uncertain').length;
    if (open > 0 && !explicitlySkipOpen) throw new Error('Offene Importvorschläge müssen entschieden oder ausdrücklich übersprungen werden.');
    const openDrafts = draft.filter((entry) => entry.status === 'proposed' || entry.status === 'uncertain');
    if (explicitlySkipOpen && (openArtifacts.length > 0 || openDrafts.length > 0)) {
      for (const artifact of openArtifacts) await this.repository.reviewManuscriptAnalysisArtifact(artifact.id, 'skipped', true);
      for (const entry of openDrafts) await this.repository.reviewManuscriptAnalysisDraftLedger(entry.id, 'uncertain');
      await this.repository.saveManuscriptAnalysisReviewAudit({ jobId: job.id, projectId: job.projectId, action: 'skip_open_artifacts', artifactIds: [...openArtifacts.map((artifact) => artifact.id), ...openDrafts.map((entry) => entry.id)], artifactTypes: [...new Set([...openArtifacts.map((artifact) => artifact.artifactType), ...(openDrafts.length ? ['import_draft_state' as const] : [])])] as ManuscriptAnalysisArtifactType[], note: `Nutzer überspringt ausdrücklich ${openArtifacts.length + openDrafts.length} offene Ergebnisse.`, });
    }
    await this.repository.saveManuscriptAnalysisReviewAudit({ jobId: job.id, projectId: job.projectId, action: 'complete_review', artifactIds: artifacts.map((artifact) => artifact.id), artifactTypes: [...new Set(artifacts.map((artifact) => artifact.artifactType))] as ManuscriptAnalysisArtifactType[], note: explicitlySkipOpen ? 'Review mit ausdrücklich übersprungenen Ergebnissen abgeschlossen.' : 'Review aller jobgebundenen Ergebnisse abgeschlossen.' });
    const [units, phaseResults] = await Promise.all([this.repository.listManuscriptAnalysisUnits(job.id), this.repository.listManuscriptAnalysisPhaseResults(job.id)]);
    const recognizedScenes = [...new Map(units.map((unit) => [unit.sceneId, { chapterId: unit.chapterId, sceneId: unit.sceneId, orderIndex: unit.orderIndex }])).values()];
    const statusItems = artifacts.map((artifact) => ({ artifactId: artifact.artifactId, artifactType: artifact.artifactType, reviewStatus: artifact.reviewStatus, explicitlySkipped: artifact.explicitlySkipped }));
    const truncations = phaseResults.flatMap((result) => { const budget = result.payload.budget as { truncatedSections?: unknown } | undefined; return Array.isArray(budget?.truncatedSections) ? budget.truncatedSections.filter((item): item is string => typeof item === 'string') : []; });
    await this.repository.saveManuscriptAnalysisCompletionReport({ jobId: job.id, projectId: job.projectId, contentHash: contentHash(units.map((unit) => unit.contentHash).join('|')), payload: { recognizedScenes, entities: artifacts.filter((item) => item.artifactType === 'provisional_entity').map((item) => item.artifactId), merges: artifacts.filter((item) => item.artifactType === 'provisional_merge').map((item) => item.artifactId), bibleDecisions: statusItems.filter((item) => item.artifactType === 'bible_proposal'), memoryDecisions: statusItems.filter((item) => item.artifactType === 'character_memory_proposal'), continuityFindings: statusItems.filter((item) => item.artifactType === 'continuity_finding' || item.artifactType === 'global_countercheck_finding'), timelineEvents: statusItems.filter((item) => item.artifactType === 'timeline_event'), graphEdges: statusItems.filter((item) => item.artifactType === 'story_graph_edge'), plotThreads: statusItems.filter((item) => item.artifactType === 'plot_thread_proposal'), skippedResults: statusItems.filter((item) => item.explicitlySkipped), uncertainResults: statusItems.filter((item) => item.reviewStatus === 'uncertain'), rejectedResults: statusItems.filter((item) => item.reviewStatus === 'rejected'), providers: [...new Set([job.providerId, ...phaseResults.map((result) => result.providerId)])], promptVersions: [...new Set(phaseResults.map((result) => result.promptVersion))], warnings: phaseResults.flatMap((result) => Array.isArray(result.payload.warnings) ? result.payload.warnings.filter((item): item is string => typeof item === 'string') : []), truncations: [...new Set(truncations)] } });
    const progress = { ...job.phaseProgress, user_review: { ...(job.phaseProgress.user_review ?? this.emptyProgress('user_review', open, job.providerId)), status: 'completed' as const, totalUnits: open, completedUnits: open, errorMessage: open > 0 ? 'Offene Vorschläge wurden vom Nutzer ausdrücklich übersprungen.' : undefined, updatedAt: new Date().toISOString() } };
    await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'completed', currentPhase: 'completed', phaseProgress: progress, errorMessage: undefined, completedAt: new Date().toISOString() });
  }

  async retryFailed(): Promise<void> {
    const job = await this.repository.getManuscriptAnalysisJob(this.jobId);
    const units = await this.repository.listManuscriptAnalysisUnits(this.jobId);
    const retryStart = Math.min(...units.filter((item) => item.status === 'failed').map((item) => item.orderIndex), Number.MAX_SAFE_INTEGER);
    if (retryStart !== Number.MAX_SAFE_INTEGER) await this.invalidateFrom(units, retryStart);
    for (const unit of units.filter((item) => item.status === 'failed')) {
      await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'pending', retryCount: unit.retryCount + 1, errorMessage: undefined, errorCode: undefined, continuityRunId: undefined });
    }
    const progress = { ...job.phaseProgress };
    if (job.currentPhase !== 'completed') progress[job.currentPhase] = { ...(progress[job.currentPhase] ?? this.emptyProgress(job.currentPhase, units.length, job.providerId)), status: 'pending', failedUnits: 0, errorCode: undefined, errorMessage: undefined };
    await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'pending', phaseProgress: progress, phaseErrors: { ...job.phaseErrors, [job.currentPhase]: undefined }, errorMessage: undefined });
    return this.start();
  }

  private emptyProgress(_phase: ManuscriptAnalysisPhase, totalUnits: number, providerId: string): ManuscriptAnalysisPhaseProgress { return { status: 'pending', totalUnits, completedUnits: 0, failedUnits: 0, requestedProvider: providerId, updatedAt: new Date().toISOString() }; }

  private async markIntegratedPassagePhase(job: ManuscriptAnalysisJob, phase: ManuscriptAnalysisPhase, units: ManuscriptAnalysisUnit[], provider: Provider): Promise<void> {
    const current = await this.repository.getManuscriptAnalysisJob(job.id);
    const progress = current.phaseProgress[phase] ?? this.emptyProgress(phase, units.length, provider.id);
    progress.status = 'completed'; progress.totalUnits = units.length; progress.completedUnits = units.length; progress.failedUnits = 0; progress.requestedProvider = provider.id; progress.actualProvider = provider.id; progress.lastSuccessfulUnitId = units.at(-1)?.id; progress.updatedAt = new Date().toISOString();
    await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: phase, phaseProgress: { ...current.phaseProgress, [phase]: progress } });
  }

  private async savePhase(job: ManuscriptAnalysisJob, phase: ManuscriptAnalysisPhase, patch: Partial<ManuscriptAnalysisPhaseProgress>, status: ManuscriptAnalysisJob['status'], errorMessage?: string): Promise<ManuscriptAnalysisJob> {
    const current = job.phaseProgress[phase] ?? this.emptyProgress(phase, job.totalUnits, this.currentProvider?.id ?? job.providerId);
    const next = { ...job.phaseProgress, [phase]: { ...current, ...patch, updatedAt: new Date().toISOString() } };
    return this.repository.updateManuscriptAnalysisJob({ id: job.id, status, currentPhase: phase, phaseProgress: next, errorMessage });
  }

  private async saveStructuredPhaseResult(job: ManuscriptAnalysisJob, phase: ManuscriptAnalysisPhase, resultKind: string, result: unknown, provider: Provider, artifactIds: Array<{ artifactType: ManuscriptAnalysisArtifactType; artifactId: string; unitId?: string }> = []): Promise<string> {
    const payload = result as Record<string, unknown>;
    const phaseResult = await this.repository.saveManuscriptAnalysisPhaseResult({ jobId: job.id, projectId: job.projectId, phase, resultKind, payload, contentHash: contentHash(JSON.stringify(payload)), providerId: provider.id, promptVersion: PROMPT_VERSION, reviewStatus: 'pending' });
    if (artifactIds.length) await this.repository.saveManuscriptAnalysisArtifacts(job.id, artifactIds.map((artifact) => ({ jobId: job.id, projectId: job.projectId, phase, unitId: artifact.unitId, artifactType: artifact.artifactType, artifactId: artifact.artifactId, reviewStatus: 'pending', explicitlySkipped: false })));
    return phaseResult.id;
  }

  private async checkControl(job: ManuscriptAnalysisJob): Promise<boolean> {
    if (this.cancelled) { await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'cancelled', currentUnitId: undefined, errorMessage: 'Analyse wurde abgebrochen.' }); return false; }
    if (this.paused) { await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'paused', currentUnitId: undefined }); return false; }
    return true;
  }

  private async invalidateFrom(units: ManuscriptAnalysisUnit[], orderIndex: number): Promise<void> {
    await this.repository.invalidateManuscriptAnalysisFrom(this.jobId, orderIndex);
    const later = units.filter((unit) => unit.orderIndex >= orderIndex);
    for (const unit of later) {
      await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'stale', continuityRunId: undefined, errorMessage: 'Durch eine frühere Textänderung veraltet.', errorCode: 'STALE_CONTEXT' });
    }
    const entries = await this.repository.listManuscriptAnalysisDraftLedger(this.jobId);
    const orderByUnit = new Map(units.map((unit) => [unit.id, unit.orderIndex]));
    for (const entry of entries) if ((orderByUnit.get(entry.unitId) ?? -1) >= orderIndex && entry.status !== 'superseded') await this.repository.reviewManuscriptAnalysisDraftLedger(entry.id, 'superseded');
    const currentJob = await this.repository.getManuscriptAnalysisJob(this.jobId);
    const previous = units.filter((unit) => unit.orderIndex < orderIndex && (unit.status === 'completed' || unit.status === 'skipped')).at(-1);
    const resetPhases = PHASES.filter((phase) => phase !== 'structure' && phase !== 'user_review' && phase !== 'completed');
    const phaseProgress = { ...currentJob.phaseProgress };
    for (const phase of resetPhases) {
      const progress = phaseProgress[phase];
      if (!progress) continue;
      phaseProgress[phase] = { ...progress, status: 'pending', completedUnits: previous ? Math.min(progress.completedUnits, orderIndex) : 0, lastSuccessfulUnitId: previous?.id, errorCode: undefined, errorMessage: undefined, updatedAt: new Date().toISOString() };
    }
    await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'pending', currentPhase: 'passage_continuity', currentUnitId: undefined, lastSuccessfulUnitId: previous?.id, phaseProgress, errorMessage: 'Analyse ab der geänderten Einheit erneut erforderlich.' });
  }

  private async verifyPreviousHashes(units: ManuscriptAnalysisUnit[], workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>): Promise<void> {
    for (const [index, unit] of units.entries()) {
      if (unit.status !== 'completed' && unit.status !== 'skipped') continue;
      const chapter = workspace.chapters.find((item) => item.id === unit.chapterId);
      const scene = chapter?.scenes.find((item) => item.id === unit.sceneId);
      if (!scene) continue;
      const current = Array.from(editorContentToPlainText(scene.content)).slice(unit.startOffset, unit.endOffset).join('');
      if (contentHash(current) !== unit.contentHash) { await this.invalidateFrom(units, unit.orderIndex); return; }
      if (index === units.length - 1) return;
    }
  }

  private async execute(): Promise<void> {
    let job = await this.repository.getManuscriptAnalysisJob(this.jobId);
    if (job.status === 'cancelled' || job.status === 'awaiting_structure_review' || (job.status === 'completed' && job.currentPhase === 'completed')) return;
    const active = this.providerOverride ? { provider: this.providerOverride, settings: await providerRouter.getSettings() } : await providerRouter.getActiveProvider();
    this.currentProvider = active.provider;
    const workspace = await this.repository.loadWorkspace();
    const units = (await this.repository.listManuscriptAnalysisUnits(this.jobId)).sort((a, b) => a.orderIndex - b.orderIndex);
    const chapters = [...workspace.chapters].sort((a, b) => a.orderIndex - b.orderIndex);
    if (job.status === 'awaiting_user_review') {
      await this.verifyPreviousHashes(units, workspace);
      job = await this.repository.getManuscriptAnalysisJob(this.jobId);
      if (job.status === 'awaiting_user_review') return;
    } else {
      await this.verifyPreviousHashes(units, workspace);
      job = await this.repository.getManuscriptAnalysisJob(this.jobId);
    }
    job = await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: job.currentPhase, errorMessage: undefined });

    for (const phase of PHASES) {
      if (phase === 'completed' || phaseIndex(phase) < phaseIndex(job.currentPhase)) continue;
      if (!await this.checkControl(job)) return;
      try {
        job = await this.savePhase(job, phase, { status: 'running', requestedProvider: active.provider.id, errorCode: undefined, errorMessage: undefined }, 'running');
        if (phase === 'structure') await this.runStructure(job, workspace, chapters, units, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'passage_continuity') await this.runContinuity(job, workspace, units, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'bible_extraction' || phase === 'character_memory') await this.markIntegratedPassagePhase(job, phase, units, active.provider);
        else if (phase === 'scene_or_chapter_synthesis') await this.runChapterSynthesis(job, workspace, chapters, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'narrative_summaries') await this.runNarrativeSummaries(job, workspace, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'plot_thread_synthesis') await this.runPlotThreadSynthesis(job, workspace, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'book_end_state') await this.runBookEndState(job, workspace, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'global_countercheck') await this.runGlobalCountercheck(job, workspace, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        job = await this.repository.getManuscriptAnalysisJob(this.jobId);
        job = await this.savePhase(job, phase, { status: 'completed', failedUnits: 0, actualProvider: active.provider.id }, 'running');
        if (phase === 'structure' && typeof active.provider.analyzeManuscriptStructure === 'function') { await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'awaiting_structure_review', currentPhase: 'structure', errorMessage: 'Strukturanalyse abgeschlossen. Szenenvorschläge müssen vor der Passageanalyse geprüft und übernommen werden.' }); return; }
        if (phase === 'global_countercheck') { await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'awaiting_user_review', currentPhase: 'user_review', errorMessage: 'AI-Phasen abgeschlossen. Nutzerreview erforderlich; offene Vorschläge müssen ausdrücklich übersprungen oder entschieden werden.' }); return; }
        if (phase !== 'user_review') job = await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: PHASES[phaseIndex(phase) + 1] ?? 'completed', errorMessage: undefined });
      } catch (error) {
        if (this.cancelled || this.paused) { await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: this.cancelled ? 'cancelled' : 'paused', currentUnitId: undefined, errorMessage: this.cancelled ? 'Analyse wurde abgebrochen.' : undefined }); return; }
        const message = errorText(error);
        const code = errorCode(error);
        const failedJob = await this.repository.getManuscriptAnalysisJob(this.jobId);
        const errors = { ...failedJob.phaseErrors, [phase]: { code, message, at: new Date().toISOString(), unitId: failedJob.currentUnitId } };
        await this.savePhase({ ...failedJob, phaseErrors: errors }, phase, { status: 'failed', errorCode: code, errorMessage: message }, 'failed', message);
        throw error;
      }
    }
    const finalJob = await this.repository.getManuscriptAnalysisJob(this.jobId);
    await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'completed', currentPhase: 'completed', phaseProgress: { ...finalJob.phaseProgress, completed: { ...(finalJob.phaseProgress.completed ?? this.emptyProgress('completed', 1, active.provider.id)), status: 'completed', totalUnits: 1, completedUnits: 1, actualProvider: active.provider.id, updatedAt: new Date().toISOString() } }, currentUnitId: undefined, errorMessage: undefined });
  }

  private async runStructure(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, chapters: Chapter[], units: ManuscriptAnalysisUnit[], provider: Provider, timeout: number): Promise<void> {
    if (!chapters.length || units.length === 0) throw new Error('Das Manuskript enthält keine analysierbaren Kapitelprüfeinheiten.');
    if (chapters.some((chapter) => chapter.scenes.length === 0)) throw new Error('Ein Kapitel besitzt keine implizite Importszene. Die Struktur muss zuerst repariert werden.');
    const progress = job.phaseProgress.structure ?? this.emptyProgress('structure', chapters.length, provider.id);
    if (typeof provider.analyzeManuscriptStructure !== 'function') {
      await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', phaseProgress: { ...job.phaseProgress, structure: { ...progress, status: 'completed', totalUnits: chapters.length, completedUnits: chapters.length, updatedAt: new Date().toISOString() } } });
      return;
    }
    for (let index = 0; index < chapters.length; index += 1) {
      if (!await this.checkControl(job)) return;
      const chapter = chapters[index];
      const text = chapterText(chapter);
      const hash = contentHash(text);
      const existing = (await this.repository.listManuscriptStructureRuns(workspace.project.id, chapter.id)).find((run) => run.contentHash === hash && ['completed', 'reviewed'].includes(run.status));
      if (existing) { progress.completedUnits = index + 1; progress.lastSuccessfulUnitId = existing.id; continue; }
      const run = await this.repository.createManuscriptStructureRun(workspace.project.id, chapter.id, hash, provider.id, `${PROMPT_VERSION}-structure`);
      await this.repository.updateManuscriptStructureRun(run.id, 'running');
      try {
        const [lore, rules] = await Promise.all([this.repository.getLoreMetadata(workspace.project.id), this.repository.listProjectRules(workspace.project.id, true)]);
        const structureChapter = { ...chapter, scenes: chapter.scenes.map((scene) => ({ ...scene, content: editorContentToPlainText(scene.content) })) };
        const result = await provider.analyzeManuscriptStructure({ projectId: workspace.project.id, chapter: structureChapter, pageMarkers: job.pageMarkers.filter((marker) => marker.chapterId === chapter.id), localHints: localStructureHints(text), confirmedLore: lore.filter((item) => workspace.entities.some((entity) => entity.id === item.entityId && entity.authorConfirmed)), confirmedRules: rules.filter((rule) => rule.status === 'confirmed' && rule.authorConfirmed) }, timeout);
        const proposals = result.scenes.map((proposal) => ({ ...proposal, runId: run.id, projectId: workspace.project.id, chapterId: chapter.id, reviewStatus: 'proposed' as const, evidenceExcerpt: proposal.evidenceExcerpt }));
        validateManuscriptStructure(text, proposals);
        await this.repository.saveManuscriptStructureProposals(run.id, proposals);
        await this.repository.updateManuscriptStructureRun(run.id, 'completed');
        progress.completedUnits = index + 1; progress.lastSuccessfulUnitId = run.id; progress.actualProvider = provider.id; progress.updatedAt = new Date().toISOString();
        await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: 'structure', phaseProgress: { ...(await this.repository.getManuscriptAnalysisJob(job.id)).phaseProgress, structure: progress } });
      } catch (error) {
        await this.repository.updateManuscriptStructureRun(run.id, 'failed', errorText(error));
        throw error;
      }
    }
  }

  private async runContinuity(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, units: ManuscriptAnalysisUnit[], provider: Provider, timeout: number): Promise<void> {
    const progress = job.phaseProgress.passage_continuity ?? this.emptyProgress('passage_continuity', units.length, provider.id);
    const lastId = progress.lastSuccessfulUnitId;
    const resumeIndex = lastId ? units.findIndex((candidate) => candidate.id === lastId) + 1 : 0;
    for (let index = 0; index < units.length; index += 1) {
      const unit = units[index];
      if (index < resumeIndex) continue;
      if (!await this.checkControl(job)) return;
      const chapter = workspace.chapters.find((item) => item.id === unit.chapterId);
      const scene = chapter?.scenes.find((item) => item.id === unit.sceneId);
      if (!chapter || !scene) throw new Error('Kapitel oder Szene der Prüfeinheit wurde nicht gefunden.');
      const sceneText = editorContentToPlainText(scene.content);
      const currentContent = Array.from(sceneText).slice(unit.startOffset, unit.endOffset).join('');
      const currentHash = contentHash(currentContent);
      const currentUnit = (await this.repository.listManuscriptAnalysisUnits(this.jobId)).find((candidate) => candidate.id === unit.id) ?? unit;
      if ((currentUnit.status === 'completed' || currentUnit.status === 'skipped') && currentUnit.contentHash === currentHash) { progress.completedUnits += 1; progress.lastSuccessfulUnitId = unit.id; continue; }
      if (currentUnit.status === 'failed') throw new Error(currentUnit.errorMessage ?? 'Eine Prüfeinheit ist fehlgeschlagen.');
      await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'running', currentPhase: 'passage_continuity', currentUnitId: unit.id, errorMessage: undefined });
      await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'running', requestedProvider: provider.id, actualProvider: undefined, promptVersion: PROMPT_VERSION, inputHash: currentHash, errorMessage: undefined, errorCode: undefined, content: currentContent, contentHash: currentHash });
      try {
        const previous = units[index - 1];
        const previousContent = (await this.repository.listManuscriptAnalysisUnits(this.jobId)).find((candidate) => candidate.id === previous?.id)?.content ?? previous?.content;
        await this.resolveProvisionalEntities(workspace, job, unit, currentContent, previousContent?.slice(-2000) ?? '', provider);
      // resolveProvisionalEntities has persisted the current passage before
      // downstream providers run. Inclusive ordering makes the first mention
      // immediately available to Bible, memory and continuity without leaking
      // later units.
      const priorProvisional = await this.listPriorProvisionalEntities(job.id, unit.orderIndex, true);
        await this.runBibleUnit(job, workspace, unit, provider, timeout, priorProvisional);
        await this.runCharacterMemoryUnit(job, workspace, unit, provider, timeout, priorProvisional);
        const afterPassage = await this.repository.getManuscriptAnalysisJob(this.jobId);
        const bibleProgress = afterPassage.phaseProgress.bible_extraction ?? this.emptyProgress('bible_extraction', units.length, provider.id);
        const memoryProgress = afterPassage.phaseProgress.character_memory ?? this.emptyProgress('character_memory', units.length, provider.id);
        bibleProgress.status = 'completed'; bibleProgress.completedUnits = index + 1; bibleProgress.totalUnits = units.length; bibleProgress.lastSuccessfulUnitId = unit.id; bibleProgress.actualProvider = provider.id; bibleProgress.updatedAt = new Date().toISOString();
        memoryProgress.status = 'completed'; memoryProgress.completedUnits = index + 1; memoryProgress.totalUnits = units.length; memoryProgress.lastSuccessfulUnitId = unit.id; memoryProgress.actualProvider = provider.id; memoryProgress.updatedAt = new Date().toISOString();
        await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'running', phaseProgress: { ...afterPassage.phaseProgress, bible_extraction: bibleProgress, character_memory: memoryProgress } });
        const allDraftEntries = await this.repository.listManuscriptAnalysisDraftLedger(this.jobId);
        const orderByUnit = new Map(units.map((candidate) => [candidate.id, candidate.orderIndex]));
        const draftLedger = allDraftEntries.filter((entry) => (orderByUnit.get(entry.unitId) ?? Number.MAX_SAFE_INTEGER) < unit.orderIndex && entry.status !== 'superseded').map(draftEntryToLedger);
        const rulesBefore = new Set((await this.repository.listProjectRuleProposals(workspace.project.id)).map((proposal) => proposal.id));
        const chronologicalScene = passageScene(scene, currentContent);
        const chronologicalChapter = passageChapter(chapter, scene, currentContent);
        const result = await runContinuityReview(this.repository, { project: workspace.project, chapter: chronologicalChapter, scene: chronologicalScene, currentText: currentContent, previousText: previousContent?.slice(-2000), chronological: true, sourceKind: unit.pageNumber === undefined ? 'word_threshold' : 'page_marker', startOffset: unit.startOffset, endOffset: unit.endOffset, draftLedger, provisionalEntities: priorProvisional, provisionalAliases: priorProvisional.flatMap((entity) => entity.aliases.map((alias) => ({ id: `${entity.id}:${alias}`, provisionalEntityId: entity.id, alias, confidence: entity.confidence, reviewStatus: 'proposed' as const, createdAt: entity.createdAt }))), provider, persistStateProposals: false, isCancelled: () => this.cancelled, forceAnalysis: true });
        const draftEntries = await Promise.all(result.draftStateChanges.map(async (change) => ({ jobId: this.jobId, unitId: unit.id, projectId: workspace.project.id, entityId: change.entityId, relatedEntityId: change.relatedEntityId, stateKind: change.stateKind, previousState: change.previousState, newState: change.newState, chapterId: unit.chapterId, sceneId: unit.sceneId, startOffset: change.startOffset ?? unit.startOffset, endOffset: change.endOffset ?? unit.endOffset, sourceExcerpt: change.evidenceExcerpt, sourceReferenceId: change.sourceReferenceId ?? (await this.repository.createSourceReference({ projectId: workspace.project.id, entityId: change.entityId, chapterId: unit.chapterId, sceneId: unit.sceneId, excerpt: change.evidenceExcerpt, startOffset: change.startOffset ?? unit.startOffset, endOffset: change.endOffset ?? unit.endOffset })).id, confidence: change.confidence, status: 'proposed' as const })));
        const savedDraftEntries = await this.repository.replaceManuscriptAnalysisDraftLedger(unit.id, draftEntries);
        await this.repository.saveManuscriptAnalysisArtifacts(this.jobId, savedDraftEntries.map((entry) => ({ jobId: this.jobId, projectId: workspace.project.id, phase: 'passage_continuity' as const, unitId: unit.id, artifactType: 'import_draft_state' as const, artifactId: entry.id, reviewStatus: 'pending' as const, explicitlySkipped: false })));
        const continuityFindings = await this.repository.listContinuityReviewFindings(workspace.project.id, result.runId);
        await this.repository.saveManuscriptAnalysisArtifacts(this.jobId, continuityFindings.map((finding) => ({ jobId: this.jobId, projectId: workspace.project.id, phase: 'passage_continuity' as const, unitId: unit.id, artifactType: 'continuity_finding' as const, artifactId: finding.id, reviewStatus: 'pending' as const, explicitlySkipped: false })));
        const newRuleProposals = (await this.repository.listProjectRuleProposals(workspace.project.id)).filter((proposal) => !rulesBefore.has(proposal.id));
        await this.repository.saveManuscriptAnalysisArtifacts(this.jobId, newRuleProposals.map((proposal) => ({ jobId: this.jobId, projectId: workspace.project.id, phase: 'passage_continuity' as const, unitId: unit.id, artifactType: 'project_rule_proposal' as const, artifactId: proposal.id, reviewStatus: 'pending' as const, explicitlySkipped: false })));
        await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'completed', continuityRunId: result.runId, actualProvider: provider.id, outputHash: contentHash(JSON.stringify(result)), inputHash: currentHash, content: currentContent, contentHash: currentHash, errorMessage: undefined, errorCode: undefined });
        progress.completedUnits += 1; progress.lastSuccessfulUnitId = unit.id; progress.updatedAt = new Date().toISOString();
        await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'running', currentPhase: 'passage_continuity', currentUnitId: undefined, lastSuccessfulUnitId: unit.id, phaseProgress: { ...(await this.repository.getManuscriptAnalysisJob(this.jobId)).phaseProgress, passage_continuity: progress } });
      } catch (error) {
        if (this.cancelled || this.paused) throw error;
        const message = errorText(error); await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'failed', actualProvider: provider.id, errorCode: errorCode(error), errorMessage: message }); throw error;
      }
    }
  }

  private async runBibleUnit(_job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, unit: ManuscriptAnalysisUnit, provider: Provider, timeout: number, provisionalEntities: ProvisionalEntity[]): Promise<void> {
    const chapter = workspace.chapters.find((item) => item.id === unit.chapterId);
    const scene = chapter?.scenes.find((item) => item.id === unit.sceneId);
    if (!chapter || !scene) throw new Error('Kapitel oder Szene der Bible-Einheit wurde nicht gefunden.');
    const currentContent = Array.from(editorContentToPlainText(scene.content)).slice(unit.startOffset, unit.endOffset).join('');
    const currentScene = passageScene(scene, currentContent);
    const currentChapter = passageChapter(chapter, scene, currentContent);
    await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'running', requestedProvider: provider.id, promptVersion: PROMPT_VERSION, inputHash: contentHash(currentContent), errorMessage: undefined, errorCode: undefined });
    const sources = await this.repository.listSourceReferences(workspace.project.id);
    const availableEntities = entitiesAtOrBefore(workspace.entities, sources, workspace.chapters, unit);
    const run = await this.repository.createBibleUpdateRun({ projectId: workspace.project.id, sceneId: scene.id, sceneUpdatedAt: scene.updatedAt ?? '', contentHash: contentHash(currentContent), extractorId: provider.id, analyzedContent: currentContent });
    const result = await provider.extractBiblePatch({ project: workspace.project, chapter: currentChapter, scene: currentScene, existingEntities: availableEntities, provisionalEntities, provisionalAliases: provisionalEntities.flatMap((entity) => entity.aliases.map((alias) => ({ id: `${entity.id}:${alias}`, provisionalEntityId: entity.id, alias, confidence: entity.confidence, reviewStatus: 'proposed' as const, createdAt: entity.createdAt }))), relevantSources: sources.filter((source) => sourceIsAtOrBefore(source, workspace.chapters, unit)), previousAnalyzedContent: '', changedRange: { start: 0, end: Array.from(currentContent).length } }, timeout);
    const savedProposals = await this.repository.saveBibleProposals(run.id, result.proposals, workspace.project.id, scene.id);
    await this.repository.saveManuscriptAnalysisArtifacts(this.jobId, savedProposals.map((proposal) => ({ jobId: this.jobId, projectId: workspace.project.id, phase: 'bible_extraction' as const, unitId: unit.id, artifactType: 'bible_proposal' as const, artifactId: proposal.id, reviewStatus: 'pending' as const, explicitlySkipped: false })));
    await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'running', actualProvider: provider.id, outputHash: contentHash(JSON.stringify(result)) });
  }

  private async runCharacterMemoryUnit(_job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, unit: ManuscriptAnalysisUnit, provider: Provider, timeout: number, provisionalEntities: ProvisionalEntity[]): Promise<void> {
    const chapter = workspace.chapters.find((item) => item.id === unit.chapterId);
    const scene = chapter?.scenes.find((item) => item.id === unit.sceneId);
    if (!chapter || !scene) throw new Error('Kapitel oder Szene der Character-Memory-Einheit wurde nicht gefunden.');
    const currentContent = Array.from(editorContentToPlainText(scene.content)).slice(unit.startOffset, unit.endOffset).join('');
    const currentScene = passageScene(scene, currentContent);
    const currentChapter = passageChapter(chapter, scene, currentContent);
    await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'running', requestedProvider: provider.id, promptVersion: PROMPT_VERSION, inputHash: contentHash(currentContent), errorMessage: undefined, errorCode: undefined });
    const contextBuilder = new DeterministicProjectContextBuilder(this.repository);
    const context = await contextBuilder.build({ projectId: workspace.project.id, currentChapterId: chapter.id, currentSceneId: scene.id, userQuestion: currentContent, includeProposedSummaries: true, passageText: currentContent, passageStartOffset: unit.startOffset, passageEndOffset: unit.endOffset });
    const run = await this.repository.createCharacterMemoryUpdateRun({ projectId: workspace.project.id, sceneId: scene.id, contentHash: contentHash(currentContent), extractorId: provider.id, analyzedContent: currentContent });
    const sources = await this.repository.listSourceReferences(workspace.project.id);
    const availableEntities = entitiesAtOrBefore(workspace.entities, sources, workspace.chapters, unit);
    const characters = availableEntities.filter((entity) => entity.type === 'character');
    const result = await provider.extractCharacterMemoryPatch({ project: workspace.project, chapter: currentChapter, scene: currentScene, characters, existingEntities: availableEntities, provisionalEntities, provisionalAliases: provisionalEntities.flatMap((entity) => entity.aliases.map((alias) => ({ id: `${entity.id}:${alias}`, provisionalEntityId: entity.id, alias, confidence: entity.confidence, reviewStatus: 'proposed' as const, createdAt: entity.createdAt }))), context, changedRange: { start: 0, end: Array.from(currentContent).length } }, timeout);
    const savedProposals = await this.repository.saveCharacterMemoryProposals(run.id, result.proposals);
    await this.repository.saveManuscriptAnalysisArtifacts(this.jobId, savedProposals.map((proposal) => ({ jobId: this.jobId, projectId: workspace.project.id, phase: 'character_memory' as const, unitId: unit.id, artifactType: 'character_memory_proposal' as const, artifactId: proposal.id, reviewStatus: 'pending' as const, explicitlySkipped: false })));
    await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'running', actualProvider: provider.id, outputHash: contentHash(JSON.stringify(result)) });
  }

  private async resolveProvisionalEntities(workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, job: ManuscriptAnalysisJob, unit: ManuscriptAnalysisUnit, passageText: string, previousContext: string, provider: Provider): Promise<void> {
    if (typeof provider.resolveManuscriptEntityMentions !== 'function') return;
    const sources = await this.repository.listSourceReferences(workspace.project.id);
    const availableEntities = entitiesAtOrBefore(workspace.entities, sources, workspace.chapters, unit);
    const previousMentions = await this.repository.listProvisionalEntityMentions(job.id);
    const units = (await this.repository.listManuscriptAnalysisUnits(job.id)).sort((a, b) => a.orderIndex - b.orderIndex);
    const orderByUnit = new Map(units.map((candidate) => [candidate.id, candidate.orderIndex]));
    const priorIds = new Set(previousMentions.filter((mention) => (orderByUnit.get(mention.passageUnitId) ?? Number.MAX_SAFE_INTEGER) < unit.orderIndex).flatMap((mention) => [mention.resolvedProvisionalEntityId, ...mention.alternativeEntityIds]).filter((id): id is string => Boolean(id)));
    const previousEntities = (await this.repository.listProvisionalEntities(job.id)).filter((entity) => priorIds.has(entity.id));
    const result = await provider.resolveManuscriptEntityMentions({ projectId: workspace.project.id, jobId: job.id, unit, passageText, previousContext, confirmedEntities: availableEntities, previousProvisionalEntities: previousEntities, previousAliases: previousEntities.flatMap((entity) => entity.aliases.map((alias) => ({ id: `${entity.id}:${alias}`, provisionalEntityId: entity.id, alias, confidence: entity.confidence, reviewStatus: 'proposed' as const, createdAt: entity.createdAt }))) }, 120);
    const idByTemporary = new Map<string, string>();
    for (const entity of result.entities) {
      const existing = matchPriorProvisionalEntity(entity.canonicalName, entity.aliases, previousEntities);
      const id = existing?.id ?? provisionalEntityId(job.id, entity.temporaryId);
      idByTemporary.set(entity.temporaryId, id);
      const savedEntity = await this.repository.saveProvisionalEntity({ id, jobId: job.id, projectId: workspace.project.id, entityType: entity.entityType, canonicalName: entity.canonicalName, aliases: entity.aliases, description: entity.description, confidence: entity.confidence, existingEntityId: entity.existingEntityId && availableEntities.some((candidate) => candidate.id === entity.existingEntityId && candidate.projectId === workspace.project.id) ? entity.existingEntityId : undefined, reviewStatus: 'proposed' });
      await this.repository.saveManuscriptAnalysisArtifacts(job.id, [{ jobId: job.id, projectId: workspace.project.id, phase: 'passage_continuity', unitId: unit.id, artifactType: 'provisional_entity', artifactId: savedEntity.id, reviewStatus: 'pending', explicitlySkipped: false }]);
    }
    const mentions = result.mentions.map((mention) => ({ jobId: job.id, projectId: workspace.project.id, passageUnitId: unit.id, chapterId: unit.chapterId, sceneId: unit.sceneId, startOffset: unit.startOffset + mention.startOffset, endOffset: unit.startOffset + mention.endOffset, excerpt: mention.excerpt, mentionText: mention.mentionText, resolvedProvisionalEntityId: mention.temporaryEntityId ? idByTemporary.get(mention.temporaryEntityId) : undefined, alternativeEntityIds: mention.alternativeTemporaryIds.map((id) => idByTemporary.get(id)).filter((id): id is string => Boolean(id)), confidence: mention.confidence, resolutionReason: mention.resolutionReason }));
    if (mentions.length) await this.repository.saveProvisionalMentions(mentions);
    const chapter = workspace.chapters.find((item) => item.id === unit.chapterId);
    const book = workspace.books.find((item) => item.id === chapter?.bookId);
    for (const relation of result.relations) {
      const source = idByTemporary.get(relation.sourceTemporaryId); const target = idByTemporary.get(relation.targetTemporaryId);
      if (source && target) {
        const relationSource = chapter ? await this.repository.createSourceReference({ projectId: workspace.project.id, chapterId: unit.chapterId, sceneId: unit.sceneId, excerpt: passageText, startOffset: unit.startOffset, endOffset: unit.endOffset }) : undefined;
        await this.repository.saveProvisionalRelation({ id: `provisional-relation-${job.id}-${unit.id}-${result.relations.indexOf(relation)}`, jobId: job.id, projectId: workspace.project.id, sourceProvisionalEntityId: source, targetProvisionalEntityId: target, relationType: relation.relationType, label: relation.label, confidence: relation.confidence, reviewStatus: 'proposed', sourceReferenceId: relationSource?.id });
        if (chapter && book) { const edge = await this.repository.saveStoryGraphEdge({ id: `story-graph-${job.id}-${unit.id}-${result.relations.indexOf(relation)}`, projectId: workspace.project.id, sourceEntityId: source, targetEntityId: target, relationType: relation.relationType, label: relation.label, validFromChapterId: unit.chapterId, validFromSceneId: unit.sceneId, validFromOffset: unit.startOffset, sourceReferenceIds: relationSource ? [relationSource.id] : [], confidence: relation.confidence, status: 'proposed', authorConfirmed: false, origin: 'manuscript_analysis' }); await this.repository.saveManuscriptAnalysisArtifacts(job.id, [{ jobId: job.id, projectId: workspace.project.id, phase: 'passage_continuity', unitId: unit.id, artifactType: 'story_graph_edge', artifactId: edge.id, reviewStatus: 'pending', explicitlySkipped: false }]); }
      }
    }
    for (const [eventIndex, event] of result.events.entries()) {
      const participants = event.participantTemporaryIds.map((id) => idByTemporary.get(id)).filter((id): id is string => Boolean(id));
      const startOffset = unit.startOffset + event.startOffset; const endOffset = unit.startOffset + event.endOffset;
      const source = chapter && event.excerpt ? await this.repository.createSourceReference({ projectId: workspace.project.id, chapterId: unit.chapterId, sceneId: unit.sceneId, excerpt: event.excerpt, startOffset, endOffset }) : undefined;
      await this.repository.saveProvisionalEvent({ id: `provisional-event-${job.id}-${unit.id}-${eventIndex}`, jobId: job.id, projectId: workspace.project.id, passageUnitId: unit.id, chapterId: unit.chapterId, sceneId: unit.sceneId, title: event.title, summary: event.summary, participantEntityIds: participants, startOffset, endOffset, confidence: event.confidence, reviewStatus: 'proposed', sourceReferenceId: source?.id });
      if (chapter && book) { const timeline = await this.repository.saveTimelineEvent({ id: `timeline-event-${job.id}-${unit.id}-${eventIndex}`, projectId: workspace.project.id, bookId: book.id, chapterId: unit.chapterId, sceneId: unit.sceneId, passageUnitId: unit.id, title: event.title, summary: event.summary, storyTimeText: chapter.scenes.find((scene) => scene.id === unit.sceneId)?.storyTime ?? '', temporalOrder: unit.orderIndex * 1_000_000 + startOffset, timeCertainty: chapter.scenes.find((scene) => scene.id === unit.sceneId)?.storyTime ? 'relative' : 'unknown', participatingEntityIds: participants, causeEventIds: [], consequenceEventIds: [], knowledgeChanges: [], stateChanges: [], relatedPlotThreadIds: [], sourceReferenceIds: source ? [source.id] : [], confidence: event.confidence, status: 'proposed', authorConfirmed: false, origin: 'manuscript_analysis' }); await this.repository.saveManuscriptAnalysisArtifacts(job.id, [{ jobId: job.id, projectId: workspace.project.id, phase: 'passage_continuity', unitId: unit.id, artifactType: 'timeline_event', artifactId: timeline.id, reviewStatus: 'pending', explicitlySkipped: false }]); }
    }
    for (const merge of result.mergeProposals) { const left = idByTemporary.get(merge.leftTemporaryId); if (!left) continue; const savedMerge = await this.repository.saveProvisionalMergeProposal({ jobId: job.id, projectId: workspace.project.id, leftProvisionalEntityId: left, rightProvisionalEntityId: merge.rightTemporaryId ? idByTemporary.get(merge.rightTemporaryId) : undefined, existingEntityId: merge.existingEntityId && workspace.entities.some((entity) => entity.id === merge.existingEntityId && entity.projectId === workspace.project.id) ? merge.existingEntityId : undefined, reason: merge.reason, confidence: merge.confidence, reviewStatus: 'proposed' }); await this.repository.saveManuscriptAnalysisArtifacts(job.id, [{ jobId: job.id, projectId: workspace.project.id, phase: 'passage_continuity', unitId: unit.id, artifactType: 'provisional_merge', artifactId: savedMerge.id, reviewStatus: 'pending', explicitlySkipped: false }]); }
    void previousMentions;
  }

  private async listPriorProvisionalEntities(jobId: string, orderIndex: number, includeCurrent = false): Promise<ProvisionalEntity[]> {
    const [entities, mentions, units] = await Promise.all([this.repository.listProvisionalEntities(jobId), this.repository.listProvisionalEntityMentions(jobId), this.repository.listManuscriptAnalysisUnits(jobId)]);
    const orderByUnit = new Map(units.map((unit) => [unit.id, unit.orderIndex]));
    const firstOrder = new Map<string, number>();
    for (const mention of mentions) {
      const order = orderByUnit.get(mention.passageUnitId);
      if (order !== undefined && (includeCurrent ? order <= orderIndex : order < orderIndex) && mention.resolvedProvisionalEntityId && (!firstOrder.has(mention.resolvedProvisionalEntityId) || order < firstOrder.get(mention.resolvedProvisionalEntityId)!)) firstOrder.set(mention.resolvedProvisionalEntityId, order);
    }
    return entities.filter((entity) => (firstOrder.get(entity.id) ?? Number.MAX_SAFE_INTEGER) < orderIndex).sort((a, b) => (firstOrder.get(a.id) ?? Number.MAX_SAFE_INTEGER) - (firstOrder.get(b.id) ?? Number.MAX_SAFE_INTEGER)).slice(0, 160);
  }

  private async runChapterSynthesis(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, chapters: Chapter[], provider: Provider, timeout: number): Promise<void> {
    const progress = job.phaseProgress.scene_or_chapter_synthesis ?? this.emptyProgress('scene_or_chapter_synthesis', chapters.length, provider.id); const start = progress.lastSuccessfulUnitId ? Math.max(0, chapters.findIndex((chapter) => chapter.id === progress.lastSuccessfulUnitId) + 1) : 0;
    for (let index = start; index < chapters.length; index += 1) { if (!await this.checkControl(job)) return; const chapter = chapters[index]; const text = chapterText(chapter); const bounded = truncateUnicode(text, 12000); const providerText = bounded.truncated ? `${bounded.value}\n\n[WARNUNG: Kapiteltext für die Kapitel-Synthese gekürzt; Quellenpositionen bleiben auf dem vollständigen Kapiteltext definiert.]` : bounded.value; const result = await provider.summarize('chapter', chapter.id, `Führe eine strukturierte Kapitel-Synthese durch. Bewerte wichtige Ereignisse, Wissen, Beziehungen, offene Threads und Endzustände.\n\n${providerText}`, timeout); const summary = await this.repository.saveNarrativeSummary(synthesisToSummary(result, workspace.project.id, 'chapter', chapter.id, text)); await this.repository.saveManuscriptAnalysisArtifacts(this.jobId, [{ jobId: this.jobId, projectId: workspace.project.id, phase: 'scene_or_chapter_synthesis' as const, artifactType: 'narrative_summary' as const, artifactId: summary.id, reviewStatus: 'pending' as const, explicitlySkipped: false }]); progress.completedUnits += 1; progress.lastSuccessfulUnitId = chapter.id; progress.actualProvider = provider.id; progress.updatedAt = new Date().toISOString(); await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: 'scene_or_chapter_synthesis', phaseProgress: { ...(await this.repository.getManuscriptAnalysisJob(job.id)).phaseProgress, scene_or_chapter_synthesis: progress } }); }
  }

  private async phaseInput(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>): Promise<ManuscriptPhaseInput> {
    const chapters = [...workspace.chapters].sort((a, b) => a.orderIndex - b.orderIndex);
    const [chapterSummaries, findings, threads, rules, ledger, sources, timelineEvents, units, draftLedger] = await Promise.all([
      this.repository.listNarrativeSummaries(workspace.project.id, 'chapter'),
      this.repository.listContinuityReviewFindings(workspace.project.id),
      this.repository.listPlotThreadLifecycleProposals(workspace.project.id),
      this.repository.listProjectRules(workspace.project.id),
      this.repository.listContinuityStateLedger(workspace.project.id),
      this.repository.listSourceReferences(workspace.project.id),
      this.repository.listTimelineEvents(workspace.project.id),
      this.repository.listManuscriptAnalysisUnits(job.id),
      this.repository.listManuscriptAnalysisDraftLedger(job.id),
    ]);
    const hierarchical = buildHierarchicalPhaseContext({
      chapters,
      chapterSummaries,
      sourceReferences: sources,
      timelineEvents,
      draftLedger: draftLedger.filter((entry) => units.some((unit) => unit.id === entry.unitId)),
      confirmedEntities: workspace.entities.filter((entity) => entity.status === 'confirmed' && entity.authorConfirmed),
      confirmedRules: rules.filter((rule) => rule.status === 'confirmed' && rule.authorConfirmed),
      confirmedStates: ledger.filter((entry) => entry.status === 'confirmed' && entry.authorConfirmed),
      proposedFindings: findings.filter((finding) => finding.reviewStatus === 'open'),
      proposedThreads: threads.filter((thread) => thread.reviewStatus === 'pending'),
    });
    return { projectId: workspace.project.id, bookId: job.bookId, ...hierarchical, contextLevel: 'hierarchical', contentHash: contentHash(chapters.map(chapterText).join('\n\n')) };
  }

  private async saveBookPhase(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, phase: ManuscriptAnalysisPhase, summary: { summary: string; importantEvents: string[]; openThreads: string[]; characterChanges: string[] }, provider: Provider): Promise<string> {
    const inputHash = contentHash(`${phase}:${(await this.phaseInput(job, workspace)).contentHash}`);
    const saved = await this.repository.saveNarrativeSummary({ projectId: workspace.project.id, scopeType: 'book', scopeId: job.bookId, contentHash: inputHash, summary: summary.summary || `${phase} ohne zusätzliche Zusammenfassung.`, importantEvents: summary.importantEvents, openThreads: summary.openThreads, characterChanges: summary.characterChanges, status: 'proposed', authorConfirmed: false });
    const progress = job.phaseProgress[phase] ?? this.emptyProgress(phase, 1, provider.id);
    progress.completedUnits = 1; progress.totalUnits = 1; progress.lastSuccessfulUnitId = job.bookId; progress.actualProvider = provider.id; progress.inputHash = inputHash; progress.updatedAt = new Date().toISOString();
    await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: phase, phaseProgress: { ...(await this.repository.getManuscriptAnalysisJob(job.id)).phaseProgress, [phase]: progress } }); return saved.id;
  }

  private async runNarrativeSummaries(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, provider: Provider, timeout: number): Promise<void> {
    const input = await this.phaseInput(job, workspace); const result = typeof provider.analyzeNarrativeSummaries === 'function' ? await provider.analyzeNarrativeSummaries(input, timeout) : await provider.summarize('book', job.bookId, input.chapters.map((chapter) => chapter.text).join('\n\n'), timeout);
    const summaryId = await this.saveBookPhase(job, workspace, 'narrative_summaries', result, provider); await this.saveStructuredPhaseResult(job, 'narrative_summaries', 'narrative_summaries', result, provider, [{ artifactType: 'narrative_summary', artifactId: summaryId }]);
  }

  private async runPlotThreadSynthesis(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, provider: Provider, timeout: number): Promise<void> {
    const input = await this.phaseInput(job, workspace); const result: PlotThreadSynthesisResult = typeof provider.synthesizePlotThreads === 'function' ? await provider.synthesizePlotThreads(input, timeout) : { summary: '', openQuestions: [], threadGoals: [], developments: [], closureCandidates: [], partiallyResolved: [], reopened: [], warnings: ['Provider unterstützt diese dedizierte Phase nicht.'] };
    const phaseRun = await this.repository.createContinuityReviewRun({ projectId: workspace.project.id, sourceKind: 'manual', contentHash: input.contentHash, providerId: provider.id }); const proposals = [];
    for (const proposal of result.threadProposals ?? []) proposals.push(await this.repository.savePlotThreadLifecycleProposal({ ...proposal, runId: phaseRun.id, projectId: workspace.project.id, reviewStatus: 'pending' }));
    await this.repository.updateContinuityReviewRunStatus({ id: phaseRun.id, status: 'completed', completedAt: new Date().toISOString() }); await this.saveStructuredPhaseResult(job, 'plot_thread_synthesis', 'plot_thread_synthesis', result, provider, proposals.map((proposal) => ({ artifactType: 'plot_thread_proposal' as const, artifactId: proposal.id })));
  }

  private async runBookEndState(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, provider: Provider, timeout: number): Promise<void> {
    const input = await this.phaseInput(job, workspace); const result: BookEndStateResult = typeof provider.analyzeBookEndState === 'function' ? await provider.analyzeBookEndState(input, timeout) : { summary: '', characterEndStates: [], knowledgeStates: [], falseBeliefs: [], relationships: [], objectOwners: [], injuries: [], locations: [], openActions: [], unresolvedThreads: [], warnings: ['Provider unterstützt diese dedizierte Phase nicht.'] };
    const phaseResultId = await this.saveStructuredPhaseResult(job, 'book_end_state', 'book_end_state', result, provider); await this.repository.saveManuscriptAnalysisArtifacts(this.jobId, (result.endStateProposals ?? []).map((_proposal, index) => ({ jobId: this.jobId, projectId: workspace.project.id, phase: 'book_end_state' as const, artifactType: 'book_end_state_proposal' as const, artifactId: `${phaseResultId}:${index}`, reviewStatus: 'pending' as const, explicitlySkipped: false })));
  }

  private async runGlobalCountercheck(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, provider: Provider, timeout: number): Promise<void> {
    const input = await this.phaseInput(job, workspace); const result: GlobalCountercheckResult = typeof provider.globalCountercheck === 'function' ? await provider.globalCountercheck(input, timeout) : { summary: '', contradictoryFacts: [], prematureKnowledge: [], lostOrDestroyedObjects: [], timeAndLocationConflicts: [], contradictoryRules: [], unclearExceptions: [], uncertainSources: [], warnings: ['Provider unterstützt diese dedizierte Phase nicht.'] };
    const phaseRun = await this.repository.createContinuityReviewRun({ projectId: workspace.project.id, sourceKind: 'manual', contentHash: input.contentHash, providerId: provider.id }); const knownSources = await this.repository.listSourceReferences(workspace.project.id); const sourceIds = new Set(knownSources.map((source) => source.id)); const findings: SaveContinuityFindingInput[] = (result.countercheckFindings ?? []).map((finding) => ({ runId: phaseRun.id, projectId: workspace.project.id, findingType: finding.category.includes('knowledge') ? 'probable_contradiction' : 'missing_explanation', severity: finding.severity, relatedEntityIds: [], relatedStateIds: [], relatedRuleIds: [], objectiveConflict: finding.objectiveConflict, loreExplanations: [], evidenceExcerpt: finding.evidenceExcerpt ?? '', sourceReferenceId: finding.sourceReferenceId && sourceIds.has(finding.sourceReferenceId) ? finding.sourceReferenceId : undefined, counterEvidenceExcerpts: [], counterEvidence: [], confidence: finding.confidence, reason: finding.reason })); const savedFindings = findings.length ? await this.repository.saveContinuityReviewFindings(phaseRun.id, findings) : []; await this.repository.updateContinuityReviewRunStatus({ id: phaseRun.id, status: 'completed', completedAt: new Date().toISOString() }); await this.saveStructuredPhaseResult(job, 'global_countercheck', 'global_countercheck', result, provider, savedFindings.map((finding) => ({ artifactType: 'global_countercheck_finding' as const, artifactId: finding.id })));
  }

}

export function createManuscriptAnalysisUnits(chapters: Chapter[], unitsByChapter: Array<Array<{ text: string; startOffset: number; endOffset: number; page?: number }>>, scenes: Array<{ id: string; chapterId: string }>): CreateManuscriptAnalysisJobInput['units'] {
  let orderIndex = 0;
  return [...chapters].sort((a, b) => a.orderIndex - b.orderIndex).flatMap((chapter, chapterIndex) => { const scene = scenes.find((candidate) => candidate.chapterId === chapter.id); if (!scene) return []; return (unitsByChapter[chapterIndex] ?? []).map((unit) => ({ chapterId: chapter.id, sceneId: scene.id, orderIndex: orderIndex++, pageNumber: unit.page, startOffset: unit.startOffset, endOffset: unit.endOffset, content: unit.text, contentHash: contentHash(unit.text) })); });
}

export async function loadManuscriptAnalysisProgress(repository: StoryRepository, jobId: string): Promise<ManuscriptAnalysisProgress> {
  const [job, units, draftLedger, phaseResults, artifacts, completionReport] = await Promise.all([repository.getManuscriptAnalysisJob(jobId), repository.listManuscriptAnalysisUnits(jobId), repository.listManuscriptAnalysisDraftLedger(jobId), repository.listManuscriptAnalysisPhaseResults(jobId), repository.listManuscriptAnalysisArtifacts(jobId), repository.getManuscriptAnalysisCompletionReport(jobId)]);
  return { job, units, draftLedger, phaseResults, artifacts, completionReport };
}
