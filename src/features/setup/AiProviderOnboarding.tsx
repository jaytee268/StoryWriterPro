import { CheckCircle2, Eye, EyeOff, KeyRound, LockKeyhole, ShieldCheck, TerminalSquare, Wifi } from 'lucide-react';
import { useEffect, useState } from 'react';
import { completeAiSetup, getOpenAiApiKeyStatus, setOpenAiApiKey, testOpenAiConnection } from '../../services/aiSetupService';
import { providerRouter } from '../../services/aiProviderService';
import type { AiProviderSettings, AiSetupState } from '../../types/domain';
import type { ProviderStatusView } from '../../services/aiProviderService';

const privacyText = 'Deine Projekte und Manuskripte werden lokal gespeichert. Inhalte werden nur für eine von dir gestartete KI-Funktion an den ausgewählten Anbieter übertragen.';

export function AiProviderOnboarding({ settings, onSettingsChange, onCompleted }: { settings: AiProviderSettings; onSettingsChange: (settings: AiProviderSettings) => void; onCompleted: (state: AiSetupState) => void }) {
  const [mode, setMode] = useState<'api' | 'codex-cli' | 'offline'>();
  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [apiModel, setApiModel] = useState(settings.apiModelOverride ?? '');
  const [apiConfigured, setApiConfigured] = useState(false);
  const [apiStatus, setApiStatus] = useState<{ connected: boolean; detail: string }>();
  const [apiCheckSkipped, setApiCheckSkipped] = useState(false);
  const [codexStatus, setCodexStatus] = useState<ProviderStatusView>();
  const [privacyAcknowledged, setPrivacyAcknowledged] = useState(Boolean(settings.codexPrivacyAcknowledgedAt));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => { void getOpenAiApiKeyStatus().then((status) => setApiConfigured(status.configured)); }, []);

  const chooseApi = async () => { setError(''); setMode('api'); try { setCodexStatus(undefined); setApiConfigured((await getOpenAiApiKeyStatus()).configured); } catch (cause) { setError(cause instanceof Error ? cause.message : 'Der API-Schlüsselstatus konnte nicht geprüft werden.'); } };
  const chooseCodex = async () => { setError(''); setMode('codex-cli'); setCodexStatus(undefined); try { setCodexStatus(await providerRouter.getProviderStatus('codex-cli')); } catch (cause) { setError(cause instanceof Error ? cause.message : 'Codex konnte nicht geprüft werden.'); } };
  const saveMode = async (selected: 'api' | 'codex-cli' | 'offline') => {
    setBusy(true); setError('');
    try {
      const nextSettings: AiProviderSettings = selected === 'api'
        ? { ...settings, activeProvider: 'openai-api', apiModelOverride: apiModel.trim() || undefined, allowLocalFallback: false }
        : selected === 'codex-cli'
          ? { ...settings, activeProvider: 'codex-cli', codexPrivacyAcknowledgedAt: new Date().toISOString(), allowLocalFallback: false }
          : { ...settings, activeProvider: 'offline', allowLocalFallback: false };
      const saved = await providerRouter.saveSettings(nextSettings);
      onSettingsChange(saved);
      onCompleted(await completeAiSetup(selected));
    } catch (cause) { setError(cause instanceof Error ? cause.message : 'Die KI-Einrichtung konnte nicht gespeichert werden.'); } finally { setBusy(false); }
  };
  const saveApi = async () => { if (!apiConfigured) { setError('Bitte hinterlege zuerst einen API-Schlüssel.'); return; } if ((!apiStatus || !apiStatus.connected) && !apiCheckSkipped) { setError('Bitte prüfe den API-Schlüssel erfolgreich oder bestätige ausdrücklich, dass du die Prüfung überspringen möchtest.'); return; } await saveMode('api'); };
  const saveKey = async () => { setBusy(true); setError(''); try { const status = await setOpenAiApiKey(apiKey); setApiConfigured(status.configured); setApiKey(''); setApiStatus(undefined); setApiCheckSkipped(false); } catch (cause) { setError(cause instanceof Error ? cause.message : 'Der API-Schlüssel konnte nicht sicher gespeichert werden.'); } finally { setBusy(false); } };
  const testApi = async () => { setBusy(true); setError(''); try { const status = await testOpenAiConnection(); setApiStatus(status); setApiCheckSkipped(false); } catch (cause) { setError(cause instanceof Error ? cause.message : 'Die Verbindung konnte nicht geprüft werden.'); } finally { setBusy(false); } };

  return <main className="ai-setup-screen"><section className="ai-setup-card" aria-labelledby="ai-setup-title">
    <span className="eyebrow">ERSTE EINRICHTUNG</span>
    <h1 id="ai-setup-title">KI für StoryMemory einrichten</h1>
    <p className="ai-setup-intro">StoryMemory kann dein Manuskript lokal verwalten und vollständig offline als Schreibprogramm verwendet werden. Viele besonders leistungsfähige Funktionen wie Lore-Auswertung, Figurenwissen, Kontinuitätsprüfung, Szenenerkennung und Zusammenfassungen benötigen jedoch ein KI-Modell.</p>
    <div className="ai-setup-cards">
      <article className={`ai-setup-option ${mode === 'api' ? 'selected' : ''}`}><div className="ai-setup-option-icon"><KeyRound size={21} /></div><div className="ai-setup-option-copy"><h2>API-Schlüssel verwenden</h2><p>Verbinde StoryMemory direkt mit einem unterstützten KI-Anbieter. Die Nutzung wird über dein Konto beim jeweiligen Anbieter abgerechnet.</p><span className="ai-setup-badge">Einfachste Einrichtung</span></div><button className="primary-button" type="button" onClick={() => void chooseApi()}>API-Schlüssel einrichten</button></article>
      <article className={`ai-setup-option ${mode === 'codex-cli' ? 'selected' : ''}`}><div className="ai-setup-option-icon"><TerminalSquare size={21} /></div><div className="ai-setup-option-copy"><h2>Codex CLI verwenden</h2><p>Nutze deine lokal installierte und angemeldete Codex CLI. StoryMemory übergibt nur die für die jeweilige Analyse benötigten Inhalte.</p></div><button className="ghost-button" type="button" onClick={() => void chooseCodex()}>Codex CLI prüfen</button></article>
      <article className={`ai-setup-option offline-option ${mode === 'offline' ? 'selected' : ''}`}><div className="ai-setup-option-icon"><LockKeyhole size={21} /></div><div className="ai-setup-option-copy"><h2>Ohne KI fortfahren</h2><p>Du kannst StoryMemory auch ohne angebundene KI verwenden. Schreiben, Projekte, Kapitel, manuelle Story Bible und lokale Speicherung bleiben verfügbar. Automatische Analysen und viele Komfortfunktionen stehen dann jedoch nicht oder nur eingeschränkt zur Verfügung.</p></div><button className="ghost-button" type="button" onClick={() => void saveMode('offline')} disabled={busy}>Offline fortfahren</button></article>
    </div>
    {mode === 'api' && <div className="ai-setup-details"><div className="ai-setup-details-head"><div><span className="eyebrow">OPENAI API</span><h2>API-Zugang sicher einrichten</h2></div><span className="ai-setup-status"><ShieldCheck size={15} /> Schlüssel wird nicht im Projekt gespeichert</span></div><label className="settings-field"><span>API-Schlüssel</span><div className="ai-setup-secret-field"><input type={showKey ? 'text' : 'password'} value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={apiConfigured ? 'Schlüssel ist eingerichtet · zum Ersetzen eingeben' : 'API-Schlüssel eingeben'} autoComplete="off" /><button type="button" className="icon-button" aria-label={showKey ? 'API-Schlüssel verbergen' : 'API-Schlüssel anzeigen'} onClick={() => setShowKey((visible) => !visible)}>{showKey ? <EyeOff size={16} /> : <Eye size={16} />}</button></div></label>{apiKey && <button type="button" className="ghost-button" onClick={() => void saveKey()} disabled={busy}>Schlüssel sicher speichern</button>}<label className="settings-field"><span>Modell <small>optional</small></span><input value={apiModel} onChange={(event) => setApiModel(event.target.value)} placeholder="Standardmodell verwenden" /></label><div className="ai-setup-test-row"><button type="button" className="ghost-button" onClick={() => void testApi()} disabled={busy || !apiConfigured}><Wifi size={15} /> Verbindung testen</button><button type="button" className="text-button" onClick={() => setApiCheckSkipped(true)} disabled={busy || !apiConfigured || Boolean(apiStatus?.connected)}>Prüfung überspringen</button>{apiStatus && <span className={apiStatus.connected ? 'ai-setup-success' : 'ai-setup-error'}>{apiStatus.connected ? <CheckCircle2 size={15} /> : null}{apiStatus.detail}</span>}</div><p className="ai-setup-cost-note">Für API-Aufrufe können Kosten beim Anbieter entstehen. StoryMemory zeigt keine konkreten Preisversprechen an.</p><div className="ai-setup-details-actions"><button type="button" className="ghost-button" onClick={() => void saveApi()} disabled={busy || !apiConfigured}>API-Modus verwenden</button></div><p className="ai-setup-privacy"><ShieldCheck size={15} /> Der API-Schlüssel wird im Schlüsselbund deines Betriebssystems gespeichert und nicht in deinem StoryMemory-Projekt abgelegt.</p></div>}
    {mode === 'codex-cli' && <div className="ai-setup-details"><div className="ai-setup-details-head"><div><span className="eyebrow">CODEX CLI</span><h2>Lokale Codex-Prüfung</h2></div><button type="button" className="ghost-button" onClick={() => void chooseCodex()} disabled={busy}>Erneut prüfen</button></div><div className="ai-setup-codex-status"><TerminalSquare size={19} /><span><strong>{codexStatus?.label ?? 'Wird geprüft …'}</strong><small>{codexStatus?.capabilities?.binaryPath ?? 'Kein Binary erkannt'} · {codexStatus?.capabilities?.version ?? 'Version unbekannt'}</small><small>{codexStatus?.detail ?? 'Authentifizierungsstatus wird geprüft.'}</small></span>{codexStatus?.available ? <CheckCircle2 size={18} /> : null}</div><p className="ai-setup-privacy">StoryMemory übergibt Codex nur die für eine von dir gestartete Funktion benötigten Inhalte. Die bestehende lokale Zugriffserklärung bleibt erhalten.</p><label className="provider-choice"><input type="checkbox" checked={privacyAcknowledged} onChange={(event) => setPrivacyAcknowledged(event.target.checked)} /> Ich verstehe die lokale Codex-Zugriffsgrenze.</label><button type="button" className="primary-button" onClick={() => void saveMode('codex-cli')} disabled={busy || !privacyAcknowledged || !codexStatus?.available}>Codex CLI verwenden</button></div>}
    {error && <div className="save-error" role="alert">{error}</div>}
    <div className="ai-setup-footer"><ShieldCheck size={16} /><span>{privacyText}</span></div>
  </section></main>;
}
