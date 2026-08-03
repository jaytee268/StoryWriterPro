import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { BookOpen, BrainCircuit, FileText, Gauge, MessageCircle, PanelLeftClose, PanelLeftOpen, RefreshCw, Settings2, Upload, X } from 'lucide-react';
import { useAppStore } from './stores/useAppStore';
import { demoEvents, mindEdges, mindNodes } from './services/mockData';
import { createStoryRepository, type RuntimeMode, type StoryRepository } from './services/storyRepository';
import type { AppView, BibleProposal, BibleUpdateRun, ChatMessage, Chapter, ReviewBibleProposalInput, Scene, StoryEntity, StorySourceReference, UpdateChapterInput, WorkspaceSnapshot } from './types/domain';
import { DeterministicProjectContextBuilder } from './services/contextBuilder';
import { LocalPrototypeBibleExtractor, changedRange, contentHash } from './services/bibleExtractor';
import { Dashboard } from './features/projects/Dashboard';
import { EditorView, type EditorSaveController } from './features/editor/EditorView';
import { ChatPanel } from './features/chat/ChatPanel';
import { StoryBibleView } from './features/story-bible/StoryBibleView';
import { TimelineView } from './features/timeline/TimelineView';
import { MindmapView } from './features/mindmap/MindmapView';
import { SettingsView } from './features/settings/SettingsView';

const repository: StoryRepository = createStoryRepository();
type LoadState = { status: 'loading' } | { status: 'ready'; workspace: WorkspaceSnapshot } | { status: 'error'; message: string; detail?: string };
export type SaveStatus = 'saved' | 'dirty' | 'saving' | 'error';

const navItems: { view: AppView; label: string; description: string; icon: typeof BookOpen }[] = [
  { view: 'editor', label: 'Schreiben', description: 'Manuskript öffnen', icon: FileText },
  { view: 'bible', label: 'Story Bible', description: 'Figuren & Fakten', icon: BookOpen },
  { view: 'timeline', label: 'Timeline', description: 'Was wann passiert', icon: Gauge },
  { view: 'mindmap', label: 'Mindmap', description: 'Zusammenhänge sehen', icon: BrainCircuit },
];

