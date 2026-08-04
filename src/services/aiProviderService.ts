import { z } from 'zod';
import { desktopInvoke, isTauriRuntime } from './desktop';
import { LocalPrototypeBibleExtractor } from './bibleExtractor';
import { LocalPrototypeCharacterMemoryExtractor } from './characterMemoryExtractor';
import { answerFromProjectContext } from './providerBridge';
import type { AiProviderSettings, BibleExtractionInput, BibleExtractionResult, CharacterMemoryExtractionInput, CharacterMemoryExtractionResult, ChatSource, CodexCliCapabilities, ContinuityAnalysisResult, ContinuityFindingDecision, ContinuityStateLedgerEntry, GroundedChatResult, LoreMetadata, NarrativeSummaryAnalysisResult, ProjectContext, ProjectRule, ProjectStyle, ProjectStyleAnalysisResult, StoryEntity, StorySourceReference, CharacterProfile, CharacterExperience, CharacterDialogueMemory, RelationshipMemory, CharacterKnowledgeState, CharacterVoicePattern, StyleReference, ManuscriptPhaseInput, PlotThreadSynthesisResult, BookEndStateResult, GlobalCountercheckResult, LoreCrafterAnalysis, BuildLoreSheetResult, ManuscriptStructureInput, ManuscriptStructureResult } from '../types/domain';
import { canonicalizeSceneForAi } from '../utils/aiText';
import { characterMemoryPayloadSchema, validateCharacterMemoryPayload } from './characterMemorySchemas';

