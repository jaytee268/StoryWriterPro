import { useEffect, useMemo, useState } from 'react';
import { Check, ChevronLeft, FileText, LockKeyhole, Plus, Save, X } from 'lucide-react';
import type { Chapter, ChapterGenerationJob, ChapterGenerationPlan, ChapterGenerationReview, ChapterGenerationSection, ContinuityStateLedgerEntry, NarrativeSummary, Project, ProjectContext, StoryDirection, StoryEntity, WritingPreferences } from '../../types/domain';
import type { LongformRepository } from '../../services/longformRepository';
import { buildPreflight, contextHashForLongform, parseLongformIntent, targetWords } from '../../services/longformWorkflow';
import { createLongformAiProvider } from '../../services/longformAiService';
import { providerRouter } from '../../services/aiProviderService';
import { createStoryRepository } from '../../services/storyRepository';
import { LongformContextBundleBuilder } from '../../services/longformContext';
import { editorContentToPlainText } from '../../utils/editorContent';
import { findingsToLongformReviews, runContinuityReview } from '../../services/continuityReview';

interface Props { project: Project; chapters: Chapter[]; entities: StoryEntity[]; repository: LongformRepository; instruction: string; activeProvider: string; onClose: () => void; onAccepted: (plan: ChapterGenerationPlan, sections: ChapterGenerationSection[]) => Promise<void>; }

const emptyDirection = (projectId: string): StoryDirection => ({ projectId, premise: '', currentStoryPhase: '', bookGoal: '', plannedEnding: '', endingStatus: 'open', centralTwist: '', thematicGoal: '', mustHappen: [], mustNotHappen: [], nextTurningPoint: '', revealConstraints: [], authorNotes: '', createdAt: '', updatedAt: '' });