export function App() {
  const { view, setView } = useAppStore();
  const [loadState, setLoadState] = useState<LoadState>({ status: 'loading' });
  const [selectedSceneId, setSelectedSceneId] = useState('');
  const [assistantOpen, setAssistantOpen] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [saveStatus, setSaveStatus] = useState<SaveStatus>('saved');
  const editorSaveController = useRef<EditorSaveController | null>(null);
  const allowNextClose = useRef(false);
  const [closePrompt, setClosePrompt] = useState('');
  const [viewError, setViewError] = useState('');
  const [activeReviewRun, setActiveReviewRun] = useState<BibleUpdateRun>();
  const [reviewProposals, setReviewProposals] = useState<BibleProposal[]>([]);
  const contextBuilder = useMemo(() => new DeterministicProjectContextBuilder(repository), []);
  const registerSaveController = useCallback((controller: EditorSaveController | null) => { editorSaveController.current = controller; }, []);
  const [messages, setMessages] = useState<ChatMessage[]>([{ id: 'welcome', role: 'assistant', content: 'Willkommen. Stelle eine Frage zu deiner aktuellen Szene oder zur bestätigten Story Bible.', time: 'Jetzt' }]);
  const [activeModal, setActiveModal] = useState<'bible' | 'research' | 'import' | null>(null);

  const loadWorkspace = useCallback(async () => {
    setLoadState({ status: 'loading' });
    try { setLoadState({ status: 'ready', workspace: await repository.loadWorkspace() }); }
    catch (error) { setLoadState({ status: 'error', message: error instanceof Error ? error.message : 'Der Workspace konnte nicht geladen werden.', detail: error instanceof Error ? error.stack : undefined }); }
  }, []);
  useEffect(() => { void loadWorkspace(); }, [loadWorkspace]);

  const workspace = loadState.status === 'ready' ? loadState.workspace : undefined;
  useEffect(() => {
    if (!workspace) return;
    const sceneIds = workspace.chapters.flatMap((chapter) => chapter.scenes).map((scene) => scene.id);
    if (!sceneIds.includes(selectedSceneId)) setSelectedSceneId(workspace.chapters.at(-1)?.scenes.at(-1)?.id ?? '');
  }, [workspace, selectedSceneId]);

  const currentScene = useMemo(() => workspace?.chapters.flatMap((chapter) => chapter.scenes).find((scene) => scene.id === selectedSceneId) ?? workspace?.chapters[0]?.scenes[0], [selectedSceneId, workspace]);
  const currentChapter = workspace?.chapters.find((chapter) => chapter.id === currentScene?.chapterId);

  const requestViewChange = useCallback(async (targetView: AppView) => {
    if (targetView === view) return;
    const controller = editorSaveController.current;
    if (view === 'editor' && controller) {
      await controller.flush();
      if (controller.getStatus() === 'error') {
        const reason = controller.getError();
        setSaveStatus('error');
        setViewError(reason instanceof Error ? reason.message : 'Die Szene konnte nicht gespeichert werden. Der Wechsel wurde abgebrochen.');
        return;
      }
      setSaveStatus('saved');
    }
    setViewError('');
    setView(targetView);
  }, [setView, view]);

  useEffect(() => {
    if (repository.mode !== 'desktop') {
      const beforeUnload = (event: BeforeUnloadEvent) => {
        if (editorSaveController.current?.hasPendingChanges()) {
          event.preventDefault();
          event.returnValue = '';
        }
      };
      window.addEventListener('beforeunload', beforeUnload);
      return () => window.removeEventListener('beforeunload', beforeUnload);
    }

    const appWindow = getCurrentWindow();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void appWindow.onCloseRequested(async (event) => {
      if (allowNextClose.current) {
        allowNextClose.current = false;
        return;
      }
      const controller = editorSaveController.current;
      if (!controller?.hasPendingChanges()) return;
      event.preventDefault();
      await controller.flush();
      if (controller.getStatus() === 'error') {
        const reason = controller.getError();
        setClosePrompt(reason instanceof Error ? reason.message : 'Die letzte Änderung konnte nicht gespeichert werden.');
        return;
      }
      allowNextClose.current = true;
      await appWindow.close();
    }).then((remove) => { if (disposed) remove(); else unlisten = remove; });
    return () => { disposed = true; unlisten?.(); };
  }, []);

  const finishClose = useCallback(async (force: boolean) => {
    const controller = editorSaveController.current;
    if (!force && controller) {
      await controller.flush();
      if (controller.getStatus() === 'error') {
        const reason = controller.getError();
        setClosePrompt(reason instanceof Error ? reason.message : 'Die letzte Änderung konnte nicht gespeichert werden.');
        return;
      }
    }
    allowNextClose.current = true;
    setClosePrompt('');
    await getCurrentWindow().close();
  }, []);

  const retryEditorSave = useCallback(async () => {
    const controller = editorSaveController.current;
    if (!controller) return;
    await controller.flush();
    if (controller.getStatus() === 'error') {
      const reason = controller.getError();
      setViewError(reason instanceof Error ? reason.message : 'Die Szene konnte nicht gespeichert werden.');
      return;
    }
    setSaveStatus('saved');
    setViewError('');
  }, []);

  const replaceScene = useCallback((saved: Scene) => {
    setLoadState((current) => current.status !== 'ready' ? current : { status: 'ready', workspace: { ...current.workspace, project: { ...current.workspace.project, updatedAt: saved.updatedAt ?? current.workspace.project.updatedAt }, chapters: current.workspace.chapters.map((chapter) => chapter.id === saved.chapterId ? { ...chapter, scenes: chapter.scenes.map((scene) => scene.id === saved.id ? saved : scene) } : chapter) } });
  }, []);
  const saveScene = useCallback(async (scene: Scene): Promise<Scene> => { const saved = await repository.updateScene(scene); replaceScene(saved); return saved; }, [replaceScene]);
  const replaceEntity = useCallback((saved: StoryEntity) => { setLoadState((current) => current.status !== 'ready' ? current : { status: 'ready', workspace: { ...current.workspace, entities: [saved, ...current.workspace.entities.filter((entity) => entity.id !== saved.id)] } }); }, []);
  const listSceneVersions = useCallback((sceneId: string) => repository.listSceneVersions(sceneId), []);
  const createSceneVersion = useCallback((sceneId: string) => repository.createSceneVersion({ sceneId, reason: 'manual' }), []);
  const restoreSceneVersion = useCallback(async (sceneId: string, versionId: string): Promise<Scene> => { const restored = await repository.restoreSceneVersion(sceneId, versionId); replaceScene(restored); return restored; }, [replaceScene]);
  const getEditorPreferences = useCallback(() => repository.getEditorPreferences(), []);
  const saveEditorPreferences = useCallback((input: Parameters<StoryRepository['saveEditorPreferences']>[0]) => repository.saveEditorPreferences(input), []);
  const createChapter = useCallback(async (title: string): Promise<Chapter> => {
    if (!workspace?.books[0]) throw new Error('Kein Buch für dieses Projekt gefunden.');
    const chapter = await repository.createChapter({ bookId: workspace.books[0].id, title });
    setLoadState((current) => current.status !== 'ready' ? current : { status: 'ready', workspace: { ...current.workspace, chapters: [...current.workspace.chapters, chapter] } });
    return chapter;
  }, [workspace]);
  const updateChapter = useCallback(async (input: UpdateChapterInput): Promise<Chapter> => {
    const saved = await repository.updateChapter(input);
    setLoadState((current) => current.status !== 'ready' ? current : { status: 'ready', workspace: { ...current.workspace, chapters: current.workspace.chapters.map((item) => item.id === saved.id ? saved : item) } });
    return saved;
  }, []);
  const createScene = useCallback(async (chapterId: string, title: string): Promise<Scene> => {
    const scene = await repository.createScene({ chapterId, title });
    setLoadState((current) => current.status !== 'ready' ? current : { status: 'ready', workspace: { ...current.workspace, chapters: current.workspace.chapters.map((chapter) => chapter.id === chapterId ? { ...chapter, scenes: [...chapter.scenes, scene] } : chapter) } });
    return scene;
  }, []);
  const runBibleUpdate = useCallback(async (): Promise<void> => {
    if (!workspace?.project || !currentScene || !currentChapter) throw new Error('Bitte zuerst eine Szene auswählen.');
    const controller = editorSaveController.current;
    if (controller) { await controller.flush(); if (controller.getStatus() === 'error') throw new Error('Die Szene konnte vor dem Bible Update nicht gespeichert werden.'); }
    const savedScene = controller?.getDraft() ?? currentScene;
    const existingRuns = await repository.listBibleUpdateRuns(workspace.project.id, savedScene.id);
    const previousRun = existingRuns.find((run) => ['completed', 'reviewed'].includes(run.status));
    const run = await repository.createBibleUpdateRun({ projectId: workspace.project.id, sceneId: savedScene.id, sceneUpdatedAt: savedScene.updatedAt ?? '', contentHash: contentHash(savedScene.content), extractorId: 'local-prototype-extractor' });
    let proposals = await repository.listBibleProposals(run.id);
    if (!proposals.length || run.id !== previousRun?.id) {
      const extraction = await new LocalPrototypeBibleExtractor().extract({ project: workspace.project, chapter: currentChapter, scene: savedScene, existingEntities: workspace.entities, previousAnalyzedContent: undefined, changedRange: changedRange(undefined, savedScene.content) });
      proposals = await repository.saveBibleProposals(run.id, extraction.proposals, workspace.project.id, savedScene.id);
    }
    setActiveReviewRun(run);
    setReviewProposals(proposals);
    await requestViewChange('bible');
  }, [currentChapter, currentScene, requestViewChange, workspace]);
  const reviewProposal = useCallback(async (input: ReviewBibleProposalInput) => { const saved = await repository.reviewBibleProposal(input); setReviewProposals((current) => current.map((proposal) => proposal.id === saved.id ? saved : proposal)); const refreshed = await repository.loadWorkspace(); setLoadState({ status: 'ready', workspace: refreshed }); }, []);
  const completeBibleReview = useCallback(async () => { if (activeReviewRun) { await repository.completeBibleReview(activeReviewRun.id); setActiveReviewRun(undefined); setReviewProposals([]); setLoadState({ status: 'ready', workspace: await repository.loadWorkspace() }); } }, [activeReviewRun]);
  const openSourceReference = useCallback(async (reference: StorySourceReference) => { setSelectedSceneId(reference.sceneId); await requestViewChange('editor'); }, [requestViewChange]);

  const renderView = () => {
    if (!workspace) return null;
    if (view === 'dashboard') return <Dashboard project={workspace.project} onOpen={() => void requestViewChange('editor')} onImport={() => setActiveModal('import')} />;
    if (view === 'editor') return <EditorView chapters={workspace.chapters} scene={currentScene} chapter={currentChapter} onBack={() => void requestViewChange('dashboard')} onSelectScene={setSelectedSceneId} onSave={saveScene} onCreateChapter={createChapter} onUpdateChapter={updateChapter} onCreateScene={createScene} onListVersions={listSceneVersions} onCreateVersion={createSceneVersion} onRestoreVersion={restoreSceneVersion} onGetEditorPreferences={getEditorPreferences} onSaveEditorPreferences={saveEditorPreferences} onBibleUpdate={runBibleUpdate} onOpenAssistant={() => setAssistantOpen(true)} onSaveStateChange={setSaveStatus} onRegisterSaveController={registerSaveController} />;
    if (view === 'bible' || view === 'characters' || view === 'threads') return <StoryBibleView entities={workspace.entities} projectId={workspace.project.id} chapters={workspace.chapters} repository={repository} activeRun={activeReviewRun} proposals={reviewProposals} onEntityChanged={replaceEntity} onOpenSourceReference={openSourceReference} onReview={reviewProposal} onCompleteReview={completeBibleReview} onCloseReview={() => { setActiveReviewRun(undefined); setReviewProposals([]); }} initialFilter={view === 'characters' ? 'character' : view === 'threads' ? 'plot_thread' : undefined} />;
    if (view === 'timeline') return <TimelineView events={demoEvents} />;
    if (view === 'mindmap') return <MindmapView nodes={mindNodes} edges={mindEdges} />;
    if (view === 'settings') return <SettingsView mode={repository.mode} project={workspace.project} onReload={loadWorkspace} />;
    return <Dashboard project={workspace.project} onOpen={() => void requestViewChange('editor')} onImport={() => setActiveModal('import')} />;
  };

  const project = workspace?.project;
  const saveLabel: Record<SaveStatus, string> = { saved: 'Gespeichert', dirty: 'Nicht gespeichert', saving: 'Speichert …', error: 'Speicherfehler' };
  const topLabel = view === 'dashboard' ? 'Übersicht' : view === 'settings' ? 'Einstellungen' : navItems.find((item) => item.view === view)?.label ?? 'Arbeitsbereich';

  return <div className={`app-shell simple-mode ${sidebarOpen ? 'sidebar-open' : 'sidebar-collapsed'} ${view === 'editor' ? 'writing-mode' : ''}`}>
    <aside className="simple-sidebar">
      <button className="simple-brand" onClick={() => setView('dashboard')} aria-label="Zum Start"><span className="brand-mark">SM</span><span><strong>StoryMemory</strong><small>Dein Buch vergisst nichts.</small></span></button>
      <div className="simple-project"><span className="eyebrow">DEIN PROJEKT</span><strong>{project?.title ?? 'Workspace'}</strong><span>{project ? 'Band 1 · Entwurf' : 'Wird geladen …'}</span></div>
      <nav className="simple-nav" aria-label="Hauptnavigation">{navItems.map(({ view: target, label, description, icon: Icon }) => <button key={target} title={sidebarOpen ? undefined : label} className={`simple-nav-button ${view === target ? 'active' : ''}`} onClick={() => void requestViewChange(target)}><Icon size={21} /><span><strong>{label}</strong><small>{description}</small></span></button>)}</nav>
      <div className="simple-sidebar-bottom"><button className="simple-nav-button" title={sidebarOpen ? undefined : 'Importieren'} onClick={() => setActiveModal('import')}><Upload size={21} /><span><strong>Importieren</strong><small>TXT oder Markdown</small></span></button><button className="simple-nav-button" title={sidebarOpen ? undefined : 'Einstellungen'} onClick={() => void requestViewChange('settings')}><Settings2 size={21} /><span><strong>Einstellungen</strong><small>App anpassen</small></span></button><div className="provider-status"><span className="status-dot green" /><span className="provider-label">{repository.mode === 'desktop' ? 'Lokaler Desktop-Modus' : 'Browser-Demo-Modus'}</span></div></div>
    </aside>
    <main className="main-area">
      <header className="topbar simple-topbar"><div className="topbar-title"><button className="sidebar-toggle" onClick={() => setSidebarOpen((open) => !open)} aria-label={sidebarOpen ? 'Sidebar einklappen' : 'Sidebar öffnen'} title={sidebarOpen ? 'Sidebar einklappen' : 'Sidebar öffnen'}>{sidebarOpen ? <PanelLeftClose size={18} /> : <PanelLeftOpen size={18} />}</button><div className="topbar-copy"><span className="eyebrow">{view === 'dashboard' ? 'START' : project?.title ?? 'STORYMEMORY'}</span><strong>{topLabel}</strong></div></div><div className="topbar-actions"><span className={`save-state save-state-${saveStatus}`}><span className={`status-dot status-dot-${saveStatus}`} /> {saveLabel[saveStatus]}</span><button className="assistant-button" onClick={() => setAssistantOpen(true)}><MessageCircle size={18} /> Assistent öffnen</button></div></header>
      <div className="content-scroll">{viewError && <div className="save-error workspace-save-error" role="alert"><strong>Speichern erforderlich</strong><span>{viewError}</span><button className="text-button" onClick={() => void retryEditorSave()}>Erneut versuchen</button></div>}{loadState.status === 'loading' && <LoadingView mode={repository.mode} />}{loadState.status === 'error' && <ErrorView message={loadState.message} detail={loadState.detail} onRetry={() => void loadWorkspace()} />}{loadState.status === 'ready' && renderView()}</div>
    </main>
    {assistantOpen && <div className="assistant-drawer"><button className="drawer-close" onClick={() => setAssistantOpen(false)} aria-label="Assistent schließen"><X size={20} /></button>{workspace && <ChatPanel messages={messages} onMessagesChange={setMessages} contextBuilder={contextBuilder} contextRequest={{ projectId: workspace.project.id, currentChapterId: currentChapter?.id, currentSceneId: currentScene?.id }} onOpenSourceReference={(reference) => void openSourceReference(reference)} />}</div>}
    {activeModal && <Modal type={activeModal} onClose={() => setActiveModal(null)} />}
    {closePrompt && <ClosePrompt message={closePrompt} onRetry={() => void finishClose(false)} onForceClose={() => void finishClose(true)} onCancel={() => setClosePrompt('')} />}
  </div>;
}

