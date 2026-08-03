import { useMemo, useState } from 'react';
import { CalendarClock, ChevronRight, Maximize2, Minus, Plus } from 'lucide-react';
import type { TimelineEvent } from '../../types/domain';

const tracks = ['Alle', 'Haupthandlung', 'Marek', 'Leserwissen', 'Tatsächliche Wahrheit'];
const paperWidth = 1640;

export function TimelineView({ events }: { events: TimelineEvent[] }) {
  const [selected, setSelected] = useState(events[2]?.id ?? events[0]?.id);
  const [activeTrack, setActiveTrack] = useState('Alle');
  const [zoom, setZoom] = useState(.82);
  const active = events.find((event) => event.id === selected) ?? events[0];
  const shown = useMemo(() => activeTrack === 'Alle' ? events : events.filter((event) => event.track === activeTrack), [activeTrack, events]);
  const shownTracks = activeTrack === 'Alle' ? tracks.slice(1) : [activeTrack];
  const reset = () => setZoom(.82);

  return <section className="timeline-view simple-timeline-view">
    <div className="view-heading">
      <div><span className="eyebrow">DEIN GESCHICHTEN-PAPIER</span><h1>Timeline</h1><p>Ordne deine Geschichte auf einem Blatt Papier.</p></div>
      <div className="timeline-paper-actions"><button className="icon-button" title="Herauszoomen" onClick={() => setZoom((value) => Math.max(.55, value - .1))}><Minus size={15} /></button><span>{Math.round(zoom * 100)}%</span><button className="icon-button" title="Hineinzoomen" onClick={() => setZoom((value) => Math.min(1.35, value + .1))}><Plus size={15} /></button><button className="icon-button" title="Ansicht zurücksetzen" onClick={reset}><Maximize2 size={15} /></button></div>
    </div>
    <div className="timeline-paper-filters"><span>Zeige:</span>{tracks.map((track) => <button key={track} className={activeTrack === track ? 'active' : ''} onClick={() => setActiveTrack(track)}>{track}</button>)}</div>
    <div className="timeline-paper-board">
      <div className="timeline-paper-world" style={{ width: paperWidth, transform: `scale(${zoom})` }}>
        <div className="timeline-paper-ruler"><span>Montag · 18:40</span><span>Dienstag · 08:15</span><span>Dienstag · 13:20</span><span>Dienstag · 16:00</span></div>
        <div className="timeline-paper-spine" />
        {shownTracks.map((track, trackIndex) => <div className="timeline-paper-track" key={track}>
          <div className="timeline-paper-track-label"><i className={`timeline-track-dot track-${trackIndex}`} />{track}</div>
          <div className="timeline-paper-events">{shown.filter((event) => event.track === track).map((event, eventIndex) => <button key={event.id} className={`timeline-paper-card card-${(eventIndex + trackIndex) % 4} ${event.id === active?.id ? 'selected' : ''} ${event.status === 'uncertain' ? 'uncertain' : ''}`} onClick={() => setSelected(event.id)}><span className="timeline-paper-time"><CalendarClock size={13} /> {event.storyTime}</span><strong>{event.title}</strong><span>{event.chapter} · {event.location}</span></button>)}</div>
        </div>)}
        {!shown.length && <div className="timeline-paper-empty">Für diese Spur gibt es noch keine Ereignisse.</div>}
      </div>
      <div className="timeline-paper-hint">Ziehe das Papier horizontal, um weiterzulesen.</div>
    </div>
    {active && <div className="timeline-paper-detail"><div><span className="eyebrow">AUSGEWÄHLT</span><h2>{active.title}</h2><p>{active.summary}</p></div><div className="timeline-paper-facts"><span><b>Zeit</b>{active.storyTime}</span><span><b>Figuren</b>{active.characters.join(', ')}</span><span><b>Ort</b>{active.location}</span><span><b>Wissen</b>{active.knowledge}</span></div><button className="text-button">Zur Szene springen <ChevronRight size={14} /></button></div>}
  </section>;
}
