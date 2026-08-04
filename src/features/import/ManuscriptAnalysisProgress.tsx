import { Pause, Play, RotateCcw, Square } from 'lucide-react';
import type { ManuscriptAnalysisJob, ManuscriptAnalysisPhase, ManuscriptAnalysisUnit } from '../../types/domain';

interface Props { job: ManuscriptAnalysisJob; units: ManuscriptAnalysisUnit[]; error?: string; onResume: () => void; onRetry: () => void; onPause: () => void; onCancel: () => void; onCompleteReview: () => void; }

export function ManuscriptAnalysisProgress({ job, units, error, onResume, onRetry, onPause, onCancel, onCompleteReview }: Props) {
  const current = job.currentUnitId ? units.find((unit) => unit.id === job.currentUnitId) : undefined;
  const currentChapter = current ? units.filter((unit) => unit.chapterId === current.chapterId).findIndex((unit) => unit.id === current.id) + 1 : 0;
  const chapters = new Set(units.map((unit) => unit.chapterId)).size;
  const currentChapterNumber = current ? [...new Set(units.map((unit) => unit.chapterId))].indexOf(current.chapterId) + 1 : 0;
  const failed = units.find((unit) => unit.status === 'failed');
  const active = job.status === 'running';
  const phaseLabels: Record<ManuscriptAnalysisPhase, string> = { structure: 'Manuskript eingelesen', passage_continuity: 'Prüfeinheiten analysiert', bible_extraction: 'Bible-Vorschläge', character_memory: 'Character Memories', scene_or_chapter_synthesis: 'Kapitel-Synthesen', narrative_summaries: 'Zusammenfassungen', plot_thread_synthesis: 'Handlungsstränge', book_end_state: 'Buch-Endzustand', global_countercheck: 'Globale Gegenprüfung', user_review: 'Review', completed: 'Abgeschlossen' };
  const phaseEntries = Object.entries(job.phaseProgress) as Array<[ManuscriptAnalysisPhase, NonNullable<ManuscriptAnalysisJob['phaseProgress'][ManuscriptAnalysisPhase]>]>;
  return <section className="provider-notice manuscript-analysis-progress" role="status">
    <div><strong>Manuskript-Analyse</strong><span>Phase: {phaseLabels[job.currentPhase]} · Kapitel {currentChapterNumber || '—'} von {chapters} · Prüfeinheit {currentChapter || job.completedUnits} von {job.totalUnits}</span><span>{job.completedUnits} abgeschlossen · {job.failedUnits} fehlgeschlagen · {units.filter((unit) => unit.status === 'pending').length} offen · Provider {job.providerId}</span>{phaseEntries.length > 0 && <div className="manuscript-analysis-phases">{phaseEntries.map(([phase, progress]) => <span key={phase}>{phaseLabels[phase]}: {progress.completedUnits}/{progress.totalUnits} · {progress.status}</span>)}</div>}{failed && <span className="save-error">Fehler in Kapitel {failed.chapterId}{failed.pageNumber === undefined ? '' : ` · Seite ${failed.pageNumber}`} · {failed.errorCode ?? 'ANALYSIS_FAILED'} · Provider {failed.actualProvider ?? failed.requestedProvider ?? job.providerId} · {new Date(failed.updatedAt).toLocaleString('de-DE')}: {failed.errorMessage ?? 'Unbekannter Fehler'}</span>}{error && <span className="save-error">{error}</span>}</div>
    <div className="longform-actions"><button className="ghost-button" disabled={active || job.status === 'awaiting_user_review'} onClick={onResume}><Play size={14} /> Analyse fortsetzen</button><button className="ghost-button" disabled={job.failedUnits === 0 || active} onClick={onRetry}><RotateCcw size={14} /> Fehlgeschlagene erneut prüfen</button><button className="ghost-button" disabled={!active} onClick={onPause}><Pause size={14} /> Analyse pausieren</button><button className="text-button" disabled={job.status === 'cancelled' || job.status === 'completed'} onClick={onCancel}><Square size={14} /> Analyse abbrechen</button>{job.status === 'awaiting_user_review' && <button className="ghost-button" onClick={onCompleteReview}>Review abschließen · offene Vorschläge bewusst überspringen</button>}</div>
  </section>;
}
