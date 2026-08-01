import { useEffect, useState } from 'react';
import { Clock3, Download, LoaderCircle, RotateCcw, X } from 'lucide-react';
import type { Scene, SceneVersion } from '../../types/domain';

interface VersionHistoryProps {
  scene: Scene;
  onClose: () => void;
  onLoad: (sceneId: string) => Promise<SceneVersion[]>;
  onRestore: (sceneId: string, versionId: string) => Promise<Scene>;
}

export function VersionHistory({ scene, onClose, onLoad, onRestore }: VersionHistoryProps) {
  const [versions, setVersions] = useState<SceneVersion[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [restoring, setRestoring] = useState('');

  useEffect(() => {
    let active = true;
    setLoading(true);
    void onLoad(scene.id)
      .then((loaded) => { if (active) setVersions(loaded); })
      .catch((reason: unknown) => { if (active) setError(reason instanceof Error ? reason.message : 'Der Verlauf konnte nicht geladen werden.'); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [onLoad, scene.id]);

  const download = (version: SceneVersion) => {
    const blob = new Blob([version.content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `${scene.title.replace(/[^a-z0-9äöüß]+/gi, '-').replace(/^-|-$/g, '') || 'szene'}-version-${version.versionNumber}.txt`;
    link.click();
    URL.revokeObjectURL(url);
  };

  const restore = async (version: SceneVersion) => {
    setRestoring(version.id);
    setError('');
    try { await onRestore(scene.id, version.id); onClose(); }
    catch (reason: unknown) { setError(reason instanceof Error ? reason.message : 'Die Version konnte nicht wiederhergestellt werden.'); }
    finally { setRestoring(''); }
  };

  return <div className="history-backdrop" role="dialog" aria-modal="true" aria-labelledby="history-title">
    <section className="history-panel">
      <header className="history-head"><div><span className="eyebrow">SICHER GESPEICHERT</span><h2 id="history-title">Verlauf</h2><p>{scene.title} · Jede Speicherung ist eine eigene Version.</p></div><button className="history-close" onClick={onClose} aria-label="Verlauf schließen"><X size={20} /></button></header>
      {loading && <div className="history-state"><LoaderCircle className="spin" size={20} /> Verlauf wird geladen …</div>}
      {!loading && error && <div className="history-error" role="alert">{error}</div>}
      {!loading && !error && versions.length === 0 && <div className="history-state"><Clock3 size={21} /> Noch keine gespeicherten Versionen. Die erste erscheint nach dem nächsten Speichern.</div>}
      {!loading && !error && versions.length > 0 && <div className="history-list">{versions.map((version, index) => <article className={`history-entry ${index === 0 ? 'current' : ''}`} key={version.id}><div className="history-entry-main"><span className="history-version">Version {version.versionNumber}{index === 0 && <em>Letzter Stand</em>}</span><strong>{new Date(version.createdAt).toLocaleString('de-DE')}</strong><small>{version.content.trim() ? `${version.content.trim().split(/\s+/).length.toLocaleString('de-DE')} Wörter` : 'Leere Szene'} · {version.scene.pov || 'Keine Perspektivfigur'}</small></div><div className="history-entry-actions"><button className="icon-button" onClick={() => download(version)} aria-label={`Version ${version.versionNumber} herunterladen`} title="Herunterladen"><Download size={17} /></button><button className="ghost-button" onClick={() => void restore(version)} disabled={restoring !== ''} title="Version wiederherstellen">{restoring === version.id ? <LoaderCircle className="spin" size={16} /> : <RotateCcw size={16} />} Wiederherstellen</button></div></article>)}</div>}
      <footer className="history-foot"><span>Versionen werden lokal in SQLite gespeichert.</span><button className="ghost-button" onClick={onClose}>Schließen</button></footer>
    </section>
  </div>;
}
