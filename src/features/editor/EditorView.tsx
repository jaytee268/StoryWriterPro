import { useEffect, useMemo, useRef, useState } from 'react';
import { Check, ChevronDown, FileDown, LoaderCircle, Plus, Save, SpellCheck, WandSparkles } from 'lucide-react';
import type { Chapter, Scene } from '../../types/domain';
import { MockCorrectionService } from '../../services/correctionService';
import { SceneSaveQueue, type SceneSaveStatus } from '../../services/sceneSaveQueue';

interface EditorProps {
  chapters: Chapter[];
  scene?: Scene;
  chapter?: Chapter;
  onSelectScene: (id: string) => void;
  onSave: (scene: Scene) => Promise<Scene>;
  onCreateChapter: (title: string) => Promise<Chapter>;
  onCreateScene: (chapterId: string, title: string) => Promise<Scene>;
  onSaveStateChange: (status: SceneSaveStatus) => void;
  onBibleUpdate: () => void;
  onResearch: () => void;
}

const saveLabels: Record<SceneSaveStatus, string> = { saved: 'Gespeichert', dirty: 'Nicht gespeichert', saving: 'Speichert …', error: 'Speicherfehler' };

export function EditorView({ chapters, scene, chapter, onSelectScene, onSave, onCreateChapter, onCreateScene, onSaveStateChange, onBibleUpdate, onResearch }: EditorProps) {
  const [draftScene, setDraftScene] = useState<Scene | undefined>(scene);
  const latestDraft = useRef<Scene | undefined>(scene);
  const queue = useRef<SceneSaveQueue | undefined>(undefined);
  const [saveStatus, setSaveStatus] = useState<SceneSaveStatus>('saved');
  const [saveError, setSaveError] = useState('');
  const [correctionMessage, setCorrectionMessage] = useState('');
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [busyAction, setBusyAction] = useState<'chapter' | string | null>(null);

  // The effect intentionally keys on the scene id only. Parent state changes after a save
  // must not reset the user's current draft while the same scene remains selected.
  useEffect(() => {
    const next = scene ? { ...scene } : undefined;
    setDraftScene(next); latestDraft.current = next; setSaveStatus('saved'); setSaveError(''); setDetailsOpen(false); onSaveStateChange('saved');
    const nextQueue = new SceneSaveQueue(onSave, { onStatus: (status) => { setSaveStatus(status); onSaveStateChange(status); if (status !== 'error') setSaveError(''); }, onSaved: (saved) => { latestDraft.current = saved; setDraftScene((current) => current?.id === saved.id ? { ...current, updatedAt: saved.updatedAt } : current); }, onError: (error) => { setSaveError(error instanceof Error ? error.message : 'Die Szene konnte nicht gespeichert werden.'); } });
    queue.current = nextQueue;
    return () => { void nextQueue.dispose(); };
  }, [scene?.id, onSave, onSaveStateChange]); // eslint-disable-line react-hooks/exhaustive-deps

  const updateDraft = (changes: Partial<Scene>) => {
    const base = latestDraft.current ?? scene;
    if (!base) return;
    const next = { ...base, ...changes };
    latestDraft.current = next; setDraftScene(next); queue.current?.schedule(next);
  };
  const selectScene = async (id: string) => { await queue.current?.flush(); onSelectScene(id); };
  const addScene = async (chapterId: string) => { setBusyAction(chapterId); setSaveError(''); try { await queue.current?.flush(); const created = await onCreateScene(chapterId, 'Neue Szene'); onSelectScene(created.id); } catch (error) { setSaveError(error instanceof Error ? error.message : 'Die Szene konnte nicht angelegt werden.'); } finally { setBusyAction(null); } };
  const addChapter = async () => { setBusyAction('chapter'); setSaveError(''); try { await queue.current?.flush(); const createdChapter = await onCreateChapter(`Kapitel ${chapters.length + 1}`); const createdScene = await onCreateScene(createdChapter.id, 'Neue Szene'); onSelectScene(createdScene.id); } catch (error) { setSaveError(error instanceof Error ? error.message : 'Das Kapitel konnte nicht angelegt werden.'); } finally { setBusyAction(null); } };
  const check = async () => { const result = await new MockCorrectionService().check(draftScene?.content ?? ''); setCorrectionMessage(result.message ?? `${result.corrections.length} Korrekturvorschläge gefunden.`); };
  const wordCount = useMemo(() => draftScene?.content.trim() ? draftScene.content.trim().split(/\s+/).length : 0, [draftScene?.content]);

  return <section className="editor-view simple-editor">
    <div className="simple-editor-head"><div><span className="eyebrow">DU SCHREIBST IN</span><h1>{chapter?.title ?? 'Kapitel auswählen'}</h1><div className="scene-selector simple-scene-selector"><ChevronDown size={16} /> {draftScene?.title ?? 'Keine Szene'}</div></div><div className="simple-editor-actions"><span className={`save-state save-state-${saveStatus}`}><Save size={15} /> {saveLabels[saveStatus]}</span><button className="ghost-button large" onClick={() => void check()} disabled={!draftScene}><SpellCheck size={17} /> Text prüfen</button><button className="primary-button large" onClick={onBibleUpdate}><WandSparkles size={17} /> Story Bible aktualisieren</button></div></div>
    <div className="editor-layout simple-editor-layout"><aside className="document-tree simple-tree"><div className="tree-head"><span><strong>Dein Manuskript</strong><small>Kapitel und Szenen</small></span><button className="text-button" onClick={() => void addChapter()} disabled={busyAction !== null}>{busyAction === 'chapter' ? <LoaderCircle className="spin" size={16} /> : <Plus size={16} />} Kapitel</button></div><div className="tree-list">{chapters.map((item) => <div key={item.id} className="tree-chapter"><div className={`tree-row ${item.id === chapter?.id ? 'active' : ''}`}><ChevronDown size={15} /><span>{item.title}</span></div>{item.scenes.map((itemScene) => <button key={itemScene.id} className={`tree-scene ${itemScene.id === draftScene?.id ? 'active' : ''}`} onClick={() => void selectScene(itemScene.id)} disabled={busyAction !== null}><span className="scene-marker" />{itemScene.title}</button>)}<button className="add-row" onClick={() => void addScene(item.id)} disabled={busyAction !== null}>{busyAction === item.id ? <LoaderCircle className="spin" size={15} /> : <Plus size={15} />} Szene hinzufügen</button></div>)}</div><button className="export-button"><FileDown size={16} /> Exportieren</button></aside><div className="manuscript-column simple-manuscript"><div className="simple-writing-bar"><span>{wordCount.toLocaleString('de-DE')} Wörter</span><span>Autosave aktiv · 750 ms</span></div><article className="manuscript-page">{draftScene ? <><div className="page-kicker">{chapter?.title} · {draftScene.title}</div><textarea className="editable" value={draftScene.content} onChange={(event) => updateDraft({ content: event.target.value })} aria-label="Szenentext" />{correctionMessage && <div className="correction-notice"><Check size={16} /> {correctionMessage}</div>}<div className="page-footer"><span>Lokale Entwurfsversion</span><span>{wordCount.toLocaleString('de-DE')} Wörter</span></div></> : <div className="empty-state">Lege ein Kapitel und eine Szene an, um zu schreiben.</div>}</article></div></div>
    <div className="simple-editor-bottom"><button className="details-toggle" onClick={() => setDetailsOpen((open) => !open)} disabled={!draftScene}><span><strong>Szeneninfos</strong><small>{draftScene?.pov || 'Perspektive nicht festgelegt'} · {draftScene?.location || 'Ort nicht festgelegt'}</small></span><ChevronDown size={18} className={detailsOpen ? 'rotated' : ''} /></button><button className="ghost-button large" onClick={onResearch}>Konsistenz prüfen</button></div>
    {saveError && <div className="save-error" role="alert"><strong>{saveLabels.error}</strong><span>{saveError}</span><button className="text-button" onClick={() => { if (latestDraft.current) queue.current?.schedule(latestDraft.current); }}>Erneut speichern</button></div>}
    {detailsOpen && draftScene && <div className="scene-details-simple"><MetaField label="Perspektivfigur" value={draftScene.pov} onChange={(value) => updateDraft({ pov: value })} /><MetaField label="Ort" value={draftScene.location} onChange={(value) => updateDraft({ location: value })} /><MetaField label="Zeitpunkt" value={draftScene.storyTime} onChange={(value) => updateDraft({ storyTime: value })} /><label className="field-label">Status<select value={draftScene.status} onChange={(event) => updateDraft({ status: event.target.value as Scene['status'] })}><option value="draft">Entwurf</option><option value="revised">Überarbeitet</option><option value="final">Final</option></select></label><label className="field-label">Szenenziel<textarea value={draftScene.goal} onChange={(event) => updateDraft({ goal: event.target.value })} rows={2} /></label><label className="field-label">Notizen<textarea value={draftScene.notes} onChange={(event) => updateDraft({ notes: event.target.value })} rows={2} /></label></div>}
  </section>;
}

function MetaField({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) { return <label className="field-label">{label}<input value={value} onChange={(event) => onChange(event.target.value)} placeholder="Noch nicht festgelegt" /></label>; }
