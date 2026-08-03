import type { Chapter, ProjectContext, Project, StoryDirection, StoryEntity, WritingPreferences } from '../types/domain';
import type { StoryRepository } from './storyRepository';
import { DeterministicProjectContextBuilder } from './contextBuilder';

export interface LongformContextBundle extends ProjectContext {
  project: Project;
  storyDirection?: StoryDirection;
  writingPreferences: WritingPreferences;
  recentScenes: Array<{ id: string; chapterId: string; title: string; content: string; pov: string; location: string; storyTime: string }>;
  relevantEarlierScenes: Array<{ id: string; chapterId: string; title: string; content: string; pov: string; location: string; storyTime: string }>;
  previousSections?: Array<{ orderIndex: number; continuationSummary: string; contentTail: string; continuityState: unknown }>;
  targetWords: number;
  remainingWords: number;
}

export class LongformContextBundleBuilder {
  private readonly builder: DeterministicProjectContextBuilder;
  constructor(repository: StoryRepository) { this.builder = new DeterministicProjectContextBuilder(repository); }
  async build(input: { project: Project; chapters: Chapter[]; entities: StoryEntity[]; direction?: StoryDirection; preferences: WritingPreferences; userQuestion: string; currentSceneId?: string; targetWords: number; remainingWords: number; previousSections?: LongformContextBundle['previousSections'] }): Promise<LongformContextBundle> {
    const context = await this.builder.build({ projectId: input.project.id, currentSceneId: input.currentSceneId, userQuestion: input.userQuestion });
    const scenes = input.chapters.flatMap((chapter) => chapter.scenes.map((scene) => ({ id: scene.id, chapterId: scene.chapterId, title: scene.title, content: scene.content, pov: scene.pov, location: scene.location, storyTime: scene.storyTime })));
    return { ...context, project: input.project, storyDirection: input.direction, writingPreferences: input.preferences, recentScenes: scenes.slice(-8), relevantEarlierScenes: scenes.slice(0, -8).slice(-12), previousSections: input.previousSections, targetWords: input.targetWords, remainingWords: input.remainingWords };
  }
}
