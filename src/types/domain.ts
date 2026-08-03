export type EntityStatus = 'confirmed' | 'proposed' | 'uncertain' | 'contradicted' | 'retconned' | 'archived';
export type EntityType = 'character' | 'relationship' | 'place' | 'organization' | 'world_rule' | 'object' | 'event' | 'fact' | 'clue' | 'secret' | 'plot_thread' | 'retcon' | 'author_note';
export type AppView = 'dashboard' | 'editor' | 'bible' | 'timeline' | 'mindmap' | 'characters' | 'threads' | 'research' | 'files' | 'settings';
export type AiTaskType = 'chat' | 'bible_update' | 'consistency_check' | 'grammar_review' | 'manuscript_import' | 'deep_research' | 'timeline_validation' | 'character_analysis';
export type CorrectionKind = 'spelling' | 'grammar' | 'punctuation' | 'capitalization' | 'whitespace';

export type SceneStatus = 'draft' | 'revised' | 'final';
export type ManuscriptFont = 'serif' | 'sans' | 'typewriter';
export interface EditorPreferences { fontFamily: ManuscriptFont; fontSize: number; lineHeight: number; }
export interface Project { id: string; title: string; author: string; description: string; updatedAt: string; createdAt?: string; wordCount: number; openWarnings: number; bibleProgress: number; }
export interface Book { id: string; projectId: string; title: string; volume: number; createdAt?: string; updatedAt?: string; }
export interface Chapter { id: string; bookId: string; title: string; orderIndex: number; scenes: Scene[]; createdAt?: string; updatedAt?: string; }
export interface Scene { id: string; chapterId: string; title: string; orderIndex: number; content: string; pov: string; location: string; storyTime: string; status: SceneStatus; goal: string; notes: string; createdAt?: string; updatedAt?: string; }
export type SceneVersionReason = 'manual' | 'before_correction' | 'before_ai_change' | 'before_import' | 'automatic_checkpoint';
export interface SceneVersion { id: string; sceneId: string; versionNumber: number; content: string; reason: SceneVersionReason; createdAt: string; scene: Scene; }
export type StoryEntityOrigin = 'manual' | 'bible_update' | 'edited';
export interface StoryEntity { id: string; projectId: string; name: string; type: EntityType; description: string; status: EntityStatus; confidence: number; source: string; chapter: string; scene: string; authorConfirmed: boolean; updatedAt: string; createdAt?: string; tags: string[]; origin: StoryEntityOrigin; }
export interface StorySourceReference { id: string; projectId: string; entityId?: string; proposalId?: string; chapterId: string; sceneId: string; excerpt: string; startOffset?: number; endOffset?: number; createdAt: string; }
export type BibleRunStatus = 'pending' | 'running' | 'completed' | 'failed' | 'reviewed';
export type BibleProposalAction = 'create_entity' | 'update_entity' | 'add_source' | 'mark_contradiction' | 'create_open_question' | 'create_author_note';
export type BibleClassification = 'observable_fact' | 'interpretation' | 'open_question' | 'possible_contradiction' | 'author_note';
export type BibleReviewStatus = 'pending' | 'accepted' | 'edited' | 'rejected';
export interface BibleUpdateRun { id: string; projectId: string; sceneId: string; sceneUpdatedAt: string; contentHash: string; extractorId: string; analyzedContent?: string; status: BibleRunStatus; createdAt: string; completedAt?: string; errorMessage?: string; }
export interface BibleProposal { id: string; runId: string; projectId: string; sceneId: string; targetEntityId?: string; proposalAction: BibleProposalAction; entityType: EntityType; candidateName: string; candidateDescription: string; candidateStatus: EntityStatus; confidence: number; classification: BibleClassification; evidenceExcerpt: string; startOffset?: number; endOffset?: number; reason: string; reviewStatus: BibleReviewStatus; reviewedAt?: string; createdAt: string; }
export interface BibleProposalDraft { targetEntityId?: string; proposalAction: BibleProposalAction; entityType: EntityType; candidateName: string; candidateDescription: string; candidateStatus: EntityStatus; confidence: number; classification: BibleClassification; evidenceExcerpt: string; startOffset?: number; endOffset?: number; reason: string; }
export interface BibleExtractionInput { project: Project; chapter: Chapter; scene: Scene; existingEntities: StoryEntity[]; relevantSources?: StorySourceReference[]; previousAnalyzedContent?: string; changedRange?: { start: number; end: number }; }
export interface BibleExtractionResult { proposals: BibleProposalDraft[]; warnings: string[]; }
export interface BibleExtractor { readonly id: string; extract(input: BibleExtractionInput): Promise<BibleExtractionResult>; }
export interface ContextRequest { projectId: string; currentChapterId?: string; currentSceneId?: string; userQuestion: string; includeProposedSummaries?: boolean; }
export type LoreCategory = 'world_rule' | 'history' | 'objective_truth' | 'belief' | 'myth' | 'mystery' | 'terminology';
export type LoreScope = 'series' | 'book' | 'arc';
export type LoreRevealState = 'author_only' | 'foreshadowed' | 'reader_revealed';
export type LoreImportance = 'core' | 'supporting' | 'background';
export interface ProjectContext { projectId: string; currentScene?: Scene; currentChapter?: Chapter; relevantEntities: StoryEntity[]; relevantSources: StorySourceReference[]; openPlotThreads: StoryEntity[]; possibleContradictions: StoryEntity[]; lore?: LoreMetadata[]; entityRelations?: StoryEntityRelation[]; characterProfiles?: CharacterProfile[]; characterStates?: CharacterSceneState[]; characterVoicePatterns?: CharacterVoicePattern[]; characterExperiences?: CharacterExperience[]; characterDialogueMemories?: CharacterDialogueMemory[]; relationshipMemories?: RelationshipMemory[]; characterKnowledgeStates?: CharacterKnowledgeState[]; projectStyle?: ProjectStyle; styleReferences?: StyleReference[]; acceptedStyleObservations?: ProjectStyleObservation[]; narrativeSummaries?: NarrativeSummary[]; }
export interface LoreMetadata { entityId: string; projectId: string; category: LoreCategory; scope: LoreScope; revealState: LoreRevealState; importance: LoreImportance; truthStatement: string; rulesText: string; exceptionsText: string; authorKnowledge: string; readerKnowledge: string; revealPlan: string; createdAt: string; updatedAt: string; }
export interface CharacterProfile { entityId: string; projectId: string; coreWant: string; coreNeed: string; fears: string; falseBelief: string; values: string; strengths: string; flaws: string; pressureBehavior: string; voice: string; backstory: string; arcSummary: string; createdAt: string; updatedAt: string; }
export interface CharacterSceneState { id: string; projectId: string; characterEntityId: string; sceneId: string; emotionalState: string; physicalState: string; goal: string; conflict: string; knowledgeNotes: string; relationshipState: string; changeNote: string; createdAt: string; updatedAt: string; }
export type CharacterMemoryStatus = 'proposed' | 'confirmed' | 'uncertain' | 'rejected' | 'retired' | 'retconned';
export type CharacterVoicePatternType = 'signature_word' | 'signature_phrase' | 'filler_word' | 'nickname' | 'address_pattern' | 'sentence_pattern' | 'humor_pattern' | 'metaphor_pattern' | 'avoidance_pattern' | 'lie_pattern' | 'stress_pattern' | 'relationship_specific_voice' | 'dialogue_rule';
export type CharacterExperienceSignificance = 'minor' | 'supporting' | 'major' | 'defining';
export type CharacterMemorySignificance = 'minor' | 'supporting' | 'important' | 'core';
export type CharacterMemoryReliability = 'reliable' | 'uncertain' | 'distorted' | 'implanted' | 'forgotten';
export type CharacterDialogueKind = 'statement' | 'promise' | 'threat' | 'lie' | 'confession' | 'reveal' | 'argument' | 'inside_joke' | 'nickname' | 'secret_shared' | 'secret_hidden' | 'boundary' | 'callback' | 'question' | 'accusation' | 'apology';
export type DialogueTruthfulness = 'true' | 'false' | 'partially_true' | 'speaker_believes_true' | 'unknown';
export type RelationshipMemoryType = 'inside_joke' | 'nickname' | 'shared_memory' | 'shared_secret' | 'promise' | 'betrayal' | 'argument' | 'trust_gain' | 'trust_loss' | 'relationship_shift' | 'debt' | 'favor' | 'fear' | 'attraction' | 'resentment' | 'callback' | 'boundary';
export type CharacterKnowledgeKind = 'knows' | 'suspects' | 'believes_false' | 'denies' | 'forgot' | 'unknown';
export type CharacterMemoryKind = 'voice_pattern' | 'experience' | 'dialogue_memory' | 'relationship_memory' | 'knowledge_state' | 'profile_observation';
export type CharacterMemoryClassification = 'observable' | 'interpretation' | 'author_decision_required' | 'possible_contradiction';
export type CharacterMemoryProposalKind = 'voice_pattern' | 'experience' | 'dialogue_memory' | 'relationship_memory' | 'knowledge_change' | 'profile_observation' | 'character_relation';
export type MemoryEvidenceRole = 'primary' | 'supporting' | 'contradicting';
export type DialogueParticipantRole = 'speaker' | 'listener' | 'present' | 'mentioned';
export interface CharacterVoicePattern { id: string; projectId: string; characterId: string; relatedCharacterId?: string; patternType: CharacterVoicePatternType; patternText: string; description: string; contextCondition: string; confidence: number; status: CharacterMemoryStatus; authorConfirmed: boolean; occurrenceCount: number; firstObservedSceneId?: string; lastObservedSceneId?: string; retiredSceneId?: string; createdAt: string; updatedAt: string; }
export interface CharacterExperience { id: string; projectId: string; characterId: string; eventEntityId?: string; sceneId?: string; title: string; objectiveSummary: string; subjectiveInterpretation: string; emotionalImpact: string; lastingEffect: string; significance: CharacterExperienceSignificance; memoryReliability: CharacterMemoryReliability; status: CharacterMemoryStatus; authorConfirmed: boolean; createdAt: string; updatedAt: string; }
export interface DialogueMemoryParticipant { dialogueMemoryId: string; characterId: string; role: DialogueParticipantRole; }
export interface CharacterDialogueMemory { id: string; projectId: string; speakerId: string; sceneId: string; dialogueKind: CharacterDialogueKind; topic: string; summary: string; exactExcerpt: string; emotionalTone: string; hiddenIntent: string; significance: CharacterMemorySignificance; truthfulness: DialogueTruthfulness; status: CharacterMemoryStatus; authorConfirmed: boolean; participants: DialogueMemoryParticipant[]; createdAt: string; updatedAt: string; }
export interface RelationshipMemory { id: string; projectId: string; characterAId: string; characterBId: string; sceneId?: string; memoryType: RelationshipMemoryType; title: string; summary: string; privateMeaning: string; relationshipEffect: string; significance: CharacterMemorySignificance; status: CharacterMemoryStatus; authorConfirmed: boolean; createdAt: string; updatedAt: string; }
export interface CharacterKnowledgeState { id: string; projectId: string; characterId: string; factEntityId: string; knowledgeState: CharacterKnowledgeKind; acquiredSceneId?: string; changedSceneId?: string; effectiveFromSceneId?: string; effectiveUntilSceneId?: string; sourceCharacterId?: string; certainty: number; notes: string; status: CharacterMemoryStatus; authorConfirmed: boolean; createdAt: string; updatedAt: string; }
export interface CharacterMemoryEvidence { id: string; projectId: string; memoryKind: CharacterMemoryKind; memoryId: string; sourceReferenceId: string; evidenceRole: MemoryEvidenceRole; createdAt: string; }
export interface CharacterMemoryUpdateRun { id: string; projectId: string; sceneId: string; contentHash: string; extractorId: string; analyzedContent?: string; status: BibleRunStatus; createdAt: string; completedAt?: string; errorMessage?: string; }
export interface VoicePatternProposalPayload { patternType: CharacterVoicePatternType; patternText: string; description: string; contextCondition: string; relatedCharacterId?: string; }
export interface ExperienceProposalPayload { title: string; objectiveSummary: string; subjectiveInterpretation: string; emotionalImpact: string; lastingEffect: string; significance: CharacterExperienceSignificance; memoryReliability: CharacterMemoryReliability; eventEntityId?: string; }
export interface DialogueMemoryProposalPayload { dialogueKind: CharacterDialogueKind; topic: string; summary: string; exactExcerpt: string; emotionalTone: string; hiddenIntent: string; significance: CharacterMemorySignificance; truthfulness: DialogueTruthfulness; participants: Array<{ characterId: string; role: DialogueParticipantRole }>; }
export interface RelationshipMemoryProposalPayload { relatedCharacterId: string; memoryType: RelationshipMemoryType; title: string; summary: string; privateMeaning: string; relationshipEffect: string; significance: CharacterMemorySignificance; }
export interface KnowledgeChangeProposalPayload { factEntityId: string; knowledgeState: CharacterKnowledgeKind; certainty: number; sourceCharacterId?: string; notes: string; }
export interface ProfileObservationProposalPayload { field: string; observedBehavior: string; possibleInterpretation: string; }
export interface CharacterRelationProposalPayload { relationType: StoryEntityRelationType; label: string; }
export type CharacterMemoryPayload = VoicePatternProposalPayload | ExperienceProposalPayload | DialogueMemoryProposalPayload | RelationshipMemoryProposalPayload | KnowledgeChangeProposalPayload | ProfileObservationProposalPayload | CharacterRelationProposalPayload;
export interface CharacterMemoryProposal { id: string; runId: string; projectId: string; sceneId: string; proposalKind: CharacterMemoryProposalKind; subjectCharacterId?: string; relatedCharacterId?: string; targetEntityId?: string; payload: CharacterMemoryPayload; classification: CharacterMemoryClassification; confidence: number; evidenceExcerpt: string; startOffset?: number; endOffset?: number; reason: string; reviewStatus: BibleReviewStatus; reviewedAt?: string; analyzedContentHash?: string; acceptedMemoryId?: string; acceptedMemoryKind?: CharacterMemoryKind; createdAt: string; }
export interface CharacterMemoryProposalDraft { proposalKind: CharacterMemoryProposalKind; subjectCharacterId?: string; relatedCharacterId?: string; targetEntityId?: string; payload: CharacterMemoryPayload; classification: CharacterMemoryClassification; confidence: number; evidenceExcerpt: string; startOffset?: number; endOffset?: number; reason: string; }
export interface CharacterMemoryExtractionInput { project: Project; chapter: Chapter; scene: Scene; characters: StoryEntity[]; existingEntities: StoryEntity[]; context: ProjectContext; changedRange?: { start: number; end: number }; }
export interface CharacterMemoryExtractionResult { proposals: CharacterMemoryProposalDraft[]; warnings: string[]; }
export interface SaveCharacterVoicePatternInput extends Omit<CharacterVoicePattern, 'id' | 'createdAt' | 'updatedAt'> { id?: string; }
export interface SaveCharacterExperienceInput extends Omit<CharacterExperience, 'id' | 'createdAt' | 'updatedAt'> { id?: string; }
export interface SaveCharacterDialogueMemoryInput extends Omit<CharacterDialogueMemory, 'id' | 'createdAt' | 'updatedAt' | 'participants'> { id?: string; participants: Omit<DialogueMemoryParticipant, 'dialogueMemoryId'>[]; }
export type SaveRelationshipMemoryInput = Omit<RelationshipMemory, 'id' | 'createdAt' | 'updatedAt'> & { id?: string; };
export interface SaveCharacterKnowledgeStateInput extends Omit<CharacterKnowledgeState, 'id' | 'createdAt' | 'updatedAt'> { id?: string; }
export interface AddCharacterMemoryEvidenceInput { projectId: string; memoryKind: CharacterMemoryKind; memoryId: string; sourceReferenceId: string; evidenceRole: MemoryEvidenceRole; }
export interface CreateCharacterMemoryUpdateRunInput { projectId: string; sceneId: string; contentHash: string; extractorId: string; analyzedContent?: string; }
export interface ReviewCharacterMemoryProposalInput { proposalId: string; reviewStatus: Exclude<BibleReviewStatus, 'pending'>; decision?: 'accept' | 'uncertain' | 'reject'; payload?: CharacterMemoryPayload; }
export type StoryEntityRelationType = 'affects' | 'explains' | 'contradicts' | 'reveals' | 'hides' | 'depends_on' | 'applies_to' | 'caused_by' | 'connected_to';
export interface StoryEntityRelation { id: string; projectId: string; sourceEntityId: string; targetEntityId: string; relationType: StoryEntityRelationType; label: string; authorConfirmed: boolean; createdAt: string; updatedAt: string; }
export interface ProjectStyle { projectId: string; narrativePov: string; tense: string; sentenceStyle: string; dialogueStyle: string; descriptionDensity: string; innerMonologue: string; preferredPatterns: string[]; avoidedPatterns: string[]; notes: string; createdAt: string; updatedAt: string; }
export type ProjectStyleAnalysisStatus = 'pending' | 'running' | 'completed' | 'failed';
export interface ProjectStyleAnalysisRun { id: string; projectId: string; sourceHash: string; providerId: string; status: ProjectStyleAnalysisStatus; createdAt: string; completedAt?: string; errorMessage?: string; }
export type ProjectStyleObservationType = 'narrative_pov' | 'tense' | 'sentence_rhythm' | 'dialogue' | 'description' | 'inner_monologue' | 'humor' | 'pacing' | 'vocabulary' | 'transitions' | 'tension' | 'avoidance' | 'character_voice_separation';
export type ProjectStyleObservationReviewStatus = 'pending' | 'accepted' | 'edited' | 'rejected';
export interface ProjectStyleObservationEvidence { sourceId?: string; styleReferenceId?: string; excerpt?: string; }
export interface ProjectStyleObservation { id: string; runId: string; projectId: string; observationType: ProjectStyleObservationType; observationText: string; recommendation: string; confidence: number; evidence: ProjectStyleObservationEvidence[]; reviewStatus: ProjectStyleObservationReviewStatus; reviewedAt?: string; createdAt: string; }
export interface ProjectStyleAnalysisDraft { observationType: ProjectStyleObservationType; observationText: string; recommendation: string; confidence: number; evidence: string[]; }
export interface ProjectStyleAnalysisResult { observations: ProjectStyleAnalysisDraft[]; overallSummary: string; warnings: string[]; }
export interface NarrativeSummaryAnalysisResult { summary: string; importantEvents: string[]; openThreads: string[]; characterChanges: string[]; knowledgeChanges: string[]; relationshipEffects: string[]; warnings: string[]; }
export interface CreateProjectStyleAnalysisRunInput { projectId: string; sourceHash: string; providerId: string; }
export interface SaveProjectStyleObservationInput { runId: string; projectId: string; observationType: ProjectStyleObservationType; observationText: string; recommendation: string; confidence: number; evidence: ProjectStyleObservationEvidence[]; reviewStatus?: ProjectStyleObservationReviewStatus; }
export type SaveLoreMetadataInput = Omit<LoreMetadata, 'createdAt' | 'updatedAt'>;
export type SaveCharacterProfileInput = Omit<CharacterProfile, 'createdAt' | 'updatedAt'>;
export interface SaveCharacterSceneStateInput extends Omit<CharacterSceneState, 'id' | 'createdAt' | 'updatedAt'> { id?: string }
export type SaveProjectStyleInput = Omit<ProjectStyle, 'createdAt' | 'updatedAt'>;
export interface NarrativeSummary { id: string; projectId: string; scopeType: 'scene' | 'chapter' | 'book' | 'project'; scopeId: string; contentHash: string; summary: string; importantEvents: string[]; openThreads: string[]; characterChanges: string[]; status: 'proposed' | 'confirmed' | 'outdated' | 'rejected'; authorConfirmed: boolean; createdAt: string; updatedAt: string; }
export type SaveNarrativeSummaryInput = Omit<NarrativeSummary, 'id' | 'createdAt' | 'updatedAt'>;
export type StyleReferenceCategory = 'general' | 'dialogue' | 'tension' | 'description' | 'inner_monologue' | 'humor';
export interface StyleReference { id: string; projectId: string; chapterId?: string; sceneId: string; startOffset?: number; endOffset?: number; category: StyleReferenceCategory; label: string; excerpt: string; notes: string; weight: number; createdAt: string; updatedAt: string; }
export type CreateStyleReferenceInput = Omit<StyleReference, 'id' | 'createdAt' | 'updatedAt'>;
export interface UpdateStyleReferenceInput { id: string; projectId: string; label: string; category: StyleReferenceCategory; notes: string; weight: number; }
export interface CreateStoryEntityRelationInput { projectId: string; sourceEntityId: string; targetEntityId: string; relationType: StoryEntityRelationType; label: string; authorConfirmed: boolean; }
export interface CreateLoreEntryInput { projectId: string; name: string; entityType: Extract<EntityType, 'world_rule' | 'fact' | 'event' | 'secret' | 'clue' | 'organization' | 'place' | 'object' | 'plot_thread' | 'author_note'>; description: string; status: EntityStatus; category: LoreCategory; scope: LoreScope; revealState: LoreRevealState; importance: LoreImportance; truthStatement: string; rulesText: string; exceptionsText: string; authorKnowledge: string; readerKnowledge: string; revealPlan: string; tags: string[]; }
export interface LoreEntry { entity: StoryEntity; metadata: LoreMetadata; }
export interface ChatSource { id: string; label: string; chapterId?: string; sceneId?: string; entityId?: string; excerpt?: string; startOffset?: number; endOffset?: number; }
export interface TimelineEvent { id: string; title: string; storyTime: string; chapter: string; scene: string; location: string; characters: string[]; pov: string; summary: string; consequences: string; knowledge: string; clue?: string; status: EntityStatus; track: string; }
export interface MindNode { id: string; label: string; type: string; x: number; y: number; status?: EntityStatus; }
export interface MindEdge { id: string; source: string; target: string; label: string; }
export interface ChatMessage { id: string; role: 'user' | 'assistant'; content: string; sources?: ChatSource[]; time: string; }
export interface AiTask { id: string; type: AiTaskType; prompt: string; context: string[]; }
export interface ProviderStatus { available: boolean; label: string; detail: string; }
export type CodexAuthenticationState = 'authenticated' | 'notAuthenticated' | 'unknown';
export interface CodexCliCapabilities { installed: boolean; binaryPath?: string; version?: string; supportsExec: boolean; supportsJson: boolean; supportsEphemeral: boolean; supportsOutputSchema: boolean; supportsReadOnlySandbox: boolean; supportsSkipGitCheck: boolean; supportsModel: boolean; supportsDisableFeatures: boolean; authentication: CodexAuthenticationState; compatible: boolean; detail: string; }
export interface AiProviderSettings { activeProvider: 'local-prototype' | 'codex-cli'; codexBinaryPath?: string; codexModelOverride?: string; bibleUpdateTimeoutSeconds: number; chatTimeoutSeconds: number; allowLocalFallback: boolean; codexPrivacyAcknowledgedAt?: string; }
export type CodexTaskKind = 'extractBiblePatch' | 'extractCharacterMemoryPatch' | 'answerWithProjectContext' | 'analyzeProjectStyle' | 'summarizeScene' | 'summarizeChapter' | 'summarizeBook' | 'planChapterDraft' | 'draftChapterSection' | 'reviewChapterSection' | 'reviewCompleteChapter';
export interface GroundedChatResult { answer: string; usedEntityIds: string[]; usedSourceIds: string[]; uncertainty: 'low' | 'medium' | 'high'; warnings: string[]; }
export interface Correction { id: string; kind: CorrectionKind; from: string; to: string; reason: string; start: number; end: number; }
export interface CorrectionResult { id: string; sourceText: string; corrections: Correction[]; provider: string; message?: string; }