const defaultSettings: AiProviderSettings = { activeProvider: 'local-prototype', bibleUpdateTimeoutSeconds: 120, chatTimeoutSeconds: 90, allowLocalFallback: true };
const settingsSchema = z.object({ activeProvider: z.enum(['local-prototype', 'codex-cli']), codexBinaryPath: z.string().optional(), codexModelOverride: z.string().optional(), bibleUpdateTimeoutSeconds: z.number().int().min(1).max(900), chatTimeoutSeconds: z.number().int().min(1).max(900), allowLocalFallback: z.boolean(), codexPrivacyAcknowledgedAt: z.string().optional() });
const capabilitiesSchema = z.object({ installed: z.boolean(), binaryPath: z.string().optional(), version: z.string().optional(), supportsExec: z.boolean(), supportsJson: z.boolean(), supportsEphemeral: z.boolean(), supportsOutputSchema: z.boolean(), supportsReadOnlySandbox: z.boolean(), supportsSkipGitCheck: z.boolean(), supportsModel: z.boolean(), supportsDisableFeatures: z.boolean(), authentication: z.enum(['authenticated', 'notAuthenticated', 'unknown']), compatible: z.boolean(), detail: z.string() });
const bibleResultSchema = z.object({ proposals: z.array(z.object({ targetEntityId: z.string().nullable().optional(), proposalAction: z.enum(['create_entity', 'update_entity', 'add_source', 'mark_contradiction', 'create_open_question', 'create_author_note']), entityType: z.enum(['character', 'relationship', 'place', 'organization', 'world_rule', 'object', 'event', 'fact', 'clue', 'secret', 'plot_thread', 'retcon', 'author_note']), candidateName: z.string().min(1), candidateDescription: z.string(), candidateStatus: z.enum(['confirmed', 'proposed', 'uncertain', 'contradicted', 'retconned']), confidence: z.number().min(0).max(1), classification: z.enum(['observable_fact', 'interpretation', 'open_question', 'possible_contradiction', 'author_note']), evidenceExcerpt: z.string(), startOffset: z.number().int().nonnegative().nullable().optional(), endOffset: z.number().int().nonnegative().nullable().optional(), reason: z.string() })), warnings: z.array(z.string()) });
const chatResultSchema = z.object({ answer: z.string().min(1), usedEntityIds: z.array(z.string()), usedSourceIds: z.array(z.string()).max(8), uncertainty: z.enum(['low', 'medium', 'high']), warnings: z.array(z.string()) });
const characterMemoryResultSchema = z.object({ proposals: z.array(z.object({ proposalKind: z.string(), subjectCharacterId: z.string().nullable().optional(), relatedCharacterId: z.string().nullable().optional(), targetEntityId: z.string().nullable().optional(), payload: characterMemoryPayloadSchema, classification: z.string(), confidence: z.number().min(0).max(1), evidenceExcerpt: z.string(), startOffset: z.number().int().nonnegative().nullable().optional(), endOffset: z.number().int().nonnegative().nullable().optional(), reason: z.string() }).strict()), warnings: z.array(z.string()) }).strict();
const styleAnalysisResultSchema = z.object({ observations: z.array(z.object({ observationType: z.string(), observationText: z.string().min(1), recommendation: z.string(), confidence: z.number().min(0).max(1), evidence: z.array(z.string()) }).strict()), overallSummary: z.string(), warnings: z.array(z.string()) }).strict();
const summaryResultSchema = z.object({ summary: z.string().min(1), importantEvents: z.array(z.string()), openThreads: z.array(z.string()), characterChanges: z.array(z.string()), knowledgeChanges: z.array(z.string()), relationshipEffects: z.array(z.string()), warnings: z.array(z.string()) }).strict();
const structureResultSchema = z.object({ scenes: z.array(z.object({ temporaryId: z.string(), chapterId: z.string(), startOffset: z.number().int().nonnegative(), endOffset: z.number().int().nonnegative(), title: z.string(), povCharacterName: z.string().nullable().optional(), povEntityId: z.string().nullable().optional(), location: z.string(), storyTime: z.string(), participatingCharacterNames: z.array(z.string()), goal: z.string(), conflict: z.string(), importantEvents: z.array(z.string()), transitionType: z.enum(['location_change', 'time_jump', 'pov_change', 'character_group_change', 'flashback_start', 'flashback_end', 'dream_start', 'dream_end', 'action_break', 'narrative_transition', 'chapter_continuation']), boundaryReason: z.string(), confidence: z.number().min(0).max(1), evidenceExcerpt: z.string(), manualChanges: z.record(z.string(), z.unknown()).nullable().optional() }).strict()), warnings: z.array(z.string()) }).strict();
const plotThreadProposalSchema = z.object({ entityId: z.string(), proposedStatus: z.enum(['open', 'closure_candidate', 'partially_resolved', 'reopened', 'abandoned']), evidenceExcerpt: z.string(), reason: z.string(), confidence: z.number().min(0).max(1), sourceReferenceId: z.string().nullable().optional() }).strict();
const plotThreadSynthesisResultSchema = z.object({ summary: z.string(), openQuestions: z.array(z.string()), threadGoals: z.array(z.string()), developments: z.array(z.string()), closureCandidates: z.array(z.string()), partiallyResolved: z.array(z.string()), reopened: z.array(z.string()), threadProposals: z.array(plotThreadProposalSchema).default([]), warnings: z.array(z.string()) }).strict();
const bookEndStateResultSchema = z.object({ summary: z.string(), characterEndStates: z.array(z.string()), knowledgeStates: z.array(z.string()), falseBeliefs: z.array(z.string()), relationships: z.array(z.string()), objectOwners: z.array(z.string()), injuries: z.array(z.string()), locations: z.array(z.string()), openActions: z.array(z.string()), unresolvedThreads: z.array(z.string()), endStateProposals: z.array(z.object({ category: z.string(), entityId: z.string().nullable().optional(), statement: z.string(), confidence: z.number().min(0).max(1), evidenceExcerpt: z.string().nullable().optional(), sourceReferenceId: z.string().nullable().optional() }).strict()).default([]), warnings: z.array(z.string()) }).strict();
const globalCountercheckResultSchema = z.object({ summary: z.string(), contradictoryFacts: z.array(z.string()), prematureKnowledge: z.array(z.string()), lostOrDestroyedObjects: z.array(z.string()), timeAndLocationConflicts: z.array(z.string()), contradictoryRules: z.array(z.string()), unclearExceptions: z.array(z.string()), uncertainSources: z.array(z.string()), countercheckFindings: z.array(z.object({ severity: z.enum(['info', 'warning', 'critical']), category: z.string(), objectiveConflict: z.string(), reason: z.string(), confidence: z.number().min(0).max(1), evidenceExcerpt: z.string().nullable().optional(), sourceReferenceId: z.string().nullable().optional() }).strict()).default([]), warnings: z.array(z.string()) }).strict();
const loreCrafterAnalysisSchema = z.object({ understandingSummary: z.string(), confirmedStatements: z.array(z.string()), proposedWorldRules: z.array(z.string()), prerequisites: z.array(z.string()), effects: z.array(z.string()), limitations: z.array(z.string()), costs: z.array(z.string()), exceptions: z.array(z.string()), terminology: z.array(z.string()), relevantOrganizations: z.array(z.string()), relevantLocations: z.array(z.string()), historicalBackground: z.array(z.string()), unresolvedQuestions: z.array(z.string()), contradictions: z.array(z.string()), excludedContent: z.array(z.object({ content: z.string(), suggestedTarget: z.enum(['character_memory', 'plot_thread', 'continuity_state', 'manuscript', 'style']), reason: z.string() }).strict()), clarificationQuestions: z.array(z.string()), confidence: z.number().min(0).max(1), warnings: z.array(z.string()) }).strict();
const loreSheetResultSchema = z.object({ title: z.string(), premise: z.string(), categories: z.array(z.string()), worldRules: z.array(z.string()), worldRuleObjects: z.array(z.object({ temporaryId: z.string(), title: z.string(), statement: z.string(), prerequisites: z.array(z.string()), effects: z.array(z.string()), limitations: z.array(z.string()), costs: z.array(z.string()), exceptions: z.array(z.string()), relatedTerminology: z.array(z.string()), connectedItemIds: z.array(z.string()), sourceSpans: z.array(z.object({ excerpt: z.string(), startOffset: z.number().int().nonnegative(), endOffset: z.number().int().nonnegative() }).strict()), confidence: z.number().min(0).max(1) }).strict()).default([]), prerequisites: z.array(z.string()), effects: z.array(z.string()), limitations: z.array(z.string()), costs: z.array(z.string()), exceptions: z.array(z.string()), terminology: z.array(z.string()), organizations: z.array(z.string()), locations: z.array(z.string()), historicalEvents: z.array(z.string()), knownAspects: z.array(z.string()), unknownAspects: z.array(z.string()), ruleConnections: z.array(z.string()), openQuestions: z.array(z.string()), warnings: z.array(z.string()) }).strict();
const nullableString = z.string().nullable().optional();
const nullableOffset = z.number().int().nonnegative().nullable().optional();
const counterEvidenceSchema = z.object({ sourceReferenceId: nullableString, excerpt: z.string(), chapterId: nullableString, sceneId: nullableString, startOffset: nullableOffset, endOffset: nullableOffset }).strict();
const observedPassageSchema = z.object({ summary: z.string(), evidenceExcerpt: z.string(), entityIds: z.array(z.string()), startOffset: nullableOffset, endOffset: nullableOffset }).strict();
const stateChangeSchema = z.object({ entityId: z.string(), relatedEntityId: nullableString, stateKind: z.enum(['item_existence', 'item_availability', 'ownership', 'location', 'physical_condition', 'injury', 'property', 'knowledge', 'relationship', 'promise', 'goal', 'open_action']), previousState: z.string(), newState: z.string(), confidence: z.number().min(0).max(1), evidenceExcerpt: z.string(), sourceReferenceId: z.string().nullable(), startOffset: nullableOffset, endOffset: nullableOffset, reason: z.string() }).strict();
const contradictionSchema = z.object({ findingType: z.enum(['critical_contradiction', 'probable_contradiction', 'missing_explanation', 'lore_compatible_anomaly', 'possible_intentional_exception']), subjectEntityId: nullableString, relatedEntityIds: z.array(z.string()), relatedStateIds: z.array(z.string()), objectiveConflict: z.string(), evidenceExcerpt: z.string(), sourceReferenceId: z.string().nullable(), counterEvidenceExcerpts: z.array(z.string()), counterEvidence: z.array(counterEvidenceSchema).nullable().optional(), confidence: z.number().min(0).max(1), startOffset: nullableOffset, endOffset: nullableOffset, reason: z.string() }).strict();
export const continuityResultSchema = z.object({ observedActions: z.array(observedPassageSchema), proposedStateChanges: z.array(stateChangeSchema), objectiveContradictions: z.array(contradictionSchema), missingExplanations: z.array(contradictionSchema), matchedLoreRules: z.array(z.object({ ruleId: z.string(), rationale: z.string(), confidence: z.number().min(0).max(1) }).strict()), newRuleProposals: z.array(z.object({ projectId: z.string(), targetRuleId: nullableString, title: z.string(), statement: z.string(), scope: z.enum(['project', 'book', 'arc']), prerequisites: z.array(z.string()), effects: z.array(z.string()), exceptions: z.array(z.string()), connectedLoreIds: z.array(z.string()), sourceReferenceIds: z.array(z.string()), evidenceExcerpt: z.string(), chapterId: nullableString, sceneId: nullableString, startOffset: nullableOffset, endOffset: nullableOffset, confidence: z.number().min(0).max(1), reason: z.string() }).strict()), plotThreadChanges: z.array(z.object({ entityId: z.string(), proposedStatus: z.enum(['open', 'closure_candidate', 'partially_resolved', 'reopened', 'abandoned']), evidenceExcerpt: z.string(), sourceReferenceId: z.string().nullable(), startOffset: nullableOffset, endOffset: nullableOffset, reason: z.string(), confidence: z.number().min(0).max(1) }).strict()), confidence: z.number().min(0).max(1), evidence: z.array(z.object({ id: z.string(), label: z.string(), chapterId: nullableString, sceneId: nullableString, entityId: nullableString, excerpt: nullableString, sourceReferenceId: nullableString, startOffset: nullableOffset, endOffset: nullableOffset }).strict()), warnings: z.array(z.string()) }).strict();

