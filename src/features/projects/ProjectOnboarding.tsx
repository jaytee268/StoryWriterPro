import { useState } from 'react';
import { ArrowRight, BookOpen, FileText, FolderPlus, SkipForward } from 'lucide-react';
import type { Project, ProjectOnboardingState, ProjectOnboardingStep } from '../../types/domain';
import type { StoryRepository } from '../../services/storyRepository';

interface Props {
  repository: StoryRepository;
  project?: Project;
  state?: ProjectOnboardingState;
  onCreated: (project: Project, state: ProjectOnboardingState) => void;
  onContinue: (state: ProjectOnboardingState) => void;
  onOpenLore: () => void;
  onOpenImport: () => void;
}

const steps: Array<{ id: ProjectOnboardingStep; title: string; description: string }> = [
  { id: 'project', title: 'Projekt', description: 'Titel, Autor und Grunddaten' },
  { id: 'lore', title: 'Lore', description: 'Weltwissen optional vorbereiten' },
  { id: 'manuscript', title: 'Manuskript', description: 'Text einfügen oder Datei laden' },
  { id: 'summary', title: 'Zusammenfassung', description: 'Den Einstieg abschließen' },
];

export function ProjectOnboarding({ repository, project, state, onCreated, onContinue, onOpenLore, onOpenImport }: Props) {
  const [title, setTitle] = useState('');
  const [author, setAuthor] = useState('');
  const [volumeTitle, setVolumeTitle] = useState('');
  const [language, setLanguage] = useState(state?.language ?? 'de');
  const [genre, setGenre] = useState(state?.genre ?? '');
  const [step, setStep] = useState<ProjectOnboardingStep>(state?.currentStep ?? 'project');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  const persist = async (nextStep: ProjectOnboardingStep, completed: string[], skipped = state?.skippedSteps ?? []) => {
    if (!project) return;
    const saved = await repository.saveProjectOnboardingState({ projectId: project.id, currentStep: nextStep, completedSteps: completed, skippedSteps: skipped, language, genre, loreCrafterRunId: state?.loreCrafterRunId, importId: state?.importId });
    setStep(nextStep); onContinue(saved);
  };

  const create = async () => {
    if (!title.trim()) { setError('Ein Projekttitel wird benötigt.'); return; }
    setBusy(true); setError('');
    try {
      const created = await repository.createProject({ title: title.trim(), author: author.trim(), volumeTitle: volumeTitle.trim() || title.trim(), description: '' });
      const saved = await repository.saveProjectOnboardingState({ projectId: created.id, currentStep: 'lore', completedSteps: ['project'], skippedSteps: [], language, genre, loreCrafterRunId: undefined, importId: undefined });
      onCreated(created, saved);
    } catch (reason) { setError(reason instanceof Error ? reason.message : 'Das Projekt konnte nicht angelegt werden.'); }
    finally { setBusy(false); }
  };

  const advance = async (target: ProjectOnboardingStep, skip = false) => {
    if (!project || !state) return;
    setBusy(true); setError('');
    try { await persist(target, Array.from(new Set([...state.completedSteps, step])), skip ? Array.from(new Set([...state.skippedSteps, step])) : state.skippedSteps); }
    catch (reason) { setError(reason instanceof Error ? reason.message : 'Der Onboardingstatus konnte nicht gespeichert werden.'); }
    finally { setBusy(false); }
  };

  if (!project) return <section className="state-view onboarding-view"><span className="eyebrow">NEUES PROJEKT</span><h1>Dein Buch beginnt hier</h1><p>Lege ein leeres Projekt an. Du entscheidest anschließend selbst, ob du Lore oder Manuskript zuerst vorbereitest.</p><div className="onboarding-form"><label className="field-label">Projekttitel<input autoFocus value={title} onChange={(event) => setTitle(event.target.value)} placeholder="Arbeitstitel" /></label><label className="field-label">Autor<input value={author} onChange={(event) => setAuthor(event.target.value)} placeholder="Dein Name" /></label><label className="field-label">Bandtitel <span>(optional)</span><input value={volumeTitle} onChange={(event) => setVolumeTitle(event.target.value)} placeholder="Wie der Projekttitel" /></label><div className="onboarding-two-columns"><label className="field-label">Sprache<select value={language} onChange={(event) => setLanguage(event.target.value)}><option value="de">Deutsch</option><option value="en">Englisch</option><option value="other">Andere</option></select></label><label className="field-label">Genre <span>(optional)</span><input value={genre} onChange={(event) => setGenre(event.target.value)} placeholder="z. B. Roman" /></label></div>{error && <div className="save-error">{error}</div>}<button className="primary-button large" disabled={busy} onClick={() => void create()}><FolderPlus size={17} /> Projekt anlegen <ArrowRight size={17} /></button></div></section>;

  const current = steps.find((item) => item.id === step) ?? steps[0];
  return <section className="state-view onboarding-view"><span className="eyebrow">PROJEKT EINRICHTEN</span><h1>{current.title}</h1><p>{current.description}</p><div className="onboarding-stepper">{steps.map((item, index) => <span key={item.id} className={item.id === step ? 'active' : state?.completedSteps.includes(item.id) ? 'completed' : ''}>{index + 1}. {item.title}</span>)}</div>{step === 'lore' && <div className="onboarding-choice"><BookOpen size={25} /><h2>Möchtest du deine Welt vorbereiten?</h2><p>Du kannst freie Notizen im Lore Crafter strukturieren lassen. Es wird noch nichts automatisch in deine Story Bible übernommen.</p><div className="onboarding-two-columns"><button className="primary-button large" onClick={onOpenLore}><BookOpen size={17} /> Lore einfügen</button><button className="ghost-button large" onClick={onOpenLore}><BookOpen size={17} /> Lore gemeinsam aufbauen</button></div><div className="onboarding-two-columns"><button className="ghost-button large" disabled={busy} onClick={() => void advance('manuscript')}><ArrowRight size={16} /> Keine Lore vorhanden</button><button className="ghost-button large" disabled={busy} onClick={() => void advance('manuscript', true)}><SkipForward size={16} /> Später</button></div></div>}{step === 'manuscript' && <div className="onboarding-choice"><FileText size={25} /><h2>Gibt es schon Manuskripttext?</h2><p>Du kannst Text einfügen oder TXT, Markdown und DOCX als Vorschau prüfen. Analyse startet erst nach deiner Bestätigung.</p><div className="onboarding-two-columns"><button className="primary-button large" onClick={onOpenImport}><FileText size={17} /> Manuskript einfügen</button><button className="ghost-button large" onClick={onOpenImport}><FileText size={17} /> Datei importieren</button></div><button className="ghost-button large" disabled={busy} onClick={() => void advance('summary', true)}><SkipForward size={16} /> Von null beginnen</button></div>}{step === 'summary' && <div className="onboarding-choice"><h2>Projekt bereit</h2><p>Dein Projekt ist angelegt. Du kannst jederzeit Lore ergänzen oder mit dem Schreiben beginnen.</p><button className="primary-button large" disabled={busy} onClick={() => void advance('completed')}><ArrowRight size={17} /> Zum Projekt</button></div>}{error && <div className="save-error">{error}</div>}</section>;
}
