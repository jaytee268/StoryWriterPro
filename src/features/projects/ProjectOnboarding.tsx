import { useEffect, useState } from 'react';
import { ArrowRight, BookOpen, Check, FileText, FolderPlus, SkipForward } from 'lucide-react';
import type { Project, ProjectOnboardingState, ProjectOnboardingStep } from '../../types/domain';
import type { StoryRepository } from '../../services/storyRepository';
import { genreById } from '../../data/genreCatalog';
import { GenreCombobox } from './GenreCombobox';
import { GenreMultiSelect } from './GenreMultiSelect';

interface Props {
  repository: StoryRepository;
  project?: Project;
  state?: ProjectOnboardingState;
  onCreated: (project: Project, state: ProjectOnboardingState, loreNotes?: string) => void;
  onContinue: (state: ProjectOnboardingState) => void;
  onOpenLore: () => void;
  onOpenImport: () => void;
  onAbort?: () => void;
}

const onboardingDraftStorageKey = 'storymemory.new-project-onboarding.v1';
interface NewProjectOnboardingDraft { title: string; author: string; volumeTitle: string; language: string; genre: string; primaryGenreId: string; secondaryGenreIds: string[]; customGenre: string; customGenreOpen: boolean; loreNotes: string; step: ProjectOnboardingStep; completedSteps: string[]; skippedSteps: string[]; }
const emptyNewProjectDraft: NewProjectOnboardingDraft = { title: '', author: '', volumeTitle: '', language: 'de', genre: '', primaryGenreId: '', secondaryGenreIds: [], customGenre: '', customGenreOpen: false, loreNotes: '', step: 'project', completedSteps: [], skippedSteps: [] };
function readNewProjectDraft(): NewProjectOnboardingDraft {
  try {
    const stored = window.localStorage.getItem(onboardingDraftStorageKey);
    if (!stored) return emptyNewProjectDraft;
    const parsed = JSON.parse(stored) as Partial<NewProjectOnboardingDraft>;
    return { ...emptyNewProjectDraft, ...parsed, secondaryGenreIds: parsed.secondaryGenreIds ?? [], completedSteps: parsed.completedSteps ?? [], skippedSteps: parsed.skippedSteps ?? [] };
  } catch { return emptyNewProjectDraft; }
}

const steps: Array<{ id: ProjectOnboardingStep; title: string; description: string }> = [
  { id: 'project', title: 'Projekt', description: 'Titel, Autor und Grunddaten' },
  { id: 'lore', title: 'Lore', description: 'Weltwissen optional vorbereiten' },
  { id: 'manuscript', title: 'Manuskript', description: 'Text einfügen oder Datei laden' },
  { id: 'summary', title: 'Zusammenfassung', description: 'Den Einstieg abschließen' },
];