export function normalizeContinuityResultNulls(result: ContinuityAnalysisResult): ContinuityAnalysisResult {
  const optional = (value: object): object => Object.fromEntries(Object.entries(value).map(([key, item]) => [key, item === null ? undefined : item]));
  return { ...result, observedActions: result.observedActions.map((item) => optional(item)), proposedStateChanges: result.proposedStateChanges.map((item) => optional(item)), objectiveContradictions: result.objectiveContradictions.map((item) => optional({ ...item, counterEvidence: item.counterEvidence?.map((counter) => optional(counter)) })), missingExplanations: result.missingExplanations.map((item) => optional({ ...item, counterEvidence: item.counterEvidence?.map((counter) => optional(counter)) })), newRuleProposals: result.newRuleProposals.map((item) => optional(item)), plotThreadChanges: result.plotThreadChanges.map((item) => optional(item)), evidence: result.evidence.map((item) => optional(item)) } as unknown as ContinuityAnalysisResult;
}

export interface StyleAnalysisInput { projectId: string; projectStyle?: ProjectStyle; styleReferences: StyleReference[]; passages: Array<{ id: string; sceneId: string; excerpt: string }>; }
export interface ContinuityAnalysisInput { projectId: string; passage: { text: string; changedText: string; chapterId?: string; sceneId?: string; startOffset?: number; endOffset?: number; passageStartOffset: number; passageEndOffset: number; coordinateSystem: 'unicode_codepoints'; }; previousContext: string; followingContext: string; confirmedStoryBible: StoryEntity[]; confirmedLore: LoreMetadata[]; confirmedRules: ProjectRule[]; continuityStatesBeforePosition: ContinuityStateLedgerEntry[]; draftLedger: ContinuityStateLedgerEntry[]; characterKnowledge: CharacterKnowledgeState[]; characterProfiles: CharacterProfile[]; characterMemories: Array<CharacterVoicePattern | CharacterExperience | CharacterDialogueMemory | RelationshipMemory>; activePlotThreads: StoryEntity[]; relevantSources: StorySourceReference[]; openFindings: Array<{ id: string; objectiveConflict: string; reviewStatus: string }>; continuityDecisions: Array<Pick<ContinuityFindingDecision, 'id' | 'findingId' | 'status' | 'decisionKind' | 'ruleId' | 'sourceReferenceId' | 'exceptionReason'>>; }
export interface LoreCrafterAnalysisInput { projectId: string; originalText: string; existingLore: LoreMetadata[]; existingRules: ProjectRule[]; }
export interface LoreCrafterSheetInput { projectId: string; originalText: string; analysis: LoreCrafterAnalysis; clarifications: Array<{ question: string; answer?: string; status: string }>; }
export interface StoryAiProvider { readonly id: 'local-prototype' | 'codex-cli'; getStatus(): Promise<ProviderStatusView>; extractBiblePatch(input: BibleExtractionInput, timeoutSeconds: number): Promise<BibleExtractionResult>; extractCharacterMemoryPatch(input: CharacterMemoryExtractionInput, timeoutSeconds: number): Promise<CharacterMemoryExtractionResult>; analyzeContinuityPassage(input: ContinuityAnalysisInput, timeoutSeconds: number): Promise<ContinuityAnalysisResult>; analyzeManuscriptStructure(input: ManuscriptStructureInput, timeoutSeconds: number): Promise<ManuscriptStructureResult>; analyzeProjectStyle(input: StyleAnalysisInput, timeoutSeconds: number): Promise<ProjectStyleAnalysisResult>; summarize(scopeType: 'scene' | 'chapter' | 'book' | 'project', scopeId: string, content: string, timeoutSeconds: number): Promise<NarrativeSummaryAnalysisResult>; analyzeNarrativeSummaries(input: ManuscriptPhaseInput, timeoutSeconds: number): Promise<NarrativeSummaryAnalysisResult>; synthesizePlotThreads(input: ManuscriptPhaseInput, timeoutSeconds: number): Promise<PlotThreadSynthesisResult>; analyzeBookEndState(input: ManuscriptPhaseInput, timeoutSeconds: number): Promise<BookEndStateResult>; globalCountercheck(input: ManuscriptPhaseInput, timeoutSeconds: number): Promise<GlobalCountercheckResult>; analyzeLoreDraft(input: LoreCrafterAnalysisInput, timeoutSeconds: number): Promise<LoreCrafterAnalysis>; buildLoreSheet(input: LoreCrafterSheetInput, timeoutSeconds: number): Promise<BuildLoreSheetResult>; answerWithProjectContext(question: string, context: ProjectContext, timeoutSeconds: number): Promise<GroundedChatResult>; cancel(taskId: string): Promise<void>; cancelActive(): Promise<void>; }
export interface ProviderStatusView { id: string; available: boolean; label: string; detail: string; capabilities?: CodexCliCapabilities; }

