import type { AiTask, AiTaskType, ChatSource, ProjectContext, ProviderStatus } from '../types/domain';

export interface AiTaskResult { taskId: string; text: string; structured?: Record<string, unknown>; sources: ChatSource[]; }
export interface AiProvider { id: string; displayName: string; isAvailable(): Promise<boolean>; getStatus(): Promise<ProviderStatus>; runTask(task: AiTask): Promise<AiTaskResult>; cancelTask(taskId: string): Promise<void>; }

const responses: Record<AiTaskType, string> = {
  chat: 'Laut Band 1, Kapitel 3, Szene 2 erfährt Marek von der veränderten Paketnummer. Die aktuelle Szene spielt danach. Die Information ist daher konsistent.',
  bible_update: '7 neue Fakten erkannt, 2 Charakterveränderungen, 1 möglicher Widerspruch und 1 neues Timeline-Ereignis.',
  consistency_check: 'Die Szene ist weitgehend konsistent. Ein möglicher Konflikt betrifft die Uhrzeit im Café Meridian.',
  grammar_review: 'Die lokale Korrekturprüfung ist bereit. Änderungen werden einzeln als Diff angeboten.',
  manuscript_import: 'Manuskript lokal eingelesen und in überprüfbare Abschnitte gegliedert.',
  deep_research: 'Die Analyse wurde als lokaler, fortsetzbarer Job vorbereitet.',
  timeline_validation: 'Keine harten Zeitkonflikte in den geladenen Beispieldaten gefunden.',
  character_analysis: 'Mareks Reaktion passt zum bestätigten Charakterbogen; seine Lüge markiert eine neue Eskalationsstufe.',
};

export class MockProvider implements AiProvider {
  id = 'mock';
  displayName = 'Mock Provider';
  async isAvailable(): Promise<boolean> { return true; }
  async getStatus(): Promise<ProviderStatus> { return { available: true, label: 'Bereit', detail: 'Lokale Demo-Antworten, kein Netzwerkzugriff' }; }
  async runTask(task: AiTask): Promise<AiTaskResult> { await new Promise((resolve) => setTimeout(resolve, 240)); return { taskId: task.id, text: responses[task.type], sources: [] }; }
  async cancelTask(): Promise<void> { return Promise.resolve(); }
}

export function answerFromProjectContext(question: string, context: ProjectContext): { text: string; sources: ChatSource[] } {
  const lower = question.toLocaleLowerCase();
  const sources: ChatSource[] = context.relevantSources.map((source) => ({ id: source.id, label: source.excerpt ? source.excerpt.slice(0, 42) : 'Quellenstelle', chapterId: source.chapterId, sceneId: source.sceneId, entityId: source.entityId, excerpt: source.excerpt }));
  const entities = context.relevantEntities;
  if (lower.includes('figur') || lower.includes('charakter')) {
    const characters = entities.filter((entity) => entity.type === 'character');
    return { text: characters.length ? `In der aktuellen Projektumgebung sind ${characters.map((entity) => entity.name).join(', ')} als Figuren relevant. ${characters.map((entity) => `${entity.name}: ${entity.status === 'confirmed' ? 'bestätigt' : 'noch nicht bestätigt'}`).join(' · ')}` : 'Im aktuellen Projektkontext ist keine Figur sicher belegt.', sources };
  }
  if (lower.includes('handlungsstrang') || lower.includes('offen')) {
    return { text: context.openPlotThreads.length ? `Offene Handlungsstränge: ${context.openPlotThreads.map((entity) => `${entity.name} (${entity.status})`).join(', ')}.` : 'Für diese Szene ist kein offener Handlungsstrang im geladenen Kontext vermerkt.', sources };
  }
  if (lower.includes('vermut') || lower.includes('unbestätigt')) {
    const uncertain = entities.filter((entity) => !entity.authorConfirmed || entity.status === 'uncertain' || entity.status === 'proposed');
    return { text: uncertain.length ? `Noch unbestätigt sind: ${uncertain.map((entity) => `${entity.name} (${entity.status})`).join(', ')}.` : 'Im relevanten Kontext sind keine unbestätigten Einträge vorhanden.', sources };
  }
  if (lower.includes('weiß') || lower.includes('wissen') || lower.includes('paketnummer')) {
    const matches = entities.filter((entity) => `${entity.name} ${entity.description} ${entity.tags.join(' ')}`.toLocaleLowerCase().includes('paket') || entity.type === 'character');
    return { text: matches.length ? `Im bestätigten beziehungsweise relevanten Kanon steht: ${matches.map((entity) => `${entity.name}: ${entity.description}`).join(' ')}` : 'Dazu ist im aktuellen bestätigten Kontext keine Information gespeichert.', sources };
  }
  return { text: context.currentScene ? `Ich habe die aktuelle Szene „${context.currentScene.title}“ und ${entities.length} relevante Story-Bible-Einträge geprüft. Für diese konkrete Frage finde ich im gespeicherten Kontext noch keine sichere Antwort.` : 'Ich finde keine aktuell ausgewählte Szene und kann deshalb keine quellengebundene Antwort geben.', sources };
}

export class CliProviderPlaceholder implements AiProvider {
  constructor(public id: string, public displayName: string) {}
  async isAvailable(): Promise<boolean> { return false; }
  async getStatus(): Promise<ProviderStatus> { return { available: false, label: 'Nicht verbunden', detail: 'Offizieller lokaler Client noch nicht konfiguriert' }; }
  async runTask(): Promise<AiTaskResult> { throw new Error(`${this.displayName} ist für diese MVP-Version noch nicht verbunden.`); }
  async cancelTask(): Promise<void> { return Promise.resolve(); }
}

export const providers: AiProvider[] = [new MockProvider(), new CliProviderPlaceholder('codex-cli', 'Codex CLI'), new CliProviderPlaceholder('claude-cli', 'Claude CLI'), new CliProviderPlaceholder('grok-cli', 'Grok Build'), new CliProviderPlaceholder('gemini-cli', 'Gemini CLI'), new CliProviderPlaceholder('local-model', 'Lokales Modell')];
