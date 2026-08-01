import { invoke } from '@tauri-apps/api/core';

export class DesktopCommandError extends Error {
  constructor(public readonly command: string, message: string, public readonly cause?: unknown) {
    super(message);
    this.name = 'DesktopCommandError';
  }
}

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function desktopInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) throw new DesktopCommandError(command, 'Dieser Tauri-Befehl ist nur in der Desktop-App verfügbar.');
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    const message = cause instanceof Error ? cause.message : typeof cause === 'string' ? cause : 'Unbekannter Desktop-Fehler';
    if (import.meta.env.DEV) console.error(`[StoryMemory] Tauri-Command ${command} fehlgeschlagen`, { argumentKeys: Object.keys(args ?? {}), cause });
    throw new DesktopCommandError(command, message, cause);
  }
}
