import type { ManuscriptStructureProposal } from '../types/domain';

export function codePointLength(text: string): number { return Array.from(text).length; }

export function validateManuscriptStructure(text: string, proposals: Array<Pick<ManuscriptStructureProposal, 'startOffset' | 'endOffset' | 'evidenceExcerpt'>>): void {
  if (proposals.length === 0) throw new Error('Die Strukturanalyse muss mindestens eine Szene vorschlagen.');
  const sorted = [...proposals].sort((a, b) => a.startOffset - b.startOffset);
  const characters = Array.from(text);
  if (sorted[0].startOffset !== 0 || sorted[sorted.length - 1].endOffset !== characters.length) throw new Error('Szenenvorschläge müssen den vollständigen Kapiteltext abdecken.');
  let expected = 0;
  for (const proposal of sorted) {
    if (!Number.isInteger(proposal.startOffset) || !Number.isInteger(proposal.endOffset) || proposal.startOffset < 0 || proposal.endOffset < proposal.startOffset || proposal.endOffset > characters.length) throw new Error('Szenenposition ist keine gültige Unicode-Codepoint-Position.');
    if (proposal.startOffset !== expected) throw new Error('Szenenvorschläge enthalten eine Lücke oder Überlappung.');
    const excerpt = characters.slice(proposal.startOffset, proposal.endOffset).join('');
    if (excerpt !== proposal.evidenceExcerpt) throw new Error('Der Szenenbeleg stimmt nicht mit dem Kapiteltext überein.');
    expected = proposal.endOffset;
  }
}

export function sceneTextsFromStructure(text: string, proposals: Array<Pick<ManuscriptStructureProposal, 'startOffset' | 'endOffset'>>): string[] {
  validateManuscriptStructure(text, proposals.map((proposal) => ({ ...proposal, evidenceExcerpt: Array.from(text).slice(proposal.startOffset, proposal.endOffset).join('') })));
  const characters = Array.from(text);
  return [...proposals].sort((a, b) => a.startOffset - b.startOffset).map((proposal) => characters.slice(proposal.startOffset, proposal.endOffset).join(''));
}

export function localStructureHints(text: string): Array<{ startOffset: number; endOffset: number; reason: string }> {
  const characters = Array.from(text);
  const hints: Array<{ startOffset: number; endOffset: number; reason: string }> = [];
  for (let index = 0; index < characters.length; index += 1) if (characters[index] === '\n' && characters[index + 1] === '\n') hints.push({ startOffset: index, endOffset: index + 2, reason: 'Absatzgrenze als lokaler Hinweis; keine semantische Entscheidung.' });
  return hints;
}
