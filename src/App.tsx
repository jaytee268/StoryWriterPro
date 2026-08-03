import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { BookOpen, BrainCircuit, FileText, Gauge, MessageCircle, PanelLeftClose, PanelLeftOpen, RefreshCw, Settings2, Upload, X } from 'lucide-react';
import { useAppStore } from './stores/useAppStore';
import { demoEvents, mindEdges, mindNodes } from './services/mockData';
import { createStoryRepository, type RuntimeMode, type StoryRepository } from './services/storyRepository';
import type { AiProviderSettings, AppView, BibleProposal, BibleUpdateRun, CharacterMemoryProposal, CharacterMemoryUpdateRun, ChatMessage, Chapter, CreateStyleReferenceInput, PendingSourceNavigation, ReviewBibleProposalInput, ReviewCharacterMemoryProposalInput, Scene, StoryEntity, StorySourceReference, StyleReference, UpdateChapterInput, WorkspaceSnapshot } from './types/domain';
import { DeterministicProjectContextBuilder } from './services/contextBuilder';
import { changedRange } from './services/bibleExtractor';
import { Dashboard } from './features/projects/Dashboard';
import { EditorView, type EditorSaveController } from './features/editor/EditorView';
import { ChatPanel } from './features/chat/ChatPanel';
import { StoryBibleView } from './features/story-bible/StoryBibleView';
import { TimelineView } from './features/timeline/TimelineView';
import { MindmapView } from './features/mindmap/MindmapView';
import { SettingsView } from './features/settings/SettingsView';
import { canonicalizeSceneForAi } from './utils/aiText';
import { defaultAiProviderSettings, providerRouter, type StoryAiProvider } from './services/aiProviderService';
import { createLongformRepository } from './services/longformRepository';
import { LongformDraftView } from './features/longform/LongformDraftView';
import { ManuscriptImportModal } from './features/import/ManuscriptImportModal';