function canonicalizeContextForAi(context: ProjectContext): ProjectContext {
  const currentScene = context.currentScene ? canonicalizeSceneForAi(context.currentScene).scene : undefined;
  const currentChapter = context.currentChapter ? { ...context.currentChapter, scenes: context.currentChapter.scenes.map((scene) => canonicalizeSceneForAi(scene).scene) } : undefined;
  return { ...context, currentScene, currentChapter, narrativeSummaries: context.narrativeSummaries?.filter((summary) => summary.status === 'confirmed' || summary.status === 'proposed') };
}

export function buildCodexBibleRequest(input: BibleExtractionInput) {
  const canonical = canonicalizeSceneForAi(input.scene);
  return { projectId: input.project.id, sceneId: canonical.scene.id, project: { id: input.project.id, title: input.project.title, author: input.project.author }, chapter: { ...input.chapter, scenes: input.chapter.scenes.map((scene) => canonicalizeSceneForAi(scene).scene) }, scene: canonical.scene, changedRange: input.changedRange, existingEntities: input.existingEntities.map(({ id: entityId, projectId, name, type, description, status, confidence, authorConfirmed, tags }) => ({ id: entityId, projectId, name, type, description, status, confidence, authorConfirmed, tags })), relevantSources: input.relevantSources ?? [] };
}

