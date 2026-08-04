import type { Chapter, ContinuityStateLedgerEntry, CreateManuscriptAnalysisJobInput, ManuscriptAnalysisDraftLedgerEntry, ManuscriptAnalysisJob, ManuscriptAnalysisUnit } from '../types/domain';
import type { StoryRepository } from './storyRepository';
import { contentHash } from '../utils/aiText';
import { editorContentToPlainText } from '../utils/editorContent';
import { providerRouter, type StoryAiProvider as Provider } from './aiProviderService';
import { runContinuityReview } from './continuityReview';

const activeJobs = new Map<string, Promise<void>>();

function draftEntryToLedger(entry: ManuscriptAnalysisDraftLedgerEntry): ContinuityStateLedgerEntry {
  return { id: entry.id, projectId: entry.projectId, entityId: entry.entityId, relatedEntityId: entry.relatedEntityId, stateKind: entry.stateKind, previousState: entry.previousState, newState: entry.newState, chapterId: entry.chapterId, sceneId: entry.sceneId, startOffset: entry.startOffset, endOffset: entry.endOffset, status: 'proposed', confidence: entry.confidence, authorConfirmed: false, createdAt: entry.createdAt, updatedAt: entry.updatedAt };
}

function errorText(error: unknown): string { return error instanceof Error ? error.message : String(error); }

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
    if (activeJobs.size > 0) throw new Error('Eine andere Manuskript-Continuity-Analyse läuft bereits.');
    this.paused = false;
    this.cancelled = false;
    this.runPromise = this.execute();
    activeJobs.set(this.jobId, this.runPromise);
    void this.runPromise.then(() => { if (activeJobs.get(this.jobId) === this.runPromise) activeJobs.delete(this.jobId); }, () => { if (activeJobs.get(this.jobId) === this.runPromise) activeJobs.delete(this.jobId); });
    return this.runPromise;
  }

  async pause(): Promise<void> {
    this.paused = true;
    if (!this.runPromise) await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'paused' });
  }

  async cancel(): Promise<void> {
    this.cancelled = true;
    await this.currentProvider?.cancelActive();
    if (!this.runPromise) await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'cancelled', errorMessage: 'Analyse wurde abgebrochen.' });
  }

  async retryFailed(): Promise<void> {
    const units = await this.repository.listManuscriptAnalysisUnits(this.jobId);
    for (const unit of units.filter((item) => item.status === 'failed')) {
      await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'pending', retryCount: unit.retryCount + 1, errorMessage: undefined, continuityRunId: undefined });
    }
    await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'pending', errorMessage: undefined });
    return this.start();
  }

  private async execute(): Promise<void> {
    const job = await this.repository.getManuscriptAnalysisJob(this.jobId);
    if (['completed', 'cancelled'].includes(job.status)) return;
    const active = this.providerOverride ? { provider: this.providerOverride, settings: await providerRouter.getSettings() } : await providerRouter.getActiveProvider();
    this.currentProvider = active.provider;
    await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'running', errorMessage: undefined });
    const workspace = await this.repository.loadWorkspace();
    const units = (await this.repository.listManuscriptAnalysisUnits(this.jobId)).sort((a, b) => a.orderIndex - b.orderIndex);
    for (let index = 0; index < units.length; index += 1) {
      const unit = units[index];
      if (this.cancelled) { await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'cancelled', currentUnitId: undefined, errorMessage: 'Analyse wurde abgebrochen.' }); return; }
      if (this.paused) { await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'paused', currentUnitId: undefined }); return; }
      const chapter = workspace.chapters.find((item) => item.id === unit.chapterId);
      const scene = chapter?.scenes.find((item) => item.id === unit.sceneId);
      if (!chapter || !scene) {
        const message = 'Kapitel oder Szene der Prüfeinheit wurde nicht gefunden.';
        await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'failed', errorMessage: message });
        await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'failed', currentUnitId: unit.id, errorMessage: message });
        throw new Error(message);
      }
      const sceneText = editorContentToPlainText(scene.content);
      const currentContent = Array.from(sceneText).slice(unit.startOffset, unit.endOffset).join('');
      if (unit.status === 'completed' || unit.status === 'skipped') {
        if (contentHash(currentContent) === unit.contentHash) continue;
        await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'pending', content: currentContent, contentHash: contentHash(currentContent), errorMessage: undefined, continuityRunId: undefined });
      }
      if (unit.status === 'failed') {
        await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'failed', currentUnitId: unit.id, errorMessage: unit.errorMessage ?? 'Eine Prüfeinheit ist fehlgeschlagen.' });
        return;
      }
      await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'running', currentUnitId: unit.id, errorMessage: undefined });
      await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'running', errorMessage: undefined });
      const allDraftEntries = await this.repository.listManuscriptAnalysisDraftLedger(this.jobId);
      const draftLedger = allDraftEntries.filter((entry) => entry.unitId !== unit.id).map(draftEntryToLedger);
      const previous = units[index - 1];
      const following = units[index + 1];
      try {
        const result = await runContinuityReview(this.repository, { project: workspace.project, chapter, scene, currentText: currentContent, previousText: previous?.content, followingText: following?.content, sourceKind: unit.pageNumber === undefined ? 'word_threshold' : 'page_marker', startOffset: unit.startOffset, endOffset: unit.endOffset, draftLedger, provider: active.provider, persistStateProposals: false, isCancelled: () => this.cancelled, forceAnalysis: true });
        const draftEntries = result.draftStateChanges.map((change) => ({ jobId: this.jobId, unitId: unit.id, projectId: workspace.project.id, entityId: change.entityId, relatedEntityId: change.relatedEntityId, stateKind: change.stateKind, previousState: change.previousState, newState: change.newState, chapterId: unit.chapterId, sceneId: unit.sceneId, startOffset: change.startOffset ?? unit.startOffset, endOffset: change.endOffset ?? unit.endOffset, confidence: change.confidence, status: 'proposed' as const }));
        await this.repository.replaceManuscriptAnalysisDraftLedger(unit.id, draftEntries);
        await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'completed', continuityRunId: result.runId, content: currentContent, contentHash: contentHash(currentContent), errorMessage: undefined });
        const updated = await this.repository.getManuscriptAnalysisJob(this.jobId);
        await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: index === units.length - 1 || updated.completedUnits + 1 >= updated.totalUnits ? 'completed' : 'running', currentUnitId: undefined, errorMessage: undefined });
      } catch (error) {
        if (this.cancelled) {
          await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'pending', errorMessage: 'Analyse wurde abgebrochen.' });
          await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'cancelled', currentUnitId: undefined, errorMessage: 'Analyse wurde abgebrochen.' });
          return;
        }
        const message = errorText(error);
        await this.repository.updateManuscriptAnalysisUnit({ id: unit.id, status: 'failed', errorMessage: message });
        await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'failed', currentUnitId: unit.id, errorMessage: message });
        throw error;
      }
    }
    await this.repository.updateManuscriptAnalysisJob({ id: this.jobId, status: 'completed', currentUnitId: undefined, errorMessage: undefined });
  }
}

export function createManuscriptAnalysisUnits(chapters: Chapter[], unitsByChapter: Array<Array<{ text: string; startOffset: number; endOffset: number; page?: number }>>, scenes: Array<{ id: string; chapterId: string }>): CreateManuscriptAnalysisJobInput['units'] {
  let orderIndex = 0;
  return chapters.flatMap((chapter, chapterIndex) => {
    const scene = scenes.find((candidate) => candidate.chapterId === chapter.id);
    if (!scene) return [];
    return (unitsByChapter[chapterIndex] ?? []).map((unit) => ({ chapterId: chapter.id, sceneId: scene.id, orderIndex: orderIndex++, pageNumber: unit.page, startOffset: unit.startOffset, endOffset: unit.endOffset, content: unit.text, contentHash: contentHash(unit.text) }));
  });
}

export async function loadManuscriptAnalysisProgress(repository: StoryRepository, jobId: string): Promise<ManuscriptAnalysisProgress> {
  const [job, units, draftLedger] = await Promise.all([repository.getManuscriptAnalysisJob(jobId), repository.listManuscriptAnalysisUnits(jobId), repository.listManuscriptAnalysisDraftLedger(jobId)]);
  return { job, units, draftLedger };
}
