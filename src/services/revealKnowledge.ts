import type { LoreMetadata, RevealAudienceState, RevealClueRule, RevealComplianceInput, RevealComplianceResult, RevealContext, RevealPosition, RevealScope, RevealState, StoryEntity } from '../types/domain';
import type { StoryRepository } from './storyRepository';

type ResolvedPosition = RevealPosition & { bookOrder: number; chapterOrder: number; sceneOrder: number; offsetValue: number };

const positionFields = ['bookId', 'chapterId', 'sceneId', 'bookOrderIndex', 'chapterOrderIndex', 'sceneOrderIndex', 'offset'] as const;

export function compareRevealPositions(left: RevealPosition, right: RevealPosition): number {
  const values: Array<[number | undefined, number | undefined]> = [
    [left.bookOrderIndex, right.bookOrderIndex],
    [left.chapterOrderIndex !== undefined ? left.chapterOrderIndex : undefined, right.chapterOrderIndex !== undefined ? right.chapterOrderIndex : undefined],
    [left.sceneOrderIndex !== undefined ? left.sceneOrderIndex : undefined, right.sceneOrderIndex !== undefined ? right.sceneOrderIndex : undefined],
    [left.offset ?? 0, right.offset ?? 0],
  ];
  for (const [a, b] of values) {
    if (a === undefined || b === undefined) continue;
    if (a !== b) return a < b ? -1 : 1;
  }
  return 0;
}

function validatePosition(position: RevealPosition, label: string): void {
  for (const field of ['bookOrderIndex', 'chapterOrderIndex', 'sceneOrderIndex', 'offset'] as const) {
    const value = position[field];
    if (value !== undefined && (!Number.isInteger(value) || value < 0)) throw new Error(`${label}: ${field} muss eine nichtnegative Unicode-Position sein.`);
  }
}

async function resolvePosition(repository: StoryRepository, projectId: string, position: RevealPosition): Promise<ResolvedPosition> {
  validatePosition(position, 'Reveal-Position');
  const workspace = await repository.loadWorkspace(projectId);
  const scene = position.sceneId ? workspace.chapters.flatMap((item) => item.scenes).find((item) => item.id === position.sceneId) : undefined;
  const chapter = position.chapterId ? workspace.chapters.find((item) => item.id === position.chapterId) : scene ? workspace.chapters.find((item) => item.id === scene.chapterId) : undefined;
  if (position.chapterId && !chapter) throw new Error('Reveal-Position verweist auf ein unbekanntes Kapitel.');
  if (position.sceneId && !scene) throw new Error('Reveal-Position verweist auf eine unbekannte Szene.');
  if (chapter && !workspace.books.some((book) => book.id === chapter.bookId)) throw new Error('Reveal-Position verweist auf einen ungültigen Band.');
  if (scene && chapter && scene.chapterId !== chapter.id) throw new Error('Reveal-Position verweist auf eine ungültige Szene.');
  const book = position.bookId ? workspace.books.find((item) => item.id === position.bookId) : chapter ? workspace.books.find((item) => item.id === chapter.bookId) : undefined;
  if (position.bookId && !book) throw new Error('Reveal-Position verweist auf einen ungültigen Band.');
  if (chapter && book && chapter.bookId !== book.id) throw new Error('Reveal-Kapitel gehört nicht zum angegebenen Band.');
  return { ...position, bookId: book?.id ?? position.bookId, chapterId: chapter?.id ?? position.chapterId, sceneId: scene?.id ?? position.sceneId, bookOrderIndex: position.bookOrderIndex ?? book?.volume ?? 0, bookOrder: book?.volume ?? 0, chapterOrder: position.chapterOrderIndex ?? chapter?.orderIndex ?? 0, sceneOrder: position.sceneOrderIndex ?? scene?.orderIndex ?? 0, offsetValue: position.offset ?? 0 };
}

export async function validateRevealPositionForRepository(repository: StoryRepository, projectId: string, position: RevealPosition): Promise<void> {
  await resolvePosition(repository, projectId, position);
}

function compareResolved(left: ResolvedPosition, right: ResolvedPosition): number {
  for (const [a, b] of [[left.bookOrder, right.bookOrder], [left.chapterOrder, right.chapterOrder], [left.sceneOrder, right.sceneOrder], [left.offsetValue, right.offsetValue]]) {
    if (a !== b) return a < b ? -1 : 1;
  }
  return 0;
}

async function applies(repository: StoryRepository, projectId: string, position: ResolvedPosition, from?: RevealPosition, until?: RevealPosition): Promise<boolean> {
  const start = await resolvePosition(repository, projectId, from ?? {});
  if (compareResolved(start, position) > 0) return false;
  if (!until) return true;
  const end = await resolvePosition(repository, projectId, until);
  return compareResolved(position, end) < 0;
}

function specificity(position?: RevealPosition): number {
  if (!position) return 0;
  return positionFields.reduce((count, field) => count + (position[field] !== undefined ? 1 : 0), 0);
}

