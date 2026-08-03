import { useEffect, useMemo, useRef, useState, type CSSProperties, type MouseEvent } from 'react';
import { AlignCenter, AlignLeft, AlignRight, ArrowLeft, Bold, Check, ChevronDown, History, Image, IndentDecrease, IndentIncrease, Italic, Link, List, ListOrdered, LoaderCircle, MessageCircle, MoreHorizontal, Plus, Quote, Sparkles, StickyNote, Strikethrough, Table2, Type, Underline, X } from 'lucide-react';
import type { Chapter, CreateStyleReferenceInput, EditorPreferences, PendingSourceNavigation, Scene, SceneVersion, StyleReference, UpdateChapterInput, StyleReferenceCategory } from '../../types/domain';
import { SceneSaveQueue, type SceneSaveStatus } from '../../services/sceneSaveQueue';
import { editorContentToHtml, editorContentToPlainText } from '../../utils/editorContent';
import { unicodeIndexOf, unicodeSlice } from '../../utils/aiText';
import { selectionToUnicodeOffsets, type EditorSelectionSnapshot } from '../../utils/editorSelection';
import { VersionHistory } from './VersionHistory';

interface EditorProps {
  projectId: string;
  chapters: Chapter[];
  scene?: Scene;
  chapter?: Chapter;
  pendingSourceNavigation?: PendingSourceNavigation;
  onSourceNavigationConsumed: () => void;
  onBack: () => void;
  onSelectScene: (id: string) => void;
  onSave: (scene: Scene) => Promise<Scene>;
  onCreateChapter: (title: string) => Promise<Chapter>;
  onUpdateChapter: (input: UpdateChapterInput) => Promise<Chapter>;
  onCreateScene: (chapterId: string, title: string) => Promise<Scene>;
  onListVersions: (sceneId: string) => Promise<SceneVersion[]>;
  onCreateVersion: (sceneId: string) => Promise<SceneVersion>;
  onRestoreVersion: (sceneId: string, versionId: string) => Promise<Scene>;
  onGetEditorPreferences: () => Promise<EditorPreferences>;
  onSaveEditorPreferences: (preferences: EditorPreferences) => Promise<EditorPreferences>;
  onBibleUpdate: () => Promise<void>;
  bibleUpdateBusy: boolean;
  onCancelBibleUpdate: () => Promise<void>;
  onOpenAssistant: () => void;
  onSaveStateChange: (status: SceneSaveStatus) => void;
  onRegisterSaveController: (controller: EditorSaveController | null) => void;
  onCreateStyleReference: (input: CreateStyleReferenceInput) => Promise<StyleReference>;
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

export function EditorView({ projectId, chapters, scene, chapter, pendingSourceNavigation, onSourceNavigationConsumed, onBack, onSelectScene, onSave, onCreateChapter, onUpdateChapter, onCreateScene, onListVersions, onCreateVersion, onRestoreVersion, onGetEditorPreferences, onSaveEditorPreferences, onBibleUpdate, bibleUpdateBusy, onCancelBibleUpdate, onOpenAssistant, onSaveStateChange, onRegisterSaveController, onCreateStyleReference }: EditorProps) {
  const [draftScene, setDraftScene] = useState<Scene | undefined>(scene);
  const latestDraft = useRef<Scene | undefined>(scene);
  const queue = useRef<SceneSaveQueue | undefined>(undefined);
  const [saveStatus, setSaveStatus] = useState<SceneSaveStatus>('saved');
  const [saveError, setSaveError] = useState('');
  const [sourceNavigationNotice, setSourceNavigationNotice] = useState('');
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [displayOpen, setDisplayOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [preferences, setPreferences] = useState<EditorPreferences>(defaultPreferences);
  const [preferencesLoaded, setPreferencesLoaded] = useState(false);
  const [busyAction, setBusyAction] = useState<'chapter' | string | null>(null);
  const [headerMenuOpen, setHeaderMenuOpen] = useState(false);
  const editorRef = useRef<HTMLDivElement>(null);
  const [blockFormat, setBlockFormat] = useState<'paragraph' | 'heading'>('paragraph');
  const [toolbarState, setToolbarState] = useState({ bold: false, italic: false, underline: false, strikeThrough: false, unorderedList: false, orderedList: false, justifyLeft: false, justifyCenter: false, justifyRight: false });
  const [chapterTitleEditing, setChapterTitleEditing] = useState(false);
  const [chapterTitleDraft, setChapterTitleDraft] = useState(chapter?.title ?? '');
  const [sceneTitleEditing, setSceneTitleEditing] = useState<string | null>(null);
  const [sceneTitleDraft, setSceneTitleDraft] = useState('');
  const [styleReferenceDraft, setStyleReferenceDraft] = useState<{ selection: EditorSelectionSnapshot; category: StyleReferenceCategory; label: string; notes: string; weight: number }>();

  const refreshToolbarState = () => {
    const selection = document.getSelection();
    if (!editorRef.current || !selection || !selection.anchorNode || !editorRef.current.contains(selection.anchorNode)) {
      setToolbarState({ bold: false, italic: false, underline: false, strikeThrough: false, unorderedList: false, orderedList: false, justifyLeft: false, justifyCenter: false, justifyRight: false });
      return;
    }
    setToolbarState({ bold: document.queryCommandState('bold'), italic: document.queryCommandState('italic'), underline: document.queryCommandState('underline'), strikeThrough: document.queryCommandState('strikeThrough'), unorderedList: document.queryCommandState('insertUnorderedList'), orderedList: document.queryCommandState('insertOrderedList'), justifyLeft: document.queryCommandState('justifyLeft'), justifyCenter: document.queryCommandState('justifyCenter'), justifyRight: document.queryCommandState('justifyRight') });
    const format = document.queryCommandValue('formatBlock').toLocaleLowerCase();
    setBlockFormat(format.includes('h1') || format.includes('h2') || format.includes('h3') ? 'heading' : 'paragraph');
  };

  useEffect(() => {
    document.addEventListener('selectionchange', refreshToolbarState);
    return () => document.removeEventListener('selectionchange', refreshToolbarState);
  }, []);

  useEffect(() => {
    let active = true;
    void onGetEditorPreferences().then((loaded) => { if (active) setPreferences(loaded); }).catch(() => undefined).finally(() => { if (active) setPreferencesLoaded(true); });
    return () => { active = false; };
  }, [onGetEditorPreferences]);

  useEffect(() => {
    setChapterTitleDraft(chapter?.title ?? '');
    setChapterTitleEditing(false);
  }, [chapter?.id, chapter?.title]);

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
    if (!editorRef.current) return;
    editorRef.current.innerHTML = editorContentToHtml(scene?.content ?? '');
    setBlockFormat('paragraph');
    // Only reset the DOM when switching scenes. Autosave responses must not move the caret.
  }, [scene?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!pendingSourceNavigation || pendingSourceNavigation.sceneId !== scene?.id || !editorRef.current) return;
    const root = editorRef.current;
    const plainText = editorContentToPlainText(scene?.content ?? draftScene?.content ?? '');
    let start = pendingSourceNavigation.startOffset;
    let end = pendingSourceNavigation.endOffset;
    const offsetsValid = start !== undefined && end !== undefined && start >= 0 && end >= start && end <= Array.from(plainText).length && (!pendingSourceNavigation.excerpt || unicodeSlice(plainText, start, end).includes(pendingSourceNavigation.excerpt) || pendingSourceNavigation.excerpt.includes(unicodeSlice(plainText, start, end)));
    if (!offsetsValid && pendingSourceNavigation.excerpt) {
      const fallback = unicodeIndexOf(plainText, pendingSourceNavigation.excerpt);
      if (fallback >= 0) { start = fallback; end = fallback + Array.from(pendingSourceNavigation.excerpt).length; }
      else { start = undefined; end = undefined; setSourceNavigationNotice('Die Quellenstelle wurde seit der Erfassung möglicherweise verschoben.'); }
    }
    if (start !== undefined && end !== undefined) {
      const segments: Array<{ node: Text; start: number; end: number }> = [];
      let cursor = 0;
      const collect = (node: Node) => {
        if (node.nodeType === Node.TEXT_NODE) {
          const textNode = node as Text;
          const length = Array.from(textNode.nodeValue ?? '').length;
          segments.push({ node: textNode, start: cursor, end: cursor + length });
          cursor += length;
          return;
        }
        if (node.nodeType !== Node.ELEMENT_NODE) return;
        const element = node as HTMLElement;
        if (element.tagName === 'BR') { cursor += 1; return; }
        for (const child of Array.from(element.childNodes)) collect(child);
        if (/^(P|DIV|LI|BLOCKQUOTE)$/.test(element.tagName)) cursor += 1;
      };
      for (const child of Array.from(root.childNodes)) collect(child);
      const range = document.createRange();
      let startSet = false;
      for (const segment of segments) {
        const textNode = segment.node;
        const nodeText = textNode.nodeValue ?? '';
        const toDomOffset = (offset: number) => unicodeSlice(nodeText, 0, offset).length;
        if (!startSet && start >= segment.start && start <= segment.end) { range.setStart(textNode, toDomOffset(Math.max(0, start - segment.start))); startSet = true; }
        if (startSet && end <= segment.end) { range.setEnd(textNode, toDomOffset(Math.max(0, end - segment.start))); break; }
      }
      if (startSet) { const selection = document.getSelection(); selection?.removeAllRanges(); selection?.addRange(range); root.focus(); }
    }
    onSourceNavigationConsumed();
  }, [draftScene?.content, onSourceNavigationConsumed, pendingSourceNavigation, scene?.content, scene?.id]);

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
  const beginChapterTitleEdit = () => { if (!chapter) return; setChapterTitleDraft(chapter.title); setChapterTitleEditing(true); };
  const saveChapterTitle = async () => {
    if (!chapter || busyAction === 'chapter-title') return;
    const title = chapterTitleDraft.trim();
    if (!title) { setSaveError('Der Kapitelname darf nicht leer sein.'); return; }
    if (title === chapter.title) { setChapterTitleEditing(false); return; }
    setBusyAction('chapter-title'); setSaveError('');
    try { if (!await flushBeforeLeaving()) return; await onUpdateChapter({ id: chapter.id, title }); setChapterTitleEditing(false); }
    catch (error) { setSaveError(error instanceof Error ? error.message : 'Der Kapitelname konnte nicht gespeichert werden.'); }
    finally { setBusyAction(null); }
  };
  const beginSceneTitleEdit = (itemScene: Scene) => { setSceneTitleEditing(itemScene.id); setSceneTitleDraft(itemScene.title); };
  const saveSceneTitle = async (itemScene: Scene) => {
    const action = `scene-title:${itemScene.id}`;
    if (busyAction === action) return;
    const title = sceneTitleDraft.trim();
    if (!title) { setSaveError('Der Szenenname darf nicht leer sein.'); return; }
    if (title === itemScene.title) { setSceneTitleEditing(null); return; }
    setBusyAction(action); setSaveError('');
    try {
      if (itemScene.id === draftScene?.id && !await flushBeforeLeaving()) return;
      const saved = await onSave({ ...(itemScene.id === draftScene?.id ? (latestDraft.current ?? itemScene) : itemScene), title });
      if (saved.id === draftScene?.id) { latestDraft.current = saved; setDraftScene(saved); setSaveStatus('saved'); onSaveStateChange('saved'); }
      setSceneTitleEditing(null);
    } catch (error) { setSaveError(error instanceof Error ? error.message : 'Der Szenenname konnte nicht gespeichert werden.'); }
    finally { setBusyAction(null); }
  };
  const openHistory = async () => { if (await flushBeforeLeaving()) setHistoryOpen(true); };
  const updateBible = async () => { setSaveError(''); try { if (await flushBeforeLeaving()) await onBibleUpdate(); } catch (error) { setSaveError(error instanceof Error ? error.message : 'Das Bible Update konnte nicht gestartet werden.'); } };
  const restoreVersion = async (sceneId: string, versionId: string) => { const restored = await onRestoreVersion(sceneId, versionId); latestDraft.current = restored; setDraftScene(restored); if (editorRef.current) editorRef.current.innerHTML = editorContentToHtml(restored.content); setSaveStatus('saved'); onSaveStateChange('saved'); return restored; };
  const wordCount = useMemo(() => { const text = editorContentToPlainText(draftScene?.content ?? '').trim(); return text ? text.split(/\s+/).length : 0; }, [draftScene?.content]);
  const paperStyle = { '--manuscript-font': manuscriptFonts[preferences.fontFamily], '--manuscript-size': `${preferences.fontSize}px`, '--manuscript-leading': preferences.lineHeight } as CSSProperties;
  const syncEditorDraft = () => { if (editorRef.current) updateDraft({ content: editorRef.current.innerHTML }); };
  const runEditorCommand = (command: string, value?: string) => { editorRef.current?.focus(); document.execCommand(command, false, value); syncEditorDraft(); refreshToolbarState(); };
  const keepEditorSelection = (event: MouseEvent<HTMLButtonElement>) => event.preventDefault();
  const applyBlockFormat = (value: 'paragraph' | 'heading') => { setBlockFormat(value); runEditorCommand('formatBlock', value === 'heading' ? 'h2' : 'p'); };
  const addLink = () => { const url = window.prompt('Link-Adresse eingeben'); if (url?.trim()) runEditorCommand('createLink', url.trim()); };
  const addImage = () => { const url = window.prompt('Bild-Adresse eingeben'); if (url?.trim()) runEditorCommand('insertImage', url.trim()); };
  const addComment = () => { const comment = window.prompt('Kurze Notiz für diese Stelle'); if (comment?.trim()) runEditorCommand('insertText', `〔Notiz: ${comment.trim()}〕`); };
  const addTable = () => runEditorCommand('insertHTML', '<table><tbody><tr><td> </td><td> </td></tr><tr><td> </td><td> </td></tr></tbody></table><p><br></p>');
  const prepareStyleReference = async () => {
    const selection = document.getSelection();
    const range = selection && selection.rangeCount ? selection.getRangeAt(0) : undefined;
    const snapshot = editorRef.current && range ? selectionToUnicodeOffsets(editorRef.current, range) : undefined;
    if (!snapshot) { setSourceNavigationNotice('Markiere zuerst eine Textpassage.'); return; }
    if (!await flushBeforeLeaving()) return;
    setStyleReferenceDraft({ selection: snapshot, category: 'general', label: '', notes: '', weight: 1 });
  };
  const saveStyleReference = async () => {
    if (!styleReferenceDraft || !draftScene || !chapter) return;
    try {
      await onCreateStyleReference({ projectId, chapterId: chapter.id, sceneId: draftScene.id, excerpt: styleReferenceDraft.selection.excerpt, startOffset: styleReferenceDraft.selection.startOffset, endOffset: styleReferenceDraft.selection.endOffset, category: styleReferenceDraft.category, label: styleReferenceDraft.label.trim() || 'Stilreferenz', notes: styleReferenceDraft.notes, weight: styleReferenceDraft.weight });
      setStyleReferenceDraft(undefined); setSourceNavigationNotice('Stilreferenz gespeichert.');
    } catch (error) { setSourceNavigationNotice(error instanceof Error ? error.message : 'Stilreferenz konnte nicht gespeichert werden.'); }
  };

  const visibleScenes = chapter?.scenes ?? [];
  return <section className="editor-view writing-workspace">
    <header className="writing-workspace-header">
      <div className="writing-header-navigation"><button className="writing-back-button" onClick={() => void requestBack()} aria-label="Zurück zur Übersicht" title="Zurück"><ArrowLeft size={18} /></button><div className="writing-chapter-picker"><span>KAPITEL</span><select value={chapter?.id ?? ''} onChange={(event) => { const selected = chapters.find((item) => item.id === event.target.value); if (selected) void selectChapter(selected); }} aria-label="Kapitel auswählen">{chapters.map((item) => <option key={item.id} value={item.id}>{item.orderIndex}</option>)}</select><ChevronDown size={17} /></div></div>
      <div className="writing-title-block">{chapterTitleEditing ? <div className="writing-title-editing"><input value={chapterTitleDraft} onChange={(event) => setChapterTitleDraft(event.target.value)} onKeyDown={(event) => { if (event.key === 'Enter') { event.preventDefault(); void saveChapterTitle(); } if (event.key === 'Escape') { setChapterTitleEditing(false); setChapterTitleDraft(chapter?.title ?? ''); } }} onBlur={() => void saveChapterTitle()} autoFocus aria-label="Kapitelname bearbeiten" /><button onMouseDown={keepEditorSelection} onClick={() => void saveChapterTitle()} aria-label="Kapitelname speichern" title="Speichern"><Check size={17} /></button><button onMouseDown={(event) => event.preventDefault()} onClick={() => { setChapterTitleEditing(false); setChapterTitleDraft(chapter?.title ?? ''); }} aria-label="Bearbeitung abbrechen" title="Abbrechen"><X size={17} /></button></div> : <div><h1>{chapter?.title ?? 'Kapitel auswählen'}</h1><button className="writing-edit-title" onClick={beginChapterTitleEdit} aria-label="Kapitelname bearbeiten" title="Kapitelname bearbeiten"><PencilIcon /></button></div>}<span>{wordCount.toLocaleString('de-DE')} Wörter <b>·</b> Autosave aktiv</span></div>
      <div className="writing-header-actions"><span className={`save-state save-state-${saveStatus}`}><span className={`status-dot status-dot-${saveStatus}`} /> {saveLabels[saveStatus]}</span><div className="writing-menu-wrap"><button className="writing-icon-button" onClick={() => setHeaderMenuOpen((open) => !open)} aria-label="Weitere Aktionen" title="Weitere Aktionen"><MoreHorizontal size={21} /></button>{headerMenuOpen && <div className="writing-menu"><button onClick={() => { setHeaderMenuOpen(false); onOpenAssistant(); }}><MessageCircle size={16} /> Assistent öffnen</button><button onClick={() => { setHeaderMenuOpen(false); void openHistory(); }}><History size={16} /> Verlauf öffnen</button><button onClick={() => { setHeaderMenuOpen(false); void addChapter(); }}><Plus size={16} /> Neues Kapitel</button></div>}</div></div>
    </header>
    <div className="writing-workspace-body">
      <aside className="writing-scene-sidebar" aria-label="Szenen">
        <div className="writing-scenes-head"><span>SZENEN</span><button className="writing-add-button" onClick={() => chapter && void addScene(chapter.id)} disabled={!chapter || busyAction !== null} aria-label="Neue Szene hinzufügen" title="Neue Szene hinzufügen">{busyAction === chapter?.id ? <LoaderCircle className="spin" size={18} /> : <Plus size={20} />}</button></div>
        <div className="writing-scene-list">{visibleScenes.map((itemScene, index) => <div key={itemScene.id} className={`writing-scene-row ${itemScene.id === draftScene?.id ? 'active' : ''}`} role="button" tabIndex={0} onClick={() => { if (!sceneTitleEditing) void selectScene(itemScene.id); }} onDoubleClick={() => beginSceneTitleEdit(itemScene)} onKeyDown={(event) => { if (event.key === 'Enter') void selectScene(itemScene.id); if (event.key === 'F2') beginSceneTitleEdit(itemScene); }}><span className="writing-scene-number">{String(index + 1).padStart(2, '0')}</span>{sceneTitleEditing === itemScene.id ? <input className="writing-scene-title-input" value={sceneTitleDraft} onChange={(event) => setSceneTitleDraft(event.target.value)} onClick={(event) => event.stopPropagation()} onDoubleClick={(event) => event.stopPropagation()} onKeyDown={(event) => { if (event.key === 'Enter') { event.preventDefault(); void saveSceneTitle(itemScene); } if (event.key === 'Escape') { setSceneTitleEditing(null); setSceneTitleDraft(''); } }} onBlur={() => void saveSceneTitle(itemScene)} autoFocus aria-label="Szenenname bearbeiten" /> : <span className="writing-scene-copy"><strong>{itemScene.title}</strong>{itemScene.id === draftScene?.id && <small>Aktiv</small>}</span>}{itemScene.id === draftScene?.id && <span className="writing-active-dot" />}</div>)}</div>
        <button className="writing-add-scene" onClick={() => chapter && void addScene(chapter.id)} disabled={!chapter || busyAction !== null}>{busyAction === chapter?.id ? <LoaderCircle className="spin" size={17} /> : <Plus size={18} />} Szene hinzufügen</button>
        <div className="writing-sidebar-spacer" />
        <button className="writing-notes-link" onClick={() => setDetailsOpen((open) => !open)} disabled={!draftScene}><StickyNote size={16} /> Szeneninfos &amp; Notizen</button>
      </aside>
      <main className="writing-editor-area">
        <div className="writing-format-toolbar" aria-label="Textwerkzeuge"><select aria-label="Absatzformat" value={blockFormat} onChange={(event) => applyBlockFormat(event.target.value as 'paragraph' | 'heading')}><option value="paragraph">Absatz</option><option value="heading">Überschrift</option></select><span className="writing-toolbar-divider" /><button className={toolbarState.bold ? 'active' : ''} aria-pressed={toolbarState.bold} title="Fett" onMouseDown={keepEditorSelection} onClick={() => runEditorCommand('bold')}><Bold size={19} /></button><button className={toolbarState.italic ? 'active' : ''} aria-pressed={toolbarState.italic} title="Kursiv" onMouseDown={keepEditorSelection} onClick={() => runEditorCommand('italic')}><Italic size={19} /></button><button className={toolbarState.underline ? 'active' : ''} aria-pressed={toolbarState.underline} title="Unterstrichen" onMouseDown={keepEditorSelection} onClick={() => runEditorCommand('underline')}><Underline size={19} /></button><button className={toolbarState.strikeThrough ? 'active' : ''} aria-pressed={toolbarState.strikeThrough} title="Durchgestrichen" onMouseDown={keepEditorSelection} onClick={() => runEditorCommand('strikeThrough')}><Strikethrough size={18} /></button><span className="writing-toolbar-divider" /><button className={toolbarState.unorderedList ? 'active' : ''} aria-pressed={toolbarState.unorderedList} title="Aufzählung" onMouseDown={keepEditorSelection} onClick={() => runEditorCommand('insertUnorderedList')}><List size={19} /></button><button className={toolbarState.orderedList ? 'active' : ''} aria-pressed={toolbarState.orderedList} title="Nummerierte Liste" onMouseDown={keepEditorSelection} onClick={() => runEditorCommand('insertOrderedList')}><ListOrdered size={19} /></button><button title="Einzug verringern" onMouseDown={keepEditorSelection} onClick={() => runEditorCommand('outdent')}><IndentDecrease size={18} /></button><button title="Einzug vergrößern" onMouseDown={keepEditorSelection} onClick={() => runEditorCommand('indent')}><IndentIncrease size={18} /></button><span className="writing-toolbar-divider" /><button title="Link" onMouseDown={keepEditorSelection} onClick={addLink}><Link size={18} /></button><button title="Bild" onMouseDown={keepEditorSelection} onClick={addImage}><Image size={18} /></button><button title="Kommentar einfügen" onMouseDown={keepEditorSelection} onClick={addComment}><MessageCircle size={18} /></button><span className="writing-toolbar-divider" /><button title="Tabelle einfügen" onMouseDown={keepEditorSelection} onClick={addTable}><Table2 size={18} /></button><button title="Zitat" onMouseDown={keepEditorSelection} onClick={() => runEditorCommand('formatBlock', 'blockquote')}><Quote size={19} /></button><span className="writing-toolbar-divider" /><button className={toolbarState.justifyLeft ? 'active' : ''} aria-pressed={toolbarState.justifyLeft} title="Linksbündig" onMouseDown={keepEditorSelection} onClick={() => runEditorCommand('justifyLeft')}><AlignLeft size={18} /></button><button className={toolbarState.justifyCenter ? 'active' : ''} aria-pressed={toolbarState.justifyCenter} title="Zentriert" onMouseDown={keepEditorSelection} onClick={() => runEditorCommand('justifyCenter')}><AlignCenter size={18} /></button><button className={toolbarState.justifyRight ? 'active' : ''} aria-pressed={toolbarState.justifyRight} title="Rechtsbündig" onMouseDown={keepEditorSelection} onClick={() => runEditorCommand('justifyRight')}><AlignRight size={18} /></button><span className="writing-toolbar-spacer" /><button title="Schrift und Layout" onClick={() => setDisplayOpen((open) => !open)}><Type size={18} /></button></div>
        {displayOpen && <div className="writing-preferences writing-preferences-inline"><label>Schrift<select value={preferences.fontFamily} onChange={(event) => setPreferences((current) => ({ ...current, fontFamily: event.target.value as EditorPreferences['fontFamily'] }))}><option value="serif">Roman · Serif</option><option value="sans">Klar · Sans</option><option value="typewriter">Schreibmaschine · Mono</option></select></label><label>Größe<strong>{preferences.fontSize}px</strong><input type="range" min="14" max="28" step="1" value={preferences.fontSize} onChange={(event) => setPreferences((current) => ({ ...current, fontSize: Number(event.target.value) }))} /></label><label>Zeilenabstand<strong>{preferences.lineHeight.toFixed(2)}</strong><input type="range" min="1.3" max="2.5" step="0.05" value={preferences.lineHeight} onChange={(event) => setPreferences((current) => ({ ...current, lineHeight: Number(event.target.value) }))} /></label></div>}
        <div className="writing-editor-tools"><span>{draftScene?.title ?? 'Neue Szene'}</span><div><span>{wordCount.toLocaleString('de-DE')} Wörter</span><button className="writing-tool-link" onClick={() => void openHistory()} disabled={!draftScene || bibleUpdateBusy}><History size={15} /> Verlauf</button><button className="writing-tool-link" onClick={() => void prepareStyleReference()} disabled={!draftScene}><StickyNote size={15} /> Als Stilreferenz</button>{bibleUpdateBusy ? <button className="writing-tool-link writing-tool-primary" onClick={() => void onCancelBibleUpdate()}><LoaderCircle className="spin" size={15} /> Abbrechen</button> : <button className="writing-tool-link writing-tool-primary" onClick={() => void updateBible()} disabled={!draftScene}><Sparkles size={15} /> Story &amp; Gedächtnis aktualisieren</button>}</div></div>
        {sourceNavigationNotice && <div className="source-navigation-notice" role="status">{sourceNavigationNotice}<button className="text-button" onClick={() => setSourceNavigationNotice('')}>Schließen</button></div>}
        <article className="writing-page" style={paperStyle}>{draftScene ? <div ref={editorRef} className="writing-textarea" contentEditable role="textbox" aria-label="Szenentext" lang="de" spellCheck onInput={syncEditorDraft} data-placeholder="Beginne mit dem Schreiben deiner Szene …" /> : <div className="empty-state">Lege ein Kapitel und eine Szene an, um zu schreiben.</div>}</article>
        {saveError && <div className="save-error writing-save-error" role="alert"><strong>{saveLabels.error}</strong><span>{saveError}</span><button className="text-button" onClick={() => { if (latestDraft.current) queue.current?.schedule(latestDraft.current); }}>Erneut speichern</button></div>}
        {detailsOpen && draftScene && <div className="scene-details-simple writing-scene-details"><MetaField label="Perspektivfigur" value={draftScene.pov} onChange={(value) => updateDraft({ pov: value })} /><MetaField label="Ort" value={draftScene.location} onChange={(value) => updateDraft({ location: value })} /><MetaField label="Zeitpunkt" value={draftScene.storyTime} onChange={(value) => updateDraft({ storyTime: value })} /><label className="field-label">Status<select value={draftScene.status} onChange={(event) => updateDraft({ status: event.target.value as Scene['status'] })}><option value="draft">Entwurf</option><option value="revised">Überarbeitet</option><option value="final">Final</option></select></label><label className="field-label">Szenenziel<textarea value={draftScene.goal} onChange={(event) => updateDraft({ goal: event.target.value })} rows={2} /></label><label className="field-label">Notizen<textarea value={draftScene.notes} onChange={(event) => updateDraft({ notes: event.target.value })} rows={2} /></label></div>}
      </main>
    </div>
    {historyOpen && draftScene && <VersionHistory scene={draftScene} onClose={() => setHistoryOpen(false)} onLoad={onListVersions} onCreate={onCreateVersion} onRestore={restoreVersion} />}
    {styleReferenceDraft && <div className="modal-backdrop" role="dialog" aria-modal="true"><div className="modal simple-modal"><div className="modal-head"><div><span className="eyebrow">PROJEKTSTIL</span><h2>Als Stilreferenz speichern</h2></div><button className="icon-button" onClick={() => setStyleReferenceDraft(undefined)}><X size={17} /></button></div><p className="modal-intro">„{styleReferenceDraft.selection.excerpt}“</p><div className="form-grid"><label className="field-label">Kategorie<select value={styleReferenceDraft.category} onChange={(event) => setStyleReferenceDraft({ ...styleReferenceDraft, category: event.target.value as StyleReferenceCategory })}>{[['general', 'Allgemein'], ['dialogue', 'Dialog'], ['tension', 'Spannung'], ['description', 'Beschreibung'], ['inner_monologue', 'Innerer Monolog'], ['humor', 'Humor']].map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label><label className="field-label">Bezeichnung<input autoFocus value={styleReferenceDraft.label} onChange={(event) => setStyleReferenceDraft({ ...styleReferenceDraft, label: event.target.value })} placeholder="z. B. knapper Dialog" /></label><label className="field-label">Gewichtung<input type="number" min="0.1" max="5" step="0.1" value={styleReferenceDraft.weight} onChange={(event) => setStyleReferenceDraft({ ...styleReferenceDraft, weight: Number(event.target.value) })} /></label><label className="field-label full-field">Notizen<textarea rows={3} value={styleReferenceDraft.notes} onChange={(event) => setStyleReferenceDraft({ ...styleReferenceDraft, notes: event.target.value })} /></label></div><div className="modal-actions"><button className="ghost-button" onClick={() => setStyleReferenceDraft(undefined)}>Abbrechen</button><button className="primary-button" onClick={() => void saveStyleReference()}>Speichern</button></div></div></div>}
  </section>;
}

function MetaField({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) { return <label className="field-label">{label}<input value={value} onChange={(event) => onChange(event.target.value)} placeholder="Noch nicht festgelegt" /></label>; }

function PencilIcon() { return <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true"><path d="m15 5 4 4M4 20l3.5-.8L18 8.7a2.8 2.8 0 0 0-4-4L3.5 15.2 3 20h1Z" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" /></svg>; }
