import { Database, RefreshCw, ShieldCheck } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { Project } from '../../types/domain';
import type { DatabaseInfo, RuntimeMode } from '../../services/storyRepository';
import { createStoryRepository } from '../../services/storyRepository';

export function SettingsView({ mode, project, onReload }: { mode: RuntimeMode; project: Project; onReload: () => Promise<void> }) {
  const [database, setDatabase] = useState<DatabaseInfo>();
  const [loading, setLoading] = useState(true);
  useEffect(() => { let active = true; void createStoryRepository().getDatabaseInfo().then((info) => { if (active) setDatabase(info); }).finally(() => { if (active) setLoading(false); }); return () => { active = false; }; }, []);
  return <section className="settings-view simple-settings"><div className="view-heading"><div><span className="eyebrow">RUHIG UND LOKAL</span><h1>Einstellungen</h1><p>Die wichtigsten Informationen zu deiner lokalen StoryMemory-App.</p></div><button className="ghost-button large" onClick={() => void onReload()}><RefreshCw size={17} /> Daten neu laden</button></div><div className="settings-list"><div className="settings-row"><Database size={21} /><span><strong>App-Modus</strong><small>{mode === 'desktop' ? 'Desktop-App mit SQLite' : 'Browser-Demo mit localStorage'}</small></span><em>{mode === 'desktop' ? 'Desktop' : 'Demo'}</em></div><div className="settings-row"><Database size={21} /><span><strong>Datenbankstatus</strong><small>{loading ? 'Wird geprüft …' : database?.detail ?? 'Nicht verfügbar'}</small></span><em className={database?.connected ? 'settings-ok' : ''}>{database?.connected ? 'Verbunden' : 'Unbekannt'}</em></div><div className="settings-row"><ShieldCheck size={21} /><span><strong>Aktuelles Projekt</strong><small>{project.title} · {project.author || 'Autor nicht angegeben'}</small></span><em>Lokal</em></div></div>{database && <details className="settings-path"><summary>Speicherort anzeigen</summary><code>{database.path}</code></details>}<div className="privacy-note"><ShieldCheck size={17} /><span>Deine Manuskripte und Story-Bible-Daten werden lokal gespeichert. Inhalte werden nur dann an einen KI-Anbieter gesendet, wenn du eine entsprechende Analyse startest.</span></div></section>;
}
