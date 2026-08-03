import type { Chapter, ContextRequest, ProjectContext, StorySourceReference } from '../types/domain';
import type { StoryRepository } from './storyRepository';
import { editorContentToPlainText } from '../utils/editorContent';

const tokens = (value: string): string[] => value.toLocaleLowerCase().split(/[^\p{L}\p{N}]+/u).filter((token) => token.length > 2);

export interface ProjectContextBuilder { build(input: ContextRequest): Promise<ProjectContext>; }

export class DeterministicProjectContextBuilder implements ProjectContextBuilder {
  constructor(private readonly repository: StoryRepository) {}

  async build(input: ContextRequest): Promise<ProjectContext> {
    const workspace = await this.repository.loadWorkspace();
    const currentChapter = workspace.chapters.find((chapter) => chapter.id === input.currentChapterId) ?? workspace.chapters.find((chapter) => chapter.scenes.some((scene) => scene.id === input.currentSceneId));
    const currentScene = currentChapter?.scenes.find((scene) => scene.id === input.currentSceneId) ?? currentChapter?.scenes[0];
    const entities = workspace.entities.filter((entity) => entity.projectId === input.projectId && entity.status !== 'archived');
    const sources = await this.repository.listSourceReferences(input.projectId);
    const questionTokens = tokens(input.userQuestion);
    const sceneTokens = tokens(editorContentToPlainText(currentScene?.content ?? ''));
    const relevantEntities = entities.filter((entity) => {
      const searchable = tokens(`${entity.name} ${entity.description} ${entity.tags.join(' ')} ${entity.chapter} ${entity.scene}`);
      const sameChapter = currentChapter && (entity.chapter === currentChapter.title || sources.some((source) => source.entityId === entity.id && source.chapterId === currentChapter.id));
      const mentioned = sceneTokens.includes(entity.name.toLocaleLowerCase());
      const questionMatch = questionTokens.some((token) => searchable.includes(token));
      return Boolean(sameChapter || mentioned || questionMatch || entity.type === 'plot_thread' || entity.status === 'contradicted');
    }).slice(0, 30);
    const relevantIds = new Set(relevantEntities.map((entity) => entity.id));
    const relevantSources = sources.filter((source) => (currentScene && source.sceneId === currentScene.id) || (source.entityId && relevantIds.has(source.entityId))).slice(0, 30);
    return { projectId: input.projectId, currentScene, currentChapter, relevantEntities, relevantSources, openPlotThreads: relevantEntities.filter((entity) => entity.type === 'plot_thread' && entity.status !== 'confirmed'), possibleContradictions: relevantEntities.filter((entity) => entity.status === 'contradicted') };
  }
}

export function sourceToChatLabel(source: StorySourceReference, chapters: Chapter[]): string {
  const chapter = chapters.find((item) => item.id === source.chapterId);
  const scene = chapter?.scenes.find((item) => item.id === source.sceneId);
  return `${chapter?.title ?? 'Kapitel'} · ${scene?.title ?? 'Szene'}`;
}
