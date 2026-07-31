import { useMemo, useState } from 'react';
import { BookOpen, BrainCircuit, CheckCircle2, ChevronDown, FileText, Gauge, PanelLeft, PanelRight, Search, Settings2, Sparkles, Upload, X } from 'lucide-react';
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

const navItems: { view: AppView; label: string; icon: typeof BookOpen }[] = [
  { view: 'editor', label: 'Schreiben', icon: FileText }, { view: 'bible', label: 'Story Bible', icon: BookOpen }, { view: 'timeline', label: 'Timeline', icon: Gauge }, { view: 'mindmap', label: 'Mindmap', icon: BrainCircuit }, { view: 'characters', label: 'Charaktere', icon: Sparkles }, { view: 'threads', label: 'Handlungsstränge', icon: CheckCircle2 }, { view: 'research', label: 'Recherche', icon: Search }, { view: 'files', label: 'Projektdateien', icon: Upload },
];

export function App() {
  const { view, setView, sidebarOpen, inspectorOpen, toggleSidebar, toggleInspector, focusMode } = useAppStore();
  const [state, setState] = useState(getLocalState);
  const [selectedSceneId, setSelectedSceneId] = useState('scene-3');
  const [messages, setMessages] = useState<ChatMessage[]>([{ id: 'welcome', role: 'assistant', content: 'Willkommen zurück. Ich habe die Story Bible von „Zugestellt“ geladen. Wobei soll ich dich unterstützen?', time: '09:42' }, { id: 'question', role: 'user', content: 'Weiß Marek an dieser Stelle bereits von der veränderten Paketnummer?', time: '09:43' }, { id: 'answer', role: 'assistant', content: 'Laut Band 1, Kapitel 3, Szene 2 erfährt Marek davon. Die aktuelle Szene spielt danach. Die Information ist daher konsistent.', sources: ['Band 1', 'Kapitel 3', 'Szene 2'], time: '09:43' }]);
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

  return <div className={`app-shell ${focusMode ? 'focus-mode' : ''}`}>
    <aside className="rail">
      <button className="brand-mark" onClick={() => setView('dashboard')} aria-label="Zum Dashboard"><span>SM</span></button>
      <div className="rail-group">
        {navItems.map(({ view: target, label, icon: Icon }) => <button key={target} className={`rail-button ${view === target ? 'active' : ''}`} onClick={() => setView(target)} title={label} aria-label={label}><Icon size={18} /></button>)}
      </div>
      <div className="rail-bottom"><button className="rail-button" onClick={() => setView('settings')} title="Einstellungen" aria-label="Einstellungen"><Settings2 size={18} /></button><div className="provider-dot" title="Mock Provider bereit" /></div>
    </aside>
    {!focusMode && sidebarOpen && <ChatPanel messages={messages} onMessagesChange={setMessages} />}
    <main className="main-area">
      <header className="topbar"><div className="breadcrumb"><button className="icon-button" onClick={toggleSidebar} aria-label="Kontextbereich ein- oder ausblenden"><PanelLeft size={17} /></button><span className="muted">Zugestellt</span><ChevronDown size={14} className="muted" /><span>{view === 'editor' ? currentChapter?.title ?? 'Kapitel' : navItems.find((item) => item.view === view)?.label ?? 'Übersicht'}</span></div><div className="topbar-actions"><span className="save-state"><span className="status-dot green" /> Lokal gespeichert</span><button className="icon-button" onClick={toggleInspector} aria-label="Inspector ein- oder ausblenden"><PanelRight size={17} /></button></div></header>
      <div className="content-scroll">{renderView()}</div>
    </main>
    {!focusMode && inspectorOpen && <Inspector scene={currentScene} />}
    {activeModal && <Modal type={activeModal} onClose={() => setActiveModal(null)} />}
  </div>;
}

