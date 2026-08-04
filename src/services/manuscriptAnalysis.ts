import type { Chapter, ContinuityStateLedgerEntry, CreateManuscriptAnalysisJobInput, ManuscriptAnalysisDraftLedgerEntry, ManuscriptAnalysisJob, ManuscriptAnalysisPhase, ManuscriptAnalysisPhaseProgress, ManuscriptAnalysisUnit, ManuscriptSynthesisResult, NarrativeSummaryAnalysisResult, ManuscriptPhaseInput, PlotThreadSynthesisResult, BookEndStateResult, GlobalCountercheckResult, ManuscriptAnalysisArtifactType, SaveContinuityFindingInput } from '../types/domain';
import type { StoryRepository } from './storyRepository';
import { contentHash } from '../utils/aiText';
import { editorContentToPlainText } from '../utils/editorContent';
import { providerRouter, type StoryAiProvider as Provider } from './aiProviderService';
import { runContinuityReview } from './continuityReview';
import { DeterministicProjectContextBuilder } from './contextBuilder';
import { validateManuscriptStructure, localStructureHints } from './manuscriptStructure';
import { matchPriorProvisionalEntity, provisionalEntityId } from './provisionalGraph';

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
function synthesisToSummary(result: ManuscriptSynthesisResult | NarrativeSummaryAnalysisResult, projectId: string, scopeType: 'chapter' | 'book', scopeId: string, sourceText: string) {
  const extended = result as ManuscriptSynthesisResult;
  const characterChanges = 'characterChanges' in result ? result.characterChanges : [];
  return { projectId, scopeType, scopeId, contentHash: contentHash(sourceText), summary: result.summary, importantEvents: result.importantEvents, openThreads: result.openThreads, characterChanges: [...characterChanges, ...(extended.knowledgeChanges ?? []), ...(extended.relationshipChanges ?? []), ...(extended.characterEndStates ?? [])], status: 'proposed' as const, authorConfirmed: false };
}