function LoadingView({ mode }: { mode: RuntimeMode }) { return <section className="state-view"><RefreshCw className="spin" size={26} /><span className="eyebrow">{mode === 'desktop' ? 'LOKALE DATENBANK' : 'BROWSER-DEMO'}</span><h1>Workspace wird geladen</h1><p>Deine Projekte, Kapitel und Szenen werden vorbereitet.</p></section>; }
function ErrorView({ message, detail, onRetry }: { message: string; detail?: string; onRetry: () => void }) { return <section className="state-view state-error"><span className="eyebrow">DATENBANKFEHLER</span><h1>Workspace konnte nicht geladen werden</h1><p>{message}</p><button className="primary-button large" onClick={onRetry}><RefreshCw size={17} /> Erneut laden</button>{detail && <details><summary>Technische Details</summary><pre>{detail}</pre></details>}</section>; }

function ClosePrompt({ message, onRetry, onForceClose, onCancel }: { message: string; onRetry: () => void; onForceClose: () => void; onCancel: () => void }) {
  return <div className="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="close-save-title"><div className="modal simple-modal close-save-modal"><div className="modal-head"><div><span className="eyebrow">LETZTE ÄNDERUNG</span><h2 id="close-save-title">Noch nicht gespeichert</h2></div></div><p className="modal-intro">{message} Was möchtest du tun?</p><div className="close-save-actions"><button className="primary-button large" onClick={onRetry}>Erneut versuchen</button><button className="ghost-button large" onClick={onForceClose}>Trotzdem schließen</button><button className="text-button" onClick={onCancel}>Abbrechen</button></div></div></div>;
}