export function LongformDraftView({ project, chapters, entities, repository, instruction, activeProvider, onClose, onAccepted }: Props) {
  const intent = useMemo(() => parseLongformIntent(instruction), [instruction]);
  const sourceRepository = useMemo(() => createStoryRepository(), []);
  const contextBuilder = useMemo(() => new LongformContextBundleBuilder(sourceRepository), [sourceRepository]);
  const [direction, setDirection] = useState<StoryDirection>();
  const [directionDraft, setDirectionDraft] = useState<StoryDirection>(emptyDirection(project.id));
  const [preferences, setPreferences] = useState<WritingPreferences>();
  const [context, setContext] = useState<ProjectContext>();
  const [job, setJob] = useState<ChapterGenerationJob>();
  const [plan, setPlan] = useState<ChapterGenerationPlan>();
  const [sections, setSections] = useState<ChapterGenerationSection[]>([]);
  const [reviews, setReviews] = useState<ChapterGenerationReview[]>([]);
  const [step, setStep] = useState<'preflight' | 'plan' | 'draft'>('preflight');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [contextOverrideRequired, setContextOverrideRequired] = useState(false);
  const [summaryBusy, setSummaryBusy] = useState(false);
  const [summaryStatus, setSummaryStatus] = useState<NarrativeSummary[]>([]);
  const ai = useMemo(() => preferences ? createLongformAiProvider(activeProvider, preferences) : undefined, [activeProvider, preferences]);

  useEffect(() => {
    void Promise.all([repository.getStoryDirection(project.id), repository.getWritingPreferences(project.id)])
      .then(([loadedDirection, loadedPreferences]) => { setDirection(loadedDirection); setDirectionDraft(loadedDirection ?? emptyDirection(project.id)); setPreferences(loadedPreferences); })
      .catch((cause) => setError(cause instanceof Error ? cause.message : 'Langformdaten konnten nicht geladen werden.'));
  }, [project.id, repository]);


  useEffect(() => {
    void repository.listJobs(project.id).then(async (jobs) => {
      const resumable = jobs.find((item) => !['accepted', 'cancelled', 'failed'].includes(item.status));
      if (!resumable) return;
      const [loadedPlan, loadedSections, loadedReviews] = await Promise.all([repository.getPlan(resumable.id), repository.listSections(resumable.id), repository.listReviews(resumable.id)]);
      setJob(resumable); setPlan(loadedPlan); setSections(loadedSections); setReviews(loadedReviews);
      if (loadedPlan) setStep(loadedSections.some((section) => section.content.trim()) ? 'draft' : 'plan');
    }).catch((cause) => setError(cause instanceof Error ? cause.message : 'Offene Schreibaufträge konnten nicht geladen werden.'));
  }, [project.id, repository]);

  useEffect(() => {
    void sourceRepository.listNarrativeSummaries(project.id).then(setSummaryStatus).catch(() => setSummaryStatus([]));
  }, [project.id, sourceRepository]);

  const totalWords = preferences ? targetWords(intent, preferences) : 0;
  useEffect(() => {
    if (!preferences) return;
    void contextBuilder.build({ project, chapters, entities, direction, preferences, userQuestion: instruction, currentSceneId: chapters.at(-1)?.scenes.at(-1)?.id, targetWords: totalWords, remainingWords: totalWords })
      .then(setContext).catch(() => setContext(undefined));
  }, [chapters, contextBuilder, direction, entities, instruction, preferences, project, totalWords]);

  const preflight = preferences ? buildPreflight(project, chapters, entities, direction, preferences, intent) : undefined;

  const saveDirection = async () => {
    setBusy(true); setError('');
    try { const saved = await repository.saveStoryDirection(directionDraft); setDirection(saved); setDirectionDraft(saved); }
    catch (cause) { setError(cause instanceof Error ? cause.message : 'Story-Richtung konnte nicht gespeichert werden.'); }
    finally { setBusy(false); }
  };

  const createProjectSummary = async () => {
    if (!preferences) return;
    setSummaryBusy(true); setError('');
    try {
      const active = await providerRouter.getActiveProvider();
      const content = chapters.flatMap((chapter) => chapter.scenes.map((scene) => editorContentToPlainText(scene.content))).join('\n\n');
      const result = await active.provider.summarize('project', project.id, Array.from(content).slice(0, 40000).join(''), active.settings.bibleUpdateTimeoutSeconds);
      if (!result.summary.trim()) throw new Error(result.warnings[0] ?? 'Keine Zusammenfassung erhalten.');
      await sourceRepository.saveNarrativeSummary({ projectId: project.id, scopeType: 'project', scopeId: project.id, contentHash: contextHashForLongform(project, chapters, direction, context), summary: result.summary, importantEvents: result.importantEvents, openThreads: result.openThreads, characterChanges: result.characterChanges, status: 'proposed', authorConfirmed: false });
      setSummaryStatus(await sourceRepository.listNarrativeSummaries(project.id));
      setError('Zusammenfassung wurde als Vorschlag gespeichert. Bitte vor der Planung bestätigen.');
    } catch (cause) { setError(cause instanceof Error ? cause.message : 'Zusammenfassung konnte nicht erzeugt werden.'); }
    finally { setSummaryBusy(false); }
  };


  const start = async () => {
    if (!preferences || !preflight?.canPlan || !preflight.targetBookId || !ai) return;
    setBusy(true); setError('');
    try {
      const created = await repository.createJob({ projectId: project.id, targetBookId: preflight.targetBookId, targetAfterChapterId: preflight.afterChapterId, requestedPages: intent.pages, targetWords: totalWords, requestedSceneCount: intent.sceneCount ?? preferences.defaultSceneCount, userInstruction: instruction, activeProvider, contentContextHash: contextHashForLongform(project, chapters, direction, context) });
      setJob(created);
      const frame = await ai.createPlan({ project, chapters, entities, direction, preferences, job: created, context });
      const savedPlan = await repository.savePlan({ ...frame, jobId: created.id, reviewStatus: 'pending' });
      setPlan(savedPlan); setStep('plan');
    } catch (cause) { setError(cause instanceof Error ? cause.message : 'Schreibauftrag konnte nicht gestartet werden.'); }
    finally { setBusy(false); }
  };

  const reviewSection = async (ai: ReturnType<typeof createLongformAiProvider>, currentJob: ChapterGenerationJob, currentPlan: ChapterGenerationPlan, section: ChapterGenerationSection, previousSections: ChapterGenerationSection[]) => {
    if (activeProvider !== 'codex-cli') return [];
    return ai.reviewSection({ project, chapters, entities, direction, preferences: preferences!, job: currentJob, plan: currentPlan, section, previousSections, context });
  };

  const generateSection = async (ai: ReturnType<typeof createLongformAiProvider>, currentJob: ChapterGenerationJob, currentPlan: ChapterGenerationPlan, beat: ChapterGenerationPlan['beats'][number], previousSections: ChapterGenerationSection[], draftLedger: ContinuityStateLedgerEntry[]) => {
    const emptySection: ChapterGenerationSection = await repository.saveSection({ jobId: currentJob.id, planBeatId: beat.id, orderIndex: beat.orderIndex, targetWords: beat.targetWords, content: '', continuationSummary: '', continuityState: { currentLocation: '', currentStoryTime: '', presentCharacterIds: beat.participatingCharacterIds, characterStates: [], establishedFacts: [], knowledgeChanges: beat.knowledgeChanges, relationshipChanges: beat.relationshipChanges, movedObjects: [], injuries: [], cluesIntroduced: beat.cluesUsed, promisesCreated: [], unresolvedActions: [], lastParagraphSummary: '' }, status: 'pending', providerId: activeProvider });
    if (activeProvider !== 'codex-cli') return { section: emptySection, findings: [] as Awaited<ReturnType<typeof runContinuityReview>>['findings'], draftLedger };
    const generated = await ai.draftSection({ project, chapters, entities, direction, preferences: preferences!, job: currentJob, plan: currentPlan, section: emptySection, previousSections, draftLedger, context });
    const saved = await repository.saveSection({ ...emptySection, content: generated.content, continuationSummary: generated.continuationSummary, continuityState: generated.continuityState, status: 'generated', providerId: 'codex-cli' });
    const continuity = await runContinuityReview(sourceRepository, { project, chapter: chapters.at(-1), scene: chapters.at(-1)?.scenes.at(-1), currentText: generated.content, previousText: previousSections.at(-1)?.content, sourceKind: 'longform_section', draftLedger });
    if (continuity.findings.length) await repository.saveReviews(currentJob.id, findingsToLongformReviews(continuity.findings, currentJob.id, saved.id));
    return { section: saved, findings: continuity.findings, draftLedger: [...draftLedger, ...continuity.stateProposals] };
  };

  const confirmPlan = async () => {
    if (!job || !plan || !preferences) return;
    setBusy(true); setError('');
    try {
      const savedPlan = await repository.savePlan({ ...plan, reviewStatus: 'accepted' });
      setPlan(savedPlan);
      if (!ai) throw new Error('Der Longform-Anbieter ist noch nicht bereit.');
      const nextSections: ChapterGenerationSection[] = [];
      let draftLedger: ContinuityStateLedgerEntry[] = [];
      for (const beat of savedPlan.beats) {
        const existing = sections.find((section) => section.orderIndex === beat.orderIndex && section.content.trim());
        const generatedResult = existing ? { section: existing, findings: [] as Awaited<ReturnType<typeof runContinuityReview>>['findings'], draftLedger } : await generateSection(ai, job, savedPlan, beat, nextSections, draftLedger);
        const section = generatedResult.section;
        draftLedger = generatedResult.draftLedger;
        nextSections.push(section);
        if (generatedResult.findings.some((finding) => finding.severity === 'critical')) {
          await repository.updateJobStatus(job.id, 'reviewing'); setJob((current) => current ? { ...current, status: 'reviewing' } : current); setSections(nextSections); setReviews(await repository.listReviews(job.id)); setStep('draft'); return;
        }
        const issues = await reviewSection(ai, job, savedPlan, section, nextSections.slice(0, -1));
        if (issues.length) {
          const persisted = await repository.saveReviews(job.id, issues);
          setReviews((current) => [...current, ...persisted]);
          if (issues.some((issue) => issue.severity === 'blocking')) {
            await repository.updateJobStatus(job.id, 'reviewing'); setJob((current) => current ? { ...current, status: 'reviewing' } : current); setSections(nextSections); setStep('draft'); return;
          }
        }
      }
      setSections(nextSections);
      setReviews(await repository.listReviews(job.id));
      const completeIssues = activeProvider === 'codex-cli' ? await ai.reviewComplete({ project, chapters, entities, direction, preferences, job, plan: savedPlan, previousSections: nextSections, context }) : [];
      if (completeIssues.length) { const persisted = await repository.saveReviews(job.id, completeIssues); setReviews((current) => [...current, ...persisted]); }
      const finalBlocked = completeIssues.some((issue) => issue.severity === 'blocking');
      await repository.updateJobStatus(job.id, finalBlocked ? 'reviewing' : 'draft_ready'); setJob((current) => current ? { ...current, status: finalBlocked ? 'reviewing' : 'draft_ready' } : current); setStep('draft');
    } catch (cause) { setError(cause instanceof Error ? cause.message : 'Plan oder Abschnitt konnte nicht verarbeitet werden.'); }
    finally { setBusy(false); }
  };

  const regenerateSection = async (section: ChapterGenerationSection) => {
    if (!job || !plan || !preferences) return;
    setBusy(true); setError('');
    try {
      await repository.deleteReviewsForSection(job.id, section.id);
      const previous = sections.filter((item) => item.orderIndex < section.orderIndex).sort((a, b) => a.orderIndex - b.orderIndex);
      if (!ai) throw new Error('Der Longform-Anbieter ist noch nicht bereit.');
      const beat = plan.beats.find((item) => item.orderIndex === section.orderIndex);
      if (!beat) throw new Error('Der zugehörige Plan-Beat wurde nicht gefunden.');
      const generated = await ai.draftSection({ project, chapters, entities, direction, preferences, job, plan, section, previousSections: previous, draftLedger: [], context });
      const saved = await repository.saveSection({ ...section, content: generated.content, continuationSummary: generated.continuationSummary, continuityState: generated.continuityState, status: 'generated', providerId: 'codex-cli' });
      const continuity = await runContinuityReview(sourceRepository, { project, chapter: chapters.at(-1), scene: chapters.at(-1)?.scenes.at(-1), currentText: generated.content, previousText: previous.at(-1)?.content, sourceKind: 'longform_section', draftLedger: [] });
      const issues = await reviewSection(ai, job, plan, saved, previous);
      const continuityIssues = continuity.findings.length ? findingsToLongformReviews(continuity.findings, job.id, saved.id) : [];
      const persisted = issues.length || continuityIssues.length ? await repository.saveReviews(job.id, [...issues, ...continuityIssues]) : [];
      setSections((current) => current.map((item) => item.id === saved.id ? saved : item));
      setReviews((current) => [...current.filter((item) => item.sectionId !== section.id), ...persisted]);
      if (!issues.some((issue) => issue.severity === 'blocking')) { await repository.updateJobStatus(job.id, 'draft_ready'); setJob((current) => current ? { ...current, status: 'draft_ready' } : current); }
    } catch (cause) { setError(cause instanceof Error ? cause.message : 'Abschnitt konnte nicht neu erzeugt werden.'); }
    finally { setBusy(false); }
  };

  const updateSection = async (section: ChapterGenerationSection, content: string) => {
    const saved = await repository.saveSection({ ...section, content, status: 'generated' });
    await repository.deleteReviewsForSection(section.jobId, section.id);
    const laterSections = sections.filter((item) => item.orderIndex > section.orderIndex);
    await Promise.all(laterSections.map((item) => repository.saveSection({ ...item, status: 'regenerate_requested' })));
    const continuity = activeProvider === 'codex-cli' ? await runContinuityReview(sourceRepository, { project, chapter: chapters.at(-1), scene: chapters.at(-1)?.scenes.at(-1), currentText: content, previousText: sections.filter((item) => item.orderIndex < section.orderIndex).at(-1)?.content, sourceKind: 'longform_section', draftLedger: [] }) : undefined;
    const continuityReviews = continuity?.findings.length ? findingsToLongformReviews(continuity.findings, section.jobId, section.id) : [];
    const persisted = continuityReviews.length ? await repository.saveReviews(section.jobId, continuityReviews) : [];
    setReviews((current) => [...current.filter((review) => review.sectionId !== section.id), ...persisted]);
    setSections((current) => current.map((item) => item.id === saved.id ? saved : item).map((item) => laterSections.some((later) => later.id === item.id) ? { ...item, status: 'regenerate_requested' } : item));
  };

  const accept = async () => {
    if (!job || !plan || sections.some((section) => !section.content.trim())) { setError('Bitte fülle jeden Abschnitt oder verwirf den Entwurf bewusst.'); return; }
    if (job.status !== 'draft_ready' || reviews.some((review) => review.severity === 'blocking' && review.status === 'open')) { setError('Der Entwurf muss zuerst ohne offene Blockierungen geprüft werden.'); return; }
    const currentHash = contextHashForLongform(project, chapters, direction, context);
    if (currentHash !== job.contentContextHash && !job.contextOverrideAccepted) { setContextOverrideRequired(true); setError('Der Projektkontext hat sich seit Beginn dieses Entwurfs verändert. Prüfe den Kontext oder bestätige ausdrücklich die Fortsetzung mit dem alten Kontext.'); return; }
    setBusy(true); setError('');
    try { await repository.acceptJob(job.id); await onAccepted(plan, sections); }
    catch (cause) { setError(cause instanceof Error ? cause.message : 'Der Entwurf konnte nicht übernommen werden.'); }
    finally { setBusy(false); }
  };

  const continueWithOldContext = async () => {
    if (!job) return;
    setBusy(true); setError('');
    try { const saved = await repository.acceptContextOverride(job.id); setJob(saved); setContextOverrideRequired(false); }
    catch (cause) { setError(cause instanceof Error ? cause.message : 'Die Kontextentscheidung konnte nicht gespeichert werden.'); }
    finally { setBusy(false); }
  };

  const cancelGeneration = async () => {
    if (!ai || !job) return;
    try {
      await ai.cancelActive();
      const saved = await repository.updateJobStatus(job.id, 'reviewing');
      setJob(saved);
      setError('Der Codex-Aufruf wurde abgebrochen. Der Schreibauftrag kann fortgesetzt werden.');
    } catch (cause) { setError(cause instanceof Error ? cause.message : 'Der Codex-Aufruf konnte nicht abgebrochen werden.'); }
  };

  if (!preferences) return <section className="longform-view"><p>Vorbereitung wird geladen …</p></section>;
  const wordCount = sections.reduce((sum, section) => sum + section.actualWords, 0);
  return <section className="longform-view">
    <header className="longform-head"><button className="ghost-button" onClick={busy ? () => void cancelGeneration() : onClose}><ChevronLeft size={16} /> {busy ? 'Generierung abbrechen' : 'Zurück zum Assistenten'}</button><div><span className="eyebrow">GEFÜHRTER KAPITELENTWURF</span><h1>{step === 'preflight' ? 'Vorbereitung' : plan?.chapterTitle ?? 'Kapitelentwurf'}</h1><p>{instruction}</p></div><button className="icon-button" onClick={busy ? () => void cancelGeneration() : onClose} aria-label={busy ? 'Generierung abbrechen' : 'Workflow schließen'}>{busy ? <X size={18} /> : <X size={18} />}</button></header>
    {error && <div className="save-error" role="alert">{error}</div>}
    <div className="longform-steps"><span className={step === 'preflight' ? 'active' : 'done'}>1 Umfang & Kontext</span><span className={step === 'plan' ? 'active' : step === 'draft' ? 'done' : ''}>2 Plan bestätigen</span><span className={step === 'draft' ? 'active' : ''}>3 Entwurf prüfen</span></div>
    {step === 'preflight' && <div className="longform-grid"><div className="longform-card"><span className="eyebrow">ZIELUMFANG</span><strong className="longform-number">{intent.pages ? `${intent.pages} Seiten` : `${intent.words ?? totalWords} Wörter`}</strong><p>{intent.pages ? `${intent.pages} Seiten entsprechen in diesem Projekt ungefähr ${totalWords.toLocaleString('de-DE')} Wörtern.` : 'Die Zielwortzahl ist eine Schätzung für die Planung, keine exakte Export-Seitenzahl.'}</p><label className="field-label">Zielwörter<input type="number" value={totalWords} readOnly /></label></div><div className="longform-card"><span className="eyebrow">PREFLIGHT</span>{preflight?.items.map((item) => <div className={`preflight-row ${item.level}`} key={`${item.level}-${item.label}`}><span>{item.level === 'blocking' ? 'Blockierend' : item.level === 'recommended' ? 'Empfohlen' : 'Optional'}</span><div><strong>{item.label}</strong><p>{item.detail}</p></div></div>)}{summaryStatus.some((summary) => summary.status === 'outdated') && <div className="preflight-row recommended"><span>Empfohlen</span><div><strong>Zusammenfassungen</strong><p>Mindestens eine Zusammenfassung ist veraltet und wird für diesen Plan nicht verwendet. Prüfe sie vor der Planung.</p></div></div>}<button className="ghost-button" onClick={() => void createProjectSummary()} disabled={summaryBusy}>{summaryBusy ? 'Zusammenfassung wird erstellt …' : 'Zusammenfassung prüfen'}</button></div><div className="longform-card direction-card"><div className="card-title-row"><div><span className="eyebrow">STORY-RICHTUNG</span><h2>Was soll die Geschichte gerade tun?</h2></div><button className="ghost-button" onClick={() => void saveDirection()} disabled={busy}><Save size={15} /> Speichern</button></div><div className="form-grid"><label className="field-label full-field">Prämisse<textarea value={directionDraft.premise} onChange={(event) => setDirectionDraft({ ...directionDraft, premise: event.target.value })} rows={2} /></label><label className="field-label">Geplantes Ende<textarea value={directionDraft.plannedEnding} onChange={(event) => setDirectionDraft({ ...directionDraft, plannedEnding: event.target.value })} rows={2} /></label><label className="field-label">Endstatus<select value={directionDraft.endingStatus} onChange={(event) => setDirectionDraft({ ...directionDraft, endingStatus: event.target.value as StoryDirection['endingStatus'] })}><option value="open">Noch offen</option><option value="preferred">Bevorzugt</option><option value="fixed">Fest</option></select></label><label className="field-label">Nächster Wendepunkt<input value={directionDraft.nextTurningPoint} onChange={(event) => setDirectionDraft({ ...directionDraft, nextTurningPoint: event.target.value })} /></label></div></div><div className="longform-actions"><button className="ghost-button" onClick={onClose}>Auftrag abbrechen</button><button className="primary-button large" disabled={busy || !preflight?.canPlan} onClick={() => void start()}>{busy ? 'Bereite vor …' : 'Kapitelplan erstellen'}</button></div></div>}
    {step === 'plan' && plan && <div className="longform-card plan-card"><div className="card-title-row"><div><span className="eyebrow">PLAN VOR DER GENERIERUNG</span><h2>{plan.chapterTitle}</h2><p>{plan.chapterGoal}</p></div><span className="status-pill yellow"><LockKeyhole size={13} /> Noch nicht bestätigt</span></div><label className="field-label">Kapitelziel<textarea value={plan.chapterGoal} onChange={(event) => setPlan({ ...plan, chapterGoal: event.target.value })} rows={2} /></label><div className="beat-list">{plan.beats.map((beat, index) => <article className="beat-card" key={beat.id}><span className="beat-number">{index + 1}</span><div><input value={beat.title} onChange={(event) => setPlan({ ...plan, beats: plan.beats.map((item) => item.id === beat.id ? { ...item, title: event.target.value } : item) })} /><p>{beat.purpose}</p><label className="field-label">Ereignis<textarea value={beat.event} onChange={(event) => setPlan({ ...plan, beats: plan.beats.map((item) => item.id === beat.id ? { ...item, event: event.target.value } : item) })} rows={2} /></label></div><strong>{beat.targetWords} W</strong></article>)}</div><div className="longform-actions"><button className="ghost-button" onClick={() => setStep('preflight')}>Zurück</button><button className="primary-button large" disabled={busy} onClick={() => void confirmPlan()}><Check size={16} /> Plan bestätigen</button></div></div>}
    {step === 'draft' && job && plan && <div className="longform-card draft-card"><div className="card-title-row"><div><span className="eyebrow">ENTWURF · {wordCount.toLocaleString('de-DE')} WÖRTER</span><h2>{plan.chapterTitle}</h2></div><span className={`status-pill ${job.status === 'draft_ready' ? 'green' : 'yellow'}`}><FileText size={13} /> {job.status === 'draft_ready' ? 'Geprüfter Entwurf' : 'Prüfung erforderlich'}</span></div><div className="draft-notice"><LockKeyhole size={17} /><span>Der Text wird nicht automatisch ins Manuskript geschrieben. Abschnitts- und Gesamtreview bleiben sichtbar.</span></div>{contextOverrideRequired && <div className="save-error"><strong>Projektkontext geändert.</strong><button className="text-button" onClick={() => void continueWithOldContext()} disabled={busy}>Mit altem Kontext bewusst fortfahren</button></div>}{reviews.map((review) => <article className={`review-card ${review.severity}`} key={review.id}><strong>{review.title}</strong><p>{review.description}</p><small>{review.severity} · {review.suggestedAction}</small>{review.severity === 'blocking' && <button className="text-button" onClick={() => { const section = sections.find((item) => item.id === review.sectionId); if (section) void regenerateSection(section); }}>Abschnitt neu erzeugen</button>}<button className="text-button" onClick={() => void repository.updateReviewStatus(review.id, 'exception')}>Als bewusste Ausnahme markieren</button></article>)}{sections.map((section, index) => <div className="draft-section" key={section.id}><div className="section-heading"><strong>Abschnitt {index + 1}</strong><span>Ziel: {section.targetWords} Wörter · {section.actualWords} Wörter</span></div><textarea value={section.content} onChange={(event) => void updateSection(section, event.target.value)} placeholder="Abschnitt prüfen oder nach Codex-Generierung bearbeiten …" rows={10} /></div>)}<div className="longform-actions"><button className="ghost-button" onClick={() => setStep('plan')}>Plan öffnen</button><button className="primary-button large" disabled={busy || sections.some((section) => !section.content.trim()) || job.status !== 'draft_ready' || reviews.some((review) => review.severity === 'blocking' && review.status === 'open')} onClick={() => void accept()}><Plus size={16} /> Als Entwurf übernehmen</button></div></div>}
  </section>;
}
