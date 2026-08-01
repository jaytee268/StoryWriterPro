import type { Scene } from '../types/domain';

export type SceneSaveStatus = 'saved' | 'dirty' | 'saving' | 'error';
type QueueCallbacks = { onStatus: (status: SceneSaveStatus) => void; onSaved: (scene: Scene) => void; onError: (error: unknown) => void };

export class SceneSaveQueue {
  private pending?: Scene;
  private timer?: ReturnType<typeof setTimeout>;
  private inFlight: Promise<void> | undefined;
  private version = 0;

  constructor(private readonly save: (scene: Scene) => Promise<Scene>, private readonly callbacks: QueueCallbacks, private readonly debounceMs = 750) {}

  schedule(scene: Scene): void {
    this.pending = scene;
    this.version += 1;
    this.callbacks.onStatus('dirty');
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => { void this.flush(); }, this.debounceMs);
  }

  async flush(): Promise<void> {
    if (this.timer) { clearTimeout(this.timer); this.timer = undefined; }
    if (this.inFlight) await this.inFlight;
    if (!this.pending) return;
    const snapshot = this.pending;
    this.pending = undefined;
    const version = this.version;
    this.callbacks.onStatus('saving');
    this.inFlight = (async () => {
      try {
        const saved = await this.save(snapshot);
        if (version === this.version) { this.callbacks.onSaved(saved); this.callbacks.onStatus('saved'); }
      } catch (error) {
        if (version === this.version) { this.callbacks.onStatus('error'); this.callbacks.onError(error); }
      } finally { this.inFlight = undefined; }
    })();
    await this.inFlight;
    if (this.pending) await this.flush();
  }

  async dispose(): Promise<void> { await this.flush(); }
}
