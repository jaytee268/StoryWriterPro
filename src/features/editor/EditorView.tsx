import { useEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import { ArrowLeft, Bold, ChevronDown, Flag, History, Image, IndentDecrease, IndentIncrease, Italic, Link, List, ListOrdered, LoaderCircle, MessageCircle, MoreHorizontal, Plus, Quote, Search, Scissors, Sparkles, StickyNote, Strikethrough, Table2, Type, Underline } from 'lucide-react';
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
  onCreateVersion: (sceneId: string) => Promise<SceneVersion>;
  onRestoreVersion: (sceneId: string, versionId: string) => Promise<Scene>;
  onGetEditorPreferences: () => Promise<EditorPreferences>;
  onSaveEditorPreferences: (preferences: EditorPreferences) => Promise<EditorPreferences>;
  onBibleUpdate: () => Promise<void>;
  onOpenAssistant: () => void;
  onSaveStateChange: (status: SceneSaveStatus) => void;
  onRegisterSaveController: (controller: EditorSaveController | null) => void;
}

export interface EditorSaveController {
  flush: () => Promise<void>;
  getDraft: () => Scene | undefined;
  hasPendingChanges: () => boolean;
  getStatus: () => SceneSaveStatus;
  getError: () => unknown;
}

const saveLabels: Record<SceneSaveStatus, string> = { saved: 'Gespeichert', dirty: 'Nicht gespeichert', saving: 'Speichert …', error: 'Speicherfehler' };
const defaultPreferences: EditorPreferences = { fontFamily: 'serif', fontSize: 18, lineHeight: 1.95 };
const manuscriptFonts: Record<EditorPreferences['fontFamily'], string> = {
  serif: "'Libre Baskerville', Georgia, serif",
  sans: "'DM Sans', Arial, sans-serif",
  typewriter: "'DM Mono', 'Courier New', monospace",
};

