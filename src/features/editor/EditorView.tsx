import { useEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import { ArrowLeft, ChevronDown, FileDown, History, LoaderCircle, Plus, Save, StickyNote, Type } from 'lucide-react';
import type { Chapter, EditorPreferences, Scene, SceneVersion } from '../../types/domain';
import { SceneSaveQueue, type SceneSaveStatus } from '../../services/sceneSaveQueue';
import { VersionHistory } from './VersionHistory';

interface EditorProps {
  chapters: Chapter[];
  scene?: Scene;
  chapter?: Chapter;
  onBack: () => void;
  onSelectScene: (id: string) => void;
  onSave: (scene: Scene) => Promise<Scene>;
  onCreateChapter: (title: string) => Promise<Chapter>;
  onCreateScene: (chapterId: string, title: string) => Promise<Scene>;
  onListVersions: (sceneId: string) => Promise<SceneVersion[]>;
  onRestoreVersion: (sceneId: string, versionId: string) => Promise<Scene>;
  onGetEditorPreferences: () => Promise<EditorPreferences>;
  onSaveEditorPreferences: (preferences: EditorPreferences) => Promise<EditorPreferences>;
  onSaveStateChange: (status: SceneSaveStatus) => void;
}

const saveLabels: Record<SceneSaveStatus, string> = { saved: 'Gespeichert', dirty: 'Nicht gespeichert', saving: 'Speichert …', error: 'Speicherfehler' };
const defaultPreferences: EditorPreferences = { fontFamily: 'serif', fontSize: 18, lineHeight: 1.95 };
const manuscriptFonts: Record<EditorPreferences['fontFamily'], string> = {
  serif: "'Libre Baskerville', Georgia, serif",
  sans: "'DM Sans', Arial, sans-serif",
  typewriter: "'DM Mono', 'Courier New', monospace",
};

export function EditorView({ chapters, scene, chapter, onBack, onSelectScene, onSave, onCreateChapter, onCreateScene, onListVersions, onRestoreVersion, onGetEditorPreferences, onSaveEditorPreferences, onSaveStateChange }: EditorProps) {
  const [draftScene, setDraftScene] = useState<Scene | undefined>(scene);
  const latestDraft = useRef<Scene | undefined>(scene);
  const queue = useRef<SceneSaveQueue | undefined>(undefined);
  const [saveStatus, setSaveStatus] = useState<SceneSaveStatus>('saved');
  const [saveError, setSaveError] = useState('');
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [displayOpen, setDisplayOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [preferences, setPreferences] = useState<EditorPreferences>(defaultPreferences);
  const [preferencesLoaded, setPreferencesLoaded] = useState(false);
  const [busyAction, setBusyAction] = useState<'chapter' | string | null>(null);

  useEffect(() => {
    let active = true;
    void onGetEditorPreferences().then((loaded) => { if (active) setPreferences(loaded); }).catch(() => undefined).finally(() => { if (active) setPreferencesLoaded(true); });
    return () => { active = false; };
  }, [onGetEditorPreferences]);

  useEffect(() => {
    if (!preferencesLoaded) return undefined;
    const timeout = window.setTimeout(() => { void onSaveEditorPreferences(preferences); }, 450);
    return () => window.clearTimeout(timeout);
  }, [onSaveEditorPreferences, preferences, preferencesLoaded]);

  // Parent state changes after a save must not reset the user's current draft while the same scene remains selected.
  useEffect(() => {
    const next = scene ? { ...scene } : undefined;
    setDraftScene(next); latestDraft.current = next; setSaveStatus('saved'); setSaveError(''); setDetailsOpen(false); setHistoryOpen(false); onSaveStateChange('saved');
    const nextQueue = new SceneSaveQueue(onSave, { onStatus: (status) => { setSaveStatus(status); onSaveStateChange(status); if (status !== 'error') setSaveError(''); }, onSaved: (saved) => { latestDraft.current = saved; setDraftScene((current) => current?.id === saved.id ? { ...current, updatedAt: saved.updatedAt } : current); }, onError: (error) => { setSaveError(error instanceof Error ? error.message : 'Die Szene konnte nicht gespeichert werden.'); } });
    queue.current = nextQueue;
    return () => { void nextQueue.dispose(); };
  }, [scene?.id, onSave, onSaveStateChange]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const handleSaveShortcut = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') { event.preventDefault(); void queue.current?.flush(); }
    };
    window.addEventListener('keydown', handleSaveShortcut);
    return () => window.removeEventListener('keydown', handleSaveShortcut);
  }, []);

  const updateDraft = (changes: Partial<Scene>) => {
    const base = latestDraft.current ?? scene;
    if (!base) return;
    const next = { ...base, ...changes };
    latestDraft.current = next; setDraftScene(next); queue.current?.schedule(next);
  };
  const selectScene = async (id: string) => { await queue.current?.flush(); onSelectScene(id); };
  const selectChapter = async (selectedChapter: Chapter) => { const firstScene = selectedChapter.scenes[0]; if (firstScene) await selectScene(firstScene.id); };
  const addScene = async (chapterId: string) => { setBusyAction(chapterId); setSaveError(''); try { await queue.current?.flush(); const created = await onCreateScene(chapterId, 'Neue Szene'); onSelectScene(created.id); } catch (error) { setSaveError(error instanceof Error ? error.message : 'Die Szene konnte nicht angelegt werden.'); } finally { setBusyAction(null); } };
  const addChapter = async () => { setBusyAction('chapter'); setSaveError(''); try { await queue.current?.flush(); const createdChapter = await onCreateChapter(`Kapitel ${chapters.length + 1}`); const createdScene = await onCreateScene(createdChapter.id, 'Neue Szene'); onSelectScene(createdScene.id); } catch (error) { setSaveError(error instanceof Error ? error.message : 'Das Kapitel konnte nicht angelegt werden.'); } finally { setBusyAction(null); } };
  const openHistory = async () => { await queue.current?.flush(); setHistoryOpen(true); };
  const restoreVersion = async (sceneId: string, versionId: string) => { const restored = await onRestoreVersion(sceneId, versionId); latestDraft.current = restored; setDraftScene(restored); setSaveStatus('saved'); onSaveStateChange('saved'); return restored; };
  const wordCount = useMemo(() => draftScene?.content.trim() ? draftScene.content.trim().split(/\s+/).length : 0, [draftScene?.content]);
  const paperStyle = { '--manuscript-font': manuscriptFonts[preferences.fontFamily], '--manuscript-size': `${preferences.fontSize}px`, '--manuscript-leading': preferences.lineHeight } as CSSProperties;

  return <section className="editor-view simple-editor">
    <div className="simple-editor-head"><div className="editor-heading"><button className="back-button" onClick={onBack}><ArrowLeft size={16} /> Zurück</button><span className="eyebrow">SCHREIBEN</span><h1>{chapter?.title ?? 'Kapitel auswählen'}</h1><div className="scene-selector simple-scene-selector"><ChevronDown size={16} /> {draftScene?.title ?? 'Keine Szene'}</div></div><div className="simple-editor-actions"><span className={`save-state save-state-${saveStatus}`}><Save size={15} /> {saveLabels[saveStatus]}</span><button className="ghost-button large" onClick={() => void openHistory()} disabled={!draftScene}><History size={17} /> Verlauf</button></div></div>
    <div className="editor-layout simple-editor-layout outline-collapsed"><aside id="manuscript-outline" className="document-tree simple-tree chapter-sidebar-collapsed">
      <div className="chapter-rail" aria-label="Kapitelübersicht">{chapters.map((item) => <button key={item.id} className={`chapter-rail-number ${item.id === chapter?.id ? 'active' : ''}`} onClick={() => void selectChapter(item)} aria-label={`${item.title} öffnen`} title={item.title}>{String(item.orderIndex).padStart(2, '0')}</button>)}</div>
      <div className="tree-expanded-content"><div className="tree-head"><span><strong>Dein Manuskript</strong><small>Kapitel und Szenen</small></span><button className="text-button" onClick={() => void addChapter()} disabled={busyAction !== null}>{busyAction === 'chapter' ? <LoaderCircle className="spin" size={16} /> : <Plus size={16} />} Kapitel</button></div><div className="tree-list">{chapters.map((item) => <div key={item.id} className="tree-chapter"><button className={`tree-row ${item.id === chapter?.id ? 'active' : ''}`} onClick={() => void selectChapter(item)}><ChevronDown size={15} /><span>{item.title}</span><span className="scene-count">{item.scenes.length}</span></button>{item.scenes.map((itemScene) => <button key={itemScene.id} className={`tree-scene ${itemScene.id === draftScene?.id ? 'active' : ''}`} onClick={() => void selectScene(itemScene.id)} disabled={busyAction !== null}><span className="scene-marker" />{itemScene.title}</button>)}<button className="add-row" onClick={() => void addScene(item.id)} disabled={busyAction !== null}>{busyAction === item.id ? <LoaderCircle className="spin" size={15} /> : <Plus size={15} />} Szene hinzufügen</button></div>)}</div><button className="export-button"><FileDown size={16} /> Exportieren</button></div>
    </aside><div className="manuscript-column simple-manuscript"><div className="simple-writing-bar"><span>{wordCount.toLocaleString('de-DE')} Wörter</span><div className="writing-bar-actions"><span>Autosave · 750 ms</span><button className="writing-preferences-toggle" onClick={() => setDisplayOpen((open) => !open)} aria-expanded={displayOpen}><Type size={15} /> Schrift &amp; Layout</button></div></div>{displayOpen && <div className="writing-preferences"><label>Schrift<select value={preferences.fontFamily} onChange={(event) => setPreferences((current) => ({ ...current, fontFamily: event.target.value as EditorPreferences['fontFamily'] }))}><option value="serif">Roman · Serif</option><option value="sans">Klar · Sans</option><option value="typewriter">Schreibmaschine · Mono</option></select></label><label>Größe<strong>{preferences.fontSize}px</strong><input type="range" min="14" max="28" step="1" value={preferences.fontSize} onChange={(event) => setPreferences((current) => ({ ...current, fontSize: Number(event.target.value) }))} /></label><label>Zeilenabstand<strong>{preferences.lineHeight.toFixed(2)}</strong><input type="range" min="1.3" max="2.5" step="0.05" value={preferences.lineHeight} onChange={(event) => setPreferences((current) => ({ ...current, lineHeight: Number(event.target.value) }))} /></label></div>}<article className="manuscript-page" style={paperStyle}>{draftScene ? <><div className="page-kicker">{chapter?.title} · {draftScene.title}</div><textarea className="editable" value={draftScene.content} onChange={(event) => updateDraft({ content: event.target.value })} aria-label="Szenentext" lang="de" spellCheck /><div className="page-footer"><span>Lokaler Entwurf · Versionen im Verlauf</span><span>{wordCount.toLocaleString('de-DE')} Wörter</span></div></> : <div className="empty-state">Lege ein Kapitel und eine Szene an, um zu schreiben.</div>}</article></div></div>
    <div className="simple-editor-bottom"><button className="details-toggle" onClick={() => setDetailsOpen((open) => !open)} disabled={!draftScene}><span><strong><StickyNote size={15} /> Szeneninfos &amp; Notizen</strong><small>{draftScene?.pov || 'Perspektive nicht festgelegt'} · {draftScene?.location || 'Ort nicht festgelegt'}</small></span><ChevronDown size={18} className={detailsOpen ? 'rotated' : ''} /></button></div>
    {saveError && <div className="save-error" role="alert"><strong>{saveLabels.error}</strong><span>{saveError}</span><button className="text-button" onClick={() => { if (latestDraft.current) queue.current?.schedule(latestDraft.current); }}>Erneut speichern</button></div>}
    {detailsOpen && draftScene && <div className="scene-details-simple"><MetaField label="Perspektivfigur" value={draftScene.pov} onChange={(value) => updateDraft({ pov: value })} /><MetaField label="Ort" value={draftScene.location} onChange={(value) => updateDraft({ location: value })} /><MetaField label="Zeitpunkt" value={draftScene.storyTime} onChange={(value) => updateDraft({ storyTime: value })} /><label className="field-label">Status<select value={draftScene.status} onChange={(event) => updateDraft({ status: event.target.value as Scene['status'] })}><option value="draft">Entwurf</option><option value="revised">Überarbeitet</option><option value="final">Final</option></select></label><label className="field-label">Szenenziel<textarea value={draftScene.goal} onChange={(event) => updateDraft({ goal: event.target.value })} rows={2} /></label><label className="field-label">Notizen<textarea value={draftScene.notes} onChange={(event) => updateDraft({ notes: event.target.value })} rows={2} /></label></div>}
    {historyOpen && draftScene && <VersionHistory scene={draftScene} onClose={() => setHistoryOpen(false)} onLoad={onListVersions} onRestore={restoreVersion} />}
  </section>;
}

function MetaField({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) { return <label className="field-label">{label}<input value={value} onChange={(event) => onChange(event.target.value)} placeholder="Noch nicht festgelegt" /></label>; }
