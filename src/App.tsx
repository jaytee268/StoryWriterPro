import { useMemo, useState } from 'react';
import { BookOpen, BrainCircuit, FileText, Gauge, MessageCircle, Settings2, Upload, X } from 'lucide-react';
import { useAppStore } from './stores/useAppStore';
import { getLocalState, saveScene } from './services/localStore';
import { demoEvents, mindEdges, mindNodes } from './services/mockData';
import type { AppView, ChatMessage, Scene } from './types/domain';
import { Dashboard } from './features/projects/Dashboard';
import { EditorView } from './features/editor/EditorView';
import { ChatPanel } from './features/chat/ChatPanel';
import { StoryBibleView } from './features/story-bible/StoryBibleView';
import { TimelineView } from './features/timeline/TimelineView';
import { MindmapView } from './features/mindmap/MindmapView';

const navItems: { view: AppView; label: string; description: string; icon: typeof BookOpen }[] = [
  { view: 'editor', label: 'Schreiben', description: 'Manuskript öffnen', icon: FileText },
  { view: 'bible', label: 'Story Bible', description: 'Figuren & Fakten', icon: BookOpen },
  { view: 'timeline', label: 'Timeline', description: 'Was wann passiert', icon: Gauge },
  { view: 'mindmap', label: 'Mindmap', description: 'Zusammenhänge sehen', icon: BrainCircuit },
];

export function App() {
  const { view, setView } = useAppStore();
  const [state, setState] = useState(getLocalState);
  const [selectedSceneId, setSelectedSceneId] = useState('scene-3');
  const [assistantOpen, setAssistantOpen] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([{ id: 'welcome', role: 'assistant', content: 'Willkommen. Ich kenne dein Projekt und helfe dir beim Prüfen.', time: '09:42' }, { id: 'question', role: 'user', content: 'Weiß Marek bereits von der veränderten Paketnummer?', time: '09:43' }, { id: 'answer', role: 'assistant', content: 'Ja. Laut Kapitel 3 erfährt Marek davon. Die Szene ist konsistent.', sources: ['Kapitel 3', 'Szene 2'], time: '09:43' }]);
  const [activeModal, setActiveModal] = useState<'bible' | 'research' | 'import' | null>(null);
  const currentScene = useMemo(() => state.chapters.flatMap((chapter) => chapter.scenes).find((scene) => scene.id === selectedSceneId) ?? state.chapters[0]?.scenes[0], [selectedSceneId, state.chapters]);
  const currentChapter = state.chapters.find((chapter) => chapter.id === currentScene?.chapterId);
  const saveCurrentScene = async (scene: Scene) => { await saveScene(scene); setState(getLocalState()); };

  const renderView = () => {
    if (view === 'dashboard') return <Dashboard project={state.project} onOpen={() => setView('editor')} onImport={() => setActiveModal('import')} />;
    if (view === 'editor') return <EditorView chapters={state.chapters} scene={currentScene} chapter={currentChapter} onSelectScene={setSelectedSceneId} onSave={saveCurrentScene} onBibleUpdate={() => setActiveModal('bible')} onResearch={() => setActiveModal('research')} />;
    if (view === 'bible' || view === 'characters' || view === 'threads') return <StoryBibleView entities={state.entities} initialFilter={view === 'characters' ? 'character' : view === 'threads' ? 'plot_thread' : undefined} />;
    if (view === 'timeline') return <TimelineView events={demoEvents} />;
    if (view === 'mindmap') return <MindmapView nodes={mindNodes} edges={mindEdges} />;
    return <Dashboard project={state.project} onOpen={() => setView('editor')} onImport={() => setActiveModal('import')} />;
  };

  return <div className="app-shell simple-mode">
    <aside className="simple-sidebar">
      <button className="simple-brand" onClick={() => setView('dashboard')} aria-label="Zum Start"><span className="brand-mark">SM</span><span><strong>StoryMemory</strong><small>Dein Buch vergisst nichts.</small></span></button>
      <div className="simple-project"><span className="eyebrow">DEIN PROJEKT</span><strong>Zugestellt</strong><span>Band 1 · Entwurf</span></div>
      <nav className="simple-nav" aria-label="Hauptnavigation">{navItems.map(({ view: target, label, description, icon: Icon }) => <button key={target} className={`simple-nav-button ${view === target ? 'active' : ''}`} onClick={() => setView(target)}><Icon size={21} /><span><strong>{label}</strong><small>{description}</small></span></button>)}</nav>
      <div className="simple-sidebar-bottom"><button className="simple-nav-button" onClick={() => setActiveModal('import')}><Upload size={21} /><span><strong>Importieren</strong><small>TXT oder Markdown</small></span></button><button className="simple-nav-button" onClick={() => setView('settings')}><Settings2 size={21} /><span><strong>Einstellungen</strong><small>App anpassen</small></span></button><div className="provider-status"><span className="status-dot green" /> Lokaler Modus</div></div>
    </aside>
    <main className="main-area">
      <header className="topbar simple-topbar"><div><span className="eyebrow">{view === 'dashboard' ? 'START' : 'ZUGESTELLT'}</span><strong>{view === 'dashboard' ? 'Übersicht' : navItems.find((item) => item.view === view)?.label ?? 'Arbeitsbereich'}</strong></div><div className="topbar-actions"><span className="save-state"><span className="status-dot green" /> Automatisch gespeichert</span><button className="assistant-button" onClick={() => setAssistantOpen(true)}><MessageCircle size={18} /> Assistent öffnen</button></div></header>
      <div className="content-scroll">{renderView()}</div>
    </main>
    {assistantOpen && <div className="assistant-drawer"><button className="drawer-close" onClick={() => setAssistantOpen(false)} aria-label="Assistent schließen"><X size={20} /></button><ChatPanel messages={messages} onMessagesChange={setMessages} /></div>}
    {activeModal && <Modal type={activeModal} onClose={() => setActiveModal(null)} />}
  </div>;
}