export function EditorView({ chapters, scene, chapter, onBack, onSelectScene, onSave, onCreateChapter, onCreateScene, onListVersions, onCreateVersion, onRestoreVersion, onGetEditorPreferences, onSaveEditorPreferences, onBibleUpdate, onOpenAssistant, onSaveStateChange, onRegisterSaveController }: EditorProps) {
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
  const [headerMenuOpen, setHeaderMenuOpen] = useState(false);

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
    let mounted = true;
    const nextQueue = new SceneSaveQueue(onSave, { onStatus: (status) => { if (!mounted) return; setSaveStatus(status); onSaveStateChange(status); if (status !== 'error') setSaveError(''); }, onSaved: (saved) => { if (!mounted) return; latestDraft.current = saved; setDraftScene((current) => current?.id === saved.id ? { ...current, updatedAt: saved.updatedAt } : current); }, onError: (error) => { if (mounted) setSaveError(error instanceof Error ? error.message : 'Die Szene konnte nicht gespeichert werden.'); } });
    queue.current = nextQueue;
    onRegisterSaveController({ flush: () => nextQueue.flush(), getDraft: () => latestDraft.current, hasPendingChanges: () => nextQueue.hasPendingChanges(), getStatus: () => nextQueue.getStatus(), getError: () => nextQueue.getError() });
    return () => { mounted = false; if (queue.current === nextQueue) { queue.current = undefined; onRegisterSaveController(null); } void nextQueue.dispose(); };
  }, [scene?.id, onSave, onSaveStateChange, onRegisterSaveController]); // eslint-disable-line react-hooks/exhaustive-deps

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
  const flushBeforeLeaving = async () => { const currentQueue = queue.current; if (!currentQueue) return true; await currentQueue.flush(); if (currentQueue.getStatus() === 'error') { const error = currentQueue.getError(); setSaveError(error instanceof Error ? error.message : 'Die Szene konnte nicht gespeichert werden.'); return false; } return true; };
  const requestBack = async () => { if (await flushBeforeLeaving()) onBack(); };
  const selectScene = async (id: string) => { if (await flushBeforeLeaving()) onSelectScene(id); };
  const selectChapter = async (selectedChapter: Chapter) => { const firstScene = selectedChapter.scenes[0]; if (firstScene) await selectScene(firstScene.id); };
  const addScene = async (chapterId: string) => { setBusyAction(chapterId); setSaveError(''); try { if (!await flushBeforeLeaving()) return; const created = await onCreateScene(chapterId, 'Neue Szene'); onSelectScene(created.id); } catch (error) { setSaveError(error instanceof Error ? error.message : 'Die Szene konnte nicht angelegt werden.'); } finally { setBusyAction(null); } };
  const addChapter = async () => { setBusyAction('chapter'); setSaveError(''); try { if (!await flushBeforeLeaving()) return; const createdChapter = await onCreateChapter(`Kapitel ${chapters.length + 1}`); const createdScene = await onCreateScene(createdChapter.id, 'Neue Szene'); onSelectScene(createdScene.id); } catch (error) { setSaveError(error instanceof Error ? error.message : 'Das Kapitel konnte nicht angelegt werden.'); } finally { setBusyAction(null); } };
  const openHistory = async () => { if (await flushBeforeLeaving()) setHistoryOpen(true); };
  const updateBible = async () => { setSaveError(''); try { if (await flushBeforeLeaving()) await onBibleUpdate(); } catch (error) { setSaveError(error instanceof Error ? error.message : 'Das Bible Update konnte nicht gestartet werden.'); } };
  const restoreVersion = async (sceneId: string, versionId: string) => { const restored = await onRestoreVersion(sceneId, versionId); latestDraft.current = restored; setDraftScene(restored); setSaveStatus('saved'); onSaveStateChange('saved'); return restored; };
  const wordCount = useMemo(() => draftScene?.content.trim() ? draftScene.content.trim().split(/\s+/).length : 0, [draftScene?.content]);
  const paperStyle = { '--manuscript-font': manuscriptFonts[preferences.fontFamily], '--manuscript-size': `${preferences.fontSize}px`, '--manuscript-leading': preferences.lineHeight } as CSSProperties;

  const visibleScenes = chapter?.scenes ?? [];
  return <section className="editor-view writing-workspace">
    <header className="writing-workspace-header">
      <button className="writing-back-button" onClick={() => void requestBack()} aria-label="Zurück zur Übersicht" title="Zurück"><ArrowLeft size={18} /></button>
      <div className="writing-chapter-picker"><span>KAPITEL</span><select value={chapter?.id ?? ''} onChange={(event) => { const selected = chapters.find((item) => item.id === event.target.value); if (selected) void selectChapter(selected); }} aria-label="Kapitel auswählen">{chapters.map((item) => <option key={item.id} value={item.id}>{item.orderIndex}</option>)}</select><ChevronDown size={17} /></div>
      <div className="writing-header-divider" />
      <div className="writing-title-block"><div><h1>{chapter?.title ?? 'Kapitel auswählen'}</h1><button className="writing-edit-title" aria-label="Kapitelname bearbeiten" title="Kapitelname bearbeiten"><PencilIcon /></button></div><span>{wordCount.toLocaleString('de-DE')} Wörter <b>·</b> Autosave aktiv</span></div>
      <div className="writing-header-actions"><span className={`save-state save-state-${saveStatus}`}><span className={`status-dot status-dot-${saveStatus}`} /> {saveLabels[saveStatus]}</span><button className="writing-header-button" title="Szenenziel setzen"><Flag size={17} /> Ziel setzen</button><button className="writing-header-button" title="Szene teilen"><Scissors size={17} /> Szene teilen</button><button className="writing-header-button" title="Recherche öffnen"><Search size={17} /> Recherche</button><div className="writing-menu-wrap"><button className="writing-icon-button" onClick={() => setHeaderMenuOpen((open) => !open)} aria-label="Weitere Aktionen" title="Weitere Aktionen"><MoreHorizontal size={21} /></button>{headerMenuOpen && <div className="writing-menu"><button onClick={() => { setHeaderMenuOpen(false); onOpenAssistant(); }}><MessageCircle size={16} /> Assistent öffnen</button><button onClick={() => { setHeaderMenuOpen(false); void openHistory(); }}><History size={16} /> Verlauf öffnen</button><button onClick={() => { setHeaderMenuOpen(false); void addChapter(); }}><Plus size={16} /> Neues Kapitel</button></div>}</div></div>
    </header>
    <div className="writing-workspace-body">
      <aside className="writing-scene-sidebar" aria-label="Szenen">
        <div className="writing-scenes-head"><span>SZENEN</span><button className="writing-add-button" onClick={() => chapter && void addScene(chapter.id)} disabled={!chapter || busyAction !== null} aria-label="Neue Szene hinzufügen" title="Neue Szene hinzufügen">{busyAction === chapter?.id ? <LoaderCircle className="spin" size={18} /> : <Plus size={20} />}</button></div>
        <div className="writing-scene-list">{visibleScenes.map((itemScene, index) => <button key={itemScene.id} className={`writing-scene-row ${itemScene.id === draftScene?.id ? 'active' : ''}`} onClick={() => void selectScene(itemScene.id)} disabled={busyAction !== null}><span className="writing-scene-number">{String(index + 1).padStart(2, '0')}</span><span className="writing-scene-copy"><strong>{itemScene.title}</strong>{itemScene.id === draftScene?.id && <small>Aktiv</small>}</span>{itemScene.id === draftScene?.id && <span className="writing-active-dot" />}</button>)}</div>
        <button className="writing-add-scene" onClick={() => chapter && void addScene(chapter.id)} disabled={!chapter || busyAction !== null}>{busyAction === chapter?.id ? <LoaderCircle className="spin" size={17} /> : <Plus size={18} />} Szene hinzufügen</button>
        <div className="writing-sidebar-spacer" />
        <button className="writing-notes-link" onClick={() => setDetailsOpen((open) => !open)} disabled={!draftScene}><StickyNote size={16} /> Szeneninfos &amp; Notizen</button>
      </aside>
      <main className="writing-editor-area">
        <div className="writing-format-toolbar" aria-label="Textwerkzeuge"><select aria-label="Absatzformat" defaultValue="paragraph"><option value="paragraph">Absatz</option><option value="heading">Überschrift</option></select><span className="writing-toolbar-divider" /><button title="Fett"><Bold size={19} /></button><button title="Kursiv"><Italic size={19} /></button><button title="Unterstrichen"><Underline size={19} /></button><button title="Durchgestrichen"><Strikethrough size={18} /></button><span className="writing-toolbar-divider" /><button title="Aufzählung"><List size={19} /></button><button title="Nummerierte Liste"><ListOrdered size={19} /></button><span className="writing-toolbar-divider" /><button title="Einzug verringern"><IndentDecrease size={18} /></button><button title="Einzug vergrößern"><IndentIncrease size={18} /></button><span className="writing-toolbar-divider" /><button title="Link"><Link size={18} /></button><button title="Bild"><Image size={18} /></button><button title="Kommentar"><MessageCircle size={18} /></button><span className="writing-toolbar-divider" /><button title="Tabelle"><Table2 size={18} /></button><button title="Zitat"><Quote size={19} /></button><span className="writing-toolbar-spacer" /><button title="Schrift und Layout" onClick={() => setDisplayOpen((open) => !open)}><Type size={18} /></button></div>
        {displayOpen && <div className="writing-preferences writing-preferences-inline"><label>Schrift<select value={preferences.fontFamily} onChange={(event) => setPreferences((current) => ({ ...current, fontFamily: event.target.value as EditorPreferences['fontFamily'] }))}><option value="serif">Roman · Serif</option><option value="sans">Klar · Sans</option><option value="typewriter">Schreibmaschine · Mono</option></select></label><label>Größe<strong>{preferences.fontSize}px</strong><input type="range" min="14" max="28" step="1" value={preferences.fontSize} onChange={(event) => setPreferences((current) => ({ ...current, fontSize: Number(event.target.value) }))} /></label><label>Zeilenabstand<strong>{preferences.lineHeight.toFixed(2)}</strong><input type="range" min="1.3" max="2.5" step="0.05" value={preferences.lineHeight} onChange={(event) => setPreferences((current) => ({ ...current, lineHeight: Number(event.target.value) }))} /></label></div>}
        <div className="writing-editor-tools"><span>{draftScene?.title ?? 'Neue Szene'}</span><div><span>{wordCount.toLocaleString('de-DE')} Wörter</span><button className="writing-tool-link" onClick={() => void openHistory()} disabled={!draftScene}><History size={15} /> Verlauf</button><button className="writing-tool-link writing-tool-primary" onClick={() => void updateBible()} disabled={!draftScene}><Sparkles size={15} /> Story Bible aktualisieren</button></div></div>
        <article className="writing-page" style={paperStyle}>{draftScene ? <textarea className="writing-textarea" value={draftScene.content} onChange={(event) => updateDraft({ content: event.target.value })} aria-label="Szenentext" lang="de" spellCheck placeholder="Beginne mit dem Schreiben deiner Szene …" /> : <div className="empty-state">Lege ein Kapitel und eine Szene an, um zu schreiben.</div>}</article>
        {saveError && <div className="save-error writing-save-error" role="alert"><strong>{saveLabels.error}</strong><span>{saveError}</span><button className="text-button" onClick={() => { if (latestDraft.current) queue.current?.schedule(latestDraft.current); }}>Erneut speichern</button></div>}
        {detailsOpen && draftScene && <div className="scene-details-simple writing-scene-details"><MetaField label="Perspektivfigur" value={draftScene.pov} onChange={(value) => updateDraft({ pov: value })} /><MetaField label="Ort" value={draftScene.location} onChange={(value) => updateDraft({ location: value })} /><MetaField label="Zeitpunkt" value={draftScene.storyTime} onChange={(value) => updateDraft({ storyTime: value })} /><label className="field-label">Status<select value={draftScene.status} onChange={(event) => updateDraft({ status: event.target.value as Scene['status'] })}><option value="draft">Entwurf</option><option value="revised">Überarbeitet</option><option value="final">Final</option></select></label><label className="field-label">Szenenziel<textarea value={draftScene.goal} onChange={(event) => updateDraft({ goal: event.target.value })} rows={2} /></label><label className="field-label">Notizen<textarea value={draftScene.notes} onChange={(event) => updateDraft({ notes: event.target.value })} rows={2} /></label></div>}
      </main>
    </div>
    {historyOpen && draftScene && <VersionHistory scene={draftScene} onClose={() => setHistoryOpen(false)} onLoad={onListVersions} onCreate={onCreateVersion} onRestore={restoreVersion} />}
  </section>;
}

function MetaField({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) { return <label className="field-label">{label}<input value={value} onChange={(event) => onChange(event.target.value)} placeholder="Noch nicht festgelegt" /></label>; }

function PencilIcon() { return <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true"><path d="m15 5 4 4M4 20l3.5-.8L18 8.7a2.8 2.8 0 0 0-4-4L3.5 15.2 3 20h1Z" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" /></svg>; }
