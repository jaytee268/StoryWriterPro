import type { ManuscriptFormat } from '../types/domain';

export interface ManuscriptImportChapterPreview {
  id: string;
  title: string;
  content: string;
  orderIndex: number;
  wordCount: number;
  explicitNumber?: number;
  sourceHeading?: string;
}

export interface ManuscriptImportIssue {
  severity: 'warning' | 'error';
  message: string;
  chapterIndex?: number;
}

export interface ManuscriptImportPreview {
  fileName: string;
  format: ManuscriptFormat;
  chapters: ManuscriptImportChapterPreview[];
  issues: ManuscriptImportIssue[];
  duplicateChapterNumbers: number[];
  missingChapterNumbers: number[];
  pageMarkersFound: number;
}

export interface ManuscriptParseOptions {
  removePageMarkers?: boolean;
}

interface HeadingMatch {
  title: string;
  explicitNumber?: number;
}

const pageMarkerPattern = /^\s*(?:seite|page)\s+\d+(?:\s*(?:\/|von)\s*\d+)?\s*$/i;

function cleanLine(line: string): string {
  return line.replace(/^\uFEFF/, '').replace(/\r$/, '');
}

function normalizeText(text: string): string {
  return text.replace(/\r\n?/g, '\n').replace(/^\uFEFF/, '');
}

