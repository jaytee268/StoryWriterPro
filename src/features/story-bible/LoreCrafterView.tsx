import { useCallback, useEffect, useRef, useState } from 'react';
import { Check, CheckCircle2, FileText, HelpCircle, RotateCcw, X } from 'lucide-react';
import type { LoreCrafterClarification, LoreCrafterRun, LoreCrafterSourceReference, LoreSheetDraft, LoreSheetItem } from '../../types/domain';
import { providerRouter } from '../../services/aiProviderService';
import { analyzeLoreDraft, buildLoreSheet, confirmLoreCrafterRule, finishLoreCrafterReview, reviewLoreSheetItem, routeExcludedContent } from '../../services/loreCrafter';
import type { StoryRepository } from '../../services/storyRepository';

interface Props { projectId: string; repository: StoryRepository; onRunCreated?: (run: LoreCrafterRun) => void; }

export function LoreCrafterView({ projectId, repository, onRunCreated }: Props) {
  const [notes, setNotes] = useState('');
  const [runs, setRuns] = useState<LoreCrafterRun[]>([]);
  const [run, setRun] = useState<LoreCrafterRun>();
  const [clarifications, setClarifications] = useState<LoreCrafterClarification[]>([]);
  const [draft, setDraft] = useState<LoreSheetDraft>();
  const [items, setItems] = useState<LoreSheetItem[]>([]);
  const [sources, setSources] = useState<LoreCrafterSourceReference[]>([]);
  const [understandingConfirmed, setUnderstandingConfirmed] = useState(false);
  const [busy, setBusy] = useState('');
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const notesRef = useRef<HTMLTextAreaElement>(null);

  const loadRun = useCallback(async (next: LoreCrafterRun | undefined) => {
    setRun(next); setClarifications([]); setDraft(undefined); setItems([]); setSources([]); setUnderstandingConfirmed(false);
    if (!next) return;
    const nextClarifications = await repository.listLoreCrafterClarifications(next.id);
    const nextDraft = await repository.getLoreSheetDraft(next.id);
    setClarifications(nextClarifications); setDraft(nextDraft); setSources(await repository.listLoreCrafterSources(next.id)); if (nextDraft) setItems(await repository.listLoreSheetItems(nextDraft.id));
  }, [repository]);

  const refresh = useCallback(async () => { const nextRuns = await repository.listLoreCrafterRuns(projectId); setRuns(nextRuns); if (!run) await loadRun(nextRuns[0]); }, [loadRun, projectId, repository, run]);
  useEffect(() => { void refresh().catch((cause) => setError(cause instanceof Error ? cause.message : 'Lore-Crafter-Läufe konnten nicht geladen werden.')); }, [refresh]);

  const analyse = async () => {
    if (!notes.trim()) { setError('Gib zuerst freie Lore-Notizen ein.'); return; }
    setBusy('analyse'); setError(''); setMessage('Analysiert …');
    try { const { provider } = await providerRouter.getActiveProvider(); const next = await analyzeLoreDraft(repository, provider, { projectId, originalText: notes }); onRunCreated?.(next); const onboarding = await repository.getProjectOnboardingState(projectId); await repository.saveProjectOnboardingState({ ...onboarding, loreCrafterRunId: next.id, currentStep: onboarding.currentStep === 'lore' ? 'manuscript' : onboarding.currentStep, completedSteps: Array.from(new Set([...onboarding.completedSteps, 'lore'])) }); await loadRun(next); setRuns(await repository.listLoreCrafterRuns(projectId)); setNotes(next.originalText); setMessage('Analyse abgeschlossen. Prüfe jetzt das Verständnis.'); } catch (cause) { setError(cause instanceof Error ? cause.message : 'Die Lore konnte nicht analysiert werden.'); setMessage(''); } finally { setBusy(''); }
  };

  const updateClarification = async (clarification: LoreCrafterClarification, answer: string, status: LoreCrafterClarification['status']) => {
    try { const saved = await repository.saveLoreCrafterClarifications(clarification.runId, [{ ...clarification, answer, status }]); setClarifications((current) => current.map((item) => item.id === clarification.id ? saved[0] : item)); } catch (cause) { setError(cause instanceof Error ? cause.message : 'Rückfrage konnte nicht gespeichert werden.'); }
  };

  const createSheet = async () => {
    if (!run) return;
    setBusy('sheet'); setError('');
    try { const { provider } = await providerRouter.getActiveProvider(); const result = await buildLoreSheet(repository, provider, run.id, understandingConfirmed); setDraft(result.draft); setItems(result.items); setMessage('Lore Sheet ist als Vorschlag bereit.'); } catch (cause) { setError(cause instanceof Error ? cause.message : 'Lore Sheet konnte nicht erstellt werden.'); } finally { setBusy(''); }
  };

  const decide = async (item: LoreSheetItem, status: 'accepted' | 'rejected' | 'uncertain' | 'merged') => {
    setBusy(item.id); setError('');
    try { const edited = status === 'accepted' || status === 'merged' ? window.prompt('Eintrag bearbeiten (Abbrechen = unverändert)', item.content) : undefined; if (edited === null) return; const mergeTargetId = status === 'merged' ? window.prompt('ID des bestehenden Lore- oder Regel-Eintrags für das echte Zusammenführen', '') || undefined : undefined; const saved = await reviewLoreSheetItem(repository, item, status, edited, mergeTargetId); setItems((current) => current.map((entry) => entry.id === saved.id ? saved : entry)); setMessage(status === 'accepted' || status === 'merged' ? 'Vorschlag als projektgebundener Entwurf gespeichert.' : 'Entscheidung gespeichert.'); } catch (cause) { setError(cause instanceof Error ? cause.message : 'Die Entscheidung konnte nicht gespeichert werden.'); } finally { setBusy(''); }
  };
  const confirmRule = async (item: LoreSheetItem) => { setBusy(item.id); try { await confirmLoreCrafterRule(repository, item); setMessage('Projektregel ausdrücklich als aktiv bestätigt.'); } catch (cause) { setError(cause instanceof Error ? cause.message : 'Regel konnte nicht aktiviert werden.'); } finally { setBusy(''); } };
  const openSource = (source: LoreCrafterSourceReference) => { const start = Array.from(notes).slice(0, source.startOffset).join('').length; const end = Array.from(notes).slice(0, source.endOffset).join('').length; notesRef.current?.focus(); notesRef.current?.setSelectionRange(start, end); setMessage(`Quelle geöffnet: Unicode-Position ${source.startOffset}–${source.endOffset}.`); };

  const routeExcluded = async (content: string, reason: string, target: 'character_memory' | 'plot_thread' | 'continuity_state' | 'manuscript' | 'style') => { if (!run) return; setBusy('route'); try { const routed = await routeExcludedContent(repository, run, content, reason, target); setItems((current) => [routed, ...current]); setMessage(`Als ${target} weitergeleitet und als Vorschlag gespeichert.`); } catch (cause) { setError(cause instanceof Error ? cause.message : 'Ausgeschlossener Inhalt konnte nicht weitergeleitet werden.'); } finally { setBusy(''); } };
  const finish = async () => { if (!run) return; setBusy('finish'); try { setRun(await finishLoreCrafterReview(repository, run, items)); setRuns(await repository.listLoreCrafterRuns(projectId)); setMessage('Review abgeschlossen. Übernommene Regeln bleiben bis zur Autorbestätigung inaktiv.'); } catch (cause) { setError(cause instanceof Error ? cause.message : 'Review kann noch nicht abgeschlossen werden.'); } finally { setBusy(''); } };

  return <section className="foundation-layout lore-crafter-layout">
    <aside className="foundation-list lore-crafter-list"><div className="foundation-list-heading"><strong>Gespeicherte Entwürfe</strong><button className="icon-button" title="Neuer Entwurf" onClick={() => { setRun(undefined); setDraft(undefined); setItems([]); setNotes(''); setMessage('Neuer Lore-Crafter-Entwurf.'); }}><FileText size={16} /></button></div>{runs.map((item) => <button className={`foundation-list-item ${item.id === run?.id ? 'active' : ''}`} key={item.id} onClick={() => void loadRun(item).then(() => setNotes(item.originalText))}><strong>{item.originalText.slice(0, 46) || 'Leerer Entwurf'}</strong><span>{item.status.replaceAll('_', ' ')} · {item.providerId}</span></button>)}{!runs.length && <p className="empty-state">Noch keine Entwürfe gespeichert.</p>}</aside>
    <div className="foundation-form lore-crafter-form">
      <div className="foundation-form-heading"><div><span className="eyebrow">STORY BIBLE · LORE CRAFTER</span><h2>Geführte Lore-Analyse</h2><p>Freie Notizen werden erst erklärt, dann als Vorschlag strukturiert. Nichts wird ohne deine Entscheidung kanonisch.</p></div><span className="save-hint">{busy ? `${busy} …` : message || 'Bereit'}</span></div>
      {error && <div className="save-error" role="alert">{error}</div>}
      <label className="field-label full-field">Freie Lore-Notizen<textarea ref={notesRef} rows={12} value={notes} onChange={(event) => setNotes(event.target.value)} placeholder="Beschreibe Weltmechanismen, Regeln, Grenzen, Kosten, Ausnahmen, Orte, Organisationen oder Weltgeschichte …" /></label>
      <div className="lore-crafter-help"><div><h3><HelpCircle size={16} /> Was gehört in die Lore?</h3><p>Weltmechanismen, Regeln, Voraussetzungen, Grenzen, Kosten, Ausnahmen, historische Hintergründe sowie weltbedeutende Systeme, Orte und Organisationen.</p><strong>Geeignet</strong><p>„Eine Regel gilt nur unter einer bestimmten Voraussetzung und hat eine erkennbare Grenze.“</p></div><div><h3>Was gehört eher woanders hin?</h3><p>Einzelne Szenen, Dialoge, Schreibstil, kurzfristige Gefühle, normale Figurenhandlungen, Kapitelplanung und einmalige Gegenstandszustände.</p><strong>Beispiele</strong><p>Solche Inhalte werden nicht gelöscht, sondern als Charakter Memory, Plot Thread, Continuity State, Manuskript oder Stil vorgeschlagen.</p></div></div>
      <div className="form-actions"><button className="primary-button" disabled={busy !== '' || !notes.trim()} onClick={() => void analyse()}><RotateCcw size={15} /> Lore analysieren</button>{run?.status === 'awaiting_review' && <button className="ghost-button" disabled={busy !== '' || !understandingConfirmed} onClick={() => void createSheet()}><CheckCircle2 size={15} /> Verständnis bestätigen &amp; Lore Sheet erstellen</button>}</div>
      {run?.confirmationText && <section className="lore-crafter-understanding"><span className="eyebrow">VERSTÄNDNISPRÜFUNG</span><h3>So habe ich deine Lore verstanden</h3><div className="understanding-text">{run.confirmationText.split('\n').map((line, index) => <p key={`${line}-${index}`}>{line || ' '}</p>)}</div><label className="checkline"><input type="checkbox" checked={understandingConfirmed} onChange={(event) => setUnderstandingConfirmed(event.target.checked)} /> Das Verständnis ist korrekt. Meine Ergänzungen sind oben eingearbeitet.</label><button className="text-button" onClick={() => { setMessage('Bearbeite die Notizen und führe die Analyse erneut aus.'); document.querySelector<HTMLTextAreaElement>('.lore-crafter-form textarea')?.focus(); }}>Verständnis korrigieren</button></section>}
      {clarifications.length > 0 && <section className="lore-crafter-questions"><h3>Rückfragen</h3>{clarifications.map((item) => <div className="lore-question" key={item.id}><strong>{item.question}</strong><small>Status: {item.status === 'answered' ? 'beantwortet' : item.status === 'skipped' ? 'übersprungen' : 'offen'}</small><textarea rows={2} value={item.answer ?? ''} disabled={item.status === 'skipped'} onChange={(event) => setClarifications((current) => current.map((entry) => entry.id === item.id ? { ...entry, answer: event.target.value } : entry))} /><div className="review-actions"><button className="ghost-button" onClick={() => void updateClarification(item, item.answer ?? '', 'answered')}>Antwort speichern</button><button className="text-button" onClick={() => void updateClarification(item, '', 'skipped')}><X size={14} /> Frage überspringen</button></div></div>)}</section>}
      {sources.length > 0 && <section className="lore-crafter-questions"><h3>Quellen im Lore-Originaltext</h3>{sources.map((source) => <article className="review-card" key={source.id}><p>{source.excerpt}</p><small>Unicode-Position {source.startOffset}–{source.endOffset}</small><button className="ghost-button" onClick={() => openSource(source)}>Quelle öffnen</button></article>)}</section>}
      {run?.analysis?.excludedContent.length ? <section className="lore-crafter-questions"><h3>Ausgeschlossene Inhalte routen</h3>{run.analysis.excludedContent.map((item, index) => <article className="review-card" key={`${item.content}-${index}`}><strong>{item.content}</strong><p>{item.reason}</p><small>Vorschlag: {item.suggestedTarget}</small><div className="review-actions"><button className="ghost-button" onClick={() => void routeExcluded(item.content, item.reason, item.suggestedTarget)}>Als proposed weiterleiten</button><button className="text-button" onClick={() => setMessage('Ausgeschlossener Inhalt bleibt erhalten und wird nicht gelöscht.')}>Ignorieren</button></div></article>)}</section> : null}
      {draft && <section className="lore-sheet-review"><div className="foundation-form-heading"><div><span className="eyebrow">VORGESCHLAGENES LORE SHEET</span><h3>{draft.title}</h3><p>{draft.premise}</p></div><span className="status-pill purple">{draft.status}</span></div>{items.map((item) => <article className={`review-card ${item.status}`} key={item.id}><div className="review-card-main"><span className="status-pill purple">{item.title}</span><p>{item.content}</p>{item.structured && <details><summary>Strukturierte Regel anzeigen</summary><pre>{JSON.stringify(item.structured, null, 2)}</pre></details>}<small>Confidence {Math.round(item.confidence * 100)}% · {item.sourceReferenceId ? 'Quelle im Originaltext verknüpft' : 'Quelle fehlt'}</small></div>{item.status === 'proposed' && <div className="review-actions"><button className="primary-button" disabled={busy !== ''} onClick={() => void decide(item, 'accepted')}><Check size={14} /> Übernehmen</button><button className="ghost-button" disabled={busy !== ''} onClick={() => void decide(item, 'merged')}>Mit bestehendem Eintrag zusammenführen</button><button className="ghost-button" disabled={busy !== ''} onClick={() => void decide(item, 'uncertain')}>Unsicher speichern</button><button className="text-button" disabled={busy !== ''} onClick={() => void decide(item, 'rejected')}>Ablehnen</button></div>}{item.itemType === 'world_rule' && item.targetRuleId && item.status === 'accepted' && <button className="ghost-button" disabled={busy !== ''} onClick={() => void confirmRule(item)}>Als aktive Projektregel bestätigen</button>}</article>)}<div className="modal-actions"><button className="primary-button" disabled={busy !== '' || !items.length || items.some((item) => item.status === 'proposed')} onClick={() => void finish()}>Review abschließen</button></div></section>}
    </div>
  </section>;
}
