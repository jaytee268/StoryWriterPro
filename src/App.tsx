import { useCallback, useEffect, useMemo, useState } from 'react';
import { BookOpen, BrainCircuit, FileText, Gauge, MessageCircle, RefreshCw, Settings2, Upload, X } from 'lucide-react';
import { useAppStore } from './stores/useAppStore';
import { demoEvents, mindEdges, mindNodes } from './services/mockData';
import { createStoryRepository, type RuntimeMode, type StoryRepository } from './services/storyRepository';
import type { AppView, ChatMessage, Chapter, Scene, WorkspaceSnapshot } from './types/domain';
import { Dashboard } from './features/projects/Dashboard';
import { EditorView } from './features/editor/EditorView';
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
  const [saveStatus, setSaveStatus] = useState<SaveStatus>('saved');
  const [messages, setMessages] = useState<ChatMessage[]>([{ id: 'welcome', role: 'assistant', content: 'Willkommen. Ich kenne dein Projekt und helfe dir beim Prüfen.', time: '09:42' }, { id: 'question', role: 'user', content: 'Weiß Marek bereits von der veränderten Paketnummer?', time: '09:43' }, { id: 'answer', role: 'assistant', content: 'Ja. Laut Kapitel 3 erfährt Marek davon. Die Szene ist konsistent.', sources: ['Kapitel 3', 'Szene 2'], time: '09:43' }]);
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

  const replaceScene = useCallback((saved: Scene) => {
    setLoadState((current) => current.status !== 'ready' ? current : { status: 'ready', workspace: { ...current.workspace, project: { ...current.workspace.project, updatedAt: saved.updatedAt ?? current.workspace.project.updatedAt }, chapters: current.workspace.chapters.map((chapter) => chapter.id === saved.chapterId ? { ...chapter, scenes: chapter.scenes.map((scene) => scene.id === saved.id ? saved : scene) } : chapter) } });
  }, []);
  const saveScene = useCallback(async (scene: Scene): Promise<Scene> => { const saved = await repository.updateScene(scene); replaceScene(saved); return saved; }, [replaceScene]);
  const createChapter = useCallback(async (title: string): Promise<Chapter> => {
    if (!workspace?.books[0]) throw new Error('Kein Buch für dieses Projekt gefunden.');
    const chapter = await repository.createChapter({ bookId: workspace.books[0].id, title });
    setLoadState((current) => current.status !== 'ready' ? current : { status: 'ready', workspace: { ...current.workspace, chapters: [...current.workspace.chapters, chapter] } });
    return chapter;
  }, [workspace]);
  const createScene = useCallback(async (chapterId: string, title: string): Promise<Scene> => {
    const scene = await repository.createScene({ chapterId, title });
    setLoadState((current) => current.status !== 'ready' ? current : { status: 'ready', workspace: { ...current.workspace, chapters: current.workspace.chapters.map((chapter) => chapter.id === chapterId ? { ...chapter, scenes: [...chapter.scenes, scene] } : chapter) } });
    return scene;
  }, []);

  const renderView = () => {
    if (!workspace) return null;
    if (view === 'dashboard') return <Dashboard project={workspace.project} onOpen={() => setView('editor')} onImport={() => setActiveModal('import')} />;
    if (view === 'editor') return <EditorView chapters={workspace.chapters} scene={currentScene} chapter={currentChapter} onSelectScene={setSelectedSceneId} onSave={saveScene} onCreateChapter={createChapter} onCreateScene={createScene} onSaveStateChange={setSaveStatus} onBibleUpdate={() => setActiveModal('bible')} onResearch={() => setActiveModal('research')} />;
    if (view === 'bible' || view === 'characters' || view === 'threads') return <StoryBibleView entities={workspace.entities} initialFilter={view === 'characters' ? 'character' : view === 'threads' ? 'plot_thread' : undefined} />;
    if (view === 'timeline') return <TimelineView events={demoEvents} />;
    if (view === 'mindmap') return <MindmapView nodes={mindNodes} edges={mindEdges} />;
    if (view === 'settings') return <SettingsView mode={repository.mode} project={workspace.project} onReload={loadWorkspace} />;
    return <Dashboard project={workspace.project} onOpen={() => setView('editor')} onImport={() => setActiveModal('import')} />;
  };

  const project = workspace?.project;
  const saveLabel: Record<SaveStatus, string> = { saved: 'Gespeichert', dirty: 'Nicht gespeichert', saving: 'Speichert …', error: 'Speicherfehler' };
  const topLabel = view === 'dashboard' ? 'Übersicht' : view === 'settings' ? 'Einstellungen' : navItems.find((item) => item.view === view)?.label ?? 'Arbeitsbereich';

  return <div className="app-shell simple-mode">
    <aside className="simple-sidebar">
      <button className="simple-brand" onClick={() => setView('dashboard')} aria-label="Zum Start"><span className="brand-mark">SM</span><span><strong>StoryMemory</strong><small>Dein Buch vergisst nichts.</small></span></button>
      <div className="simple-project"><span className="eyebrow">DEIN PROJEKT</span><strong>{project?.title ?? 'Workspace'}</strong><span>{project ? 'Band 1 · Entwurf' : 'Wird geladen …'}</span></div>
      <nav className="simple-nav" aria-label="Hauptnavigation">{navItems.map(({ view: target, label, description, icon: Icon }) => <button key={target} className={`simple-nav-button ${view === target ? 'active' : ''}`} onClick={() => setView(target)}><Icon size={21} /><span><strong>{label}</strong><small>{description}</small></span></button>)}</nav>
      <div className="simple-sidebar-bottom"><button className="simple-nav-button" onClick={() => setActiveModal('import')}><Upload size={21} /><span><strong>Importieren</strong><small>TXT oder Markdown</small></span></button><button className="simple-nav-button" onClick={() => setView('settings')}><Settings2 size={21} /><span><strong>Einstellungen</strong><small>App anpassen</small></span></button><div className="provider-status"><span className="status-dot green" /> {repository.mode === 'desktop' ? 'Lokaler Desktop-Modus' : 'Browser-Demo-Modus'}</div></div>
    </aside>
    <main className="main-area">
      <header className="topbar simple-topbar"><div><span className="eyebrow">{view === 'dashboard' ? 'START' : project?.title ?? 'STORYMEMORY'}</span><strong>{topLabel}</strong></div><div className="topbar-actions"><span className={`save-state save-state-${saveStatus}`}><span className="status-dot green" /> {saveLabel[saveStatus]}</span><button className="assistant-button" onClick={() => setAssistantOpen(true)}><MessageCircle size={18} /> Assistent öffnen</button></div></header>
      <div className="content-scroll">{loadState.status === 'loading' && <LoadingView mode={repository.mode} />}{loadState.status === 'error' && <ErrorView message={loadState.message} detail={loadState.detail} onRetry={() => void loadWorkspace()} />}{loadState.status === 'ready' && renderView()}</div>
    </main>
    {assistantOpen && <div className="assistant-drawer"><button className="drawer-close" onClick={() => setAssistantOpen(false)} aria-label="Assistent schließen"><X size={20} /></button><ChatPanel messages={messages} onMessagesChange={setMessages} /></div>}
    {activeModal && <Modal type={activeModal} onClose={() => setActiveModal(null)} />}
  </div>;
}

