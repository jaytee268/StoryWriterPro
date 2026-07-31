# Provider-Bridges

Die interne Schnittstelle lautet:

```ts
interface AiProvider {
  id: string;
  displayName: string;
  isAvailable(): Promise<boolean>;
  getStatus(): Promise<ProviderStatus>;
  runTask(task: AiTask): Promise<AiTaskResult>;
  cancelTask(taskId: string): Promise<void>;
}
```

Der MVP enthält `MockProvider` sowie bewusst passive Platzhalter für Codex CLI, Claude CLI, Grok Build, Gemini CLI und lokale Modelle. Eine Bridge darf nur offizielle lokale Clients bzw. dokumentierte CLI-/ACP-/JSON-RPC-Wege verwenden. Gemini bleibt so lange ein manueller oder separat autorisierter Modus, bis ein ausdrücklich erlaubter App-Zugriff verfügbar ist.

Geplanter Ablauf: Kontext auswählen → Vorschau anzeigen → schreibgeschützten Snapshot erzeugen → offiziellen Prozess starten → strukturierte Ausgabe validieren → Review zeigen → nur bestätigte Patches in SQLite übernehmen.
