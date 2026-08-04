import type { Chapter, ContinuityReviewFinding, ContinuityReviewSourceKind, Project, SaveContinuityFindingInput, StoryEntity, ContinuityStateLedgerEntry, ProjectRule, Scene } from '../types/domain';
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
  sourceKind: ContinuityReviewSourceKind;
  startOffset?: number;
  endOffset?: number;
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
const hasAny = (text: string, values: string[]): boolean => values.some((value) => lower(text).includes(value));

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

function activeStateFor(entityId: string, stateKind: ContinuityStateLedgerEntry['stateKind'], ledger: ContinuityStateLedgerEntry[], chapters: Chapter[], chapter?: Chapter, scene?: Scene, offset?: number): ContinuityStateLedgerEntry | undefined {
  return ledger.filter((entry) => entry.entityId === entityId && entry.stateKind === stateKind && entry.status === 'confirmed' && entry.authorConfirmed && !isFuture(entry, chapters, chapter, scene, offset)).sort((a, b) => { const pa = positionFor(a, chapters); const pb = positionFor(b, chapters); return pb[0] - pa[0] || pb[1] - pa[1] || pb[2] - pa[2]; })[0];
}

function loreExplanationFor(entityId: string | undefined, rules: ProjectRule[]): ProjectRule[] {
  if (!entityId) return [];
  return rules.filter((rule) => rule.status === 'confirmed' && rule.authorConfirmed && rule.connectedLoreIds.includes(entityId));
}

export function detectContinuityFindings(input: ContinuityReviewRequest & { chapters: Chapter[]; entities: StoryEntity[]; ledger: ContinuityStateLedgerEntry[]; rules: ProjectRule[] }): SaveContinuityFindingInput[] {
  const changed = incrementalWordRange(input.previousText, input.currentText);
  const excerpt = Array.from(input.currentText).slice(input.startOffset ?? changed.start, input.endOffset ?? changed.end).join('').trim();
  const findings: SaveContinuityFindingInput[] = [];
  for (const entity of input.entities) {
    if (!mentions(input.currentText, entity)) continue;
    const unavailable = activeStateFor(entity.id, 'item_availability', input.ledger, input.chapters, input.chapter, input.scene, input.startOffset) ?? activeStateFor(entity.id, 'item_existence', input.ledger, input.chapters, input.chapter, input.scene, input.startOffset);
    if (unavailable && hasAny(unavailable.newState, ['weggeworfen', 'entsorgt', 'nicht verfügbar', 'nicht vorhanden', 'verloren', 'zerstört']) && hasAny(input.currentText, ['zeigt', 'zeigt den', 'gibt', 'hält', 'nimmt', 'findet', 'öffnet'])) {
      const explanations = loreExplanationFor(entity.id, input.rules);
      findings.push({ runId: '', projectId: input.project.id, chapterId: input.chapter?.id, sceneId: input.scene?.id, findingType: explanations.length ? 'lore_compatible_anomaly' : 'critical_contradiction', severity: explanations.length ? 'warning' : 'critical', subjectEntityId: entity.id, relatedEntityIds: [entity.id], relatedStateIds: [unavailable.id], relatedRuleIds: explanations.map((rule) => rule.id), objectiveConflict: `Der aktuelle Text verwendet „${entity.name}“, obwohl der letzte bestätigte Zustand „${unavailable.newState}“ lautet.`, loreExplanations: explanations.map((rule) => `${rule.title}: ${rule.statement}`), evidenceExcerpt: excerpt || input.currentText.slice(0, 240), startOffset: input.startOffset ?? changed.start, endOffset: input.endOffset ?? changed.end, reason: explanations.length ? 'Der objektive Konflikt bleibt bestehen; bestätigte Regeln liefern nur mögliche Erklärungen.' : 'Ein bestätigter Gegenstandszustand wird im neuen Abschnitt nicht erklärt.', reviewStatus: 'open' });
    }
    const condition = activeStateFor(entity.id, 'physical_condition', input.ledger, input.chapters, input.chapter, input.scene, input.startOffset);
    if (condition && hasAny(condition.newState, ['intolerant', 'allergisch', 'verträgt nicht', 'verletzt']) && hasAny(input.currentText, ['trinkt', 'isst', 'nimmt', 'berührt'])) {
      findings.push({ runId: '', projectId: input.project.id, chapterId: input.chapter?.id, sceneId: input.scene?.id, findingType: 'missing_explanation', severity: 'warning', subjectEntityId: entity.id, relatedEntityIds: [entity.id], relatedStateIds: [condition.id], relatedRuleIds: [], objectiveConflict: `Das Verhalten von ${entity.name} weicht vom bestätigten Zustand „${condition.newState}“ ab.`, loreExplanations: [], evidenceExcerpt: excerpt || input.currentText.slice(0, 240), startOffset: input.startOffset ?? changed.start, endOffset: input.endOffset ?? changed.end, reason: 'Der Text kann eine Ausnahme, eine besondere Ursache oder eine bewusste Abweichung enthalten; das ist nicht automatisch ein harter Widerspruch.', reviewStatus: 'open' });
    }
  }
  return findings;
}