export class LocalPrototypeProvider implements StoryAiProvider {
  readonly id = 'local-prototype' as const;
  async getStatus(): Promise<ProviderStatusView> { return { id: this.id, available: true, label: 'Lokaler Prototyp bereit', detail: 'Offline-Heuristiken, kein Netzwerkzugriff' }; }
  async extractBiblePatch(input: BibleExtractionInput): Promise<BibleExtractionResult> { return new LocalPrototypeBibleExtractor().extract({ ...input, scene: canonicalizeSceneForAi(input.scene).scene }); }
  async extractCharacterMemoryPatch(input: CharacterMemoryExtractionInput, _timeoutSeconds?: number): Promise<CharacterMemoryExtractionResult> { void _timeoutSeconds; return new LocalPrototypeCharacterMemoryExtractor().extract({ ...input, scene: canonicalizeSceneForAi(input.scene).scene }); }
  async analyzeContinuityPassage(): Promise<ContinuityAnalysisResult> { return { observedActions: [], proposedStateChanges: [], objectiveContradictions: [], missingExplanations: [], matchedLoreRules: [], newRuleProposals: [], plotThreadChanges: [], confidence: 0, evidence: [], warnings: ['Der lokale Prototyp entscheidet keine semantischen Kontinuitätsfälle. Aktiviere einen AI-Provider für die endgültige Prüfung.'] }; }
  async analyzeManuscriptStructure(input: ManuscriptStructureInput): Promise<ManuscriptStructureResult> { const scene = input.chapter.scenes[0]; const text = scene?.content ?? ''; return { scenes: [{ temporaryId: 'implicit-1', chapterId: input.chapter.id, startOffset: 0, endOffset: Array.from(text).length, title: scene?.title ?? input.chapter.title, povCharacterName: scene?.pov || undefined, location: scene?.location ?? '', storyTime: scene?.storyTime ?? '', participatingCharacterNames: [], goal: scene?.goal ?? '', conflict: '', importantEvents: [], transitionType: 'chapter_continuation', boundaryReason: 'Lokaler Provider bestätigt keine semantische Grenze.', confidence: 0, evidenceExcerpt: text }], warnings: ['Der lokale Provider schlägt keine semantischen Szenengrenzen vor.'] }; }
  async analyzeProjectStyle(_input: StyleAnalysisInput, _timeoutSeconds?: number): Promise<ProjectStyleAnalysisResult> { void _input; void _timeoutSeconds; return { observations: [], overallSummary: '', warnings: ['Der lokale Prototyp erstellt keine automatische Stilinterpretation.'] }; }
  async summarize(_scopeType: 'scene' | 'chapter' | 'book' | 'project', _scopeId: string, _content: string, _timeoutSeconds?: number): Promise<NarrativeSummaryAnalysisResult> { void _scopeType; void _scopeId; void _content; void _timeoutSeconds; return { summary: 'Keine semantische Zusammenfassung durch den lokalen Provider.', importantEvents: [], openThreads: [], characterChanges: [], knowledgeChanges: [], relationshipEffects: [], warnings: ['Der lokale Prototyp erstellt keine automatische Zusammenfassung. Ein authentifizierter AI-Provider ist für diese Phase erforderlich.'] }; }
  async analyzeNarrativeSummaries(): Promise<NarrativeSummaryAnalysisResult> { return { summary: 'Keine narrative Synthese durch den lokalen Provider.', importantEvents: [], openThreads: [], characterChanges: [], knowledgeChanges: [], relationshipEffects: [], warnings: ['Authentifizierter AI-Provider erforderlich.'] }; }
  async synthesizePlotThreads(): Promise<PlotThreadSynthesisResult> { return { summary: '', openQuestions: [], threadGoals: [], developments: [], closureCandidates: [], partiallyResolved: [], reopened: [], warnings: ['Authentifizierter AI-Provider erforderlich.'] }; }
  async analyzeBookEndState(): Promise<BookEndStateResult> { return { summary: '', characterEndStates: [], knowledgeStates: [], falseBeliefs: [], relationships: [], objectOwners: [], injuries: [], locations: [], openActions: [], unresolvedThreads: [], warnings: ['Authentifizierter AI-Provider erforderlich.'] }; }
  async globalCountercheck(): Promise<GlobalCountercheckResult> { return { summary: '', contradictoryFacts: [], prematureKnowledge: [], lostOrDestroyedObjects: [], timeAndLocationConflicts: [], contradictoryRules: [], unclearExceptions: [], uncertainSources: [], warnings: ['Authentifizierter AI-Provider erforderlich.'] }; }
  async analyzeLoreDraft(): Promise<LoreCrafterAnalysis> { return { understandingSummary: '', confirmedStatements: [], proposedWorldRules: [], prerequisites: [], effects: [], limitations: [], costs: [], exceptions: [], terminology: [], relevantOrganizations: [], relevantLocations: [], historicalBackground: [], unresolvedQuestions: [], contradictions: [], excludedContent: [], clarificationQuestions: [], confidence: 0, warnings: ['Der lokale Provider entscheidet keine Lore semantisch. Für eine belastbare Analyse ist ein authentifizierter AI-Provider erforderlich.'] }; }
  async buildLoreSheet(input: LoreCrafterSheetInput): Promise<BuildLoreSheetResult> { return { title: 'Vorläufiges Lore Sheet', premise: input.analysis.understandingSummary, categories: [], worldRules: input.analysis.proposedWorldRules, prerequisites: input.analysis.prerequisites, effects: input.analysis.effects, limitations: input.analysis.limitations, costs: input.analysis.costs, exceptions: input.analysis.exceptions, terminology: input.analysis.terminology, organizations: input.analysis.relevantOrganizations, locations: input.analysis.relevantLocations, historicalEvents: input.analysis.historicalBackground, knownAspects: input.analysis.confirmedStatements, unknownAspects: [...input.analysis.unresolvedQuestions, ...input.analysis.contradictions], ruleConnections: [], openQuestions: input.analysis.clarificationQuestions, warnings: ['Der lokale Provider erstellt nur einen vorläufigen Entwurf.'] }; }
  async answerWithProjectContext(question: string, context: ProjectContext): Promise<GroundedChatResult> { const canonicalContext = canonicalizeContextForAi(context); const answer = answerFromProjectContext(question, canonicalContext); return { answer: answer.text, usedEntityIds: canonicalContext.relevantEntities.filter((entity) => answer.sources.some((source) => source.entityId === entity.id)).map((entity) => entity.id), usedSourceIds: answer.sources.map((source) => source.id), uncertainty: answer.sources.length ? 'low' : 'high', warnings: [] }; }
  async cancel(): Promise<void> { return Promise.resolve(); }
  async cancelActive(): Promise<void> { return Promise.resolve(); }
}