function LoadingView({ mode }: { mode: RuntimeMode }) { return <section className="state-view"><RefreshCw className="spin" size={26} /><span className="eyebrow">{mode === 'desktop' ? 'LOKALE DATENBANK' : 'BROWSER-DEMO'}</span><h1>Workspace wird geladen</h1><p>Deine Projekte, Kapitel und Szenen werden vorbereitet.</p></section>; }
function ErrorView({ message, detail, onRetry }: { message: string; detail?: string; onRetry: () => void }) { return <section className="state-view state-error"><span className="eyebrow">DATENBANKFEHLER</span><h1>Workspace konnte nicht geladen werden</h1><p>{message}</p><button className="primary-button large" onClick={onRetry}><RefreshCw size={17} /> Erneut laden</button>{detail && <details><summary>Technische Details</summary><pre>{detail}</pre></details>}</section>; }

function Modal({ type, onClose }: { type: 'bible' | 'research' | 'import'; onClose: () => void }) { const title = type === 'bible' ? 'Story Bible aktualisieren' : type === 'research' ? 'Projekt prüfen' : 'Manuskript importieren'; return <div className="modal-backdrop" role="dialog" aria-modal="true"><div className="modal simple-modal"><div className="modal-head"><div><span className="eyebrow">{type === 'bible' ? 'VORSCHLÄGE PRÜFEN' : 'EINFACHER WORKFLOW'}</span><h2>{title}</h2></div><button className="icon-button" onClick={onClose} aria-label="Dialog schließen"><X size={20} /></button></div>{type === 'bible' && <><p className="modal-intro">Ich habe neue mögliche Fakten gefunden. Du entscheidest, was in deine Story Bible kommt.</p><div className="proposal-summary simple-summary"><div><strong>7</strong><span>neue Fakten</span></div><div><strong>2</strong><span>Figurenänderungen</span></div><div><strong>1</strong><span>möglicher Widerspruch</span></div></div><button className="primary-button large full" onClick={onClose}>Vorschläge ansehen</button></>}{type === 'research' && <><p className="modal-intro">Prüfe deine aktuelle Szene gegen die bisherige Geschichte.</p><div className="simple-choice"><strong>Was soll geprüft werden?</strong><button className="choice-button active">Aktuelle Szene</button><button className="choice-button">Aktuelles Kapitel</button><button className="choice-button">Gesamtes Buch</button></div><button className="primary-button large full" onClick={onClose}>Prüfung starten</button></>}{type === 'import' && <><p className="modal-intro">Wähle eine TXT- oder Markdown-Datei. Alles bleibt auf deinem Computer.</p><div className="drop-zone simple-drop"><Upload size={28} /><strong>Datei auswählen</strong><span>oder hier hineinziehen</span></div><button className="primary-button large full" onClick={onClose}>Import vorbereiten</button></>}</div></div>; }