function Inspector({ scene }: { scene?: Scene }) { return <aside className="inspector"><div className="inspector-head"><span className="eyebrow">INSPECTOR</span><span className="status-pill purple">Lokaler Kontext</span></div><div className="inspector-section"><h3>Aktueller Kontext</h3><div className="context-item"><span className="context-icon"><FileText size={15} /></span><div><strong>{scene?.title ?? 'Keine Szene'}</strong><span>1.284 Wörter · {scene?.status ?? 'draft'}</span></div></div><div className="context-item"><span className="context-icon"><Sparkles size={15} /></span><div><strong>Marek</strong><span>Perspektivfigur</span></div></div><div className="context-item"><span className="context-icon"><BookOpen size={15} /></span><div><strong>3 aktive Hinweise</strong><span>2 bestätigt · 1 Vermutung</span></div></div></div><div className="inspector-section"><h3>Offene Hinweise</h3><div className="warning-row"><span className="warning-mark">!</span><span>Paketnummer wurde noch nicht erklärt</span></div><div className="warning-row"><span className="warning-mark">!</span><span>Uhrzeit im Café prüfen</span></div></div><div className="privacy-note"><span className="privacy-icon">◈</span><p>Deine Manuskripte und Story-Bible-Daten werden lokal gespeichert. Inhalte werden nur bei einer Analyse an einen verbundenen Anbieter gesendet.</p></div></aside>; }

function Modal({ type, onClose }: { type: 'bible' | 'research' | 'import'; onClose: () => void }) { const title = type === 'bible' ? 'Bible Update Review' : type === 'research' ? 'Deep Research vorbereiten' : 'Manuskript importieren'; return <div className="modal-backdrop" role="dialog" aria-modal="true"><div className="modal"><div className="modal-head"><div><span className="eyebrow">{type === 'bible' ? 'KANON-REVIEW' : 'LOKALER WORKFLOW'}</span><h2>{title}</h2></div><button className="icon-button" onClick={onClose} aria-label="Dialog schließen"><X size={18} /></button></div>{type === 'bible' && <><p className="modal-intro">Neue Vorschläge werden niemals still in den Kanon geschrieben.</p><div className="proposal-summary"><div><strong>7</strong><span>neue Fakten</span></div><div><strong>2</strong><span>Charakterveränderungen</span></div><div><strong>1</strong><span>Widerspruch</span></div><div><strong>1</strong><span>Timeline-Ereignis</span></div></div>{['Marek erkennt die Abweichung der Paketnummer', 'Lena hält die eigentliche Erklärung zurück', 'Die Uhr im Café Meridian geht sieben Minuten nach'].map((item, index) => <div className="review-row" key={item}><div><span className={`status-pill ${index === 2 ? 'yellow' : 'green'}`}>{index === 2 ? 'Vermutung' : 'Fakt'}</span><strong>{item}</strong><span>Quelle: Kapitel 3 · Die zweite Nummer</span></div><div className="review-actions"><button className="ghost-button">Bearbeiten</button><button className="primary-button">Übernehmen</button></div></div>)}</>}{type === 'research' && <><p className="modal-intro">Prüfe, ob Mareks Entscheidung in Kapitel 6 durch Band 1 ausreichend vorbereitet wurde.</p><label className="field-label">Analyseumfang<select defaultValue="band"><option value="scene">Aktuelle Szene</option><option value="chapter">Aktuelles Kapitel</option><option value="band">Aktueller Band</option><option value="series">Gesamte Buchreihe</option></select></label><div className="job-steps"><span className="done">✓ Kontext laden</span><span className="current">○ Quellen prüfen</span><span>○ Analyse ausführen</span><span>○ Ergebnis zusammenfassen</span></div><div className="progress-line"><span style={{ width: '36%' }} /></div><p className="muted small">Verwendet das Kontingent des verbundenen KI-Anbieters. Der Job bleibt lokal pausierbar.</p><button className="primary-button full">Analyse simulieren</button></>}{type === 'import' && <><p className="modal-intro">TXT und Markdown werden lokal eingelesen. DOCX und EPUB sind als nächste Importstufe vorbereitet.</p><div className="drop-zone"><Upload size={24} /><strong>Datei hier ablegen</strong><span>oder eine lokale Datei auswählen</span><button className="ghost-button">Datei auswählen</button></div><div className="import-options"><label className="field-label">Titel<input defaultValue="Zugestellt" /></label><label className="field-label">Band<select defaultValue="1"><option>1</option><option>2</option><option>3</option></select></label></div><button className="primary-button full">Import vorbereiten</button></>}</div></div>; }