const repository: StoryRepository = createStoryRepository();
const longformRepository = createLongformRepository();
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
  const [pendingSourceNavigation, setPendingSourceNavigation] = useState<PendingSourceNavigation>();
  const [activeReviewRun, setActiveReviewRun] = useState<BibleUpdateRun>();
  const [reviewProposals, setReviewProposals] = useState<BibleProposal[]>([]);
  const [activeMemoryRun, setActiveMemoryRun] = useState<CharacterMemoryUpdateRun>();
  const [memoryProposals, setMemoryProposals] = useState<CharacterMemoryProposal[]>([]);
  const [resumeMemoryReview, setResumeMemoryReview] = useState<{ run: CharacterMemoryUpdateRun; proposals: CharacterMemoryProposal[] }>();
  const [providerSettings, setProviderSettings] = useState<AiProviderSettings>(defaultAiProviderSettings);
  const [providerNotice, setProviderNotice] = useState('');
  const [bibleUpdateProvider, setBibleUpdateProvider] = useState<StoryAiProvider>();
  const contextBuilder = useMemo(() => new DeterministicProjectContextBuilder(repository), []);
  const registerSaveController = useCallback((controller: EditorSaveController | null) => { editorSaveController.current = controller; }, []);
  const [messages, setMessages] = useState<ChatMessage[]>([{ id: 'welcome', role: 'assistant', content: 'Willkommen. Stelle eine Frage zu deiner aktuellen Szene oder zur bestätigten Story Bible.', time: 'Jetzt' }]);
  const [activeModal, setActiveModal] = useState<'bible' | 'research' | 'import' | null>(null);
  const [longformInstruction, setLongformInstruction] = useState<string>();

  const loadWorkspace = useCallback(async () => {
    setLoadState({ status: 'loading' });
    try { setLoadState({ status: 'ready', workspace: await repository.loadWorkspace() }); }
    catch (error) { setLoadState({ status: 'error', message: error instanceof Error ? error.message : 'Der Workspace konnte nicht geladen werden.', detail: error instanceof Error ? error.stack : undefined }); }
  }, []);
  useEffect(() => { void loadWorkspace(); }, [loadWorkspace]);
  useEffect(() => { void providerRouter.getSettings().then(setProviderSettings).catch(() => setProviderSettings(defaultAiProviderSettings)); }, []);

  const workspace = loadState.status === 'ready' ? loadState.workspace : undefined;
  useEffect(() => {
    if (!workspace?.project.id || activeMemoryRun) return;
    let cancelled = false;
    void repository.listCharacterMemoryUpdateRuns(workspace.project.id).then(async (runs) => {
      const open = runs.filter((run) => ['pending', 'running', 'completed'].includes(run.status)).sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];
      if (!open) return;
      const proposals = await repository.listCharacterMemoryProposals(open.id);
      if (!cancelled && proposals.some((proposal) => proposal.reviewStatus === 'pending')) setResumeMemoryReview({ run: open, proposals });
    }).catch(() => undefined);
    return () => { cancelled = true; };
  }, [activeMemoryRun, workspace?.project.id]);
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

  const requestSceneChange = useCallback(async (targetSceneId: string): Promise<boolean> => {
    if (targetSceneId === selectedSceneId) return true;
    const controller = editorSaveController.current;
    if (view === 'editor' && controller) {
      await controller.flush();
      if (controller.getStatus() === 'error') {
        const reason = controller.getError();
        setSaveStatus('error');
        setViewError(reason instanceof Error ? reason.message : 'Die Szene konnte nicht gespeichert werden. Der Szenenwechsel wurde abgebrochen.');
        return false;
      }
      setSaveStatus('saved');
    }
    setViewError('');
    setSelectedSceneId(targetSceneId);
    return true;
  }, [selectedSceneId, view]);

  const openImport = useCallback(async () => {
    const controller = editorSaveController.current;
    if (view === 'editor' && controller) {
      await controller.flush();
      if (controller.getStatus() === 'error') {
        const reason = controller.getError();
        setSaveStatus('error');
        setViewError(reason instanceof Error ? reason.message : 'Die Szene konnte vor dem Import nicht gespeichert werden.');
        return;
      }
    }
    setViewError('');
    setActiveModal('import');
  }, [view]);

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
    const canonical = canonicalizeSceneForAi(savedScene);
    const { provider, settings } = await providerRouter.getActiveProvider();
    setBibleUpdateProvider(provider);
    try {
      const existingRuns = await repository.listBibleUpdateRuns(workspace.project.id, savedScene.id);
      const requestedExtractor = settings.activeProvider === 'codex-cli' ? 'codex-cli' : 'local-prototype-extractor';
      const findReusableRun = (extractorId: string) => existingRuns.find((candidate) => candidate.extractorId === extractorId && candidate.contentHash === canonical.hash && ['completed', 'reviewed'].includes(candidate.status));
      let run = findReusableRun(requestedExtractor);
      let proposals: BibleProposal[] = [];
      if (run) {
        proposals = await repository.listBibleProposals(run.id);
        setProviderNotice('Angefordert: ' + (settings.activeProvider === 'codex-cli' ? 'Codex CLI' : 'Lokaler Prototyp') + ' · Verwendet: ' + (run.extractorId === 'codex-cli' ? 'Codex CLI' : 'Lokaler Prototyp') + '. Vorhandenes Ergebnis wiederverwendet.');
      } else {
        const previousRun = existingRuns.find((candidate) => candidate.extractorId === requestedExtractor && ['completed', 'reviewed'].includes(candidate.status));
        const input = { project: workspace.project, chapter: currentChapter, scene: canonical.scene, existingEntities: workspace.entities, relevantSources: await repository.listSourceReferences(workspace.project.id), previousAnalyzedContent: previousRun?.analyzedContent || undefined, changedRange: changedRange(previousRun?.analyzedContent || undefined, canonical.text) };
        let extraction: Awaited<ReturnType<StoryAiProvider['extractBiblePatch']>> | undefined;
        let actualExtractor = requestedExtractor;
        try {
          extraction = await provider.extractBiblePatch(input, settings.bibleUpdateTimeoutSeconds);
          setProviderNotice('Angefordert: ' + (settings.activeProvider === 'codex-cli' ? 'Codex CLI' : 'Lokaler Prototyp') + ' · Verwendet: ' + (requestedExtractor === 'codex-cli' ? 'Codex CLI' : 'Lokaler Prototyp') + '. Vorschläge bleiben bis zur Prüfung unbestätigt.');
        } catch (error) {
          if (settings.activeProvider !== 'codex-cli' || !settings.allowLocalFallback) throw error;
          actualExtractor = 'local-prototype-extractor';
          const localPreviousAnalyzedContent = existingRuns.find((candidate) => candidate.extractorId === actualExtractor && ['completed', 'reviewed'].includes(candidate.status))?.analyzedContent || undefined;
          const localInput = { ...input, previousAnalyzedContent: localPreviousAnalyzedContent, changedRange: changedRange(localPreviousAnalyzedContent, canonical.text) };
          const localRun = findReusableRun(actualExtractor);
          if (localRun) {
            run = localRun;
            proposals = await repository.listBibleProposals(localRun.id);
          } else {
            extraction = await providerRouter.getLocalProvider().then((local) => local.extractBiblePatch(localInput));
          }
          setProviderNotice('Angefordert: Codex CLI · Verwendet: Lokaler Prototyp · Grund: ' + (error instanceof Error ? error.message : 'unbekannter Codex-Fehler'));
        }
        if (!run) {
          run = await repository.createBibleUpdateRun({ projectId: workspace.project.id, sceneId: savedScene.id, sceneUpdatedAt: savedScene.updatedAt ?? '', contentHash: canonical.hash, extractorId: actualExtractor, analyzedContent: canonical.text });
          proposals = await repository.saveBibleProposals(run.id, extraction?.proposals ?? [], workspace.project.id, savedScene.id);
        }
      }
      setActiveReviewRun(run);
      setReviewProposals(proposals);
      await requestViewChange('bible');
    } finally {
      setBibleUpdateProvider(undefined);
    }
  }, [currentChapter, currentScene, requestViewChange, workspace]);
  const runCharacterMemoryUpdate = useCallback(async (sceneOverride?: Scene): Promise<void> => {
    const sourceWorkspace = loadState.status === 'ready' ? loadState.workspace : workspace;
    const scene = sceneOverride ?? currentScene;
    const chapter = sourceWorkspace?.chapters.find((item) => item.id === scene?.chapterId);
    if (!sourceWorkspace?.project || !scene || !chapter) throw new Error('Bitte zuerst eine Szene auswählen.');
    const controller = editorSaveController.current;
    if (controller) { await controller.flush(); if (controller.getStatus() === 'error') throw new Error('Die Szene konnte vor der Gedächtnisanalyse nicht gespeichert werden.'); }
    const canonical = canonicalizeSceneForAi(controller?.getDraft() ?? scene);
    const { provider, settings } = await providerRouter.getActiveProvider();
    const context = await contextBuilder.build({ projectId: sourceWorkspace.project.id, currentChapterId: chapter.id, currentSceneId: canonical.scene.id, userQuestion: canonical.text });
    const characters = sourceWorkspace.entities.filter((entity) => entity.type === 'character' && (canonical.text.toLocaleLowerCase().includes(entity.name.toLocaleLowerCase()) || canonical.scene.pov === entity.name || canonical.scene.pov === entity.id));
    if (!characters.length) { setProviderNotice('Keine bekannte Figur wurde in dieser Szene gefunden.'); return; }
    const extractorId = settings.activeProvider === 'codex-cli' ? 'codex-cli' : 'local-prototype-extractor';
    const existingRuns = await repository.listCharacterMemoryUpdateRuns(sourceWorkspace.project.id, canonical.scene.id);
    const reusable = existingRuns.find((item) => item.extractorId === extractorId && item.contentHash === canonical.hash && ['completed', 'reviewed'].includes(item.status));
    let run = reusable;
    let proposals: CharacterMemoryProposal[];
    if (run) proposals = await repository.listCharacterMemoryProposals(run.id);
    else {
      const input = { project: sourceWorkspace.project, chapter, scene: canonical.scene, characters, existingEntities: sourceWorkspace.entities, context, changedRange: changedRange(undefined, canonical.text) };
      let extraction;
      let actual = extractorId;
      try { extraction = await provider.extractCharacterMemoryPatch(input, settings.bibleUpdateTimeoutSeconds); }
      catch (error) {
        if (settings.activeProvider !== 'codex-cli' || !settings.allowLocalFallback) throw error;
        actual = 'local-prototype-extractor';
        const local = await providerRouter.getLocalProvider();
        const localPrevious = existingRuns.filter((item) => item.extractorId === actual && ['completed', 'reviewed'].includes(item.status)).sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];
        extraction = await local.extractCharacterMemoryPatch({ ...input, changedRange: changedRange(localPrevious?.analyzedContent, canonical.text) }, settings.bibleUpdateTimeoutSeconds);
        setProviderNotice(`Angefordert: Codex CLI · Verwendet: Lokaler Prototyp · Grund: ${error instanceof Error ? error.message : 'Codex-Fehler'}`);
      }
      run = await repository.createCharacterMemoryUpdateRun({ projectId: sourceWorkspace.project.id, sceneId: canonical.scene.id, contentHash: canonical.hash, analyzedContent: canonical.text, extractorId: actual });
      proposals = await repository.saveCharacterMemoryProposals(run.id, extraction.proposals);
      setProviderNotice(`Angefordert: ${settings.activeProvider === 'codex-cli' ? 'Codex CLI' : 'Lokaler Prototyp'} · Verwendet: ${actual === 'codex-cli' ? 'Codex CLI' : 'Lokaler Prototyp'}. Vorschläge bleiben unbestätigt.`);
    }
    if (!run) throw new Error('Character-Memory-Lauf konnte nicht erstellt werden.');
    setActiveMemoryRun(run); setMemoryProposals(proposals); await requestViewChange('bible');
  }, [contextBuilder, currentScene, loadState, requestViewChange, workspace]);
  const cancelBibleUpdate = useCallback(async () => { await bibleUpdateProvider?.cancelActive(); }, [bibleUpdateProvider]);
  const reviewProposal = useCallback(async (input: ReviewBibleProposalInput) => { const saved = await repository.reviewBibleProposal(input); setReviewProposals((current) => current.map((proposal) => proposal.id === saved.id ? saved : proposal)); const refreshed = await repository.loadWorkspace(); setLoadState({ status: 'ready', workspace: refreshed }); }, []);
  const completeBibleReview = useCallback(async () => { if (activeReviewRun) { await repository.completeBibleReview(activeReviewRun.id); setActiveReviewRun(undefined); setReviewProposals([]); const refreshed = await repository.loadWorkspace(); setLoadState({ status: 'ready', workspace: refreshed }); const refreshedScene = refreshed.chapters.flatMap((chapter) => chapter.scenes).find((scene) => scene.id === activeReviewRun.sceneId); if (refreshedScene) await runCharacterMemoryUpdate(refreshedScene); } }, [activeReviewRun, runCharacterMemoryUpdate]);
  const reviewCharacterMemory = useCallback(async (input: ReviewCharacterMemoryProposalInput) => { const saved = await repository.reviewCharacterMemoryProposal(input); setMemoryProposals((current) => current.map((proposal) => proposal.id === saved.id ? saved : proposal)); setLoadState({ status: 'ready', workspace: await repository.loadWorkspace() }); }, []);
  const completeCharacterMemoryReview = useCallback(async () => { if (!activeMemoryRun) return; await repository.completeCharacterMemoryReview(activeMemoryRun.id); setActiveMemoryRun(undefined); setMemoryProposals([]); setLoadState({ status: 'ready', workspace: await repository.loadWorkspace() }); }, [activeMemoryRun]);
  const openSourceReference = useCallback(async (reference: StorySourceReference) => {
    const changed = await requestSceneChange(reference.sceneId);
    if (!changed) return;
    setPendingSourceNavigation({ sceneId: reference.sceneId, chapterId: reference.chapterId, excerpt: reference.excerpt, startOffset: reference.startOffset, endOffset: reference.endOffset });
    await requestViewChange('editor');
  }, [requestSceneChange, requestViewChange]);
  const openStyleReference = useCallback(async (reference: StyleReference) => {
    const chapter = workspace?.chapters.find((item) => item.scenes.some((scene) => scene.id === reference.sceneId));
    if (!chapter) { setViewError('Die Szene der Stilreferenz wurde nicht gefunden.'); return; }
    await openSourceReference({ id: reference.id, projectId: reference.projectId, sceneId: reference.sceneId, chapterId: reference.chapterId ?? chapter.id, excerpt: reference.excerpt, startOffset: reference.startOffset, endOffset: reference.endOffset, createdAt: reference.createdAt });
  }, [openSourceReference, workspace]);
  const createStyleReference = useCallback((input: CreateStyleReferenceInput) => repository.createStyleReference(input), []);

  const renderView = () => {
    if (!workspace) return null;
    if (view === 'dashboard') return <Dashboard project={workspace.project} onOpen={() => void requestViewChange('editor')} onImport={() => void openImport()} />;
    if (view === 'editor') return <EditorView projectId={workspace.project.id} chapters={workspace.chapters} scene={currentScene} chapter={currentChapter} pendingSourceNavigation={pendingSourceNavigation} onSourceNavigationConsumed={() => setPendingSourceNavigation(undefined)} onBack={() => void requestViewChange('dashboard')} onSelectScene={(id) => void requestSceneChange(id)} onSave={saveScene} onCreateChapter={createChapter} onUpdateChapter={updateChapter} onCreateScene={createScene} onListVersions={listSceneVersions} onCreateVersion={createSceneVersion} onRestoreVersion={restoreSceneVersion} onGetEditorPreferences={getEditorPreferences} onSaveEditorPreferences={saveEditorPreferences} onBibleUpdate={runBibleUpdate} bibleUpdateBusy={Boolean(bibleUpdateProvider)} onCancelBibleUpdate={cancelBibleUpdate} onOpenAssistant={() => setAssistantOpen(true)} onSaveStateChange={setSaveStatus} onRegisterSaveController={registerSaveController} onCreateStyleReference={createStyleReference} />;
    if (view === 'bible' || view === 'characters' || view === 'threads') return <StoryBibleView entities={workspace.entities} projectId={workspace.project.id} chapters={workspace.chapters} repository={repository} activeRun={activeReviewRun} proposals={reviewProposals} activeMemoryRun={activeMemoryRun} memoryProposals={memoryProposals} onEntityChanged={replaceEntity} onOpenSourceReference={openSourceReference} onOpenStyleReference={openStyleReference} onReview={reviewProposal} onCompleteReview={completeBibleReview} onMemoryReview={reviewCharacterMemory} onCompleteMemoryReview={completeCharacterMemoryReview} onCloseReview={() => { setActiveReviewRun(undefined); setReviewProposals([]); setActiveMemoryRun(undefined); setMemoryProposals([]); }} initialFilter={view === 'characters' ? 'character' : view === 'threads' ? 'plot_thread' : undefined} />;
    if (view === 'timeline') return <TimelineView events={demoEvents} />;
    if (view === 'mindmap') return <MindmapView nodes={mindNodes} edges={mindEdges} />;
    if (view === 'settings') return <SettingsView mode={repository.mode} project={workspace.project} settings={providerSettings} onSettingsChange={setProviderSettings} onReload={loadWorkspace} />;
    return <Dashboard project={workspace.project} onOpen={() => void requestViewChange('editor')} onImport={() => void openImport()} />;
  };

  const project = workspace?.project;
  const saveLabel: Record<SaveStatus, string> = { saved: 'Gespeichert', dirty: 'Nicht gespeichert', saving: 'Speichert …', error: 'Speicherfehler' };
  const topLabel = view === 'dashboard' ? 'Übersicht' : view === 'settings' ? 'Einstellungen' : navItems.find((item) => item.view === view)?.label ?? 'Arbeitsbereich';

  return <div className={`app-shell simple-mode ${sidebarOpen ? 'sidebar-open' : 'sidebar-collapsed'} ${view === 'editor' ? 'writing-mode' : ''}`}>
    <aside className="simple-sidebar">
      <button className="simple-brand" onClick={() => void requestViewChange('dashboard')} aria-label="Zum Start"><span className="brand-mark">SM</span><span><strong>StoryMemory</strong><small>Dein Buch vergisst nichts.</small></span></button>
      <div className="simple-project"><span className="eyebrow">DEIN PROJEKT</span><strong>{project?.title ?? 'Workspace'}</strong><span>{project ? 'Band 1 · Entwurf' : 'Wird geladen …'}</span></div>
      <nav className="simple-nav" aria-label="Hauptnavigation">{navItems.map(({ view: target, label, description, icon: Icon }) => <button key={target} title={sidebarOpen ? undefined : label} className={`simple-nav-button ${view === target ? 'active' : ''}`} onClick={() => void requestViewChange(target)}><Icon size={21} /><span><strong>{label}</strong><small>{description}</small></span></button>)}</nav>
      <div className="simple-sidebar-bottom"><button className="simple-nav-button" title={sidebarOpen ? undefined : 'Importieren'} onClick={() => void openImport()}><Upload size={21} /><span><strong>Importieren</strong><small>TXT, Markdown oder DOCX</small></span></button><button className="simple-nav-button" title={sidebarOpen ? undefined : 'Einstellungen'} onClick={() => void requestViewChange('settings')}><Settings2 size={21} /><span><strong>Einstellungen</strong><small>App anpassen</small></span></button><div className="provider-status"><span className="status-dot green" /><span className="provider-label">{repository.mode === 'desktop' ? 'Lokaler Desktop-Modus' : 'Browser-Demo-Modus'}</span></div></div>
    </aside>
    <main className="main-area">
      <header className="topbar simple-topbar"><div className="topbar-title"><button className="sidebar-toggle" onClick={() => setSidebarOpen((open) => !open)} aria-label={sidebarOpen ? 'Sidebar einklappen' : 'Sidebar öffnen'} title={sidebarOpen ? 'Sidebar einklappen' : 'Sidebar öffnen'}>{sidebarOpen ? <PanelLeftClose size={18} /> : <PanelLeftOpen size={18} />}</button><div className="topbar-copy"><span className="eyebrow">{view === 'dashboard' ? 'START' : project?.title ?? 'STORYMEMORY'}</span><strong>{topLabel}</strong></div></div><div className="topbar-actions"><span className={`save-state save-state-${saveStatus}`}><span className={`status-dot status-dot-${saveStatus}`} /> {saveLabel[saveStatus]}</span><button className="assistant-button" onClick={() => setAssistantOpen(true)}><MessageCircle size={18} /> Assistent öffnen</button></div></header>
      <div className="content-scroll">{providerNotice && <div className="provider-notice" role="status"><span>{providerNotice}</span><button className="text-button" onClick={() => setProviderNotice('')}>Ausblenden</button></div>}{viewError && <div className="save-error workspace-save-error" role="alert"><strong>Speichern erforderlich</strong><span>{viewError}</span><button className="text-button" onClick={() => void retryEditorSave()}>Erneut versuchen</button></div>}{loadState.status === 'loading' && <LoadingView mode={repository.mode} />}{loadState.status === 'error' && <ErrorView message={loadState.message} detail={loadState.detail} onRetry={() => void loadWorkspace()} />}{loadState.status === 'ready' && renderView()}</div>
    </main>
    {assistantOpen && <div className="assistant-drawer"><button className="drawer-close" onClick={() => setAssistantOpen(false)} aria-label="Assistent schließen"><X size={20} /></button>{workspace && <ChatPanel messages={messages} onMessagesChange={setMessages} contextBuilder={contextBuilder} contextRequest={{ projectId: workspace.project.id, currentChapterId: currentChapter?.id, currentSceneId: currentScene?.id }} onOpenSourceReference={(reference) => void openSourceReference(reference)} providerRouter={providerRouter} onLongformRequest={(instruction) => { setLongformInstruction(instruction); setAssistantOpen(false); }} />}</div>}
    {resumeMemoryReview && !activeMemoryRun && <div className="provider-notice" role="status"><span>Ein offener Character-Memory-Review wartet auf deine Entscheidung.</span><button className="primary-button" onClick={() => { setActiveMemoryRun(resumeMemoryReview.run); setMemoryProposals(resumeMemoryReview.proposals); setResumeMemoryReview(undefined); void requestViewChange('bible'); }}>Review fortsetzen</button><button className="text-button" onClick={() => setResumeMemoryReview(undefined)}>Später</button></div>}
    {longformInstruction && workspace && <div className="longform-overlay"><LongformDraftView project={workspace.project} chapters={workspace.chapters} entities={workspace.entities} repository={longformRepository} instruction={longformInstruction} activeProvider={providerSettings.activeProvider} onClose={() => setLongformInstruction(undefined)} onAccepted={async () => { setLongformInstruction(undefined); await loadWorkspace(); await requestViewChange('editor'); }} /></div>}
    {activeModal && activeModal !== 'import' && <Modal type={activeModal} onClose={() => setActiveModal(null)} />}
    {activeModal === 'import' && workspace?.books[0] && <ManuscriptImportModal projectId={workspace.project.id} bookId={workspace.books[0].id} repository={repository} onClose={() => setActiveModal(null)} onImported={async (result) => { setSelectedSceneId(result.scenes[0]?.id ?? ''); setActiveModal(null); await loadWorkspace(); await requestViewChange('editor'); }} />}
    {closePrompt && <ClosePrompt message={closePrompt} onRetry={() => void finishClose(false)} onForceClose={() => void finishClose(true)} onCancel={() => setClosePrompt('')} />}
  </div>;
}

