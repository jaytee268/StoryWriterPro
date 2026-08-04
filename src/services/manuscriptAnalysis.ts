import type { Chapter, ContinuityStateLedgerEntry, CreateManuscriptAnalysisJobInput, ManuscriptAnalysisDraftLedgerEntry, ManuscriptAnalysisJob, ManuscriptAnalysisPhase, ManuscriptAnalysisPhaseProgress, ManuscriptAnalysisUnit, ManuscriptSynthesisResult, NarrativeSummaryAnalysisResult } from '../types/domain';
import type { StoryRepository } from './storyRepository';
import { contentHash } from '../utils/aiText';
import { editorContentToPlainText } from '../utils/editorContent';
import { providerRouter, type StoryAiProvider as Provider } from './aiProviderService';
import { runContinuityReview } from './continuityReview';
import { DeterministicProjectContextBuilder } from './contextBuilder';

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

export interface ManuscriptAnalysisProgress { job: ManuscriptAnalysisJob; units: ManuscriptAnalysisUnit[]; draftLedger: ManuscriptAnalysisDraftLedgerEntry[]; }

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

  async retryFailed(): Promise<void> {
    const job = await this.repository.getManuscriptAnalysisJob(this.jobId);
    const units = await this.repository.listManuscriptAnalysisUnits(this.jobId);
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

  private async checkControl(job: ManuscriptAnalysisJob): Promise<boolean> {
    if (this.cancelled) { await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'cancelled', currentUnitId: undefined, errorMessage: 'Analyse wurde abgebrochen.' }); return false; }
    if (this.paused) { await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'paused', currentUnitId: undefined }); return false; }
    return true;
  }

  private async execute(): Promise<void> {
    let job = await this.repository.getManuscriptAnalysisJob(this.jobId);
    if (job.status === 'cancelled' || (job.status === 'completed' && job.currentPhase === 'completed')) return;
    const active = this.providerOverride ? { provider: this.providerOverride, settings: await providerRouter.getSettings() } : await providerRouter.getActiveProvider();
    this.currentProvider = active.provider;
    const workspace = await this.repository.loadWorkspace();
    const units = (await this.repository.listManuscriptAnalysisUnits(this.jobId)).sort((a, b) => a.orderIndex - b.orderIndex);
    const chapters = [...workspace.chapters].sort((a, b) => a.orderIndex - b.orderIndex);
    job = await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: job.currentPhase, errorMessage: undefined });

    for (const phase of PHASES) {
      if (phase === 'completed' || phaseIndex(phase) < phaseIndex(job.currentPhase)) continue;
      if (!await this.checkControl(job)) return;
      try {
        job = await this.savePhase(job, phase, { status: 'running', requestedProvider: active.provider.id, errorCode: undefined, errorMessage: undefined }, 'running');
        if (phase === 'structure') await this.runStructure(job, chapters, units);
        else if (phase === 'passage_continuity') await this.runContinuity(job, workspace, units, active.provider);
        else if (phase === 'bible_extraction') await this.runBible(job, workspace, units, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'character_memory') await this.runCharacterMemory(job, workspace, units, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'scene_or_chapter_synthesis') await this.runChapterSynthesis(job, workspace, chapters, active.provider, active.settings.bibleUpdateTimeoutSeconds);
        else if (phase === 'narrative_summaries') await this.runBookSynthesis(job, workspace, active.provider, active.settings.bibleUpdateTimeoutSeconds, 'narrative_summaries');
        else if (phase === 'plot_thread_synthesis') await this.runBookSynthesis(job, workspace, active.provider, active.settings.bibleUpdateTimeoutSeconds, 'plot_thread_synthesis');
        else if (phase === 'book_end_state') await this.runBookSynthesis(job, workspace, active.provider, active.settings.bibleUpdateTimeoutSeconds, 'book_end_state');
        else if (phase === 'global_countercheck') await this.runBookSynthesis(job, workspace, active.provider, active.settings.bibleUpdateTimeoutSeconds, 'global_countercheck');
        job = await this.repository.getManuscriptAnalysisJob(this.jobId);
        job = await this.savePhase(job, phase, { status: 'completed', failedUnits: 0, actualProvider: active.provider.id }, 'running');
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

  private async runStructure(job: ManuscriptAnalysisJob, chapters: Chapter[], units: ManuscriptAnalysisUnit[]): Promise<void> {
    if (!chapters.length || units.length === 0) throw new Error('Das Manuskript enthält keine analysierbaren Kapitelprüfeinheiten.');
    if (chapters.some((chapter) => chapter.scenes.length === 0)) throw new Error('Ein Kapitel besitzt keine implizite Importszene. Die Struktur muss zuerst repariert werden.');
    await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', phaseProgress: { ...job.phaseProgress, structure: { ...(job.phaseProgress.structure ?? this.emptyProgress('structure', chapters.length, job.providerId)), status: 'completed', totalUnits: chapters.length, completedUnits: chapters.length, updatedAt: new Date().toISOString() } } });
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
        const allDraftEntries = await this.repository.listManuscriptAnalysisDraftLedger(this.jobId);
        const draftLedger = allDraftEntries.filter((entry) => entry.unitId !== unit.id).map(draftEntryToLedger);
        const previous = units[index - 1]; const following = units[index + 1];
        const result = await runContinuityReview(this.repository, { project: workspace.project, chapter, scene, currentText: currentContent, previousText: previous?.content, followingText: following?.content, sourceKind: unit.pageNumber === undefined ? 'word_threshold' : 'page_marker', startOffset: unit.startOffset, endOffset: unit.endOffset, draftLedger, provider, persistStateProposals: false, isCancelled: () => this.cancelled, forceAnalysis: true });
        const draftEntries = result.draftStateChanges.map((change) => ({ jobId: this.jobId, unitId: unit.id, projectId: workspace.project.id, entityId: change.entityId, relatedEntityId: change.relatedEntityId, stateKind: change.stateKind, previousState: change.previousState, newState: change.newState, chapterId: unit.chapterId, sceneId: unit.sceneId, startOffset: change.startOffset ?? unit.startOffset, endOffset: change.endOffset ?? unit.endOffset, sourceExcerpt: change.evidenceExcerpt, confidence: change.confidence, status: 'proposed' as const }));
        await this.repository.replaceManuscriptAnalysisDraftLedger(unit.id, draftEntries);
        await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'completed', continuityRunId: result.runId, actualProvider: provider.id, outputHash: contentHash(JSON.stringify(result)), inputHash: currentHash, content: currentContent, contentHash: currentHash, errorMessage: undefined, errorCode: undefined });
        progress.completedUnits += 1; progress.lastSuccessfulUnitId = unit.id; progress.updatedAt = new Date().toISOString();
        await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'running', currentPhase: 'passage_continuity', currentUnitId: undefined, lastSuccessfulUnitId: unit.id, phaseProgress: { ...(await this.repository.getManuscriptAnalysisJob(this.jobId)).phaseProgress, passage_continuity: progress } });
      } catch (error) {
        if (this.cancelled || this.paused) throw error;
        const message = errorText(error); await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'failed', actualProvider: provider.id, errorCode: errorCode(error), errorMessage: message }); throw error;
      }
    }
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
      await this.repository.saveBibleProposals(run.id, result.proposals, workspace.project.id, scene.id); await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: unit.status, actualProvider: provider.id, outputHash: contentHash(JSON.stringify(result)) }); progress.completedUnits += 1; progress.lastSuccessfulUnitId = unit.id; progress.actualProvider = provider.id; progress.updatedAt = new Date().toISOString(); await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: 'bible_extraction', phaseProgress: { ...(await this.repository.getManuscriptAnalysisJob(job.id)).phaseProgress, bible_extraction: progress } });
    }
  }

  private async runCharacterMemory(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, units: ManuscriptAnalysisUnit[], provider: Provider, timeout: number): Promise<void> {
    const progress = job.phaseProgress.character_memory ?? this.emptyProgress('character_memory', units.length, provider.id); const start = progress.lastSuccessfulUnitId ? Math.max(0, units.findIndex((unit) => unit.id === progress.lastSuccessfulUnitId) + 1) : 0; const contextBuilder = new DeterministicProjectContextBuilder(this.repository);
    for (let index = start; index < units.length; index += 1) { if (!await this.checkControl(job)) return; const unit = units[index]; const chapter = workspace.chapters.find((item) => item.id === unit.chapterId); const scene = chapter?.scenes.find((item) => item.id === unit.sceneId); if (!chapter || !scene) throw new Error('Kapitel oder Szene der Character-Memory-Einheit wurde nicht gefunden.'); await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: unit.status, requestedProvider: provider.id, promptVersion: PROMPT_VERSION, inputHash: unit.contentHash, errorMessage: undefined, errorCode: undefined }); const context = await contextBuilder.build({ projectId: workspace.project.id, currentChapterId: chapter.id, currentSceneId: scene.id, userQuestion: editorContentToPlainText(scene.content), includeProposedSummaries: true }); const run = await this.repository.createCharacterMemoryUpdateRun({ projectId: workspace.project.id, sceneId: scene.id, contentHash: unit.contentHash, extractorId: provider.id, analyzedContent: unit.content }); const characters = workspace.entities.filter((entity) => entity.type === 'character'); const result = await provider.extractCharacterMemoryPatch({ project: workspace.project, chapter, scene, characters, existingEntities: workspace.entities, context, changedRange: { start: unit.startOffset, end: unit.endOffset } }, timeout); await this.repository.saveCharacterMemoryProposals(run.id, result.proposals); await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: unit.status, actualProvider: provider.id, outputHash: contentHash(JSON.stringify(result)) }); progress.completedUnits += 1; progress.lastSuccessfulUnitId = unit.id; progress.actualProvider = provider.id; progress.updatedAt = new Date().toISOString(); await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: 'character_memory', phaseProgress: { ...(await this.repository.getManuscriptAnalysisJob(job.id)).phaseProgress, character_memory: progress } }); }
  }

  private async runChapterSynthesis(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, chapters: Chapter[], provider: Provider, timeout: number): Promise<void> {
    const progress = job.phaseProgress.scene_or_chapter_synthesis ?? this.emptyProgress('scene_or_chapter_synthesis', chapters.length, provider.id); const start = progress.lastSuccessfulUnitId ? Math.max(0, chapters.findIndex((chapter) => chapter.id === progress.lastSuccessfulUnitId) + 1) : 0;
    for (let index = start; index < chapters.length; index += 1) { if (!await this.checkControl(job)) return; const chapter = chapters[index]; const text = chapterText(chapter); const result = await provider.summarize('chapter', chapter.id, `Führe eine strukturierte Kapitel-Synthese durch. Bewerte wichtige Ereignisse, Wissen, Beziehungen, offene Threads und Endzustände.\n\n${text}`, timeout); await this.repository.saveNarrativeSummary(synthesisToSummary(result, workspace.project.id, 'chapter', chapter.id, text)); progress.completedUnits += 1; progress.lastSuccessfulUnitId = chapter.id; progress.actualProvider = provider.id; progress.updatedAt = new Date().toISOString(); await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: 'scene_or_chapter_synthesis', phaseProgress: { ...(await this.repository.getManuscriptAnalysisJob(job.id)).phaseProgress, scene_or_chapter_synthesis: progress } }); }
  }

  private async runBookSynthesis(job: ManuscriptAnalysisJob, workspace: Awaited<ReturnType<StoryRepository['loadWorkspace']>>, provider: Provider, timeout: number, phase: ManuscriptAnalysisPhase): Promise<void> {
    const chapters = [...workspace.chapters].sort((a, b) => a.orderIndex - b.orderIndex); const text = chapters.map(chapterText).join('\n\n'); const result = await provider.summarize('book', job.bookId, `Analyse diese Buchphase: ${phase}. Keine Kanonänderung vornehmen; alle Ergebnisse bleiben proposed.\n\n${text}`, timeout); const summary = synthesisToSummary(result, workspace.project.id, 'book', job.bookId, `${phase}\n${text}`); await this.repository.saveNarrativeSummary(summary); const progress = job.phaseProgress[phase] ?? this.emptyProgress(phase, 1, provider.id); progress.completedUnits = 1; progress.totalUnits = 1; progress.lastSuccessfulUnitId = job.bookId; progress.actualProvider = provider.id; progress.updatedAt = new Date().toISOString(); await this.repository.updateManuscriptAnalysisJob({ id: job.id, status: 'running', currentPhase: phase, phaseProgress: { ...(await this.repository.getManuscriptAnalysisJob(job.id)).phaseProgress, [phase]: progress } });
  }
}

export function createManuscriptAnalysisUnits(chapters: Chapter[], unitsByChapter: Array<Array<{ text: string; startOffset: number; endOffset: number; page?: number }>>, scenes: Array<{ id: string; chapterId: string }>): CreateManuscriptAnalysisJobInput['units'] {
  let orderIndex = 0;
  return [...chapters].sort((a, b) => a.orderIndex - b.orderIndex).flatMap((chapter, chapterIndex) => { const scene = scenes.find((candidate) => candidate.chapterId === chapter.id); if (!scene) return []; return (unitsByChapter[chapterIndex] ?? []).map((unit) => ({ chapterId: chapter.id, sceneId: scene.id, orderIndex: orderIndex++, pageNumber: unit.page, startOffset: unit.startOffset, endOffset: unit.endOffset, content: unit.text, contentHash: contentHash(unit.text) })); });
}

export async function loadManuscriptAnalysisProgress(repository: StoryRepository, jobId: string): Promise<ManuscriptAnalysisProgress> {
  const [job, units, draftLedger] = await Promise.all([repository.getManuscriptAnalysisJob(jobId), repository.listManuscriptAnalysisUnits(jobId), repository.listManuscriptAnalysisDraftLedger(jobId)]);
  return { job, units, draftLedger };
}
