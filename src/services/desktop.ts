import { invoke } from '@tauri-apps/api/core';

export async function desktopInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T | null> {
  try { return await invoke<T>(command, args); } catch { return null; }
}
