import { desktopInvoke, isTauriRuntime } from './desktop';
import type { AiSetupMode, AiSetupState, ApiConnectionStatus, ApiCredentialStatus } from '../types/domain';

const browserSetupKey = 'storymemory.ai-setup.v1';
let browserCredentialConfigured = false;

const pendingState = (): AiSetupState => ({ status: 'pending', updatedAt: new Date().toISOString() });

function parseState(value: unknown): AiSetupState {
  if (!value || typeof value !== 'object') return pendingState();
  const state = value as Partial<AiSetupState>;
  if (state.status !== 'completed' || !state.selectedMode || !['api', 'codex-cli', 'offline'].includes(state.selectedMode)) return pendingState();
  return { status: 'completed', selectedMode: state.selectedMode, selectedProvider: state.selectedMode === 'api' ? 'openai-api' : undefined, completedAt: state.completedAt, updatedAt: typeof state.updatedAt === 'string' ? state.updatedAt : new Date().toISOString() };
}

export async function getAiSetupState(): Promise<AiSetupState> {
  if (isTauriRuntime()) return parseState(await desktopInvoke<AiSetupState>('get_ai_setup_state'));
  try { return parseState(JSON.parse(window.localStorage.getItem(browserSetupKey) ?? 'null')); } catch { return pendingState(); }
}

export async function completeAiSetup(selectedMode: AiSetupMode): Promise<AiSetupState> {
  const input = { status: 'completed' as const, selectedMode, selectedProvider: selectedMode === 'api' ? 'openai-api' as const : undefined, completedAt: new Date().toISOString() };
  if (isTauriRuntime()) return parseState(await desktopInvoke<AiSetupState>('save_ai_setup_state', { input }));
  const state = parseState({ ...input, updatedAt: new Date().toISOString() });
  window.localStorage.setItem(browserSetupKey, JSON.stringify(state));
  return state;
}

export async function getOpenAiApiKeyStatus(): Promise<ApiCredentialStatus> {
  if (isTauriRuntime()) return desktopInvoke<ApiCredentialStatus>('get_openai_api_key_status');
  return { configured: browserCredentialConfigured };
}

export async function setOpenAiApiKey(apiKey: string): Promise<ApiCredentialStatus> {
  if (!apiKey.trim()) throw new Error('Bitte gib einen API-Schlüssel ein.');
  if (apiKey.length > 4096 || apiKey.includes('\0') || apiKey.includes('\r') || apiKey.includes('\n')) throw new Error('Der API-Schlüssel ist ungültig.');
  if (isTauriRuntime()) return desktopInvoke<ApiCredentialStatus>('set_openai_api_key', { apiKey });
  browserCredentialConfigured = true;
  return { configured: true };
}

export async function deleteOpenAiApiKey(): Promise<ApiCredentialStatus> {
  if (isTauriRuntime()) return desktopInvoke<ApiCredentialStatus>('delete_openai_api_key');
  browserCredentialConfigured = false;
  return { configured: false };
}

export async function testOpenAiConnection(): Promise<ApiConnectionStatus> {
  if (isTauriRuntime()) return desktopInvoke<ApiConnectionStatus>('test_openai_api_connection');
  return browserCredentialConfigured ? { connected: true, detail: 'Browser-Demo: sicherer Fake-Credential-Adapter aktiv.' } : { connected: false, detail: 'Noch kein API-Schlüssel eingerichtet.' };
}
