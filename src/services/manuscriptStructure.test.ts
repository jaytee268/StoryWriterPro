import { describe, expect, it } from 'vitest';
import { codePointLength, sceneTextsFromStructure, validateManuscriptStructure } from './manuscriptStructure';

describe('manuscript structure validation', () => {
  it('uses Unicode codepoints and requires exact coverage', () => {
    const text = '😀 Erst\n\nZweite';
    expect(codePointLength(text)).toBe(14);
    const first = Array.from(text).slice(0, 7).join('');
    const second = Array.from(text).slice(7).join('');
    expect(() => validateManuscriptStructure(text, [{ startOffset: 0, endOffset: 7, evidenceExcerpt: first }, { startOffset: 7, endOffset: 14, evidenceExcerpt: second }])).not.toThrow();
    expect(sceneTextsFromStructure(text, [{ startOffset: 0, endOffset: 7 }, { startOffset: 7, endOffset: 14 }])).toEqual([first, second]);
  });

  it('rejects gaps, overlaps and false evidence', () => {
    expect(() => validateManuscriptStructure('abc', [{ startOffset: 0, endOffset: 1, evidenceExcerpt: 'a' }, { startOffset: 2, endOffset: 3, evidenceExcerpt: 'c' }])).toThrow(/Lücke/);
    expect(() => validateManuscriptStructure('abc', [{ startOffset: 0, endOffset: 2, evidenceExcerpt: 'ab' }, { startOffset: 1, endOffset: 3, evidenceExcerpt: 'bc' }])).toThrow(/Lücke|Überlappung/);
    expect(() => validateManuscriptStructure('abc', [{ startOffset: 0, endOffset: 3, evidenceExcerpt: 'xyz' }])).toThrow(/Beleg|Szenenbeleg/);
  });
});