export async function runContinuityReview(repository: StoryRepository, input: ContinuityReviewRequest): Promise<{ runId: string; findings: ContinuityReviewFinding[] }> {
  const workspace = await repository.loadWorkspace();
  const settings = await repository.getContinuityReviewSettings(input.project.id);
  const currentText = editorContentToPlainText(input.currentText);
  const previousText = input.previousText ? editorContentToPlainText(input.previousText) : undefined;
  if (!shouldRunContinuityReview(previousText, currentText, settings.wordThreshold, input.sourceKind)) return { runId: '', findings: [] };
  const hash = contentHash(currentText);
  const previousRun = (await repository.listContinuityReviewRuns(input.project.id, input.chapter?.id, input.scene?.id)).find((run) => run.sourceKind === input.sourceKind && run.contentHash === hash && run.status === 'completed');
  if (previousRun) return { runId: previousRun.id, findings: await repository.listContinuityReviewFindings(input.project.id, previousRun.id) };
  const [ledger, rules, entities] = await Promise.all([repository.listContinuityStateLedger(input.project.id), repository.listProjectRules(input.project.id, true), repository.listStoryEntities(input.project.id)]);
  const run = await repository.createContinuityReviewRun({ projectId: input.project.id, chapterId: input.chapter?.id, sceneId: input.scene?.id, sourceKind: input.sourceKind, contentHash: hash, startOffset: input.startOffset, endOffset: input.endOffset });
  const findings = detectContinuityFindings({ ...input, currentText, previousText, chapters: workspace.chapters, entities, ledger, rules }).map((finding) => ({ ...finding, runId: run.id }));
  const saved = findings.length ? await repository.saveContinuityReviewFindings(run.id, findings) : [];
  const lifecycle = await repository.listPlotThreadLifecycles(input.project.id);
  const changed = incrementalWordRange(previousText, currentText);
  const excerpt = Array.from(currentText).slice(input.startOffset ?? changed.start, input.endOffset ?? changed.end).join('').trim();
  for (const thread of entities.filter((entity) => entity.type === 'plot_thread' && mentions(currentText, entity))) {
    const current = lifecycle.find((item) => item.entityId === thread.id);
    if (current?.lifecycleStatus === 'resolved' || current?.lifecycleStatus === 'abandoned') continue;
    if (hasAny(currentText, ['gelöst', 'geklärt', 'abgeschlossen', 'beendet', 'erledigt'])) {
      await repository.savePlotThreadLifecycleProposal({ runId: run.id, projectId: input.project.id, entityId: thread.id, proposedStatus: 'closure_candidate', evidenceExcerpt: excerpt || input.currentText.slice(0, 240), startOffset: input.startOffset ?? changed.start, endOffset: input.endOffset ?? changed.end, reason: `Der Handlungsstrang „${thread.name}“ scheint in diesem Abschnitt einen möglichen Abschluss zu erreichen. Bitte als abgeschlossen, teilweise abgeschlossen oder offen entscheiden.` });
    }
  }
  return { runId: run.id, findings: saved };
}

export function canonicalText(scene: Scene): string { return editorContentToPlainText(scene.content); }

export function findingsToLongformReviews(findings: ContinuityReviewFinding[], jobId: string, sectionId?: string): Array<{ jobId: string; sectionId?: string; reviewScope: 'section'; issueType: 'canon' | 'lore' | 'knowledge' | 'character'; severity: 'info' | 'warning' | 'blocking'; title: string; description: string; relatedEntityIds: string[]; relatedSourceIds: string[]; suggestedAction: string; status: string }> {
  return findings.map((finding) => ({ jobId, sectionId, reviewScope: 'section', issueType: finding.findingType === 'missing_explanation' ? 'character' : finding.relatedRuleIds.length ? 'lore' : 'canon', severity: finding.severity === 'critical' ? 'blocking' : finding.severity, title: finding.objectiveConflict, description: `${finding.reason}${finding.loreExplanations.length ? ` Bestätigte mögliche Erklärung: ${finding.loreExplanations.join(' · ')}` : ''}`, relatedEntityIds: finding.relatedEntityIds, relatedSourceIds: [], suggestedAction: 'Autorentscheidung erforderlich', status: 'open' }));
}
