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
  const sceneContexts = input.chapters.flatMap((chapter) => chapter.scenes.map((scene) => ({
    id: scene.id,
    chapterId: chapter.id,
    orderIndex: scene.orderIndex,
    title: scene.title,
    summary: truncateUnicode(editorContentToPlainText(scene.content), 900).value,
    sourceReferenceIds: input.sourceReferences.filter((source) => source.sceneId === scene.id).map((source) => source.id).slice(0, 20),
  })));
  const chapterSummaries = capTextFields(capArray([...input.chapterSummaries], 120, 'Kapitelzusammenfassungen', warnings), 'summary', 1800, 'Kapitelzusammenfassungen', warnings);
  const sourceReferences = capTextFields(capArray([...input.sourceReferences], MAX_SOURCE_REFERENCES, 'Quellen', warnings), 'excerpt', 700, 'Quellen', warnings);
  const timelineEvents = capTextFields(capArray([...input.timelineEvents], MAX_TIMELINE_EVENTS, 'Timeline-Ereignisse', warnings), 'summary', 700, 'Timeline-Ereignisse', warnings);
  const draftLedger = capTextFields([...input.draftLedger].sort((a, b) => (a.chapterId + (a.startOffset ?? 0)).localeCompare(b.chapterId + (b.startOffset ?? 0))).slice(0, 240), 'sourceExcerpt', 700, 'Draft-Ledger', warnings);
  if (input.draftLedger.length > draftLedger.length) warnings.push('Draft-Ledger wurde auf die frühesten 240 Vorschläge begrenzt.');
  const proposedFindings = capTextFields(capArray([...input.proposedFindings], 160, 'Findings', warnings), 'objectiveConflict', 700, 'Findings', warnings);
  const proposedThreads = capTextFields(capArray([...input.proposedThreads], 120, 'Handlungsstränge', warnings), 'evidenceExcerpt', 700, 'Handlungsstränge', warnings);
  const context: HierarchicalPhaseContext = { chapters, sceneContexts, chapterSummaries, sourceReferences, timelineEvents, draftLedger, confirmedEntities: capArray(input.confirmedEntities, 240, 'bestätigte Entitäten', warnings), confirmedRules: capArray(input.confirmedRules, 160, 'bestätigte Regeln', warnings), confirmedStates: capArray(input.confirmedStates, 320, 'bestätigte Zustände', warnings), proposedFindings, proposedThreads, warnings, budget: { maxCodePoints, includedCodePoints: 0, truncatedSections } };
  let includedCodePoints = codePoints(context);
  if (includedCodePoints > maxCodePoints) {
    const overBudget = includedCodePoints - maxCodePoints;
    warnings.push(`Hierarchischer Providerkontext überschreitet das Budget um ${overBudget} Codepoints; Rohtext wird weiter gekürzt.`);
    truncatedSections.push('chapter-text-budget');
    let remaining = Math.max(1000, maxCodePoints - codePoints({ ...context, chapters: [] }));
    context.chapters = context.chapters.map((chapter) => {
      const allocation = Math.min(Array.from(chapter.text).length, Math.max(0, Math.floor(remaining / Math.max(1, context.chapters.length))));
      const result = truncateUnicode(chapter.text, allocation);
      remaining -= Array.from(result.value).length;
      if (result.truncated) warnings.push(`Kapitel „${chapter.title}“ wurde wegen des Gesamtbudgets weiter gekürzt.`);
      return { ...chapter, text: result.value };
    });
    includedCodePoints = codePoints(context);
  }
  context.warnings = [...new Set(warnings)];
  context.budget = { maxCodePoints, includedCodePoints, truncatedSections: [...new Set(truncatedSections)] };
  return context;
}
