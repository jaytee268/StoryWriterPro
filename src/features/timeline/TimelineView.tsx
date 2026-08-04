import { useMemo, useState, type WheelEvent } from 'react';
import { CalendarClock, ChevronRight, Maximize2, Minus, Plus } from 'lucide-react';
import type { Chapter, PersistentTimelineEvent, StoryEntity } from '../../types/domain';

const minimumPaperWidth = 1200;
const minimumZoom = 0.65;
const maximumZoom = 1.35;

function timelineZoomFromWheel(currentZoom: number, deltaY: number): number {
  return Math.min(maximumZoom, Math.max(minimumZoom, Number((currentZoom * Math.exp(-deltaY * 0.002)).toFixed(3))));
}

export function TimelineView({ events, chapters, entities, onOpenSource, onOpenScene, onReview }: { events: PersistentTimelineEvent[]; chapters: Chapter[]; entities: StoryEntity[]; onOpenSource?: (sourceId: string) => void; onOpenScene?: (sceneId: string) => void; onReview?: (event: PersistentTimelineEvent, status: PersistentTimelineEvent['status']) => void }) {
  const [selected, setSelected] = useState(events[0]?.id);
  const [activeStatus, setActiveStatus] = useState<'Alle' | PersistentTimelineEvent['status']>('Alle');
  const [zoom, setZoom] = useState(1);
  const active = events.find((event) => event.id === selected) ?? events[0];
  const entityName = (id?: string) => entities.find((entity) => entity.id === id)?.name ?? id ?? '—';
  const chapterName = (id: string) => chapters.find((chapter) => chapter.id === id)?.title ?? 'Kapitel';
  const shown = useMemo(() => activeStatus === 'Alle' ? events : events.filter((event) => event.status === activeStatus), [activeStatus, events]);
  const groups = useMemo(() => { const grouped = new Map<string, PersistentTimelineEvent[]>(); shown.forEach((event) => grouped.set(event.storyTimeText || 'Zeit unbekannt', [...(grouped.get(event.storyTimeText || 'Zeit unbekannt') ?? []), event])); return [...grouped.entries()]; }, [shown]);
  const paperWidth = Math.max(minimumPaperWidth, groups.length * 340 + 160);
  const handleWheel = (event: WheelEvent<HTMLDivElement>) => { if (!event.ctrlKey && !event.metaKey) return; event.preventDefault(); setZoom((value) => timelineZoomFromWheel(value, event.deltaY)); };

  return <section className="timeline-view simple-timeline-view">
    <div className="view-heading"><div><span className="eyebrow">DEIN GESCHICHTEN-PAPIER</span><h1>Timeline</h1><p>AI-Ereignisse werden chronologisch vorgeschlagen und bleiben bis zur Review vorläufig.</p></div><div className="timeline-paper-actions"><button className="icon-button" title="Herauszoomen" onClick={() => setZoom((value) => Math.max(minimumZoom, value - .1))}><Minus size={15} /></button><span>{Math.round(zoom * 100)}%</span><button className="icon-button" title="Hineinzoomen" onClick={() => setZoom((value) => Math.min(maximumZoom, value + .1))}><Plus size={15} /></button><button className="icon-button" title="Ansicht zurücksetzen" onClick={() => setZoom(1)}><Maximize2 size={15} /></button></div></div>
    <div className="timeline-paper-filters"><span>Zeige:</span>{(['Alle', 'proposed', 'confirmed', 'uncertain', 'rejected'] as const).map((status) => <button key={status} className={activeStatus === status ? 'active' : ''} onClick={() => setActiveStatus(status)}>{status === 'Alle' ? 'Alle' : status === 'proposed' ? 'Vorschläge' : status === 'confirmed' ? 'Bestätigt' : status === 'uncertain' ? 'Unsicher' : 'Abgelehnt'}</button>)}</div>
    {groups.length ? <div className="timeline-paper-board" onWheel={handleWheel}><div className="timeline-paper-zoom-space" style={{ width: paperWidth * zoom, height: 850 * zoom }}><div className="timeline-paper-world timeline-paper-columns" style={{ width: paperWidth, transform: `scale(${zoom})` }}><div className="timeline-paper-axis" />{groups.map(([timestamp, timestampEvents], columnIndex) => <div className="timeline-paper-column" key={timestamp} style={{ left: 120 + columnIndex * 340 }}><div className="timeline-paper-timestamp"><CalendarClock size={15} /><span>{timestamp}</span></div><div className="timeline-paper-column-line" /><div className="timeline-paper-column-events">{timestampEvents.map((event, eventIndex) => <button key={event.id} className={`timeline-paper-card card-${(columnIndex + eventIndex) % 4} ${event.id === active?.id ? 'selected' : ''} ${event.status === 'uncertain' ? 'uncertain' : ''}`} onClick={() => setSelected(event.id)}><span className="timeline-paper-card-track">{event.status}</span><strong>{event.title}</strong><span>{chapterName(event.chapterId)} · {entityName(event.locationEntityId)}</span></button>)}</div></div>)}</div></div><div className="timeline-paper-hint">Horizontal scrollen · Pinch oder ⌘/Ctrl + Mausrad zum Zoomen</div></div> : <div className="empty-state"><h2>Noch keine Timeline-Ereignisse</h2><p>Führe eine Manuskriptanalyse aus. Vorschläge erscheinen hier ohne automatisch Kanon zu werden.</p></div>}
    {active && <div className="timeline-paper-detail"><div><span className="eyebrow">AUSGEWÄHLT · {active.status}</span><h2>{active.title}</h2><p>{active.summary}</p></div><div className="timeline-paper-facts"><span><b>Storyzeit</b>{active.storyTimeText || 'unbekannt'}</span><span><b>Kapitel</b>{chapterName(active.chapterId)}</span><span><b>Figuren</b>{active.participatingEntityIds.map(entityName).join(', ') || '—'}</span><span><b>Wissen/Zustand</b>{[...active.knowledgeChanges, ...active.stateChanges].join(' · ') || '—'}</span><span><b>Confidence</b>{Math.round(active.confidence * 100)}%</span></div><div className="button-row">{active.sourceReferenceIds[0] && <button className="text-button" onClick={() => onOpenSource?.(active.sourceReferenceIds[0])}>Quelle öffnen <ChevronRight size={14} /></button>}{onOpenScene && <button className="text-button" onClick={() => onOpenScene(active.sceneId)}>Zur Szene springen <ChevronRight size={14} /></button>}{onReview && active.status === 'proposed' && <><button className="text-button" onClick={() => onReview(active, 'confirmed')}>Bestätigen</button><button className="text-button" onClick={() => onReview(active, 'uncertain')}>Unsicher</button><button className="text-button" onClick={() => onReview(active, 'rejected')}>Ablehnen</button></>}</div></div>}
  </section>;
}
