import { useMemo, useState, type WheelEvent } from 'react';
import { CalendarClock, ChevronRight, Maximize2, Minus, Plus } from 'lucide-react';
import type { TimelineEvent } from '../../types/domain';

const tracks = ['Alle', 'Haupthandlung', 'Marek', 'Leserwissen', 'Tatsächliche Wahrheit'];
const minimumPaperWidth = 4200;
const minimumZoom = 0.65;
const maximumZoom = 1.35;

function timelineZoomFromWheel(currentZoom: number, deltaY: number): number {
  const next = currentZoom * Math.exp(-deltaY * 0.002);
  return Math.min(maximumZoom, Math.max(minimumZoom, Number(next.toFixed(3))));
}

export function TimelineView({ events }: { events: TimelineEvent[] }) {
  const [selected, setSelected] = useState(events[2]?.id ?? events[0]?.id);
  const [activeTrack, setActiveTrack] = useState('Alle');
  const [zoom, setZoom] = useState(1);
  const active = events.find((event) => event.id === selected) ?? events[0];
  const shown = useMemo(() => activeTrack === 'Alle' ? events : events.filter((event) => event.track === activeTrack), [activeTrack, events]);
  const timestampGroups = useMemo(() => {
    const groups = new Map<string, TimelineEvent[]>();
    shown.forEach((event) => groups.set(event.storyTime, [...(groups.get(event.storyTime) ?? []), event]));
    return [...groups.entries()];
  }, [shown]);
  const paperWidth = Math.max(minimumPaperWidth, timestampGroups.length * 390 + 180);
  const handleBoardWheel = (event: WheelEvent<HTMLDivElement>) => {
    // Trackpad pinch gestures arrive in WebKit as a wheel event with ctrlKey.
    // Cmd/Ctrl + wheel also gives mouse users a predictable zoom gesture while
    // leaving ordinary vertical and horizontal scrolling untouched.
    if (!event.ctrlKey && !event.metaKey) return;
    event.preventDefault();
    setZoom((value) => timelineZoomFromWheel(value, event.deltaY));
  };

  return <section className="timeline-view simple-timeline-view">
    <div className="view-heading">
      <div><span className="eyebrow">DEIN GESCHICHTEN-PAPIER</span><h1>Timeline</h1><p>Jeder Zeitpunkt ist eine Spalte. Ereignisse stehen darunter – von links nach rechts.</p></div>
      <div className="timeline-paper-actions"><button className="icon-button" title="Herauszoomen" onClick={() => setZoom((value) => Math.max(minimumZoom, value - .1))}><Minus size={15} /></button><span>{Math.round(zoom * 100)}%</span><button className="icon-button" title="Hineinzoomen" onClick={() => setZoom((value) => Math.min(maximumZoom, value + .1))}><Plus size={15} /></button><button className="icon-button" title="Ansicht zurücksetzen" onClick={() => setZoom(1)}><Maximize2 size={15} /></button></div>
    </div>
    <div className="timeline-paper-filters"><span>Zeige:</span>{tracks.map((track) => <button key={track} className={activeTrack === track ? 'active' : ''} onClick={() => setActiveTrack(track)}>{track}</button>)}</div>
    <div className="timeline-paper-board" onWheel={handleBoardWheel}>
      <div className="timeline-paper-zoom-space" style={{ width: paperWidth * zoom, height: 980 * zoom }}>
        <div className="timeline-paper-world timeline-paper-columns" style={{ width: paperWidth, transform: `scale(${zoom})` }}>
        <div className="timeline-paper-start-note">Anfang der Geschichte</div>
        <div className="timeline-paper-axis" />
        {timestampGroups.map(([timestamp, timestampEvents], columnIndex) => <div className="timeline-paper-column" key={timestamp} style={{ left: 180 + columnIndex * 390 }}>
          <div className="timeline-paper-timestamp"><CalendarClock size={15} /><span>{timestamp}</span></div>
          <div className="timeline-paper-column-line" />
          <div className="timeline-paper-column-events">{timestampEvents.map((event, eventIndex) => <button key={event.id} className={`timeline-paper-card card-${(columnIndex + eventIndex) % 4} ${event.id === active?.id ? 'selected' : ''} ${event.status === 'uncertain' ? 'uncertain' : ''}`} onClick={() => setSelected(event.id)}><span className="timeline-paper-card-track">{event.track}</span><strong>{event.title}</strong><span>{event.chapter} · {event.location}</span></button>)}</div>
        </div>)}
        <div className="timeline-paper-end-note" style={{ left: Math.max(900, 180 + timestampGroups.length * 390) }}>Hier geht deine Geschichte weiter …</div>
        </div>
      </div>
      <div className="timeline-paper-hint">Horizontal scrollen · Pinch oder ⌘/Ctrl + Mausrad zum Zoomen</div>
    </div>
    {active && <div className="timeline-paper-detail"><div><span className="eyebrow">AUSGEWÄHLT</span><h2>{active.title}</h2><p>{active.summary}</p></div><div className="timeline-paper-facts"><span><b>Zeit</b>{active.storyTime}</span><span><b>Figuren</b>{active.characters.join(', ')}</span><span><b>Ort</b>{active.location}</span><span><b>Wissen</b>{active.knowledge}</span></div><button className="text-button">Zur Szene springen <ChevronRight size={14} /></button></div>}
  </section>;
}