async function selectStates(repository: StoryRepository, projectId: string, position: ResolvedPosition, states: RevealAudienceState[], warnings: string[]): Promise<RevealAudienceState[]> {
  const confirmed = states.filter((state) => state.projectId === projectId && state.status === 'confirmed' && state.authorConfirmed);
  const active: RevealAudienceState[] = [];
  for (const state of confirmed) if (await applies(repository, projectId, position, state.validFromPosition, state.validUntilPosition)) active.push(state);
  const selected = new Map<string, RevealAudienceState>();
  for (const state of active) {
    const key = `${state.contractId}:${state.audienceKind}:${state.characterEntityId ?? 'reader'}`;
    const previous = selected.get(key);
    if (!previous) { selected.set(key, state); continue; }
    const previousStart = await resolvePosition(repository, projectId, previous.validFromPosition);
    const currentStart = await resolvePosition(repository, projectId, state.validFromPosition);
    const score = specificity(state.validFromPosition) - specificity(previous.validFromPosition);
    if (score > 0 || (score === 0 && compareResolved(currentStart, previousStart) > 0)) selected.set(key, state);
    else if (score === 0 && previous.knowledgeLevel !== state.knowledgeLevel) warnings.push(`Widersprüchliche gleichrangige Wissensstände für ${key}.`);
  }
  return [...selected.values()];
}

export async function buildRevealContext(repository: StoryRepository, input: { projectId: string; position: RevealPosition; povCharacterId?: string; participatingCharacterIds?: string[] }): Promise<RevealContext> {
  const position = await resolvePosition(repository, input.projectId, input.position);
  const contracts = (await repository.listRevealContracts(input.projectId)).filter((contract) => contract.status === 'confirmed' && contract.authorConfirmed);
  const contractIds = new Set(contracts.map((contract) => contract.id));
  const states = (await repository.listRevealAudienceStates(input.projectId)).filter((state) => contractIds.has(state.contractId));
  const clues = (await repository.listRevealClueRules(input.projectId)).filter((rule) => contractIds.has(rule.contractId) && rule.status === 'confirmed' && rule.authorConfirmed);
  const warnings: string[] = [];
  const relevantStates = await selectStates(repository, input.projectId, position, states, warnings);
  const participants = new Set(input.participatingCharacterIds ?? []);
  if (input.povCharacterId) participants.add(input.povCharacterId);
  const readerKnowledgeAtPosition = relevantStates.filter((state) => state.audienceKind === 'reader');
  const characterStates = relevantStates.filter((state) => state.audienceKind === 'character');
  const povCharacterKnowledgeAtPosition = input.povCharacterId ? characterStates.filter((state) => state.characterEntityId === input.povCharacterId) : [];
  const participantKnowledgeAtPosition = characterStates.filter((state) => state.characterEntityId && participants.has(state.characterEntityId) && state.characterEntityId !== input.povCharacterId);
  const activeClues: RevealClueRule[] = [];
  for (const rule of clues) if (await applies(repository, input.projectId, position, rule.validFromPosition, rule.validUntilPosition)) activeClues.push(rule);
  const plannedReveals = contracts.filter((contract) => contract.plannedRevealChapterId || contract.plannedRevealSceneId || contract.plannedRevealBookId);
  return { confirmedAuthorTruths: contracts, readerKnowledgeAtPosition, povCharacterKnowledgeAtPosition, participantKnowledgeAtPosition, allowedClues: activeClues.filter((rule) => rule.ruleKind === 'allowed'), forbiddenClues: activeClues.filter((rule) => rule.ruleKind === 'forbidden'), requiredClues: activeClues.filter((rule) => rule.ruleKind === 'required'), plannedReveals, warnings };
}

function sliceCodePoints(text: string, start: number, end: number): string { return Array.from(text).slice(start, end).join(''); }

export function validateRevealComplianceResultReferences(input: RevealComplianceInput, result: RevealComplianceResult): RevealComplianceResult {
  const contracts = new Map(input.revealContext.confirmedAuthorTruths.map((contract) => [contract.id, contract]));
  const allowedCharacters = new Set([...(input.participatingCharacterIds ?? []), ...(input.povCharacterId ? [input.povCharacterId] : [])]);
  const textLength = Array.from(input.text).length;
  for (const finding of result.findings) {
    const contract = contracts.get(finding.contractId);
    if (!contract || contract.subjectEntityId !== finding.subjectEntityId) throw new Error('CODEX_INVALID_REFERENCE: Reveal-Finding gehört nicht zu einem angeforderten Contract.');
    if (finding.characterEntityId && !allowedCharacters.has(finding.characterEntityId)) throw new Error('CODEX_INVALID_REFERENCE: Reveal-Finding verweist auf eine nicht beteiligte Figur.');
    if ((finding.startOffset === undefined) !== (finding.endOffset === undefined)) throw new Error('CODEX_INVALID_OFFSET: Reveal-Offsets müssen gemeinsam gesetzt werden.');
    if (finding.startOffset !== undefined && finding.endOffset !== undefined) {
      if (finding.startOffset < 0 || finding.endOffset < finding.startOffset || finding.endOffset > textLength) throw new Error('CODEX_INVALID_OFFSET: Reveal-Offset liegt außerhalb des Textes.');
      if (sliceCodePoints(input.text, finding.startOffset, finding.endOffset) !== finding.evidenceExcerpt) throw new Error('CODEX_EVIDENCE_OFFSET_MISMATCH: Belegstelle stimmt nicht mit dem Text überein.');
    }
  }
  return result;
}

