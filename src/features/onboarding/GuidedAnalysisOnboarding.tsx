import type { ReactNode } from 'react';
import { BookOpen, Check, FileText, SkipForward } from 'lucide-react';
import type { Project, ProjectOnboardingState } from '../../types/domain';

interface Props {
  project: Project;
  state: ProjectOnboardingState;
  loreNotes?: string;
  manuscriptText?: string;
  loreView: ReactNode;
  manuscriptProgress?: ReactNode;
  hasManuscriptAnalysis: boolean;
  onOpenImport: () => void;
  onSkipLore: () => void;
  onSkipManuscript: () => void;
  onAbort: () => void;
}

const steps = [
  { id: 'project', label: 'Grunddaten' },
  { id: 'lore', label: 'Lore prüfen' },
  { id: 'manuscript', label: 'Manuskript prüfen' },
  { id: 'completed', label: 'Fertig' },
];

export function GuidedAnalysisOnboarding({ project, state, loreNotes, manuscriptText, loreView, manuscriptProgress, hasManuscriptAnalysis, onOpenImport, onSkipLore, onSkipManuscript, onAbort }: Props) {
  const currentStep = state.currentStep === 'lore' ? 'lore' : state.currentStep === 'manuscript' ? 'manuscript' : 'completed';
  const currentIndex = steps.findIndex((step) => step.id === currentStep);
  const completed = new Set(state.completedSteps);
  return <main className="guided-onboarding-screen" aria-label="Geführtes Projekt-Onboarding">
    <section className="guided-onboarding-shell">
      <div className="guided-onboarding-top"><div><span className="eyebrow">PROJEKT EINRICHTEN</span><strong>{project.title}</strong></div><button type="button" className="text-button" onClick={onAbort}>Onboarding abbrechen</button></div>
      <header className="guided-onboarding-heading"><span className="eyebrow">ECHTER ANALYSEFORTSCHRITT</span><h1>Dein Buch wird gemeinsam vorbereitet</h1><p>Jede Phase wird erst nach erfolgreicher Speicherung als abgeschlossen angezeigt. Vorschläge bleiben bis zu deiner Entscheidung unverbindlich.</p></header>
      <ol className="guided-onboarding-stepper" aria-label="Analysefortschritt">{steps.map((step, index) => <li key={step.id} className={`${index === currentIndex ? 'active' : ''} ${completed.has(step.id) || index < currentIndex ? 'completed' : ''}`}><span>{completed.has(step.id) || index < currentIndex ? <Check size={14} /> : index + 1}</span><strong>{step.label}</strong></li>)}</ol>
      {currentStep === 'lore' && <section className="guided-onboarding-stage"><div className="guided-stage-intro"><div className="onboarding-choice-icon"><BookOpen size={24} /></div><div><h2>Lore einfügen und direkt analysieren</h2><p>{loreNotes?.trim() ? 'Deine vorbereiteten Notizen sind übernommen. Starte jetzt die verständliche Lore-Analyse.' : 'Füge Weltregeln, Grenzen, Kosten, Ausnahmen, Orte oder Weltgeschichte ein. Einzelne Szenen und Figurenhandlungen kannst du später gezielt weiterleiten.'}</p></div></div>{loreView}<div className="guided-stage-footer"><button className="text-button" onClick={onSkipLore}><SkipForward size={15} /> Lore überspringen</button></div></section>}
      {currentStep === 'manuscript' && <section className="guided-onboarding-stage"><div className="guided-stage-intro"><div className="onboarding-choice-icon"><FileText size={24} /></div><div><h2>Manuskript einfügen und Schritt für Schritt prüfen</h2><p>{hasManuscriptAnalysis ? 'Der Import ist gespeichert. Die Anzeige unten zeigt den echten Stand der aktuellen Analysephase.' : manuscriptText?.trim() ? 'Dein Manuskript steht bereit und wird nach der Importvorschau automatisch analysiert.' : 'Füge Text ein oder importiere eine Datei. Die Analyse startet danach sequenziell und bleibt nach einem Neustart fortsetzbar.'}</p></div></div>{hasManuscriptAnalysis ? manuscriptProgress : <div className="guided-empty-analysis"><strong>Noch kein Manuskript importiert</strong><span>Der Import öffnet eine Vorschau, bevor Kapitel und Prüfeinheiten dauerhaft gespeichert werden.</span><div className="guided-stage-actions"><button className="primary-button large" onClick={onOpenImport}><FileText size={16} /> Manuskript einfügen</button><button className="ghost-button large" onClick={onOpenImport}>Datei importieren</button></div></div>}<div className="guided-stage-footer"><button className="text-button" onClick={onSkipManuscript}><SkipForward size={15} /> Ohne Manuskript abschließen</button></div></section>}
      {currentStep === 'completed' && <section className="guided-onboarding-complete"><div className="onboarding-choice-icon onboarding-choice-icon-success"><Check size={26} /></div><h2>Onboarding abgeschlossen</h2><p>Grunddaten, Lore und Manuskriptprüfung sind gespeichert. Du kannst jetzt in dein Projekt wechseln.</p></section>}
    </section>
  </main>;
}