function LoadingView({ mode }: { mode: RuntimeMode }) { return <section className="state-view"><RefreshCw className="spin" size={26} /><span className="eyebrow">{mode === 'desktop' ? 'LOKALE DATENBANK' : 'BROWSER-DEMO'}</span><h1>Workspace wird geladen</h1><p>Deine Projekte, Kapitel und Szenen werden vorbereitet.</p></section>; }
function ErrorView({ message, detail, onRetry }: { message: string; detail?: string; onRetry: () => void }) { return <section className="state-view state-error"><span className="eyebrow">DATENBANKFEHLER</span><h1>Workspace konnte nicht geladen werden</h1><p>{message}</p><button className="primary-button large" onClick={onRetry}><RefreshCw size={17} /> Erneut laden</button>{detail && <details><summary>Technische Details</summary><pre>{detail}</pre></details>}</section>; }

function ClosePrompt({ message, onRetry, onForceClose, onCancel }: { message: string; onRetry: () => void; onForceClose: () => void; onCancel: () => void }) {
  return <div className="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="close-save-title"><div className="modal simple-modal close-save-modal"><div className="modal-head"><div><span className="eyebrow">LETZTE ÄNDERUNG</span><h2 id="close-save-title">Noch nicht gespeichert</h2></div></div><p className="modal-intro">{message} Was möchtest du tun?</p><div className="close-save-actions"><button className="primary-button large" onClick={onRetry}>Erneut versuchen</button><button className="ghost-button large" onClick={onForceClose}>Trotzdem schließen</button><button className="text-button" onClick={onCancel}>Abbrechen</button></div></div></div>;
}