export function validateRevealComplianceInput(input: RevealComplianceInput): void {
  if (!input.projectId.trim() || !['manuscript', 'generated_draft', 'dialogue', 'summary'].includes(input.textKind)) throw new Error('Reveal-Request benötigt ein gültiges Projekt und eine gültige Textart.');
  for (const contract of input.revealContext.confirmedAuthorTruths) {
    if (contract.projectId !== input.projectId || contract.status !== 'confirmed' || !contract.authorConfirmed) throw new Error('REVEAL_INVALID_CONTEXT: Nur bestätigte Contracts desselben Projekts dürfen verwendet werden.');
  }
  for (const state of [...input.revealContext.readerKnowledgeAtPosition, ...input.revealContext.povCharacterKnowledgeAtPosition, ...input.revealContext.participantKnowledgeAtPosition]) {
    if (state.projectId !== input.projectId || state.status !== 'confirmed' || !state.authorConfirmed) throw new Error('REVEAL_INVALID_CONTEXT: Ein Wissensstand ist nicht verbindlich bestätigt.');
    if (state.audienceKind === 'reader' && state.characterEntityId) throw new Error('REVEAL_INVALID_CONTEXT: Leserwissen darf keine Figuren-ID enthalten.');
    if (state.audienceKind === 'character' && !state.characterEntityId) throw new Error('REVEAL_INVALID_CONTEXT: Figurenwissen benötigt eine Figur.');
  }
  for (const clue of [...input.revealContext.allowedClues, ...input.revealContext.forbiddenClues, ...input.revealContext.requiredClues]) {
    if (clue.projectId !== input.projectId || clue.status !== 'confirmed' || !clue.authorConfirmed) throw new Error('REVEAL_INVALID_CONTEXT: Eine Hinweisregel ist nicht verbindlich bestätigt.');
  }
}

export function formatRevealContextForAi(context: RevealContext): string {
  const lines = ['AUTHOR TRUTH — NEVER COPY AUTOMATICALLY', ...context.confirmedAuthorTruths.map((item) => `- ${item.truthStatement}`), 'READER KNOWLEDGE AT CURRENT POSITION', ...context.readerKnowledgeAtPosition.map((item) => `- ${item.knowledgeLevel}: ${item.beliefText}`), 'POV CHARACTER KNOWLEDGE', ...context.povCharacterKnowledgeAtPosition.map((item) => `- ${item.characterEntityId}: ${item.knowledgeLevel}: ${item.beliefText}`), 'OTHER PARTICIPANT KNOWLEDGE', ...context.participantKnowledgeAtPosition.map((item) => `- ${item.characterEntityId}: ${item.knowledgeLevel}: ${item.beliefText}`), 'REVEAL CONSTRAINTS', 'allowed', ...context.allowedClues.map((item) => `- ${item.description}`), 'forbidden', ...context.forbiddenClues.map((item) => `- ${item.description}`), 'required', ...context.requiredClues.map((item) => `- ${item.description}`), 'planned reveals', ...context.plannedReveals.map((item) => `- ${item.title}: ${item.revealConditionText}`), 'Autorwahrheit ist kein automatisch sichtbares Erzählerwissen. Keine automatische Manuskriptänderung.'];
  return lines.join('\n');
}

export function buildLegacyRevealContractProposal(loreMetadata: LoreMetadata): { subjectEntityId: string; title: string; truthStatement: string; scope: RevealScope; status: 'proposed'; authorConfirmed: false; revealState: RevealState; revealConditionText: string; notes: string } {
  return { subjectEntityId: loreMetadata.entityId, title: loreMetadata.truthStatement.slice(0, 120) || 'Unbenannter Reveal-Contract', truthStatement: loreMetadata.truthStatement, scope: loreMetadata.scope, status: 'proposed', authorConfirmed: false, revealState: loreMetadata.revealState, revealConditionText: loreMetadata.revealPlan, notes: 'Aus vorhandener Lore-Metadatenstruktur vorgeschlagen; noch nicht bestätigt.' };
}

export function validateRevealEntities(entities: StoryEntity[], projectId: string, contract: { subjectEntityId: string }): void {
  const subject = entities.find((entity) => entity.id === contract.subjectEntityId);
  if (!subject || subject.projectId !== projectId || (subject.type !== 'secret' && subject.type !== 'fact' && subject.type !== 'world_rule')) throw new Error('Reveal-Contract benötigt eine projektgebundene Reveal-Entität.');
}