function chapterHeading(line: string): HeadingMatch | undefined {
  const stripped = line.trim().replace(/^#{1,6}\s+/, '').trim();
  if (!stripped || pageMarkerPattern.test(stripped)) return undefined;

  const simple = stripped.match(/^kapitel\s+0*(\d+)(?:\s*(?:[-–—:]\s*)?(.*))?$/i);
  if (simple) {
    const number = Number(simple[1]);
    const subtitle = simple[2]?.trim();
    return { explicitNumber: number, title: subtitle ? `Kapitel ${number} – ${subtitle}` : `Kapitel ${number}` };
  }

  const reversed = stripped.match(/^0*(\d+)\.\s*kapitel(?:\s*(?:[-–—:]\s*)?(.*))?$/i);
  if (reversed) {
    const number = Number(reversed[1]);
    const subtitle = reversed[2]?.trim();
    return { explicitNumber: number, title: subtitle ? `Kapitel ${number} – ${subtitle}` : `Kapitel ${number}` };
  }

  if (/^prolog$/i.test(stripped)) return { title: 'Prolog' };
  if (/^epilog$/i.test(stripped)) return { title: 'Epilog' };
  return undefined;
}

function wordCount(text: string): number {
  return text.trim() ? text.trim().split(/\s+/u).length : 0;
}

function nearestTitle(fileName: string): string {
  const base = fileName.replace(/\.[^.]+$/, '').trim();
  return base || 'Importiertes Manuskript';
}

function chapterIssues(chapters: ManuscriptImportChapterPreview[], pageMarkersFound: number): ManuscriptImportIssue[] {
  const issues: ManuscriptImportIssue[] = [];
  const numbered = chapters.filter((chapter) => chapter.explicitNumber !== undefined);
  const numbers = numbered.map((chapter) => chapter.explicitNumber as number);
  const duplicates = [...new Set(numbers.filter((number, index) => numbers.indexOf(number) !== index))].sort((a, b) => a - b);
  if (duplicates.length) issues.push({ severity: 'warning', message: `Doppelte Kapitelnummern: ${duplicates.join(', ')}.` });
  if (pageMarkersFound > 0) issues.push({ severity: 'warning', message: `${pageMarkersFound} Seitenmarker erkannt. Sie werden nicht als Kapitel oder Szene verwendet.` });
  if (chapters.length === 0) issues.push({ severity: 'error', message: 'Es wurde kein importierbarer Text gefunden.' });
  chapters.forEach((chapter, index) => {
    if (!chapter.title.trim()) issues.push({ severity: 'error', message: `Kapitel ${index + 1} hat keinen Titel.`, chapterIndex: index });
    if (!chapter.content.trim()) issues.push({ severity: 'warning', message: `Kapitel „${chapter.title}“ enthält keinen Text.`, chapterIndex: index });
  });
  if (numbers.length > 1) {
    const min = Math.min(...numbers);
    const max = Math.max(...numbers);
    const missing = Array.from({ length: max - min + 1 }, (_, index) => min + index).filter((number) => !numbers.includes(number));
    if (missing.length) issues.push({ severity: 'warning', message: `Möglicherweise fehlende Kapitelnummern: ${missing.join(', ')}.` });
  }
  return issues;
}

export function parseManuscriptText(text: string, fileName = 'Manuskript.txt', format: ManuscriptFormat = 'txt', options: ManuscriptParseOptions = {}): ManuscriptImportPreview {
  const removePageMarkers = options.removePageMarkers ?? true;
  const lines = normalizeText(text).split('\n').map(cleanLine);
  const pageMarkersFound = lines.filter((line) => pageMarkerPattern.test(line)).length;
  const chapters: ManuscriptImportChapterPreview[] = [];
  let current: ManuscriptImportChapterPreview | undefined;
  const fallbackText: string[] = [];

  const pushCurrent = () => {
    if (!current) return;
    current.content = current.content.replace(/\n{3,}/g, '\n\n').trim();
    current.wordCount = wordCount(current.content);
    chapters.push(current);
  };

  lines.forEach((line) => {
    const heading = chapterHeading(line);
    if (heading) {
      pushCurrent();
      current = { id: crypto.randomUUID(), title: heading.title, content: '', orderIndex: chapters.length + 1, wordCount: 0, explicitNumber: heading.explicitNumber, sourceHeading: line.trim() };
      return;
    }
    if (pageMarkerPattern.test(line) && removePageMarkers) return;
    if (current) current.content += `${line}\n`;
    else fallbackText.push(line);
  });
  pushCurrent();

  if (!chapters.length && fallbackText.join('\n').trim()) {
    const content = fallbackText.join('\n').replace(/\n{3,}/g, '\n\n').trim();
    chapters.push({ id: crypto.randomUUID(), title: nearestTitle(fileName), content, orderIndex: 1, wordCount: wordCount(content) });
  }
  chapters.forEach((chapter, index) => { chapter.orderIndex = index + 1; });
  const numbers = chapters.map((chapter) => chapter.explicitNumber).filter((number): number is number => number !== undefined);
  const duplicateChapterNumbers = [...new Set(numbers.filter((number, index) => numbers.indexOf(number) !== index))].sort((a, b) => a - b);
  const missingChapterNumbers = numbers.length > 1
    ? Array.from({ length: Math.max(...numbers) - Math.min(...numbers) + 1 }, (_, index) => Math.min(...numbers) + index).filter((number) => !numbers.includes(number))
    : [];
  return { fileName, format, chapters, issues: chapterIssues(chapters, pageMarkersFound), duplicateChapterNumbers, missingChapterNumbers, pageMarkersFound };
}

function readUint32(view: DataView, offset: number): number { return view.getUint32(offset, true); }
function readUint16(view: DataView, offset: number): number { return view.getUint16(offset, true); }

async function inflateRaw(bytes: Uint8Array): Promise<Uint8Array> {
  const streamConstructor = (globalThis as typeof globalThis & { DecompressionStream?: typeof DecompressionStream }).DecompressionStream;
  if (!streamConstructor) throw new Error('DOCX-Entpacken wird in dieser Umgebung nicht unterstützt.');
  const safeBytes = new Uint8Array(bytes.length);
  safeBytes.set(bytes);
  const stream = new Blob([safeBytes.buffer]).stream().pipeThrough(new streamConstructor('deflate-raw'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

async function zipEntry(arrayBuffer: ArrayBuffer, entryName: string): Promise<Uint8Array> {
  const bytes = new Uint8Array(arrayBuffer);
  const view = new DataView(arrayBuffer);
  let end = -1;
  for (let offset = Math.max(0, bytes.length - 65557); offset <= bytes.length - 4; offset += 1) {
    if (readUint32(view, offset) === 0x06054b50) end = offset;
  }
  if (end < 0) throw new Error('Die DOCX-Datei enthält kein gültiges ZIP-Archiv.');
  const centralSize = readUint32(view, end + 12);
  const centralOffset = readUint32(view, end + 16);
  let cursor = centralOffset;
  const decoder = new TextDecoder();
  while (cursor < centralOffset + centralSize) {
    if (readUint32(view, cursor) !== 0x02014b50) break;
    const method = readUint16(view, cursor + 10);
    const compressedSize = readUint32(view, cursor + 20);
    const nameLength = readUint16(view, cursor + 28);
    const extraLength = readUint16(view, cursor + 30);
    const commentLength = readUint16(view, cursor + 32);
    const localOffset = readUint32(view, cursor + 42);
    const name = decoder.decode(bytes.subarray(cursor + 46, cursor + 46 + nameLength));
    if (name === entryName) {
      if (readUint32(view, localOffset) !== 0x04034b50) throw new Error('Die DOCX-Datei enthält einen ungültigen Dateieintrag.');
      const localNameLength = readUint16(view, localOffset + 26);
      const localExtraLength = readUint16(view, localOffset + 28);
      const start = localOffset + 30 + localNameLength + localExtraLength;
      const compressed = bytes.slice(start, start + compressedSize);
      if (method === 0) return compressed;
      if (method === 8) return inflateRaw(compressed);
      throw new Error('Die DOCX-Datei verwendet eine nicht unterstützte Komprimierung.');
    }
    cursor += 46 + nameLength + extraLength + commentLength;
  }
  throw new Error('word/document.xml wurde in der DOCX-Datei nicht gefunden.');
}

export async function extractDocxText(arrayBuffer: ArrayBuffer): Promise<string> {
  const xml = new TextDecoder().decode(await zipEntry(arrayBuffer, 'word/document.xml'));
  if (typeof DOMParser === 'undefined') {
    const decodeXml = (value: string) => value.replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&amp;/g, '&');
    return Array.from(xml.matchAll(/<w:p\b[^>]*>([\s\S]*?)<\/w:p>/g)).map((paragraph) => Array.from(paragraph[1].matchAll(/<w:t\b[^>]*>([\s\S]*?)<\/w:t>/g)).map((text) => decodeXml(text[1])).join('')).join('\n');
  }
  const document = new DOMParser().parseFromString(xml, 'application/xml');
  if (document.querySelector('parsererror')) throw new Error('Die DOCX-Datei enthält kein gültiges Dokument.');
  const paragraphs = Array.from(document.getElementsByTagNameNS('http://schemas.openxmlformats.org/wordprocessingml/2006/main', 'p'));
  return paragraphs.map((paragraph) => {
    let content = '';
    const nodes = Array.from(paragraph.getElementsByTagNameNS('http://schemas.openxmlformats.org/wordprocessingml/2006/main', '*'));
    nodes.forEach((node) => {
      if (node.localName === 't') content += node.textContent ?? '';
      if (node.localName === 'tab') content += '\t';
      if (node.localName === 'br' || node.localName === 'cr') content += '\n';
    });
    return content;
  }).join('\n');
}

export async function parseManuscriptFile(file: File, options: ManuscriptParseOptions = {}): Promise<ManuscriptImportPreview> {
  const name = file.name.toLowerCase();
  if (name.endsWith('.docx')) return parseManuscriptText(await extractDocxText(await file.arrayBuffer()), file.name, 'docx', options);
  if (name.endsWith('.md') || name.endsWith('.markdown')) return parseManuscriptText(await file.text(), file.name, 'markdown', options);
  if (name.endsWith('.txt')) return parseManuscriptText(await file.text(), file.name, 'txt', options);
  throw new Error('Unterstützt werden nur TXT-, Markdown- und DOCX-Dateien.');
}

export function splitImportChapter(chapter: ManuscriptImportChapterPreview, offset: number): [ManuscriptImportChapterPreview, ManuscriptImportChapterPreview] {
  const safeOffset = Math.max(1, Math.min(chapter.content.length - 1, Math.round(offset)));
  const boundary = chapter.content.lastIndexOf('\n\n', safeOffset) >= Math.floor(safeOffset * 0.6) ? chapter.content.lastIndexOf('\n\n', safeOffset) : safeOffset;
  const firstContent = chapter.content.slice(0, boundary).trim();
  const secondContent = chapter.content.slice(boundary).trim();
  if (!firstContent || !secondContent) throw new Error('An dieser Stelle kann das Kapitel nicht sinnvoll geteilt werden.');
  const first = { ...chapter, id: crypto.randomUUID(), title: `${chapter.title} – Teil 1`, content: firstContent, wordCount: wordCount(firstContent), explicitNumber: undefined };
  const second = { ...chapter, id: crypto.randomUUID(), title: `${chapter.title} – Teil 2`, content: secondContent, wordCount: wordCount(secondContent), explicitNumber: undefined };
  return [first, second];
}

export function mergeImportChapters(first: ManuscriptImportChapterPreview, second: ManuscriptImportChapterPreview): ManuscriptImportChapterPreview {
  const content = `${first.content.trim()}\n\n${second.content.trim()}`.trim();
  return { ...first, id: crypto.randomUUID(), content, wordCount: wordCount(content), explicitNumber: first.explicitNumber };
}
