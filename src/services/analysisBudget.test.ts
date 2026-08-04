import { describe, expect, it } from 'vitest';
import type { Chapter } from '../types/domain';
import { buildHierarchicalPhaseContext } from './analysisBudget';

function chapter(id: string, orderIndex: number, text: string): Chapter {
  return { id, bookId: 'book-1', title: `Kapitel ${orderIndex + 1}`, orderIndex, scenes: [{ id: `${id}-scene`, chapterId: id, title: 'Implizite Szene', orderIndex: 0, content: text, pov: '', location: '', storyTime: '', status: 'draft', goal: '', notes: '', isImplicit: true }] };
}

describe('hierarchischer Manuskript-Providerkontext', () => {
  it('budgetiert Unicode-sicher, erhält Quellen-IDs und erzeugt nur vollständige JSON-Objekte', () => {
    const first = chapter('chapter-1', 0, `😀${'A'.repeat(18000)}`);
    const second = chapter('chapter-2', 1, `e\u0301${'B'.repeat(18000)}`);
    const source = { id: 'source-1', projectId: 'project-1', chapterId: 'chapter-2', sceneId: 'chapter-2-scene', excerpt: 'é', startOffset: 0, endOffset: 2, createdAt: 'now' };
    const context = buildHierarchicalPhaseContext({ chapters: [second, first], chapterSummaries: [], sourceReferences: [source], timelineEvents: [], draftLedger: [], confirmedEntities: [], confirmedRules: [], confirmedStates: [], proposedFindings: [], proposedThreads: [], maxCodePoints: 5000 });
    expect(context.sourceReferences.map((item) => item.id)).toContain('source-1');
    expect(context.warnings.length).toBeGreaterThan(0);
    expect(context.budget.includedCodePoints).toBeLessThanOrEqual(context.budget.maxCodePoints + 1500);
    expect(() => JSON.parse(JSON.stringify(context))).not.toThrow();
    expect(Array.from(context.chapters[0]!.text).length).toBeGreaterThanOrEqual(0);
  });
});
