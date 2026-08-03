import type { BibleExtractionInput, BibleExtractionResult, BibleExtractor, BibleProposalDraft } from '../types/domain';

export function contentHash(content: string): string {
  let hash = 2166136261;
  for (const character of content) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, '0');
}

function excerptFor(text: string, needle: string): { excerpt: string; startOffset: number; endOffset: number } {
  const startOffset = Math.max(0, text.indexOf(needle));
  return { excerpt: needle, startOffset, endOffset: startOffset + needle.length };
}

function knownEntityProposals(input: BibleExtractionInput): BibleProposalDraft[] {
  return input.existingEntities
    .filter((entity) => entity.status !== 'archived' && entity.name.length > 2 && input.scene.content.toLocaleLowerCase().includes(entity.name.toLocaleLowerCase()))
    .map((entity) => {
      const evidence = excerptFor(input.scene.content, entity.name);
      return { targetEntityId: entity.id, proposalAction: 'add_source', entityType: entity.type, candidateName: entity.name, candidateDescription: `„${entity.name}“ kommt in der aktuellen Szene vor.`, candidateStatus: entity.status, confidence: 0.9, classification: 'observable_fact', evidenceExcerpt: evidence.excerpt, startOffset: evidence.startOffset, endOffset: evidence.endOffset, reason: 'Bekannter Story-Bible-Eintrag wird in der aktuellen Szene erwähnt.' };
    });
}

export class LocalPrototypeBibleExtractor implements BibleExtractor {
  readonly id = 'local-prototype-extractor';

  async extract(input: BibleExtractionInput): Promise<BibleExtractionResult> {
    const proposals: BibleProposalDraft[] = [];
    const metadata: Array<{ name: string; description: string; type: 'character' | 'place' | 'event' | 'author_note'; reason: string; value: string }> = [
      { name: input.scene.pov, description: `Die Szene wird aus der Perspektive von ${input.scene.pov} erzählt.`, type: 'character', reason: 'Perspektivfigur aus den Szenenmetadaten.', value: input.scene.pov },
      { name: input.scene.location, description: `Die Szene spielt in ${input.scene.location}.`, type: 'place', reason: 'Ort aus den Szenenmetadaten.', value: input.scene.location },
      { name: input.scene.storyTime, description: `Die Szene ist auf ${input.scene.storyTime} datiert.`, type: 'event', reason: 'Zeitpunkt aus den Szenenmetadaten.', value: input.scene.storyTime },
    ];
    for (const item of metadata.filter((item) => item.value.trim())) {
      const evidence = excerptFor(input.scene.content, item.value);
      proposals.push({ proposalAction: 'add_source', entityType: item.type, candidateName: item.name, candidateDescription: item.description, candidateStatus: 'proposed', confidence: 0.92, classification: 'observable_fact', evidenceExcerpt: evidence.excerpt, startOffset: evidence.startOffset, endOffset: evidence.endOffset, reason: item.reason });
    }
    if (input.scene.goal.trim()) proposals.push({ proposalAction: 'create_author_note', entityType: 'author_note', candidateName: `Ziel: ${input.scene.title}`, candidateDescription: input.scene.goal, candidateStatus: 'proposed', confidence: 0.85, classification: 'author_note', evidenceExcerpt: input.scene.goal, reason: 'Szenenziel als Autorennotiz vorbereiten.' });
    proposals.push(...knownEntityProposals(input));

    const properNames = input.scene.content.match(/\b[A-ZÄÖÜ][a-zäöüß]{2,}\b/g) ?? [];
    for (const name of [...new Set(properNames)].slice(0, 5)) {
      if (input.existingEntities.some((entity) => entity.name.toLocaleLowerCase() === name.toLocaleLowerCase())) continue;
      const evidence = excerptFor(input.scene.content, name);
      proposals.push({ proposalAction: 'create_entity', entityType: 'character', candidateName: name, candidateDescription: `Der Name „${name}“ erscheint in der aktuellen Szene.`, candidateStatus: 'proposed', confidence: 0.52, classification: 'open_question', evidenceExcerpt: evidence.excerpt, startOffset: evidence.startOffset, endOffset: evidence.endOffset, reason: 'Möglicherweise neuer Eigenname; bitte manuell prüfen.' });
    }
    const questionMatch = input.scene.content.match(/[^.!?]*\?/);
    if (questionMatch) proposals.push({ proposalAction: 'create_open_question', entityType: 'author_note', candidateName: `Offene Frage in ${input.scene.title}`, candidateDescription: questionMatch[0].trim(), candidateStatus: 'uncertain', confidence: 0.66, classification: 'open_question', evidenceExcerpt: questionMatch[0].trim(), reason: 'Fragezeichen deutet auf eine noch offene Frage hin.' });
    const changedNote = input.changedRange ? `Geänderter Bereich: Zeichen ${input.changedRange.start}–${input.changedRange.end}.` : 'Die aktuelle gespeicherte Szenenfassung wurde geprüft.';
    return { proposals: proposals.filter((proposal, index, all) => all.findIndex((candidate) => candidate.candidateName === proposal.candidateName && candidate.proposalAction === proposal.proposalAction) === index), warnings: [changedNote, 'Der lokale Prototype-Extractor trennt beobachtbare Fakten von Vermutungen. Jede Übernahme benötigt deine Bestätigung.'] };
  }
}

export function changedRange(previous: string | undefined, current: string): { start: number; end: number } | undefined {
  if (previous === undefined || previous === current) return undefined;
  let start = 0;
  while (start < previous.length && start < current.length && previous[start] === current[start]) start += 1;
  let previousEnd = previous.length;
  let currentEnd = current.length;
  while (previousEnd > start && currentEnd > start && previous[previousEnd - 1] === current[currentEnd - 1]) { previousEnd -= 1; currentEnd -= 1; }
  return { start, end: currentEnd };
}
