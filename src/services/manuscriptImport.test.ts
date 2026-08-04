import { describe, expect, it } from 'vitest';
import { extractDocxText, parseManuscriptFile, parseManuscriptText, splitContinuityUnits } from './manuscriptImport';

function storedZip(entries: Record<string, string>): ArrayBuffer {
  const encoder = new TextEncoder();
  const localParts: Uint8Array[] = [];
  const centralParts: Uint8Array[] = [];
  let offset = 0;
  const write16 = (view: DataView, position: number, value: number) => view.setUint16(position, value, true);
  const write32 = (view: DataView, position: number, value: number) => view.setUint32(position, value, true);
  Object.entries(entries).forEach(([name, content]) => {
    const nameBytes = encoder.encode(name);
    const data = encoder.encode(content);
    const local = new Uint8Array(30 + nameBytes.length + data.length);
    const localView = new DataView(local.buffer);
    write32(localView, 0, 0x04034b50); write16(localView, 4, 20); write16(localView, 8, 0); write16(localView, 26, nameBytes.length); write16(localView, 28, 0);
    local.set(nameBytes, 30); local.set(data, 30 + nameBytes.length); localParts.push(local);
    const central = new Uint8Array(46 + nameBytes.length); const centralView = new DataView(central.buffer);
    write32(centralView, 0, 0x02014b50); write16(centralView, 4, 20); write16(centralView, 6, 20); write16(centralView, 8, 0); write16(centralView, 10, 0); write16(centralView, 28, nameBytes.length); write16(centralView, 30, 0); write16(centralView, 32, 0); write32(centralView, 20, data.length); write32(centralView, 24, data.length); write32(centralView, 42, offset);
    central.set(nameBytes, 46); centralParts.push(central); offset += local.length;
  });
  const centralSize = centralParts.reduce((sum, part) => sum + part.length, 0);
  const end = new Uint8Array(22); const endView = new DataView(end.buffer); write32(endView, 0, 0x06054b50); write16(endView, 8, centralParts.length); write16(endView, 10, centralParts.length); write32(endView, 12, centralSize); write32(endView, 16, offset);
  const output = new Uint8Array(offset + centralSize + end.length); let cursor = 0;
  [...localParts, ...centralParts, end].forEach((part) => { output.set(part, cursor); cursor += part.length; });
  return output.buffer;
}

describe('manuscript import preview', () => {
  it('erkennt nummerierte Kapitel ohne Szenen', () => {
    const preview = parseManuscriptText('Kapitel 1\nDer Anfang.\n\nKapitel 02\nDie Mitte.\n\nKAPITEL 3 – Ende\nSchluss.', 'buch.txt');
    expect(preview.chapters).toHaveLength(3);
    expect(preview.chapters.map((chapter) => chapter.title)).toEqual(['Kapitel 1', 'Kapitel 2', 'Kapitel 3 – Ende']);
    expect(preview.chapters.map((chapter) => chapter.wordCount)).toEqual([2, 2, 1]);
  });

  it('erkennt Markdown-Überschriften, Prolog und Epilog', () => {
    const preview = parseManuscriptText('# Prolog\nVorher.\n\n## 1. Kapitel\nStart.\n\n# Epilog\nDanach.', 'buch.md', 'markdown');
    expect(preview.chapters.map((chapter) => chapter.title)).toEqual(['Prolog', 'Kapitel 1', 'Epilog']);
  });

  it('verwendet bei einem Manuskript ohne Überschrift genau ein Kapitel', () => {
    const preview = parseManuscriptText('Ein Text ohne Kapitelüberschrift.', 'mein-buch.txt');
    expect(preview.chapters).toHaveLength(1);
    expect(preview.chapters[0].title).toBe('mein-buch');
  });

  it('behandelt Kapitel ohne Untertitel sowie doppelte und fehlende Nummern', () => {
    const preview = parseManuscriptText('Kapitel 1\nA\n\nKapitel 1\nB\n\nKapitel 3\nC');
    expect(preview.chapters[0].title).toBe('Kapitel 1');
    expect(preview.duplicateChapterNumbers).toEqual([1]);
    expect(preview.missingChapterNumbers).toEqual([2]);
  });

  it('erkennt Seitenmarker nicht als Kapitel und kann sie entfernen oder behalten', () => {
    const text = 'Kapitel 1\nSeite 1\nText\nSeite 2\n\nKapitel 2\nSeite 3 von 8\nWeiter';
    const removed = parseManuscriptText(text, 'buch.txt', 'txt', { removePageMarkers: true });
    const kept = parseManuscriptText(text, 'buch.txt', 'txt', { removePageMarkers: false });
    expect(removed.chapters).toHaveLength(2);
    expect(removed.pageMarkersFound).toBe(3);
    expect(removed.chapters[0].pageMarkers.map((marker) => marker.page)).toEqual([1, 2]);
    expect(removed.chapters[0].pageMarkers[1].textOffset).toBeGreaterThan(removed.chapters[0].pageMarkers[0].textOffset);
    expect(removed.chapters[0].content).not.toContain('Seite 1');
    expect(kept.chapters[0].content).toContain('Seite 1');
  });

  it('liest DOCX-Text aus word/document.xml und führt ihn durch denselben Kapitelparser', async () => {
    const xml = '<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Kapitel 1</w:t></w:r></w:p><w:p><w:r><w:t>DOCX-Inhalt</w:t></w:r></w:p></w:body></w:document>';
    const text = await extractDocxText(storedZip({ 'word/document.xml': xml }));
    expect(text).toContain('Kapitel 1');
    const file = new File([storedZip({ 'word/document.xml': xml })], 'buch.docx', { type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' });
    const preview = await parseManuscriptFile(file);
    expect(preview.format).toBe('docx');
    expect(preview.chapters[0].title).toBe('Kapitel 1');
    expect(preview.chapters[0].content).toContain('DOCX-Inhalt');
  });

  it('macht Seitenmarker und wortbasierte Einheiten zu internen Prüfeinheiten', () => {
    const preview = parseManuscriptText('Kapitel 1\nSeite 1\nErster Abschnitt.\nSeite 2\nZweiter Abschnitt.');
    const units = splitContinuityUnits(preview.chapters[0].content, preview.chapters[0].pageMarkers);
    expect(units.map((unit) => unit.page)).toEqual([1, 2]);
    expect(units[0].startOffset).toBe(0);
    expect(units[1].startOffset).toBeGreaterThan(units[0].startOffset);
    const wordUnits = splitContinuityUnits(Array.from({ length: 700 }, (_, index) => `Wort${index}`).join(' '));
    expect(wordUnits.length).toBeGreaterThan(1);
    expect(wordUnits.every((unit) => unit.text.split(/\s+/u).length <= 350 || unit === wordUnits.at(-1))).toBe(true);
  });
});