function taskId(prefix: string): string { return `${prefix}-${crypto.randomUUID()}`; }
function sourceToChatSource(source: ProjectContext['relevantSources'][number]): ChatSource { return { id: source.id, label: source.excerpt ? source.excerpt.slice(0, 42) : 'Quellenstelle', chapterId: source.chapterId, sceneId: source.sceneId, entityId: source.entityId, excerpt: source.excerpt, startOffset: source.startOffset, endOffset: source.endOffset }; }

export class CodexCliProvider implements StoryAiProvider {
  readonly id = 'codex-cli' as const;
  private activeTaskId?: string;
  constructor(private readonly getSettings: () => Promise<AiProviderSettings>) {}
  async getStatus(): Promise<ProviderStatusView> {
    if (!isTauriRuntime()) return { id: this.id, available: false, label: 'Nur in der Desktop-App', detail: 'Der Browser-Demo-Modus verwendet ausschließlich den lokalen Prototyp.' };
    const capabilities = capabilitiesSchema.parse(await desktopInvoke('get_codex_provider_status'));
    const label = !capabilities.installed ? 'Codex nicht installiert' : capabilities.authentication === 'notAuthenticated' ? 'Codex nicht angemeldet' : !capabilities.compatible ? 'Codex-Version inkompatibel' : capabilities.authentication === 'unknown' ? 'Codex-Status unbekannt' : 'Codex bereit';
    return { id: this.id, available: capabilities.compatible && capabilities.authentication === 'authenticated', label, detail: capabilities.detail, capabilities };
  }
  async extractBiblePatch(input: BibleExtractionInput, timeoutSeconds: number): Promise<BibleExtractionResult> {
    const settings = await this.getSettings();
    const id = taskId('bible');
    this.activeTaskId = id;
    const request = buildCodexBibleRequest(input);
    try {
      const result = await desktopInvoke<{ result: unknown; warnings: string[] }>('run_codex_task', { input: { taskId: id, taskKind: 'extractBiblePatch', requestJson: request, timeoutSeconds: timeoutSeconds || settings.bibleUpdateTimeoutSeconds } });
      const parsed = bibleResultSchema.parse(result.result);
      return { ...parsed, proposals: parsed.proposals.map((proposal) => ({ ...proposal, targetEntityId: proposal.targetEntityId ?? undefined, startOffset: proposal.startOffset ?? undefined, endOffset: proposal.endOffset ?? undefined })) };
    } finally { if (this.activeTaskId === id) this.activeTaskId = undefined; }
  }
  async extractCharacterMemoryPatch(input: CharacterMemoryExtractionInput, timeoutSeconds: number): Promise<CharacterMemoryExtractionResult> {
    const settings = await this.getSettings(); const id = taskId('character-memory'); this.activeTaskId = id; const scene = canonicalizeSceneForAi(input.scene).scene;
    const request = { projectId: input.project.id, chapter: { id: input.chapter.id, title: input.chapter.title }, scene, changedRange: input.changedRange, characters: input.characters.map(({ id: characterId, name, description, type }) => ({ id: characterId, name, description, type })), existingEntities: input.existingEntities.map(({ id: entityId, name, type, description, status }) => ({ id: entityId, name, type, description, status })), context: { characterProfiles: input.context.characterProfiles ?? [], characterVoicePatterns: input.context.characterVoicePatterns ?? [], characterExperiences: input.context.characterExperiences ?? [], characterDialogueMemories: input.context.characterDialogueMemories ?? [], relationshipMemories: input.context.relationshipMemories ?? [], characterKnowledgeStates: input.context.characterKnowledgeStates ?? [], relevantSources: input.context.relevantSources ?? [] } };
    try { const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId: id, taskKind: 'extractCharacterMemoryPatch', requestJson: request, timeoutSeconds: timeoutSeconds || settings.bibleUpdateTimeoutSeconds } }); const parsed = characterMemoryResultSchema.parse(result.result); return { proposals: parsed.proposals.map((proposal) => ({ ...proposal, payload: validateCharacterMemoryPayload(proposal.proposalKind, proposal.payload), subjectCharacterId: proposal.subjectCharacterId ?? undefined, relatedCharacterId: proposal.relatedCharacterId ?? undefined, targetEntityId: proposal.targetEntityId ?? undefined, startOffset: proposal.startOffset ?? undefined, endOffset: proposal.endOffset ?? undefined })) as unknown as CharacterMemoryExtractionResult['proposals'], warnings: parsed.warnings }; } finally { if (this.activeTaskId === id) this.activeTaskId = undefined; }
  }
  async analyzeContinuityPassage(input: ContinuityAnalysisInput, timeoutSeconds: number): Promise<ContinuityAnalysisResult> {
    const settings = await this.getSettings();
    if (this.activeTaskId) throw new Error('Es läuft bereits eine Codex-Analyse.');
    const id = taskId('continuity'); this.activeTaskId = id;
    const request = { task: 'AnalyzeContinuityPassage', ...input };
    try {
      const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId: id, taskKind: 'analyzeContinuityPassage', requestJson: request, timeoutSeconds: timeoutSeconds || settings.bibleUpdateTimeoutSeconds } });
      return continuityResultSchema.parse(result.result) as ContinuityAnalysisResult;
    } finally { if (this.activeTaskId === id) this.activeTaskId = undefined; }
  }
  async analyzeManuscriptStructure(input: ManuscriptStructureInput, timeoutSeconds: number): Promise<ManuscriptStructureResult> {
    const settings = await this.getSettings();
    if (this.activeTaskId) throw new Error('Es läuft bereits eine Codex-Analyse.');
    const id = taskId('structure'); this.activeTaskId = id;
    const chapterText = input.chapter.scenes.map((item) => item.content).join('\n');
    const request = { task: 'AnalyzeManuscriptStructure', projectId: input.projectId, chapter: { id: input.chapter.id, title: input.chapter.title, content: chapterText }, pageMarkers: input.pageMarkers, localHints: input.localHints, confirmedLore: input.confirmedLore ?? [], confirmedRules: input.confirmedRules ?? [], coordinateSystem: 'unicode_codepoints' };
    try { const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId: id, taskKind: 'analyzeManuscriptStructure', requestJson: request, timeoutSeconds: timeoutSeconds || settings.bibleUpdateTimeoutSeconds } }); const parsed = structureResultSchema.parse(result.result); return { scenes: parsed.scenes.map((proposal) => ({ ...proposal, chapterId: input.chapter.id, povCharacterName: proposal.povCharacterName ?? undefined, povEntityId: proposal.povEntityId ?? undefined, manualChanges: proposal.manualChanges ?? undefined })), warnings: parsed.warnings }; }
    finally { if (this.activeTaskId === id) this.activeTaskId = undefined; }
  }
  async analyzeProjectStyle(input: StyleAnalysisInput, timeoutSeconds: number): Promise<ProjectStyleAnalysisResult> {
    const settings = await this.getSettings();
    if (this.activeTaskId) throw new Error('Es läuft bereits eine Codex-Analyse.');
    const id = taskId('style');
    this.activeTaskId = id;
    const request = { projectId: input.projectId, projectStyle: input.projectStyle, styleReferences: input.styleReferences, passages: input.passages };
    try {
      const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId: id, taskKind: 'analyzeProjectStyle', requestJson: request, timeoutSeconds: timeoutSeconds || settings.bibleUpdateTimeoutSeconds } });
      return styleAnalysisResultSchema.parse(result.result) as ProjectStyleAnalysisResult;
    } finally { if (this.activeTaskId === id) this.activeTaskId = undefined; }
  }
  async summarize(scopeType: 'scene' | 'chapter' | 'book' | 'project', scopeId: string, content: string, timeoutSeconds: number): Promise<NarrativeSummaryAnalysisResult> {
    const settings = await this.getSettings();
    if (this.activeTaskId) throw new Error('Es läuft bereits eine Codex-Analyse.');
    const id = taskId('summary'); this.activeTaskId = id;
    try { const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId: id, taskKind: scopeType === 'scene' ? 'summarizeScene' : scopeType === 'chapter' ? 'summarizeChapter' : scopeType === 'book' ? 'summarizeBook' : 'summarizeBook', requestJson: { scopeType, scopeId, content: content.slice(0, 40000) }, timeoutSeconds: timeoutSeconds || settings.bibleUpdateTimeoutSeconds } }); return summaryResultSchema.parse(result.result); }
    finally { if (this.activeTaskId === id) this.activeTaskId = undefined; }
  }
  async analyzeNarrativeSummaries(input: ManuscriptPhaseInput, timeoutSeconds: number): Promise<NarrativeSummaryAnalysisResult> {
    return this.runPhaseTask('analyzeNarrativeSummaries', 'narrative_summaries', input, timeoutSeconds, summaryResultSchema) as Promise<NarrativeSummaryAnalysisResult>;
  }
  async synthesizePlotThreads(input: ManuscriptPhaseInput, timeoutSeconds: number): Promise<PlotThreadSynthesisResult> {
    return this.runPhaseTask('synthesizePlotThreads', 'plot_thread_synthesis', input, timeoutSeconds, plotThreadSynthesisResultSchema) as Promise<PlotThreadSynthesisResult>;
  }
  async analyzeBookEndState(input: ManuscriptPhaseInput, timeoutSeconds: number): Promise<BookEndStateResult> {
    return this.runPhaseTask('analyzeBookEndState', 'book_end_state', input, timeoutSeconds, bookEndStateResultSchema) as Promise<BookEndStateResult>;
  }
  async globalCountercheck(input: ManuscriptPhaseInput, timeoutSeconds: number): Promise<GlobalCountercheckResult> {
    return this.runPhaseTask('globalCountercheck', 'global_countercheck', input, timeoutSeconds, globalCountercheckResultSchema) as Promise<GlobalCountercheckResult>;
  }
  async analyzeLoreDraft(input: LoreCrafterAnalysisInput, timeoutSeconds: number): Promise<LoreCrafterAnalysis> { return this.runLoreTask('analyzeLoreDraft', input, timeoutSeconds, loreCrafterAnalysisSchema) as Promise<LoreCrafterAnalysis>; }
  async buildLoreSheet(input: LoreCrafterSheetInput, timeoutSeconds: number): Promise<BuildLoreSheetResult> { return this.runLoreTask('buildLoreSheet', input, timeoutSeconds, loreSheetResultSchema) as Promise<BuildLoreSheetResult>; }
  private async runLoreTask(taskKind: 'analyzeLoreDraft' | 'buildLoreSheet', request: LoreCrafterAnalysisInput | LoreCrafterSheetInput, timeoutSeconds: number, schema: z.ZodTypeAny): Promise<unknown> {
    const settings = await this.getSettings();
    if (this.activeTaskId) throw new Error('Es läuft bereits eine Codex-Analyse.');
    const id = taskId('lore-crafter'); this.activeTaskId = id;
    try {
      const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId: id, taskKind, requestJson: { ...request, originalText: request.originalText.slice(0, 60000) }, timeoutSeconds: timeoutSeconds || settings.bibleUpdateTimeoutSeconds } });
      return schema.parse(result.result);
    } finally { if (this.activeTaskId === id) this.activeTaskId = undefined; }
  }
  private async runPhaseTask(taskKind: string, phase: string, input: ManuscriptPhaseInput, timeoutSeconds: number, schema: z.ZodTypeAny): Promise<unknown> {
    const settings = await this.getSettings();
    if (this.activeTaskId) throw new Error('Es läuft bereits eine Codex-Analyse.');
    const id = taskId(phase); this.activeTaskId = id;
    try {
      const result = await desktopInvoke<{ result: unknown }>('run_codex_task', { input: { taskId: id, taskKind, requestJson: { phase, ...input, chapters: input.chapters.map((chapter) => ({ ...chapter, text: chapter.text.slice(0, 40000) })) }, timeoutSeconds: timeoutSeconds || settings.bibleUpdateTimeoutSeconds } });
      return schema.parse(result.result);
    } finally { if (this.activeTaskId === id) this.activeTaskId = undefined; }
  }
  async answerWithProjectContext(question: string, context: ProjectContext, timeoutSeconds: number): Promise<GroundedChatResult> {
    const settings = await this.getSettings();
    const id = taskId('chat');
    this.activeTaskId = id;
    const request = { projectId: context.projectId, sceneId: context.currentScene?.id, userQuestion: question, projectContext: canonicalizeContextForAi(context) };
    try {
      const result = await desktopInvoke<{ result: unknown; warnings: string[] }>('run_codex_task', { input: { taskId: id, taskKind: 'answerWithProjectContext', requestJson: request, timeoutSeconds: timeoutSeconds || settings.chatTimeoutSeconds } });
      return chatResultSchema.parse(result.result);
    } finally { if (this.activeTaskId === id) this.activeTaskId = undefined; }
  }
  async cancel(taskIdValue: string): Promise<void> { if (isTauriRuntime()) await desktopInvoke('cancel_codex_task', { taskId: taskIdValue }); }
  async cancelActive(): Promise<void> { if (this.activeTaskId) await this.cancel(this.activeTaskId); }
}

