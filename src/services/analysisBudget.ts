import type { Chapter, ManuscriptAnalysisDraftLedgerEntry, NarrativeSummary, PersistentTimelineEvent, PlotThreadLifecycleProposal, ProjectRule, StoryEntity, StorySourceReference, ContinuityReviewFinding, ContinuityStateLedgerEntry } from '../types/domain';
import { editorContentToPlainText } from '../utils/editorContent';

export const MANUSCRIPT_PHASE_CONTEXT_BUDGET = 60000;
const CHAPTER_TEXT_BUDGET = 12000;
const MAX_SOURCE_REFERENCES = 160;
const MAX_TIMELINE_EVENTS = 160;

export interface HierarchicalPhaseContext {
  chapters: Array<{ id: string; title: string; orderIndex: number; text: string }>;
  sceneContexts: Array<{ id: string; chapterId: string; orderIndex: number; title: string; summary?: string; sourceReferenceIds: string[] }>;
  chapterSummaries: NarrativeSummary[];
  sourceReferences: StorySourceReference[];
  timelineEvents: PersistentTimelineEvent[];
  draftLedger: ManuscriptAnalysisDraftLedgerEntry[];
  confirmedEntities: StoryEntity[];
  confirmedRules: ProjectRule[];
  confirmedStates: ContinuityStateLedgerEntry[];
  proposedFindings: ContinuityReviewFinding[];
  proposedThreads: PlotThreadLifecycleProposal[];
  warnings: string[];
  budget: { maxCodePoints: number; includedCodePoints: number; truncatedSections: string[] };
}

function codePoints(value: unknown): number {
  return Array.from(typeof value === 'string' ? value : JSON.stringify(value) ?? '').length;
}

export function truncateUnicode(text: string, maxCodePoints: number): { value: string; truncated: boolean } {
  const chars = Array.from(text);
  if (chars.length <= maxCodePoints) return { value: text, truncated: false };
  return { value: chars.slice(0, Math.max(0, maxCodePoints)).join(''), truncated: true };
}

function capArray<T>(items: T[], max: number, section: string, warnings: string[]): T[] {
  if (items.length <= max) return items;
  warnings.push(`${section} wurde auf ${max} Einträge begrenzt.`);
  return items.slice(0, max);
}

function capTextFields<T extends object>(items: T[], field: keyof T, maxCodePoints: number, section: string, warnings: string[]): T[] {
  return items.map((item) => {
    const value = item[field];
    if (typeof value !== 'string') return item;
    const truncated = truncateUnicode(value, maxCodePoints);
    if (truncated.truncated) warnings.push(`${section} enthält gekürzte Textbelege.`);
    return { ...item, [field]: truncated.value } as T;
  });
}

