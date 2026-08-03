import { z } from 'zod';

const nonEmpty = z.string().trim().min(1).max(4000);
const characterId = z.string().min(1).max(200);
const participant = z.object({ characterId, role: z.enum(['speaker', 'listener', 'present', 'mentioned']) }).strict();

export const characterMemoryPayloadSchemas = {
  voice_pattern: z.object({ patternType: z.enum(['signature_word', 'signature_phrase', 'filler_word', 'nickname', 'address_pattern', 'sentence_pattern', 'humor_pattern', 'metaphor_pattern', 'avoidance_pattern', 'lie_pattern', 'stress_pattern', 'relationship_specific_voice', 'dialogue_rule']), patternText: nonEmpty, description: z.string().max(4000), contextCondition: z.string().max(1000), relatedCharacterId: characterId.optional() }).strict(),
  experience: z.object({ title: nonEmpty, objectiveSummary: z.string().max(4000), subjectiveInterpretation: z.string().max(4000), emotionalImpact: z.string().max(2000), lastingEffect: z.string().max(2000), significance: z.enum(['minor', 'supporting', 'major', 'defining']), memoryReliability: z.enum(['reliable', 'uncertain', 'distorted', 'implanted', 'forgotten']), eventEntityId: z.string().max(200).optional() }).strict(),
  dialogue_memory: z.object({ dialogueKind: z.enum(['statement', 'promise', 'threat', 'lie', 'confession', 'reveal', 'argument', 'inside_joke', 'nickname', 'secret_shared', 'secret_hidden', 'boundary', 'callback', 'question', 'accusation', 'apology']), topic: z.string().max(1000), summary: nonEmpty, exactExcerpt: z.string().max(4000), emotionalTone: z.string().max(1000), hiddenIntent: z.string().max(2000), significance: z.enum(['minor', 'supporting', 'important', 'core']), truthfulness: z.enum(['true', 'false', 'partially_true', 'speaker_believes_true', 'unknown']), participants: z.array(participant).min(1).max(30) }).strict(),
  relationship_memory: z.object({ relatedCharacterId: characterId, memoryType: z.enum(['inside_joke', 'nickname', 'shared_memory', 'shared_secret', 'promise', 'betrayal', 'argument', 'trust_gain', 'trust_loss', 'relationship_shift', 'debt', 'favor', 'fear', 'attraction', 'resentment', 'callback', 'boundary']), title: nonEmpty, summary: nonEmpty, privateMeaning: z.string().max(3000), relationshipEffect: z.string().max(3000), significance: z.enum(['minor', 'supporting', 'important', 'core']) }).strict(),
  knowledge_change: z.object({ factEntityId: characterId, knowledgeState: z.enum(['knows', 'suspects', 'believes_false', 'denies', 'forgot', 'unknown']), certainty: z.number().min(0).max(1), sourceCharacterId: characterId.optional(), notes: z.string().max(3000) }).strict(),
  profile_observation: z.object({ field: nonEmpty, observedBehavior: nonEmpty, possibleInterpretation: z.string().max(3000) }).strict(),
  character_relation: z.object({ relationType: z.enum(['affects', 'explains', 'contradicts', 'reveals', 'hides', 'depends_on', 'applies_to', 'caused_by', 'connected_to']), label: z.string().max(160) }).strict(),
} as const;

export function validateCharacterMemoryPayload(kind: string, payload: unknown): unknown {
  const schema = characterMemoryPayloadSchemas[kind as keyof typeof characterMemoryPayloadSchemas];
  if (!schema) throw new Error(`Unbekannter Character-Memory-Typ: ${kind}`);
  return schema.parse(payload);
}

export function isCharacterMemoryEvidenceValid(text: string, excerpt: string, start?: number, end?: number): boolean {
  if (start === undefined || end === undefined) return excerpt.length > 0 && text.includes(excerpt);
  const chars = Array.from(text);
  return start >= 0 && end > start && end <= chars.length && chars.slice(start, end).join('') === excerpt;
}