export function ProjectOnboarding({ repository, project, state, onCreated, onContinue, onOpenLore, onOpenImport, onAbort }: Props) {
  const [initialDraft] = useState(readNewProjectDraft);
  const [title, setTitle] = useState(initialDraft.title);
  const [author, setAuthor] = useState(initialDraft.author);
  const [volumeTitle, setVolumeTitle] = useState(initialDraft.volumeTitle);
  const [language, setLanguage] = useState(state?.language ?? initialDraft.language);
  const [genre, setGenre] = useState(state?.genre ?? initialDraft.genre);
  const [primaryGenreId, setPrimaryGenreId] = useState(initialDraft.primaryGenreId);
  const [secondaryGenreIds, setSecondaryGenreIds] = useState<string[]>(initialDraft.secondaryGenreIds);
  const [customGenre, setCustomGenre] = useState(initialDraft.customGenre);
  const [customGenreOpen, setCustomGenreOpen] = useState(initialDraft.customGenreOpen);
  const [loreNotes, setLoreNotes] = useState(initialDraft.loreNotes);
  const [volumeTitleTouched, setVolumeTitleTouched] = useState(Boolean(initialDraft.volumeTitle && initialDraft.volumeTitle !== initialDraft.title));
  const [draftCompletedSteps, setDraftCompletedSteps] = useState<string[]>(initialDraft.completedSteps);
  const [draftSkippedSteps, setDraftSkippedSteps] = useState<string[]>(initialDraft.skippedSteps);
  const [step, setStep] = useState<ProjectOnboardingStep>(state?.currentStep ?? initialDraft.step);
  const [draftAborted, setDraftAborted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    if (project || draftAborted) return;
    window.localStorage.setItem(onboardingDraftStorageKey, JSON.stringify({ title, author, volumeTitle, language, genre, primaryGenreId, secondaryGenreIds, customGenre, customGenreOpen, loreNotes, step, completedSteps: draftCompletedSteps, skippedSteps: draftSkippedSteps } satisfies NewProjectOnboardingDraft));
  }, [author, customGenre, customGenreOpen, draftAborted, draftCompletedSteps, draftSkippedSteps, genre, language, loreNotes, primaryGenreId, project, secondaryGenreIds, step, title, volumeTitle]);

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
      const createdWorkspace = await repository.loadWorkspace(created.id);
      const createdBook = createdWorkspace.books[0];
      if (createdBook && (primaryGenreId || secondaryGenreIds.length > 0 || customGenre.trim())) {
        await repository.saveBookGenres({ bookId: createdBook.id, projectId: created.id, primaryGenreId: primaryGenreId || undefined, secondaryGenreIds, customGenreNames: customGenre.trim() ? [customGenre.trim()] : [], genreSource: 'manual', genreAuthorConfirmed: true });
      }
      const saved = await repository.saveProjectOnboardingState({ projectId: created.id, currentStep: 'completed', completedSteps: ['project', 'lore', 'manuscript', 'summary'], skippedSteps: draftSkippedSteps, language, genre, loreCrafterRunId: undefined, importId: undefined });
      window.localStorage.removeItem(onboardingDraftStorageKey);
      onCreated(created, saved, loreNotes.trim() || undefined);
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

  const advanceDraft = (target: ProjectOnboardingStep, skip = false) => {
    setDraftCompletedSteps((current) => Array.from(new Set([...current, step])));
    if (skip) setDraftSkippedSteps((current) => Array.from(new Set([...current, step])));
    setStep(target);
  };

  const abort = () => {
    window.localStorage.removeItem(onboardingDraftStorageKey);
    setDraftAborted(true);
    setTitle(''); setAuthor(''); setVolumeTitle(''); setLanguage('de'); setGenre(''); setPrimaryGenreId(''); setSecondaryGenreIds([]); setCustomGenre(''); setCustomGenreOpen(false); setLoreNotes(''); setDraftCompletedSteps([]); setDraftSkippedSteps([]); setStep('project'); setError('');
    onAbort?.();
  };

  if (!project) {
    const selectPrimaryGenre = (nextId: string) => {
      setPrimaryGenreId(nextId);
      setSecondaryGenreIds((current) => current.filter((id) => id !== nextId));
      setGenre(genreById(nextId)?.name ?? '');
    };
    if (step !== 'project') {
      const currentDraftStep = steps.find((item) => item.id === step) ?? steps[0];
      return <section className="state-view onboarding-view"><div className="onboarding-card onboarding-flow-card"><div className="onboarding-flow-top"><button type="button" className="text-button" onClick={abort}>Onboarding abbrechen</button></div><div className="onboarding-flow-heading"><span className="eyebrow">PROJEKT EINRICHTEN</span><h1>{currentDraftStep.title}</h1><p>{currentDraftStep.description}</p></div><ol className="onboarding-stepper" aria-label="Projekt einrichten">{steps.map((item, index) => <li key={item.id} className={item.id === step ? 'active' : draftCompletedSteps.includes(item.id) ? 'completed' : ''}><span>{index + 1}</span><strong>{item.title}</strong></li>)}</ol>{step === 'lore' && <div className="onboarding-choice"><div className="onboarding-choice-icon"><BookOpen size={25} /></div><h2>Möchtest du deine Welt vorbereiten?</h2><p>Füge hier freie Notizen ein. Sie werden nach dem Anlegen direkt im Lore Crafter geöffnet; vorher entsteht kein Projekt und nichts wird in die Datenbank geschrieben.</p><label className="field-label onboarding-lore-field" htmlFor="onboarding-lore-notes">Lore-Notizen <span>(optional)</span><textarea id="onboarding-lore-notes" rows={9} value={loreNotes} onChange={(event) => setLoreNotes(event.target.value)} placeholder="Weltregeln, Grenzen, Kosten, Ausnahmen, Orte oder Weltgeschichte …" /></label><p className="onboarding-inline-note">Auch wenn du den Schritt überspringst, bleiben eingegebene Notizen im Entwurf erhalten.</p><div className="onboarding-choice-actions"><button className="primary-button large" onClick={() => advanceDraft('manuscript')}><BookOpen size={17} /> Mit Lore-Notizen fortfahren</button><button className="ghost-button large" onClick={() => advanceDraft('manuscript', true)}><SkipForward size={17} /> Lore überspringen</button></div><div className="onboarding-choice-secondary"><button className="text-button" onClick={() => setStep('project')}>← Zurück</button></div></div>}{step === 'manuscript' && <div className="onboarding-choice"><div className="onboarding-choice-icon"><FileText size={25} /></div><h2>Gibt es schon Manuskripttext?</h2><p>Manuskript und Import kannst du nach dem Anlegen sicher im Projekt öffnen. Bis dahin wird nichts in die Datenbank geschrieben.</p><div className="onboarding-choice-actions"><button className="primary-button large" onClick={() => advanceDraft('summary')}><FileText size={17} /> Manuskript später hinzufügen</button><button className="ghost-button large" onClick={() => advanceDraft('summary', true)}><SkipForward size={17} /> Ohne Manuskript fortfahren</button></div><div className="onboarding-choice-secondary"><button className="text-button" onClick={() => setStep('lore')}>← Zurück</button></div></div>}{step === 'summary' && <div className="onboarding-choice"><div className="onboarding-choice-icon onboarding-choice-icon-success"><Check size={25} /></div><h2>Alles bereit?</h2><p>Prüfe deine Grunddaten. Erst mit „Projekt anlegen“ werden Projekt und Band gespeichert.</p><div className="onboarding-review-summary"><div><span>Projekttitel</span><strong>{title || '—'}</strong></div><div><span>Autor</span><strong>{author || '—'}</strong></div><div><span>Bandtitel</span><strong>{volumeTitle || title || '—'}</strong></div><div><span>Genre</span><strong>{genre || customGenre || 'Kein Genre'}</strong></div><div><span>Lore-Notizen</span><strong>{loreNotes.trim() ? 'Wird im Lore Crafter geöffnet' : 'Keine Lore-Notizen'}</strong></div></div><div className="onboarding-choice-secondary"><button className="text-button" onClick={() => setStep('manuscript')}>← Zurück</button><button className="primary-button large" disabled={busy} onClick={() => void create()}>{busy ? 'Projekt wird angelegt …' : <><FolderPlus size={17} /> Projekt anlegen</>}</button></div></div>}{error && <div className="save-error" role="alert">{error}</div>}</div></section>;
    }
    return <section className="state-view onboarding-view">
      <div className="onboarding-card"><div className="onboarding-flow-top"><button type="button" className="text-button" onClick={abort}>Onboarding abbrechen</button></div>
        <div className="onboarding-intro"><span className="eyebrow">NEUES PROJEKT</span><h1>Dein Buch beginnt hier</h1><p>Lege dein Projekt an. Lore und Manuskript kannst du anschließend hinzufügen oder zunächst überspringen.</p></div>
        <form className="onboarding-form" onSubmit={(event) => { event.preventDefault(); if (!title.trim()) { setError('Ein Projekttitel wird benötigt.'); return; } advanceDraft('lore'); }}>
          <div className="onboarding-section-heading"><span className="eyebrow">GRUNDDATEN</span><h2>Erzähl mir kurz, woran du arbeitest</h2></div>
          <div className="onboarding-fields">
            <label className="field-label onboarding-field-full" htmlFor="project-title">Projekttitel<input id="project-title" autoFocus value={title} onChange={(event) => { setTitle(event.target.value); if (!volumeTitleTouched) setVolumeTitle(event.target.value); }} placeholder="Arbeitstitel" aria-invalid={Boolean(error && !title.trim())} aria-describedby={error && !title.trim() ? 'project-title-error' : undefined} />{error && !title.trim() && <span className="field-error" id="project-title-error" role="alert">{error}</span>}</label>
            <label className="field-label" htmlFor="project-author">Autor <span>(optional)</span><input id="project-author" value={author} onChange={(event) => setAuthor(event.target.value)} placeholder="Dein Name" /></label>
            <label className="field-label" htmlFor="project-language">Sprache<select id="project-language" value={language} onChange={(event) => setLanguage(event.target.value)}><option value="de">Deutsch</option><option value="en">Englisch</option><option value="other">Andere</option></select></label>
            <label className="field-label onboarding-field-full" htmlFor="project-volume-title">Bandtitel <span>(optional)</span><input id="project-volume-title" value={volumeTitle} onChange={(event) => { setVolumeTitle(event.target.value); setVolumeTitleTouched(true); }} placeholder="Wie der Projekttitel" /></label>
          </div>
          <div className="onboarding-genre-section"><div className="onboarding-section-heading"><span className="eyebrow">GENRE</span><h2>Einordnen, wenn du möchtest</h2></div><div className="onboarding-genre-fields"><GenreCombobox value={primaryGenreId} onChange={selectPrimaryGenre} /><GenreMultiSelect value={secondaryGenreIds} onChange={setSecondaryGenreIds} excludeId={primaryGenreId} /></div><p className="onboarding-optional-note">Genre kann später automatisch erkannt oder manuell ergänzt werden.</p><button type="button" className="text-button onboarding-custom-toggle" onClick={() => setCustomGenreOpen((current) => !current)}>{customGenreOpen ? 'Eigenes Genre schließen' : 'Eigenes Genre hinzufügen'}</button>{customGenreOpen && <label className="field-label onboarding-custom-field" htmlFor="project-custom-genre">Eigenes Genre <span>(optional)</span><input id="project-custom-genre" autoFocus value={customGenre} onChange={(event) => setCustomGenre(event.target.value)} placeholder="Eigene Bezeichnung" /></label>}</div>
          {error && title.trim() && <div className="save-error" role="alert">{error}</div>}
          <div className="onboarding-actions"><button className="primary-button large" type="submit" disabled={busy}>{busy ? <><span className="button-spinner" aria-hidden="true" /> Weiter …</> : <>Weiter <ArrowRight size={17} /></>}</button></div>
        </form>
      </div>
    </section>;
  }

  const current = steps.find((item) => item.id === step) ?? steps[0];
  return <section className="state-view onboarding-view">
    <div className="onboarding-card onboarding-flow-card">
      <div className="onboarding-flow-heading"><span className="eyebrow">PROJEKT EINRICHTEN</span><h1>{current.title}</h1><p>{current.description}</p></div>
      <ol className="onboarding-stepper" aria-label="Projekt einrichten">
        {steps.map((item, index) => <li key={item.id} className={item.id === step ? 'active' : state?.completedSteps.includes(item.id) ? 'completed' : ''}><span>{index + 1}</span><strong>{item.title}</strong></li>)}
      </ol>
      {step === 'lore' && <div className="onboarding-choice"><div className="onboarding-choice-icon"><BookOpen size={25} /></div><h2>Möchtest du deine Welt vorbereiten?</h2><p>Du kannst freie Notizen im Lore Crafter strukturieren lassen. Es wird noch nichts automatisch in deine Story Bible übernommen.</p><div className="onboarding-choice-actions"><button className="primary-button large" onClick={onOpenLore}><BookOpen size={17} /> Lore einfügen</button><button className="ghost-button large" onClick={onOpenLore}><BookOpen size={17} /> Lore gemeinsam aufbauen</button></div><div className="onboarding-choice-secondary"><button className="ghost-button large" disabled={busy} onClick={() => void advance('manuscript')}><ArrowRight size={16} /> Keine Lore vorhanden</button><button className="text-button" disabled={busy} onClick={() => void advance('manuscript', true)}><SkipForward size={16} /> Später</button></div></div>}
      {step === 'manuscript' && <div className="onboarding-choice"><div className="onboarding-choice-icon"><FileText size={25} /></div><h2>Gibt es schon Manuskripttext?</h2><p>Du kannst Text einfügen oder TXT, Markdown und DOCX als Vorschau prüfen. Analyse startet erst nach deiner Bestätigung.</p><div className="onboarding-choice-actions"><button className="primary-button large" onClick={onOpenImport}><FileText size={17} /> Manuskript einfügen</button><button className="ghost-button large" onClick={onOpenImport}><FileText size={17} /> Datei importieren</button></div><div className="onboarding-choice-secondary"><button className="text-button" disabled={busy} onClick={() => void advance('summary', true)}><SkipForward size={16} /> Von null beginnen</button></div></div>}
      {step === 'summary' && <div className="onboarding-choice"><div className="onboarding-choice-icon onboarding-choice-icon-success"><ArrowRight size={25} /></div><h2>Projekt bereit</h2><p>Dein Projekt ist angelegt. Du kannst jederzeit Lore ergänzen oder mit dem Schreiben beginnen.</p><button className="primary-button large" disabled={busy} onClick={() => void advance('completed')}><ArrowRight size={17} /> Zum Projekt</button></div>}
      {error && <div className="save-error" role="alert">{error}</div>}
    </div>
  </section>;
}