export class ProviderRouter {
  private readonly local = new LocalPrototypeProvider();
  private readonly codex = new CodexCliProvider(() => this.getSettings());
  async getSettings(): Promise<AiProviderSettings> { if (!isTauriRuntime()) return { ...defaultSettings }; return settingsSchema.parse(await desktopInvoke('get_ai_provider_settings')); }
  async saveSettings(settings: AiProviderSettings): Promise<AiProviderSettings> { const parsed = settingsSchema.parse(settings); if (parsed.activeProvider === 'codex-cli' && !parsed.codexPrivacyAcknowledgedAt) throw new Error('Bitte bestätige zuerst die lokale Codex-Zugriffsgrenze.'); if (!isTauriRuntime()) return parsed; return settingsSchema.parse(await desktopInvoke('save_ai_provider_settings', { input: parsed })); }
  async getProviderStatus(providerId: AiProviderSettings['activeProvider']): Promise<ProviderStatusView> { return providerId === 'codex-cli' ? this.codex.getStatus() : this.local.getStatus(); }
  async getActiveProvider(): Promise<{ provider: StoryAiProvider; settings: AiProviderSettings }> { const settings = await this.getSettings(); return { provider: settings.activeProvider === 'codex-cli' ? this.codex : this.local, settings }; }
  async getLocalProvider(): Promise<LocalPrototypeProvider> { return this.local; }
  sourceObjects(context: ProjectContext, result: GroundedChatResult): ChatSource[] { const ids = new Set(result.usedSourceIds); return context.relevantSources.filter((source) => ids.has(source.id)).map(sourceToChatSource).slice(0, 8); }
}

export const providerRouter = new ProviderRouter();
export { defaultSettings as defaultAiProviderSettings };
