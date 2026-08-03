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
export interface BibleUpdateRun { id: string; projectId: string; sceneId: string; sceneUpdatedAt: string; contentHash: string; extractorId: string; status: BibleRunStatus; createdAt: string; completedAt?: string; errorMessage?: string; }
export interface BibleProposal { id: string; runId: string; projectId: string; sceneId: string; targetEntityId?: string; proposalAction: BibleProposalAction; entityType: EntityType; candidateName: string; candidateDescription: string; candidateStatus: EntityStatus; confidence: number; classification: BibleClassification; evidenceExcerpt: string; startOffset?: number; endOffset?: number; reason: string; reviewStatus: BibleReviewStatus; reviewedAt?: string; createdAt: string; }
export interface BibleProposalDraft { targetEntityId?: string; proposalAction: BibleProposalAction; entityType: EntityType; candidateName: string; candidateDescription: string; candidateStatus: EntityStatus; confidence: number; classification: BibleClassification; evidenceExcerpt: string; startOffset?: number; endOffset?: number; reason: string; }
export interface BibleExtractionInput { project: Project; chapter: Chapter; scene: Scene; existingEntities: StoryEntity[]; previousAnalyzedContent?: string; changedRange?: { start: number; end: number }; }
export interface BibleExtractionResult { proposals: BibleProposalDraft[]; warnings: string[]; }
export interface BibleExtractor { readonly id: string; extract(input: BibleExtractionInput): Promise<BibleExtractionResult>; }
export interface ContextRequest { projectId: string; currentChapterId?: string; currentSceneId?: string; userQuestion: string; }
export interface ProjectContext { currentScene?: Scene; currentChapter?: Chapter; relevantEntities: StoryEntity[]; relevantSources: StorySourceReference[]; openPlotThreads: StoryEntity[]; possibleContradictions: StoryEntity[]; }
export interface ChatSource { id: string; label: string; chapterId?: string; sceneId?: string; entityId?: string; excerpt?: string; }
export interface TimelineEvent { id: string; title: string; storyTime: string; chapter: string; scene: string; location: string; characters: string[]; pov: string; summary: string; consequences: string; knowledge: string; clue?: string; status: EntityStatus; track: string; }
export interface MindNode { id: string; label: string; type: string; x: number; y: number; status?: EntityStatus; }
export interface MindEdge { id: string; source: string; target: string; label: string; }
export interface ChatMessage { id: string; role: 'user' | 'assistant'; content: string; sources?: ChatSource[]; time: string; }
export interface AiTask { id: string; type: AiTaskType; prompt: string; context: string[]; }
export interface ProviderStatus { available: boolean; label: string; detail: string; }
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
export interface CreateBibleUpdateRunInput { projectId: string; sceneId: string; sceneUpdatedAt: string; contentHash: string; extractorId: string; force?: boolean; }
export interface ReviewBibleProposalInput { proposalId: string; reviewStatus: BibleReviewStatus; candidateName?: string; candidateDescription?: string; candidateStatus?: EntityStatus; classification?: BibleClassification; }