function Modal({ type, onClose }: { type: 'bible' | 'research'; onClose: () => void }) { const title = type === 'bible' ? 'Story Bible aktualisieren' : 'Projekt prüfen'; return <div className="modal-backdrop" role="dialog" aria-modal="true"><div className="modal simple-modal"><div className="modal-head"><div><span className="eyebrow">{type === 'bible' ? 'VORSCHLÄGE PRÜFEN' : 'EINFACHER WORKFLOW'}</span><h2>{title}</h2></div><button className="icon-button" onClick={onClose} aria-label="Dialog schließen"><X size={20} /></button></div>{type === 'bible' && <><p className="modal-intro">Ich habe neue mögliche Fakten gefunden. Du entscheidest, was in deine Story Bible kommt.</p><div className="proposal-summary simple-summary"><div><strong>7</strong><span>neue Fakten</span></div><div><strong>2</strong><span>Figurenänderungen</span></div><div><strong>1</strong><span>möglicher Widerspruch</span></div></div><button className="primary-button large full" onClick={onClose}>Vorschläge ansehen</button></>}{type === 'research' && <><p className="modal-intro">Prüfe deine aktuelle Szene gegen die bisherige Geschichte.</p><div className="simple-choice"><strong>Was soll geprüft werden?</strong><button className="choice-button active">Aktuelle Szene</button><button className="choice-button">Aktuelles Kapitel</button><button className="choice-button">Gesamtes Buch</button></div><button className="primary-button large full" onClick={onClose}>Prüfung starten</button></>}</div></div>; }
