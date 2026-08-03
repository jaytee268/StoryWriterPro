// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { selectionToUnicodeOffsets } from './editorSelection';

function select(root: HTMLElement, startNode: Node, start: number, endNode: Node, end: number) {
  const range = document.createRange();
  range.setStart(startNode, start);
  range.setEnd(endNode, end);
  return selectionToUnicodeOffsets(root, range);
}

describe('Editorauswahl als Unicode-Stilreferenz', () => {
  it('entfernt Formatierungstags ohne die Offsets zu verschieben', () => {
    const root = document.createElement('div');
    root.innerHTML = '<p>Marek <strong>lief</strong>.</p>';
    document.body.append(root);
    const paragraph = root.querySelector('p')!;
    const text = paragraph.childNodes[0]!;
    const end = paragraph.lastChild!;
    expect(select(root, text, 0, end, 1)).toEqual({ excerpt: 'Marek lief.', startOffset: 0, endOffset: 11 });
  });

  it('berücksichtigt br und Absatzumbrüche', () => {
    const root = document.createElement('div');
    root.innerHTML = '<p>Erste<br>Zweite</p><p>Äpfel 😀</p>';
    document.body.append(root);
    const first = root.querySelector('p')!;
    const second = root.querySelectorAll('p')[1]!;
    expect(select(root, first.firstChild!, 0, first.lastChild!, 6)?.excerpt).toBe('Erste\nZweite');
    expect(select(root, second.firstChild!, 0, second.firstChild!, 7)).toMatchObject({ excerpt: 'Äpfel 😀', startOffset: 13, endOffset: 20 });
  });
});