export interface WorkspaceSnapshot { project: Project; books: Book[]; chapters: Chapter[]; entities: StoryEntity[]; }
export interface CreateProjectInput { title: string; author: string; description?: string; volumeTitle?: string; volume?: number; }
export interface CreateChapterInput { bookId: string; title: string; }
export interface UpdateChapterInput { id: string; title: string; }
export interface CreateSceneInput { chapterId: string; title: string; }
export type UpdateSceneInput = Scene;
export interface CreateSceneVersionInput { sceneId: string; reason?: SceneVersionReason; }
export interface SaveStoryEntityInput extends StoryEntity { projectId: string; }
export interface CreateStoryEntityInput { projectId: string; name: string; type: EntityType; description: string; status: EntityStatus; confidence: number; chapterId?: string; sceneId?: string; excerpt: string; authorConfirmed: boolean; tags: string[]; }
export interface UpdateStoryEntityInput extends CreateStoryEntityInput { id: string; }
export interface CreateSourceReferenceInput { projectId: string; entityId?: string; proposalId?: string; chapterId: string; sceneId: string; excerpt: string; startOffset?: number; endOffset?: number; }
export interface CreateBibleUpdateRunInput { projectId: string; sceneId: string; sceneUpdatedAt: string; contentHash: string; extractorId: string; analyzedContent?: string; force?: boolean; }
export type BibleReviewDecision = 'accept' | 'edit_accept' | 'save_uncertain' | 'save_author_note' | 'reject' | 'keep_existing' | 'mark_contradiction' | 'accept_new_value' | 'accept_retcon' | 'defer';
export interface ReviewBibleProposalInput { proposalId: string; reviewStatus: BibleReviewStatus; decision?: BibleReviewDecision; candidateName?: string; candidateDescription?: string; candidateStatus?: EntityStatus; classification?: BibleClassification; }
export interface PendingSourceNavigation { sceneId: string; chapterId: string; excerpt: string; startOffset?: number; endOffset?: number; }

