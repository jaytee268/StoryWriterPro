import type { CharacterMemoryExtractionInput, CharacterMemoryExtractionResult, CharacterMemoryProposalDraft } from '../types/domain';
import { canonicalizeSceneForAi } from '../utils/aiText';

/** Conservative offline extractor: it only proposes directly observable, source-backed patterns. */
export class LocalPrototypeCharacterMemoryExtractor {
  readonly id = 'local-character-memory-prototype';

  async extract(input: CharacterMemoryExtractionInput): Promise<CharacterMemoryExtractionResult> {
    const scene = canonicalizeSceneForAi(input.scene).scene;
    const text = scene.content;
    const proposals: CharacterMemoryProposalDraft[] = [];
    for (const character of input.characters) {
      const quoted = new RegExp(`(?:${escapeRegExp(character.name)})\\s*(?:sagte|fragte|rief|flüsterte)`, 'iu').exec(text);
      if (quoted) {
        const start = quoted.index;
        const excerpt = Array.from(text).slice(start, start + Array.from(quoted[0]).length).join('');
        proposals.push({ proposalKind: 'dialogue_memory', subjectCharacterId: character.id, payload: { dialogueKind: 'statement', summary: `${character.name} spricht in dieser Szene.`, exactExcerpt: excerpt, significance: 'supporting', truthfulness: 'unknown' }, classification: 'observable', confidence: 0.72, evidenceExcerpt: excerpt, startOffset: Array.from(text).slice(0, start).length, endOffset: Array.from(text).slice(0, start).length + Array.from(excerpt).length, reason: 'Explizite Sprecher-Markierung im Szenentext.' });
      }
    }
    return { proposals, warnings: proposals.length ? ['Der lokale Extractor erstellt nur vorsichtige, beobachtbare Vorschläge.'] : ['Keine eindeutig belegte Charakterbeobachtung erkannt. Interpretationen werden nicht automatisch erfunden.'] };
  }
}

function escapeRegExp(value: string): string { return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'); }
