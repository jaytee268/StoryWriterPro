export type EntityStatus = 'confirmed' | 'proposed' | 'uncertain' | 'contradicted' | 'retconned' | 'archived';
export type EntityType = 'character' | 'relationship' | 'place' | 'organization' | 'world_rule' | 'object' | 'event' | 'fact' | 'clue' | 'secret' | 'plot_thread' | 'retcon' | 'author_note';
export type AppView = 'dashboard' | 'editor' | 'bible' | 'timeline' | 'mindmap' | 'characters' | 'threads' | 'research' | 'files' | 'settings';
export type AiTaskType = 'chat' | 'bible_update' | 'consistency_check' | 'grammar_review' | 'manuscript_import' | 'deep_research' | 'timeline_validation' | 'character_analysis';
export type CorrectionKind = 'spelling' | 'grammar' | 'punctuation' | 'capitalization' | 'whitespace';

export type SceneStatus = 'draft' | 'revised' | 'final';
export interface Project { id: string; title: string; author: string; description: string; updatedAt: string; createdAt?: string; wordCount: number; openWarnings: number; bibleProgress: number; }
export interface Book { id: string; projectId: string; title: string; volume: number; createdAt?: string; updatedAt?: string; }
export interface Chapter { id: string; bookId: string; title: string; orderIndex: number; scenes: Scene[]; createdAt?: string; updatedAt?: string; }
export interface Scene { id: string; chapterId: string; title: string; orderIndex: number; content: string; pov: string; location: string; storyTime: string; status: SceneStatus; goal: string; notes: string; createdAt?: string; updatedAt?: string; }
export interface StoryEntity { id: string; projectId?: string; name: string; type: EntityType; description: string; status: EntityStatus; confidence: number; source: string; chapter: string; scene: string; authorConfirmed: boolean; updatedAt: string; createdAt?: string; tags: string[]; }
export interface TimelineEvent { id: string; title: string; storyTime: string; chapter: string; scene: string; location: string; characters: string[]; pov: string; summary: string; consequences: string; knowledge: string; clue?: string; status: EntityStatus; track: string; }
export interface MindNode { id: string; label: string; type: string; x: number; y: number; status?: EntityStatus; }
export interface MindEdge { id: string; source: string; target: string; label: string; }
export interface ChatMessage { id: string; role: 'user' | 'assistant'; content: string; sources?: string[]; time: string; }
export interface AiTask { id: string; type: AiTaskType; prompt: string; context: string[]; }
export interface ProviderStatus { available: boolean; label: string; detail: string; }
export interface Correction { id: string; kind: CorrectionKind; from: string; to: string; reason: string; start: number; end: number; }
export interface CorrectionResult { id: string; sourceText: string; corrections: Correction[]; provider: string; message?: string; }
export interface BibleProposal { id: string; title: string; kind: string; description: string; status: 'new' | 'accepted' | 'edited' | 'discarded' | 'hypothesis' | 'retcon'; }

export interface WorkspaceSnapshot { project: Project; books: Book[]; chapters: Chapter[]; entities: StoryEntity[]; }
export interface CreateProjectInput { title: string; author: string; description?: string; volumeTitle?: string; volume?: number; }
export interface CreateChapterInput { bookId: string; title: string; }
export interface CreateSceneInput { chapterId: string; title: string; }
export type UpdateSceneInput = Scene;
export interface SaveStoryEntityInput extends StoryEntity { projectId: string; }