export type EndingStatus = 'fixed' | 'preferred' | 'open';
export interface RevealConstraint { entityId: string; notBeforeChapter?: number; instruction: string; }
export interface StoryDirection { projectId: string; premise: string; currentStoryPhase: string; bookGoal: string; plannedEnding: string; endingStatus: EndingStatus; centralTwist: string; thematicGoal: string; mustHappen: string[]; mustNotHappen: string[]; nextTurningPoint: string; revealConstraints: RevealConstraint[]; authorNotes: string; createdAt: string; updatedAt: string; }
export type SaveStoryDirectionInput = Omit<StoryDirection, 'createdAt' | 'updatedAt'>;
export interface WritingPreferences { projectId: string; wordsPerPage: number; preferredSectionWords: number; maximumSectionWords: number; defaultSceneCount: number; requirePlanConfirmation: boolean; requireFinalConfirmation: boolean; createdAt: string; updatedAt: string; }
export type SaveWritingPreferencesInput = Omit<WritingPreferences, 'createdAt' | 'updatedAt'>;
export type ChapterGenerationStatus = 'preparing' | 'needs_input' | 'planning' | 'plan_ready' | 'generating' | 'reviewing' | 'draft_ready' | 'accepted' | 'cancelled' | 'failed';
export interface ChapterGenerationJob { id: string; projectId: string; targetBookId: string; targetAfterChapterId?: string; requestedPages?: number; targetWords: number; requestedSceneCount?: number; userInstruction: string; status: ChapterGenerationStatus; activeProvider: string; contentContextHash: string; contextOverrideAccepted?: boolean; lastResumedAt?: string; createdAt: string; updatedAt: string; completedAt?: string; errorMessage?: string; }
export interface CreateChapterGenerationJobInput { projectId: string; targetBookId: string; targetAfterChapterId?: string; requestedPages?: number; targetWords: number; requestedSceneCount?: number; userInstruction: string; activeProvider: string; contentContextHash: string; }
export interface PlannedKnowledgeChange { characterId: string; factEntityId: string; nextState: CharacterKnowledgeKind; reason: string; }
export interface PlannedRelationshipChange { characterAId: string; characterBId: string; change: string; reason: string; }
export interface ChapterPlanBeat { id: string; orderIndex: number; title: string; purpose: string; location?: string; povCharacterId?: string; participatingCharacterIds: string[]; startingState: string; event: string; conflict: string; newInformation: string[]; knowledgeChanges: PlannedKnowledgeChange[]; relationshipChanges: PlannedRelationshipChange[]; cluesUsed: string[]; loreEntityIds: string[]; endingHook: string; targetWords: number; }
export interface ChapterGenerationPlan { id: string; jobId: string; chapterTitle: string; chapterGoal: string; povCharacterId?: string; startingState: string; endingState: string; chapterSummary: string; endingConnection: string; newInformation: string[]; withheldInformation: string[]; beats: ChapterPlanBeat[]; reviewStatus: 'pending' | 'accepted' | 'edited' | 'rejected'; reviewedAt?: string; createdAt: string; updatedAt: string; }
export type SaveChapterGenerationPlanInput = Omit<ChapterGenerationPlan, 'id' | 'createdAt' | 'updatedAt' | 'reviewedAt'> & { reviewStatus: ChapterGenerationPlan['reviewStatus']; };
export interface DraftCharacterState { characterId: string; state: string; change: string; }
export interface DraftObjectState { objectId: string; location: string; state: string; }
export interface DraftInjuryState { characterId: string; description: string; severity: string; }
export interface DraftContinuityState { currentLocation: string; currentStoryTime: string; presentCharacterIds: string[]; characterStates: DraftCharacterState[]; establishedFacts: string[]; knowledgeChanges: PlannedKnowledgeChange[]; relationshipChanges: PlannedRelationshipChange[]; movedObjects: DraftObjectState[]; injuries: DraftInjuryState[]; cluesIntroduced: string[]; promisesCreated: string[]; unresolvedActions: string[]; lastParagraphSummary: string; }
export interface ChapterGenerationSection { id: string; jobId: string; planBeatId: string; orderIndex: number; targetWords: number; actualWords: number; content: string; continuationSummary: string; continuityState: DraftContinuityState; status: 'pending' | 'generating' | 'generated' | 'reviewed' | 'regenerate_requested' | 'failed'; providerId?: string; createdAt: string; updatedAt: string; }
export type SaveChapterGenerationSectionInput = Omit<ChapterGenerationSection, 'id' | 'actualWords' | 'createdAt' | 'updatedAt'>;
export interface ChapterGenerationReview { id: string; jobId: string; sectionId?: string; reviewScope: 'section' | 'chapter'; issueType: string; severity: 'info' | 'warning' | 'blocking'; title: string; description: string; relatedEntityIds: string[]; relatedSourceIds: string[]; suggestedAction: string; status: string; createdAt: string; updatedAt: string; }
export type SaveChapterGenerationReviewInput = Omit<ChapterGenerationReview, 'id' | 'createdAt' | 'updatedAt' | 'jobId'> & { jobId?: string };
export interface ChapterDraftPlanResult extends Omit<ChapterGenerationPlan, 'id' | 'jobId' | 'reviewStatus' | 'createdAt' | 'updatedAt'> { assumptions: { type: string; text: string }[]; warnings: string[]; }
export interface ChapterSectionDraftResult { content: string; continuationSummary: string; continuityState: DraftContinuityState; usedEntityIds: string[]; usedMemoryIds: string[]; usedSourceIds: string[]; warnings: string[]; }
