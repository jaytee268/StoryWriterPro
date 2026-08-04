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
  continuity_passage: 'Die semantische Continuity-Prüfung wird über den aktiven AI-Provider ausgeführt; lokale Heuristiken markieren nur Kandidaten.',
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
  const entities = context.relevantEntities;
  const sourcesFor = (usedEntities: typeof entities, includeCurrentScene = false): ChatSource[] => {
    const ids = new Set(usedEntities.map((entity) => entity.id));
    const selected = context.relevantSources.filter((source) => (source.entityId ? ids.has(source.entityId) : includeCurrentScene && source.sceneId === context.currentScene?.id));
    return [...new Map(selected.map((source) => [source.id, { id: source.id, label: source.excerpt ? source.excerpt.slice(0, 42) : 'Quellenstelle', chapterId: source.chapterId, sceneId: source.sceneId, entityId: source.entityId, excerpt: source.excerpt, startOffset: source.startOffset, endOffset: source.endOffset }])).values()].slice(0, 8);
  };
  if (lower.includes('figur') || lower.includes('charakter')) {
    const characters = entities.filter((entity) => entity.type === 'character');
    const profiles = (context.characterProfiles ?? []).filter((profile) => characters.some((entity) => entity.id === profile.entityId));
    const profileText = profiles.map((profile) => { const entity = characters.find((item) => item.id === profile.entityId); return `${entity?.name ?? 'Figur'}: ${profile.coreWant || 'kein Ziel eingetragen'}${profile.fears ? `; Angst: ${profile.fears}` : ''}`; }).join(' · ');
    return { text: characters.length ? `In der aktuellen Projektumgebung sind ${characters.map((entity) => entity.name).join(', ')} als Figuren relevant. ${characters.map((entity) => `${entity.name}: ${entity.status === 'confirmed' ? 'bestätigt' : 'noch nicht bestätigt'}`).join(' · ')}${profileText ? ` Profile: ${profileText}` : ''}` : 'Im aktuellen Projektkontext ist keine Figur sicher belegt.', sources: sourcesFor(characters) };
  }
  if (lower.includes('stil') || lower.includes('erzählperspektive') || lower.includes('zeitform')) {
    const style = context.projectStyle;
    return { text: style ? `Der Projektstil ist als ${style.narrativePov || 'Perspektive noch offen'} und ${style.tense || 'Zeitform noch offen'} hinterlegt.${style.sentenceStyle ? ` Satzstil: ${style.sentenceStyle}` : ''}` : 'Für dieses Projekt ist noch kein Projektstil hinterlegt.', sources: [] };
  }
  if (lower.includes('verbunden') || lower.includes('relation') || lower.includes('mit daniel') || lower.includes('simulation')) {
    const relationLines = (context.entityRelations ?? []).map((relation) => { const source = entities.find((entity) => entity.id === relation.sourceEntityId); const target = entities.find((entity) => entity.id === relation.targetEntityId); return source && target ? `${source.name} ${relation.label || relation.relationType.replaceAll('_', ' ')} ${target.name}` : undefined; }).filter((line): line is string => Boolean(line));
    return { text: relationLines.length ? `Gespeicherte Verbindungen: ${relationLines.join(' · ')}` : 'Im aktuellen Projektkontext sind keine passenden Verbindungen gespeichert.', sources: sourcesFor(entities.filter((entity) => (context.entityRelations ?? []).some((relation) => relation.sourceEntityId === entity.id || relation.targetEntityId === entity.id))) };
  }
  if (lower.includes('weltregel') || lower.includes('lore') || lower.includes('welt')) {
    const lore = context.lore ?? [];
    return { text: lore.length ? `Relevante Lore: ${lore.map((item) => `${item.truthStatement || 'ohne Aussage'} (${item.category}, ${item.revealState === 'author_only' ? 'nur Autorwissen' : item.revealState === 'foreshadowed' ? 'angedeutet' : 'Leser kennt es'})`).join(' · ')}` : 'Für diese Frage ist noch keine Lore im Projektkontext hinterlegt.', sources: sourcesFor(entities.filter((entity) => lore.some((item) => item.entityId === entity.id))) };
  }
  if (lower.includes('handlungsstrang') || lower.includes('offen')) {
    return { text: context.openPlotThreads.length ? `Offene Handlungsstränge: ${context.openPlotThreads.map((entity) => `${entity.name} (${entity.status})`).join(', ')}.` : 'Für diese Szene ist kein offener Handlungsstrang im geladenen Kontext vermerkt.', sources: sourcesFor(context.openPlotThreads) };
  }
  if (lower.includes('vermut') || lower.includes('unbestätigt')) {
    const uncertain = entities.filter((entity) => !entity.authorConfirmed || entity.status === 'uncertain' || entity.status === 'proposed');
    return { text: uncertain.length ? `Noch unbestätigt sind: ${uncertain.map((entity) => `${entity.name} (${entity.status})`).join(', ')}.` : 'Im relevanten Kontext sind keine unbestätigten Einträge vorhanden.', sources: sourcesFor(uncertain) };
  }
  if (lower.includes('weiß') || lower.includes('wissen') || lower.includes('paketnummer')) {
    const matches = entities.filter((entity) => { const searchable = `${entity.name} ${entity.description} ${entity.tags.join(' ')}`.toLocaleLowerCase(); return lower.includes('paketnummer') ? searchable.includes('paket') || searchable.includes('nummer') : searchable.split(/\s+/).some((word) => word.length > 3 && lower.includes(word)); });
    return { text: matches.length ? `Im bestätigten beziehungsweise relevanten Kanon steht: ${matches.map((entity) => `${entity.name}: ${entity.description}`).join(' ')}` : 'Dazu ist im aktuellen bestätigten Kontext keine Information gespeichert.', sources: sourcesFor(matches) };
  }
  if (lower.includes('insider') || lower.includes('spitznamen') || lower.includes('spricht') || lower.includes('erlebt') || lower.includes('erlebnis')) {
    const patterns = context.characterVoicePatterns ?? []; const experiences = context.characterExperiences ?? []; const names = entities.filter((entity) => entity.type === 'character');
    if (lower.includes('insider')) { const memories = (context.relationshipMemories ?? []).filter((memory) => memory.memoryType === 'inside_joke' || memory.memoryType === 'shared_secret'); return { text: memories.length ? `Gespeicherte Insider und gemeinsame Geheimnisse: ${memories.map((memory) => memory.title || memory.summary).join(' · ')}` : 'Für dieses Figurenpaar ist kein bestätigter Insider gespeichert.', sources: [] }; }
    if (lower.includes('spitznamen') || lower.includes('spricht')) { const selected = patterns.filter((pattern) => pattern.patternType === 'nickname' || pattern.patternType === 'signature_phrase' || pattern.patternType === 'signature_word'); return { text: selected.length ? `Gespeicherte Sprachmuster: ${selected.map((pattern) => `${pattern.patternText} (${pattern.occurrenceCount} Beleg${pattern.occurrenceCount === 1 ? '' : 'e'})`).join(' · ')}` : 'Es ist noch kein bestätigtes Sprachmuster hinterlegt.', sources: [] }; }
    const selected = experiences.filter((experience) => names.some((name) => name.id === experience.characterId)); return { text: selected.length ? `Belegte Erlebnisse: ${selected.map((experience) => `${experience.title}: ${experience.objectiveSummary || 'ohne objektive Zusammenfassung'}`).join(' · ')}` : 'Für diese Figur sind noch keine belegten Erlebnisse gespeichert.', sources: [] };
  }
  if (lower.includes('wissensnotiz') || lower.includes('zustand')) {
    const states = context.characterStates ?? [];
    return { text: states.length ? `Szenenbezogene Wissensnotizen: ${states.map((state) => { const entity = entities.find((item) => item.id === state.characterEntityId); return `${entity?.name ?? 'Figur'}: ${state.knowledgeNotes || 'keine Notiz'}`; }).join(' · ')}. Diese Notizen sind freie Autorennotizen und kein strukturiertes Figurenwissen.` : 'Für diese Szene ist kein Charakterzustand gespeichert.', sources: [] };
  }
  return { text: context.currentScene ? `Ich habe die aktuelle Szene „${context.currentScene.title}“ und ${entities.length} relevante Story-Bible-Einträge geprüft. Für diese konkrete Frage finde ich im gespeicherten Kontext noch keine sichere Antwort.` : 'Ich finde keine aktuell ausgewählte Szene und kann deshalb keine quellengebundene Antwort geben.', sources: [] };
}

export class CliProviderPlaceholder implements AiProvider {
  constructor(public id: string, public displayName: string) {}
  async isAvailable(): Promise<boolean> { return false; }
  async getStatus(): Promise<ProviderStatus> { return { available: false, label: 'Nicht verbunden', detail: 'Offizieller lokaler Client noch nicht konfiguriert' }; }
  async runTask(): Promise<AiTaskResult> { throw new Error(`${this.displayName} ist für diese MVP-Version noch nicht verbunden.`); }
  async cancelTask(): Promise<void> { return Promise.resolve(); }
}

export const providers: AiProvider[] = [new MockProvider(), new CliProviderPlaceholder('codex-cli', 'Codex CLI'), new CliProviderPlaceholder('claude-cli', 'Claude CLI'), new CliProviderPlaceholder('grok-cli', 'Grok Build'), new CliProviderPlaceholder('gemini-cli', 'Gemini CLI'), new CliProviderPlaceholder('local-model', 'Lokales Modell')];
