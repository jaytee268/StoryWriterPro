import type { AiTask, AiTaskType, ProviderStatus } from '../types/domain';

export interface AiTaskResult { taskId: string; text: string; structured?: Record<string, unknown>; sources: string[]; }
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
  async runTask(task: AiTask): Promise<AiTaskResult> { await new Promise((resolve) => setTimeout(resolve, 240)); return { taskId: task.id, text: responses[task.type], sources: ['Band 1', 'Kapitel 3', 'Szene 2'] }; }
  async cancelTask(): Promise<void> { return Promise.resolve(); }
}

export class CliProviderPlaceholder implements AiProvider {
  constructor(public id: string, public displayName: string) {}
  async isAvailable(): Promise<boolean> { return false; }
  async getStatus(): Promise<ProviderStatus> { return { available: false, label: 'Nicht verbunden', detail: 'Offizieller lokaler Client noch nicht konfiguriert' }; }
  async runTask(): Promise<AiTaskResult> { throw new Error(`${this.displayName} ist für diese MVP-Version noch nicht verbunden.`); }
  async cancelTask(): Promise<void> { return Promise.resolve(); }
}

export const providers: AiProvider[] = [new MockProvider(), new CliProviderPlaceholder('codex-cli', 'Codex CLI'), new CliProviderPlaceholder('claude-cli', 'Claude CLI'), new CliProviderPlaceholder('grok-cli', 'Grok Build'), new CliProviderPlaceholder('gemini-cli', 'Gemini CLI'), new CliProviderPlaceholder('local-model', 'Lokales Modell')];