export interface ManuscriptAnalysisProgress { job: ManuscriptAnalysisJob; units: ManuscriptAnalysisUnit[]; draftLedger: ManuscriptAnalysisDraftLedgerEntry[]; phaseResults: Awaited<ReturnType<StoryRepository['listManuscriptAnalysisPhaseResults']>>; artifacts: Awaited<ReturnType<StoryRepository['listManuscriptAnalysisArtifacts']>>; }

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
    if (openArtifacts.length > 0 && explicitlySkipOpen) {
      for (const artifact of openArtifacts) await this.repository.reviewManuscriptAnalysisArtifact(artifact.id, 'skipped', true);
      await this.repository.saveManuscriptAnalysisReviewAudit({ jobId: job.id, projectId: job.projectId, action: 'skip_open_artifacts', artifactIds: openArtifacts.map((artifact) => artifact.id), artifactTypes: [...new Set(openArtifacts.map((artifact) => artifact.artifactType))] as ManuscriptAnalysisArtifactType[], note: `Nutzer überspringt ausdrücklich ${openArtifacts.length} offene Ergebnisse.`, });
    }
    await this.repository.saveManuscriptAnalysisReviewAudit({ jobId: job.id, projectId: job.projectId, action: 'complete_review', artifactIds: artifacts.map((artifact) => artifact.id), artifactTypes: [...new Set(artifacts.map((artifact) => artifact.artifactType))] as ManuscriptAnalysisArtifactType[], note: explicitlySkipOpen ? 'Review mit ausdrücklich übersprungenen Ergebnissen abgeschlossen.' : 'Review aller jobgebundenen Ergebnisse abgeschlossen.' });
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
    const later = units.filter((unit) => unit.orderIndex >= orderIndex);
    for (const unit of later) {
      await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'stale', continuityRunId: undefined, errorMessage: 'Durch eine frühere Textänderung veraltet.', errorCode: 'STALE_CONTEXT' });
    }
    const entries = await this.repository.listManuscriptAnalysisDraftLedger(this.jobId);
    const orderByUnit = new Map(units.map((unit) => [unit.id, unit.orderIndex]));
    for (const entry of entries) if ((orderByUnit.get(entry.unitId) ?? -1) >= orderIndex && entry.status !== 'superseded') await this.repository.reviewManuscriptAnalysisDraftLedger(entry.id, 'superseded');
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
    if (job.status === 'cancelled' || (job.status === 'completed' && job.currentPhase === 'completed')) return;
    const active = this.providerOverride ? { provider: this.providerOverride, settings: await providerRouter.getSettings() } : await providerRouter.getActiveProvider();
    this.currentProvider = active.provider;
    const workspace = await this.repository.loadWorkspace();
    const units = (await this.repository.listManuscriptAnalysisUnits(this.jobId)).sort((a, b) => a.orderIndex - b.orderIndex);
    const chapters = [...workspace.chapters].sort((a, b) => a.orderIndex - b.orderIndex);
    if (job.status === 'awaiting_user_review') return;
    await this.verifyPreviousHashes(units, workspace);
    job = await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: job.currentPhase, errorMessage: undefined });

    for (const phase of PHASES) {
      if (phase === 'completed' || phaseIndex(phase) < phaseIndex(job.currentPhase)) continue;
      if (!await this.checkControl(job)) return;
      try {
        job = await this.savePhase(job, phase, { status: 'running', requestedProvider: active.provider.id, errorCode: undefined, errorMessage: undefined }, 'running');
        if (phase === 'structure') await this.runStructure(job, workspace, chapters, units, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'passage_continuity') await this.runContinuity(job, workspace, units, active.provider);
        else if (phase === 'bible_extraction') await this.runBible(job, workspace, units, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'character_memory') await this.runCharacterMemory(job, workspace, units, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'scene_or_chapter_synthesis') await this.runChapterSynthesis(job, workspace, chapters, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'narrative_summaries') await this.runNarrativeSummaries(job, workspace, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'plot_thread_synthesis') await this.runPlotThreadSynthesis(job, workspace, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'book_end_state') await this.runBookEndState(job, workspace, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'global_countercheck') await this.runGlobalCountercheck(job, workspace, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        job = await this.repository.getManuscriptAnalysisJob(this.jobId);
        job = await this.savePhase(job, phase, { status: 'completed', failedUnits: 0, actualProvider: active.provider.id }, 'running');
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

  private async runContinuity(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, units: ManuscriptAnalysisUnit[], provider: Provider): Promise<void> {
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
        await this.resolveProvisionalEntities(workspace, job, unit, currentContent, units[index - 1]?.content.slice(-2000) ?? '', provider);
        const allDraftEntries = await this.repository.listManuscriptAnalysisDraftLedger(this.jobId);
        const orderByUnit = new Map(units.map((candidate) => [candidate.id, candidate.orderIndex]));
        const draftLedger = allDraftEntries.filter((entry) => (orderByUnit.get(entry.unitId) ?? Number.MAX_SAFE_INTEGER) < unit.orderIndex && entry.status !== 'superseded').map(draftEntryToLedger);
        const previous = units[index - 1]; const following = units[index + 1];
        const rulesBefore = new Set((await this.repository.listProjectRuleProposals(workspace.project.id)).map((proposal) => proposal.id));
        const result = await runContinuityReview(this.repository, { project: workspace.project, chapter, scene, currentText: currentContent, previousText: previous?.content, followingText: following?.content, sourceKind: unit.pageNumber === undefined ? 'word_threshold' : 'page_marker', startOffset: unit.startOffset, endOffset: unit.endOffset, draftLedger, provider, persistStateProposals: false, isCancelled: () => this.cancelled, forceAnalysis: true });
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

  private async resolveProvisionalEntities(workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, job: ManuscriptAnalysisJob, unit: ManuscriptAnalysisUnit, passageText: string, previousContext: string, provider: Provider): Promise<void> {
    if (typeof provider.resolveManuscriptEntityMentions !== 'function') return;
    const previousEntities = await this.repository.listProvisionalEntities(job.id);
    const previousMentions = await this.repository.listProvisionalEntityMentions(job.id);
    const result = await provider.resolveManuscriptEntityMentions({ projectId: workspace.project.id, jobId: job.id, unit, passageText, previousContext, confirmedEntities: workspace.entities, previousProvisionalEntities: previousEntities, previousAliases: previousEntities.flatMap((entity) => entity.aliases.map((alias) => ({ id: `${entity.id}:${alias}`, provisionalEntityId: entity.id, alias, confidence: entity.confidence, reviewStatus: 'proposed' as const, createdAt: entity.createdAt }))) }, 120);
    const idByTemporary = new Map<string, string>();
    for (const entity of result.entities) {
      const existing = matchPriorProvisionalEntity(entity.canonicalName, entity.aliases, previousEntities);
      const id = existing?.id ?? provisionalEntityId(job.id, entity.temporaryId);
      idByTemporary.set(entity.temporaryId, id);
      await this.repository.saveProvisionalEntity({ id, jobId: job.id, projectId: workspace.project.id, entityType: entity.entityType, canonicalName: entity.canonicalName, aliases: entity.aliases, description: entity.description, confidence: entity.confidence, existingEntityId: entity.existingEntityId && workspace.entities.some((candidate) => candidate.id === entity.existingEntityId && candidate.projectId === workspace.project.id) ? entity.existingEntityId : undefined, reviewStatus: 'proposed' });
    }
    const mentions = result.mentions.map((mention) => ({ jobId: job.id, projectId: workspace.project.id, passageUnitId: unit.id, chapterId: unit.chapterId, sceneId: unit.sceneId, startOffset: unit.startOffset + mention.startOffset, endOffset: unit.startOffset + mention.endOffset, excerpt: mention.excerpt, mentionText: mention.mentionText, resolvedProvisionalEntityId: mention.temporaryEntityId ? idByTemporary.get(mention.temporaryEntityId) : undefined, alternativeEntityIds: mention.alternativeTemporaryIds.map((id) => idByTemporary.get(id)).filter((id): id is string => Boolean(id)), confidence: mention.confidence, resolutionReason: mention.resolutionReason }));
    if (mentions.length) await this.repository.saveProvisionalMentions(mentions);
    for (const relation of result.relations) { const source = idByTemporary.get(relation.sourceTemporaryId); const target = idByTemporary.get(relation.targetTemporaryId); if (source && target) await this.repository.saveProvisionalRelation({ jobId: job.id, projectId: workspace.project.id, sourceProvisionalEntityId: source, targetProvisionalEntityId: target, relationType: relation.relationType, label: relation.label, confidence: relation.confidence, reviewStatus: 'proposed' }); }
    for (const event of result.events) { const participants = event.participantTemporaryIds.map((id) => idByTemporary.get(id)).filter((id): id is string => Boolean(id)); await this.repository.saveProvisionalEvent({ jobId: job.id, projectId: workspace.project.id, passageUnitId: unit.id, chapterId: unit.chapterId, sceneId: unit.sceneId, title: event.title, summary: event.summary, participantEntityIds: participants, startOffset: unit.startOffset + event.startOffset, endOffset: unit.startOffset + event.endOffset, confidence: event.confidence, reviewStatus: 'proposed', sourceReferenceId: undefined }); }
    for (const merge of result.mergeProposals) { const left = idByTemporary.get(merge.leftTemporaryId); if (!left) continue; await this.repository.saveProvisionalMergeProposal({ jobId: job.id, projectId: workspace.project.id, leftProvisionalEntityId: left, rightProvisionalEntityId: merge.rightTemporaryId ? idByTemporary.get(merge.rightTemporaryId) : undefined, existingEntityId: merge.existingEntityId && workspace.entities.some((entity) => entity.id === merge.existingEntityId && entity.projectId === workspace.project.id) ? merge.existingEntityId : undefined, reason: merge.reason, confidence: merge.confidence, reviewStatus: 'proposed' }); }
    void previousMentions;
  }

  private async runBible(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, units: ManuscriptAnalysisUnit[], provider: Provider, timeout: number): Promise<void> {
    const progress = job.phaseProgress.bible_extraction ?? this.emptyProgress('bible_extraction', units.length, provider.id);
    const start = progress.lastSuccessfulUnitId ? Math.max(0, units.findIndex((unit) => unit.id === progress.lastSuccessfulUnitId) + 1) : 0;
    const sources = await this.repository.listSourceReferences(workspace.project.id);
    for (let index = start; index < units.length; index += 1) {
      if (!await this.checkControl(job)) return; const unit = units[index]; const chapter = workspace.chapters.find((item) => item.id === unit.chapterId); const scene = chapter?.scenes.find((item) => item.id === unit.sceneId); if (!chapter || !scene) throw new Error('Kapitel oder Szene der Bible-Einheit wurde nicht gefunden.');
      await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: unit.status, requestedProvider: provider.id, promptVersion: PROMPT_VERSION, inputHash: unit.contentHash, errorMessage: undefined, errorCode: undefined });
      const run = await this.repository.createBibleUpdateRun({ projectId: workspace.project.id, sceneId: scene.id, sceneUpdatedAt: scene.updatedAt ?? '', contentHash: unit.contentHash, extractorId: provider.id, analyzedContent: unit.content });
      const result = await provider.extractBiblePatch({ project: workspace.project, chapter, scene, existingEntities: workspace.entities, relevantSources: sources.filter((source) => source.sceneId === scene.id), previousAnalyzedContent: '', changedRange: { start: unit.startOffset, end: unit.endOffset } }, timeout);
      const savedProposals = await this.repository.saveBibleProposals(run.id, result.proposals, workspace.project.id, scene.id); await this.repository.saveManuscriptAnalysisArtifacts(this.jobId, savedProposals.map((proposal) => ({ jobId: this.jobId, projectId: workspace.project.id, phase: 'bible_extraction' as const, unitId: unit.id, artifactType: 'bible_proposal' as const, artifactId: proposal.id, reviewStatus: 'pending' as const, explicitlySkipped: false }))); await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: unit.status, actualProvider: provider.id, outputHash: contentHash(JSON.stringify(result)) }); progress.completedUnits += 1; progress.lastSuccessfulUnitId = unit.id; progress.actualProvider = provider.id; progress.updatedAt = new Date().toISOString(); await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: 'bible_extraction', phaseProgress: { ...(await this.repository.getManuscriptAnalysisJob(job.id)).phaseProgress, bible_extraction: progress } });
    }
  }

  private async runCharacterMemory(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, units: ManuscriptAnalysisUnit[], provider: Provider, timeout: number): Promise<void> {
    const progress = job.phaseProgress.character_memory ?? this.emptyProgress('character_memory', units.length, provider.id); const start = progress.lastSuccessfulUnitId ? Math.max(0, units.findIndex((unit) => unit.id === progress.lastSuccessfulUnitId) + 1) : 0; const contextBuilder = new DeterministicProjectContextBuilder(this.repository);
    for (let index = start; index < units.length; index += 1) { if (!await this.checkControl(job)) return; const unit = units[index]; const chapter = workspace.chapters.find((item) => item.id === unit.chapterId); const scene = chapter?.scenes.find((item) => item.id === unit.sceneId); if (!chapter || !scene) throw new Error('Kapitel oder Szene der Character-Memory-Einheit wurde nicht gefunden.'); await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: unit.status, requestedProvider: provider.id, promptVersion: PROMPT_VERSION, inputHash: unit.contentHash, errorMessage: undefined, errorCode: undefined }); const context = await contextBuilder.build({ projectId: workspace.project.id, currentChapterId: chapter.id, currentSceneId: scene.id, userQuestion: editorContentToPlainText(scene.content), includeProposedSummaries: true }); const run = await this.repository.createCharacterMemoryUpdateRun({ projectId: workspace.project.id, sceneId: scene.id, contentHash: unit.contentHash, extractorId: provider.id, analyzedContent: unit.content }); const characters = workspace.entities.filter((entity) => entity.type === 'character'); const result = await provider.extractCharacterMemoryPatch({ project: workspace.project, chapter, scene, characters, existingEntities: workspace.entities, context, changedRange: { start: unit.startOffset, end: unit.endOffset } }, timeout); const savedProposals = await this.repository.saveCharacterMemoryProposals(run.id, result.proposals); await this.repository.saveManuscriptAnalysisArtifacts(this.jobId, savedProposals.map((proposal) => ({ jobId: this.jobId, projectId: workspace.project.id, phase: 'character_memory' as const, unitId: unit.id, artifactType: 'character_memory_proposal' as const, artifactId: proposal.id, reviewStatus: 'pending' as const, explicitlySkipped: false }))); await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: unit.status, actualProvider: provider.id, outputHash: contentHash(JSON.stringify(result)) }); progress.completedUnits += 1; progress.lastSuccessfulUnitId = unit.id; progress.actualProvider = provider.id; progress.updatedAt = new Date().toISOString(); await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: 'character_memory', phaseProgress: { ...(await this.repository.getManuscriptAnalysisJob(job.id)).phaseProgress, character_memory: progress } }); }
  }

  private async runChapterSynthesis(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, chapters: Chapter[], provider: Provider, timeout: number): Promise<void> {
    const progress = job.phaseProgress.scene_or_chapter_synthesis ?? this.emptyProgress('scene_or_chapter_synthesis', chapters.length, provider.id); const start = progress.lastSuccessfulUnitId ? Math.max(0, chapters.findIndex((chapter) => chapter.id === progress.lastSuccessfulUnitId) + 1) : 0;
    for (let index = start; index < chapters.length; index += 1) { if (!await this.checkControl(job)) return; const chapter = chapters[index]; const text = chapterText(chapter); const result = await provider.summarize('chapter', chapter.id, `Führe eine strukturierte Kapitel-Synthese durch. Bewerte wichtige Ereignisse, Wissen, Beziehungen, offene Threads und Endzustände.\n\n${text}`, timeout); const summary = await this.repository.saveNarrativeSummary(synthesisToSummary(result, workspace.project.id, 'chapter', chapter.id, text)); await this.repository.saveManuscriptAnalysisArtifacts(this.jobId, [{ jobId: this.jobId, projectId: workspace.project.id, phase: 'scene_or_chapter_synthesis' as const, artifactType: 'narrative_summary' as const, artifactId: summary.id, reviewStatus: 'pending' as const, explicitlySkipped: false }]); progress.completedUnits += 1; progress.lastSuccessfulUnitId = chapter.id; progress.actualProvider = provider.id; progress.updatedAt = new Date().toISOString(); await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: 'scene_or_chapter_synthesis', phaseProgress: { ...(await this.repository.getManuscriptAnalysisJob(job.id)).phaseProgress, scene_or_chapter_synthesis: progress } }); }
  }

  private async phaseInput(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>): Promise<ManuscriptPhaseInput> {
    const chapters = [...workspace.chapters].sort((a, b) => a.orderIndex - b.orderIndex);
    const [chapterSummaries, findings, threads, rules, ledger] = await Promise.all([this.repository.listNarrativeSummaries(workspace.project.id, 'chapter'), this.repository.listContinuityReviewFindings(workspace.project.id), this.repository.listPlotThreadLifecycleProposals(workspace.project.id), this.repository.listProjectRules(workspace.project.id), this.repository.listContinuityStateLedger(workspace.project.id)]);
    return { projectId: workspace.project.id, bookId: job.bookId, chapters: chapters.map((chapter) => ({ id: chapter.id, title: chapter.title, orderIndex: chapter.orderIndex, text: chapterText(chapter) })), chapterSummaries, confirmedEntities: workspace.entities.filter((entity) => entity.status === 'confirmed' && entity.authorConfirmed), confirmedRules: rules.filter((rule) => rule.status === 'confirmed' && rule.authorConfirmed), confirmedStates: ledger.filter((entry) => entry.status === 'confirmed' && entry.authorConfirmed), proposedFindings: findings.filter((finding) => finding.reviewStatus === 'open'), proposedThreads: threads.filter((thread) => thread.reviewStatus === 'pending'), contentHash: contentHash(chapters.map(chapterText).join('\n\n')) };
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
  const [job, units, draftLedger, phaseResults, artifacts] = await Promise.all([repository.getManuscriptAnalysisJob(jobId), repository.listManuscriptAnalysisUnits(jobId), repository.listManuscriptAnalysisDraftLedger(jobId), repository.listManuscriptAnalysisPhaseResults(jobId), repository.listManuscriptAnalysisArtifacts(jobId)]);
  return { job, units, draftLedger, phaseResults, artifacts };
}
