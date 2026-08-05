import { beforeEach, describe, expect, it, vi } from 'vitest';
import { desktopInvoke } from './desktop';
import { capabilitiesSchema, providerRouter, settingsSchema } from './aiProviderService';

vi.mock('./desktop', () => ({
  desktopInvoke: vi.fn(),
  isTauriRuntime: vi.fn(() => true),
}));

const invoke = vi.mocked(desktopInvoke);

const rustSettings = {
  activeProvider: 'local-prototype',
  codexBinaryPath: null,
  codexModelOverride: null,
  bibleUpdateTimeoutSeconds: 120,
  chatTimeoutSeconds: 90,
  allowLocalFallback: true,
  codexPrivacyAcknowledgedAt: null,
};

const rustCapabilities = {
  installed: false,
  binaryPath: null,
  version: null,
  supportsExec: false,
  supportsJson: false,
  supportsEphemeral: false,
  supportsOutputSchema: false,
  supportsReadOnlySandbox: false,
  supportsSkipGitCheck: false,
  supportsModel: false,
  supportsDisableFeatures: false,
  authentication: 'unknown',
  compatible: false,
  detail: 'Codex nicht installiert',
};

describe('nullable Codex-Providerdaten', () => {
  beforeEach(() => invoke.mockReset());

  it('parst Rust-Settings mit null und stellt undefined bereit', () => {
    const parsed = settingsSchema.parse(rustSettings);
    expect(parsed.codexBinaryPath).toBeUndefined();
    expect(parsed.codexModelOverride).toBeUndefined();
    expect(parsed.codexPrivacyAcknowledgedAt).toBeUndefined();
  });

  it('behält echte Stringwerte unverändert', () => {
    const parsed = settingsSchema.parse({ ...rustSettings, codexBinaryPath: '/opt/codex', codexModelOverride: 'gpt-5', codexPrivacyAcknowledgedAt: '2026-08-05T12:00:00Z' });
    expect(parsed).toMatchObject({ codexBinaryPath: '/opt/codex', codexModelOverride: 'gpt-5', codexPrivacyAcknowledgedAt: '2026-08-05T12:00:00Z' });
  });

  it('parst Capabilities mit nullable binaryPath und version', () => {
    const parsed = capabilitiesSchema.parse(rustCapabilities);
    expect(parsed.binaryPath).toBeUndefined();
    expect(parsed.version).toBeUndefined();
  });

  it('lehnt ungültige Providerwerte, Zahlen und Booleans weiterhin ab', () => {
    expect(() => settingsSchema.parse({ ...rustSettings, activeProvider: 'other' })).toThrow();
    expect(() => settingsSchema.parse({ ...rustSettings, bibleUpdateTimeoutSeconds: 0 })).toThrow();
    expect(() => settingsSchema.parse({ ...rustSettings, allowLocalFallback: 'yes' })).toThrow();
  });

  it('akzeptiert die globalen Offline- und OpenAI-Modi ohne den alten Local-Prototype anzubieten', () => {
    expect(settingsSchema.parse({ ...rustSettings, activeProvider: 'offline' }).activeProvider).toBe('offline');
    expect(settingsSchema.parse({ ...rustSettings, activeProvider: 'openai-api', apiKeyConfigured: false }).activeProvider).toBe('openai-api');
  });

  it('speichert und lädt die normalisierten Einstellungen erneut', async () => {
    let saved = rustSettings;
    invoke.mockImplementation(async (...callArgs) => {
      const [command, args] = callArgs as [string, Record<string, unknown> | undefined];
      if (!command) return saved;
      if (command === 'get_ai_provider_settings') return saved;
      if (command === 'save_ai_provider_settings') {
        saved = { ...saved, ...((args as { input: object }).input) };
        return saved;
      }
      throw new Error(`unerwarteter Befehl: ${command}`);
    });

    const written = await providerRouter.saveSettings({ ...settingsSchema.parse(rustSettings), codexModelOverride: 'gpt-5' });
    const loaded = await providerRouter.getSettings();
    expect(written.codexModelOverride).toBe('gpt-5');
    expect(loaded.codexModelOverride).toBe('gpt-5');
    expect(loaded.codexBinaryPath).toBeUndefined();
    expect(loaded.codexPrivacyAcknowledgedAt).toBeUndefined();
  });
});