function Modal({ type, onClose }: { type: 'bible' | 'research' | 'import'; onClose: () => void }) { const title = type === 'bible' ? 'Story Bible aktualisieren' : type === 'research' ? 'Projekt prüfen' : 'Manuskript importieren'; return <div className="modal-backdrop" role="dialog" aria-modal="true"><div className="modal simple-modal"><div className="modal-head"><div><span className="eyebrow">{type === 'bible' ? 'VORSCHLÄGE PRÜFEN' : 'EINFACHER WORKFLOW'}</span><h2>{title}</h2></div><button className="icon-button" onClick={onClose} aria-label="Dialog schließen"><X size={20} /></button></div>{type === 'bible' && <><p className="modal-intro">Ich habe neue mögliche Fakten gefunden. Du entscheidest, was in deine Story Bible kommt.</p><div className="proposal-summary simple-summary"><div><strong>7</strong><span>neue Fakten</span></div><div><strong>2</strong><span>Figurenänderungen</span></div><div><strong>1</strong><span>möglicher Widerspruch</span></div></div><button className="primary-button large full" onClick={onClose}>Vorschläge ansehen</button></>}{type === 'research' && <><p className="modal-intro">Prüfe deine aktuelle Szene gegen die bisherige Geschichte.</p><div className="simple-choice"><strong>Was soll geprüft werden?</strong><button className="choice-button active">Aktuelle Szene</button><button className="choice-button">Aktuelles Kapitel</button><button className="choice-button">Gesamtes Buch</button></div><button className="primary-button large full" onClick={onClose}>Prüfung starten</button></>}{type === 'import' && <><p className="modal-intro">Wähle eine TXT- oder Markdown-Datei. Alles bleibt auf deinem Computer.</p><div className="drop-zone simple-drop"><Upload size={28} /><strong>Datei auswählen</strong><span>oder hier hineinziehen</span></div><button className="primary-button large full" onClick={onClose}>Import vorbereiten</button></>}</div></div>; }