export function buildHierarchicalPhaseContext(input: {
  chapters: Chapter[];
  chapterSummaries: NarrativeSummary[];
  sourceReferences: StorySourceReference[];
  timelineEvents: PersistentTimelineEvent[];
  draftLedger: ManuscriptAnalysisDraftLedgerEntry[];
  confirmedEntities: StoryEntity[];
  confirmedRules: ProjectRule[];
  confirmedStates: ContinuityStateLedgerEntry[];
  proposedFindings: ContinuityReviewFinding[];
  proposedThreads: PlotThreadLifecycleProposal[];
  maxCodePoints?: number;
}): HierarchicalPhaseContext {
  const maxCodePoints = input.maxCodePoints ?? MANUSCRIPT_PHASE_CONTEXT_BUDGET;
  const warnings: string[] = [];
  const truncatedSections: string[] = [];
  const chapters = [...input.chapters].sort((a, b) => a.orderIndex - b.orderIndex).map((chapter) => {
    const text = chapter.scenes.map((scene) => editorContentToPlainText(scene.content)).join('\n\n');
    const result = truncateUnicode(text, CHAPTER_TEXT_BUDGET);
    if (result.truncated) {
      truncatedSections.push(`chapter:${chapter.id}`);
      warnings.push(`Kapitel „${chapter.title}“ wurde für den Providerkontext gekürzt.`);
    }
    return { id: chapter.id, title: chapter.title, orderIndex: chapter.orderIndex, text: result.value };
  });
  const chapterOrder = new Map(chapters.map((chapter, index) => [chapter.id, index]));
  const sceneOrder = new Map(input.chapters.flatMap((chapter) => chapter.scenes.map((scene) => [scene.id, scene.orderIndex] as const)));
  const position = (chapterId?: string, sceneId?: string, offset?: number) => [chapterOrder.get(chapterId ?? '') ?? Number.MAX_SAFE_INTEGER, sceneOrder.get(sceneId ?? '') ?? Number.MAX_SAFE_INTEGER, offset ?? 0] as const;
  const comparePosition = (left: readonly number[], right: readonly number[]) => left[0] - right[0] || left[1] - right[1] || left[2] - right[2];
  const sceneContexts = chapters.flatMap((chapter) => input.chapters.find((item) => item.id === chapter.id)?.scenes.map((scene) => ({
    id: scene.id,
    chapterId: chapter.id,
    orderIndex: scene.orderIndex,
    title: scene.title,
    summary: truncateUnicode(editorContentToPlainText(scene.content), 900).value,
    sourceReferenceIds: input.sourceReferences.filter((source) => source.sceneId === scene.id).map((source) => source.id).slice(0, 20),
  })) ?? []);
  const chapterSummaries = capTextFields(capArray([...input.chapterSummaries].sort((a, b) => comparePosition(position(a.scopeId), position(b.scopeId))), 120, 'Kapitelzusammenfassungen', warnings), 'summary', 1800, 'Kapitelzusammenfassungen', warnings);
  const sourceReferences = capTextFields(capArray([...input.sourceReferences].sort((a, b) => comparePosition(position(a.chapterId, a.sceneId, a.startOffset), position(b.chapterId, b.sceneId, b.startOffset))), MAX_SOURCE_REFERENCES, 'Quellen', warnings), 'excerpt', 700, 'Quellen', warnings);
  const timelineEvents = capTextFields(capArray([...input.timelineEvents].sort((a, b) => comparePosition(position(a.chapterId, a.sceneId, a.temporalOrder), position(b.chapterId, b.sceneId, b.temporalOrder))), MAX_TIMELINE_EVENTS, 'Timeline-Ereignisse', warnings), 'summary', 700, 'Timeline-Ereignisse', warnings);
  const draftLedger = capTextFields([...input.draftLedger].sort((a, b) => comparePosition(position(a.chapterId, a.sceneId, a.startOffset), position(b.chapterId, b.sceneId, b.startOffset))).slice(0, 240), 'sourceExcerpt', 700, 'Draft-Ledger', warnings);
  if (input.draftLedger.length > draftLedger.length) warnings.push('Draft-Ledger wurde auf die frühesten 240 Vorschläge begrenzt.');
  const proposedFindings = capTextFields(capArray([...input.proposedFindings], 160, 'Findings', warnings), 'objectiveConflict', 700, 'Findings', warnings);
  const proposedThreads = capTextFields(capArray([...input.proposedThreads], 120, 'Handlungsstränge', warnings), 'evidenceExcerpt', 700, 'Handlungsstränge', warnings);
  const context: HierarchicalPhaseContext = { chapters, sceneContexts, chapterSummaries, sourceReferences, timelineEvents, draftLedger, confirmedEntities: capArray(input.confirmedEntities, 240, 'bestätigte Entitäten', warnings), confirmedRules: capArray(input.confirmedRules, 160, 'bestätigte Regeln', warnings), confirmedStates: capArray(input.confirmedStates, 320, 'bestätigte Zustände', warnings), proposedFindings, proposedThreads, warnings, budget: { maxCodePoints, includedCodePoints: 0, truncatedSections } };
  const payloadSize = () => codePoints({ ...context, budget: undefined });
  let includedCodePoints = payloadSize();
  if (includedCodePoints > maxCodePoints) {
    warnings.push(`Der Providerkontext wurde auf das harte Budget von ${maxCodePoints} Unicode-Codepoints gekürzt.`);
    truncatedSections.push('total-context-budget');
    const trim = (text: string, amount: number) => truncateUnicode(text, Math.max(0, amount)).value;
    const trimCollections = (factor: number) => {
      context.chapters = context.chapters.map((chapter) => ({ ...chapter, text: trim(chapter.text, Math.floor(Array.from(chapter.text).length * factor)) }));
      context.sceneContexts = context.sceneContexts.map((item) => ({ ...item, summary: item.summary ? trim(item.summary, Math.floor(Array.from(item.summary).length * factor)) : item.summary }));
      context.chapterSummaries = context.chapterSummaries.map((item) => ({ ...item, summary: trim(item.summary, Math.floor(Array.from(item.summary).length * factor)) }));
      context.sourceReferences = context.sourceReferences.map((item) => ({ ...item, excerpt: trim(item.excerpt, Math.floor(Array.from(item.excerpt).length * factor)) }));
      context.timelineEvents = context.timelineEvents.map((item) => ({ ...item, summary: trim(item.summary, Math.floor(Array.from(item.summary).length * factor)) }));
      context.draftLedger = context.draftLedger.map((item) => ({ ...item, sourceExcerpt: item.sourceExcerpt ? trim(item.sourceExcerpt, Math.floor(Array.from(item.sourceExcerpt).length * factor)) : item.sourceExcerpt }));
    };
    trimCollections(Math.min(0.75, (maxCodePoints / Math.max(1, includedCodePoints)) * 0.5));
    includedCodePoints = payloadSize();
    const arrays: Array<keyof HierarchicalPhaseContext> = ['draftLedger', 'sourceReferences', 'timelineEvents', 'sceneContexts', 'chapterSummaries', 'proposedFindings', 'proposedThreads', 'confirmedStates', 'confirmedRules', 'confirmedEntities', 'chapters'];
    for (const key of arrays) {
      while (includedCodePoints > maxCodePoints && Array.isArray(context[key]) && (context[key] as unknown[]).length > 0) {
        (context[key] as unknown[]).pop();
        includedCodePoints = payloadSize();
      }
      if (includedCodePoints <= maxCodePoints) break;
    }
    if (includedCodePoints > maxCodePoints) { context.warnings = []; context.chapters = []; context.sceneContexts = []; context.chapterSummaries = []; context.sourceReferences = []; context.timelineEvents = []; context.draftLedger = []; context.confirmedEntities = []; context.confirmedRules = []; context.confirmedStates = []; context.proposedFindings = []; context.proposedThreads = []; includedCodePoints = payloadSize(); }
  }
  context.warnings = [...new Set(warnings)];
  includedCodePoints = payloadSize();
  while (includedCodePoints > maxCodePoints && context.warnings.length > 0) { context.warnings.pop(); includedCodePoints = payloadSize(); }
  context.budget = { maxCodePoints, includedCodePoints, truncatedSections: [...new Set(truncatedSections)] };
  while (codePoints(context) > maxCodePoints && context.warnings.length > 0) context.warnings.pop();
  context.budget.includedCodePoints = codePoints(context);
  return context;
}