function Modal({ type, onClose }: { type: 'bible' | 'research' | 'import'; onClose: () => void }) { const title = type === 'bible' ? 'Story Bible aktualisieren' : type === 'research' ? 'Projekt prüfen' : 'Manuskript importieren'; return <div className="modal-backdrop" role="dialog" aria-modal="true"><div className="modal simple-modal"><div className="modal-head"><div><span className="eyebrow">{type === 'bible' ? 'VORSCHLÄGE PRÜFEN' : 'EINFACHER WORKFLOW'}</span><h2>{title}</h2></div><button className="icon-button" onClick={onClose} aria-label="Dialog schließen"><X size={20} /></button></div>{type === 'bible' && <><p className="modal-intro">Ich habe neue mögliche Fakten gefunden. Du entscheidest, was in deine Story Bible kommt.</p><div className="proposal-summary simple-summary"><div><strong>7</strong><span>neue Fakten</span></div><div><strong>2</strong><span>Figurenänderungen</span></div><div><strong>1</strong><span>möglicher Widerspruch</span></div></div><button className="primary-button large full" onClick={onClose}>Vorschläge ansehen</button></>}{type === 'research' && <><p className="modal-intro">Prüfe deine aktuelle Szene gegen die bisherige Geschichte.</p><div className="simple-choice"><strong>Was soll geprüft werden?</strong><button className="choice-button active">Aktuelle Szene</button><button className="choice-button">Aktuelles Kapitel</button><button className="choice-button">Gesamtes Buch</button></div><button className="primary-button large full" onClick={onClose}>Prüfung starten</button></>}{type === 'import' && <><p className="modal-intro">Wähle eine TXT- oder Markdown-Datei. Alles bleibt auf deinem Computer.</p><div className="drop-zone simple-drop"><Upload size={28} /><strong>Datei auswählen</strong><span>oder hier hineinziehen</span></div><button className="primary-button large full" onClick={onClose}>Import vorbereiten</button></>}</div></div>; }
