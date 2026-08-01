import type { Scene } from '../types/domain';

export type SceneSaveStatus = 'saved' | 'dirty' | 'saving' | 'error';
type QueueCallbacks = {
  onStatus: (status: SceneSaveStatus) => void;
  onSaved: (scene: Scene) => void;
  onError: (error: unknown) => void;
};

/**
 * Serializes scene writes. The pending snapshot is deliberately retained until
 * the repository confirms the write, so a failed desktop command can always be
 * retried without losing text.
 */
export class SceneSaveQueue {
  private pending?: Scene;
  private timer?: ReturnType<typeof setTimeout>;
  private inFlight?: Promise<void>;
  private flushPromise?: Promise<void>;
  private version = 0;
  private status: SceneSaveStatus = 'saved';
  private lastError?: unknown;
  private disposed = false;

  constructor(
    private readonly save: (scene: Scene) => Promise<Scene>,
    private readonly callbacks: QueueCallbacks,
    private readonly debounceMs = 750,
  ) {}

  schedule(scene: Scene): void {
    if (this.disposed) return;
    this.pending = scene;
    this.version += 1;
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => { void this.flush(); }, this.debounceMs);
    // Keep an existing error visible until the failed snapshot is explicitly
    // retried successfully. A new draft is still retained in pending.
    if (this.status !== 'error') this.setStatus('dirty');
  }

  hasPendingChanges(): boolean {
    return this.pending !== undefined || this.inFlight !== undefined;
  }

  getStatus(): SceneSaveStatus {
    return this.status;
  }

  getError(): unknown {
    return this.lastError;
  }

  async flush(): Promise<void> {
    if (this.flushPromise) return this.flushPromise;
    this.flushPromise = this.runFlush();
    try {
      await this.flushPromise;
    } finally {
      this.flushPromise = undefined;
    }
  }

  async dispose(): Promise<void> {
    if (this.disposed) return;
    await this.flush();
    this.disposed = true;
    if (this.timer) clearTimeout(this.timer);
    this.timer = undefined;
  }

  private async runFlush(): Promise<void> {
    if (this.timer) clearTimeout(this.timer);
    this.timer = undefined;

    while (this.pending && !this.inFlight) {
      const snapshot = this.pending;
      const snapshotVersion = this.version;
      this.setStatus('saving');
      this.inFlight = (async () => {
        try {
          const saved = await this.save(snapshot);
          if (snapshotVersion === this.version) {
            this.pending = undefined;
            this.lastError = undefined;
            this.callbacks.onSaved(saved);
            this.setStatus('saved');
          } else if (this.status !== 'error') {
            this.setStatus('dirty');
          }
        } catch (error) {
          // Keep the exact snapshot if no newer draft exists. If a newer draft
          // exists, pending already points to that newer value.
          if (snapshotVersion === this.version) this.pending = snapshot;
          this.lastError = error;
          this.setStatus('error');
          this.callbacks.onError(error);
        } finally {
          this.inFlight = undefined;
        }
      })();
      await this.inFlight;

      // A failed write must stop here. The visible error and pending snapshot
      // are intentionally left for the user-triggered retry.
      if (this.status === 'error') return;
    }
  }

  private setStatus(status: SceneSaveStatus): void {
    this.status = status;
    this.callbacks.onStatus(status);
  }
}
