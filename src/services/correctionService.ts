import type { Correction, CorrectionResult } from '../types/domain';

export interface CorrectionService { check(text: string): Promise<CorrectionResult>; apply(text: string, correction: Correction): string; }
export class MockCorrectionService implements CorrectionService {
  async check(text: string): Promise<CorrectionResult> {
    const corrections: Correction[] = [];
    const typo = text.match(/wiederspiegelt/g);
    if (typo?.length) corrections.push({ id: 'corr-1', kind: 'spelling', from: 'wiederspiegelt', to: 'widerspiegelt', reason: 'Korrekte Schreibweise', start: text.indexOf('wiederspiegelt'), end: text.indexOf('wiederspiegelt') + 13 });
    const doubleSpace = text.indexOf('  ');
    if (doubleSpace >= 0) corrections.push({ id: 'corr-2', kind: 'whitespace', from: '  ', to: ' ', reason: 'Doppeltes Leerzeichen', start: doubleSpace, end: doubleSpace + 2 });
    return { id: crypto.randomUUID(), sourceText: text, corrections, provider: 'MockCorrectionService', message: corrections.length ? undefined : 'Keine vorbereiteten Korrekturen gefunden.' };
  }
  apply(text: string, correction: Correction): string { return text.slice(0, correction.start) + correction.to + text.slice(correction.end); }
}
export class LocalLanguageToolProvider implements CorrectionService {
  async check(): Promise<CorrectionResult> { return { id: crypto.randomUUID(), sourceText: '', corrections: [], provider: 'LocalLanguageToolProvider', message: 'Kein lokaler LanguageTool-Server erreichbar. Starte ihn lokal und prüfe die Verbindung erneut.' }; }
  apply(text: string, correction: Correction): string { return text.slice(0, correction.start) + correction.to + text.slice(correction.end); }
}

export function chunkText(text: string, targetWords = 600, overlapWords = 80): string[] {
  const paragraphs = text.split(/\n\s*\n/).map((p) => p.trim()).filter(Boolean);
  const chunks: string[] = []; let current: string[] = []; let words = 0;
  for (const paragraph of paragraphs.length ? paragraphs : [text]) {
    const count = paragraph.split(/\s+/).filter(Boolean).length;
    if (current.length && words + count > targetWords) {
      const previous = current.join('\n\n'); chunks.push(previous);
      const tail = previous.split(/\s+/).slice(-overlapWords).join(' '); current = tail ? [tail, paragraph] : [paragraph]; words = tail.split(/\s+/).filter(Boolean).length + count;
    } else { current.push(paragraph); words += count; }
  }
  if (current.length) chunks.push(current.join('\n\n'));
  return chunks;
}
