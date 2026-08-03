import { useMemo, useState } from 'react';
import { CalendarClock, ChevronRight, Maximize2, Minus, Plus } from 'lucide-react';
import type { TimelineEvent } from '../../types/domain';

const tracks = ['Alle', 'Haupthandlung', 'Marek', 'Leserwissen', 'Tatsächliche Wahrheit'];
const minimumPaperWidth = 4200;

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

  return <section className="timeline-view simple-timeline-view">
    <div className="view-heading">
      <div><span className="eyebrow">DEIN GESCHICHTEN-PAPIER</span><h1>Timeline</h1><p>Jeder Zeitpunkt ist eine Spalte. Ereignisse stehen darunter – von links nach rechts.</p></div>
      <div className="timeline-paper-actions"><button className="icon-button" title="Herauszoomen" onClick={() => setZoom((value) => Math.max(.65, value - .1))}><Minus size={15} /></button><span>{Math.round(zoom * 100)}%</span><button className="icon-button" title="Hineinzoomen" onClick={() => setZoom((value) => Math.min(1.35, value + .1))}><Plus size={15} /></button><button className="icon-button" title="Ansicht zurücksetzen" onClick={() => setZoom(1)}><Maximize2 size={15} /></button></div>
    </div>
    <div className="timeline-paper-filters"><span>Zeige:</span>{tracks.map((track) => <button key={track} className={activeTrack === track ? 'active' : ''} onClick={() => setActiveTrack(track)}>{track}</button>)}</div>
    <div className="timeline-paper-board">
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
      <div className="timeline-paper-hint">Horizontal scrollen, um weiterzulesen.</div>
    </div>
    {active && <div className="timeline-paper-detail"><div><span className="eyebrow">AUSGEWÄHLT</span><h2>{active.title}</h2><p>{active.summary}</p></div><div className="timeline-paper-facts"><span><b>Zeit</b>{active.storyTime}</span><span><b>Figuren</b>{active.characters.join(', ')}</span><span><b>Ort</b>{active.location}</span><span><b>Wissen</b>{active.knowledge}</span></div><button className="text-button">Zur Szene springen <ChevronRight size={14} /></button></div>}
  </section>;
}
