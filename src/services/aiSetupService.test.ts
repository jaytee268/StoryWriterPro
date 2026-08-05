import { beforeEach, describe, expect, it, vi } from 'vitest';
import { desktopInvoke } from './desktop';
import { completeAiSetup, deleteOpenAiApiKey, getAiSetupState, getOpenAiApiKeyStatus, setOpenAiApiKey, testOpenAiConnection } from './aiSetupService';

vi.mock('./desktop', () => ({ desktopInvoke: vi.fn(), isTauriRuntime: vi.fn(() => false) }));

describe('globaler KI-Einrichtungsstatus', () => {
  beforeEach(() => { const values = new Map<string, string>(); vi.stubGlobal('window', { localStorage: { clear: () => values.clear(), getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value) } }); vi.mocked(desktopInvoke).mockReset(); });

  it('zeigt bei fehlendem Status das Setup und akzeptiert Offline als Abschluss', async () => {
    expect((await getAiSetupState()).status).toBe('pending');
    const completed = await completeAiSetup('offline');
    expect(completed).toMatchObject({ status: 'completed', selectedMode: 'offline' });
    expect((await getAiSetupState()).selectedMode).toBe('offline');
  });

  it('hält den Browser-Demo-Credentialadapter im Speicher und nie in localStorage', async () => {
    await setOpenAiApiKey('fake-test-key');
    expect(await getOpenAiApiKeyStatus()).toEqual({ configured: true });
    expect(window.localStorage.getItem('storymemory.ai-setup.v1')).toBeNull();
    expect(JSON.stringify(window.localStorage)).not.toContain('fake-test-key');
    expect((await testOpenAiConnection()).connected).toBe(true);
    await deleteOpenAiApiKey();
    expect(await getOpenAiApiKeyStatus()).toEqual({ configured: false });
  });
});
