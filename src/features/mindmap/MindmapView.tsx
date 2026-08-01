import { useRef, useState, type PointerEvent as ReactPointerEvent, type WheelEvent as ReactWheelEvent } from 'react';
import { ChevronRight, Filter, Maximize2, Minus, Plus, Search } from 'lucide-react';
import type { MindEdge, MindNode } from '../../types/domain';

type Point = { x: number; y: number };
type Size = { width: number; height: number };
type DragState = { id: string; startX: number; startY: number; origin: Point };

const PAPER_WIDTH = 1600;
const PAPER_HEIGHT = 1050;
const PROJECT_ID = 'storymemory-project';
const PROJECT_SIZE = { width: 230, height: 230 };
const NODE_SIZE = { width: 190, height: 78 };
const PROJECT_POSITION: Point = { x: PAPER_WIDTH / 2 - PROJECT_SIZE.width / 2, y: PAPER_HEIGHT / 2 - PROJECT_SIZE.height / 2 };

const defaultPositions: Record<string, Point> = {
  marek: { x: 170, y: 185 },
  lena: { x: 170, y: 700 },
  photo: { x: 490, y: 95 },
  number: { x: 490, y: 760 },
  simulation: { x: 1060, y: 120 },
  apartment: { x: 430, y: 465 },
  chapter3: { x: 1060, y: 735 },
};

const rootEdges: MindEdge[] = [
  { id: 'root-marek', source: PROJECT_ID, target: 'marek', label: 'Hauptfigur' },
  { id: 'root-number', source: PROJECT_ID, target: 'number', label: 'zentraler Hinweis' },
  { id: 'root-simulation', source: PROJECT_ID, target: 'simulation', label: 'Geheimnis' },
  { id: 'root-chapter', source: PROJECT_ID, target: 'chapter3', label: 'Schlüsselkapitel' },
];

function getInitialPositions(nodes: MindNode[]): Record<string, Point> {
  return Object.fromEntries(nodes.map((node) => [node.id, defaultPositions[node.id] ?? { x: node.x, y: node.y }])) as Record<string, Point>;
}

function centreOf(position: Point, size: Size): Point {
  return { x: position.x + size.width / 2, y: position.y + size.height / 2 };
}

function anchorOnShape(from: Point, to: Point, size: Size, circle = false): Point {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const distance = Math.hypot(dx, dy) || 1;
  if (circle) {
    const radius = size.width / 2;
    return { x: from.x + (dx / distance) * radius, y: from.y + (dy / distance) * radius };
  }
  const factor = Math.min((size.width / 2) / Math.max(Math.abs(dx), 1), (size.height / 2) / Math.max(Math.abs(dy), 1));
  return { x: from.x + dx * factor, y: from.y + dy * factor };
}

export function MindmapView({ nodes, edges }: { nodes: MindNode[]; edges: MindEdge[] }) {
  const boardRef = useRef<HTMLDivElement>(null);
  const nodeDrag = useRef<DragState | null>(null);
  const paperDrag = useRef<{ startX: number; startY: number; origin: Point } | null>(null);
  const [selected, setSelected] = useState(nodes[0]?.id ?? PROJECT_ID);
  const [filter, setFilter] = useState('Alle');
  const [search, setSearch] = useState('');
  const [scale, setScale] = useState(.78);
  const [pan, setPan] = useState<Point>({ x: 0, y: 0 });
  const [positions, setPositions] = useState<Record<string, Point>>(() => getInitialPositions(nodes));

  const shown = nodes.filter((node) => {
    const matchesFilter = filter === 'Alle' || node.type === filter;
    const matchesSearch = !search.trim() || node.label.toLowerCase().includes(search.trim().toLowerCase());
    return matchesFilter && matchesSearch;
  });
  const visibleIds = new Set(shown.map((node) => node.id));
  const selectedNode = selected === PROJECT_ID ? undefined : nodes.find((node) => node.id === selected);
  const activeLabel = selected === PROJECT_ID ? 'Zugestellt' : selectedNode?.label;
  const activeType = selected === PROJECT_ID ? 'Projekt' : selectedNode?.type;

  const getPosition = (id: string): Point => (id === PROJECT_ID ? PROJECT_POSITION : positions[id] ?? { x: 0, y: 0 });
  const getSize = (id: string): Size => (id === PROJECT_ID ? PROJECT_SIZE : NODE_SIZE);
  const getAnchorPoint = (id: string, other: Point): Point => {
    const position = getPosition(id);
    const size = getSize(id);
    return anchorOnShape(centreOf(position, size), other, size, id === PROJECT_ID);
  };

  const updateZoom = (nextScale: number, cursor?: Point) => {
    const next = Math.min(1.6, Math.max(.38, nextScale));
    if (!cursor) {
      setScale(next);
      return;
    }
    const worldPoint = { x: (cursor.x - pan.x) / scale, y: (cursor.y - pan.y) / scale };
    setPan({ x: cursor.x - worldPoint.x * next, y: cursor.y - worldPoint.y * next });
    setScale(next);
  };

  const handleWheel = (event: ReactWheelEvent<HTMLDivElement>) => {
    event.preventDefault();
    const bounds = boardRef.current?.getBoundingClientRect();
    if (!bounds) return;
    const cursor = { x: event.clientX - bounds.left, y: event.clientY - bounds.top };
    updateZoom(scale * (event.deltaY < 0 ? 1.1 : .9), cursor);
  };

  const handleBoardPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest('.map-node')) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    paperDrag.current = { startX: event.clientX, startY: event.clientY, origin: pan };
  };

  const handleNodePointerDown = (event: ReactPointerEvent<HTMLButtonElement>, id: string) => {
    event.stopPropagation();
    setSelected(id);
    if (id === PROJECT_ID) return;
    const origin = positions[id] ?? { x: 0, y: 0 };
    event.currentTarget.setPointerCapture(event.pointerId);
    nodeDrag.current = { id, startX: event.clientX, startY: event.clientY, origin };
  };

  const handlePointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (nodeDrag.current) {
      const drag = nodeDrag.current;
      const dx = (event.clientX - drag.startX) / scale;
      const dy = (event.clientY - drag.startY) / scale;
      setPositions((current) => ({ ...current, [drag.id]: { x: drag.origin.x + dx, y: drag.origin.y + dy } }));
      return;
    }
    if (paperDrag.current) {
      const drag = paperDrag.current;
      setPan({ x: drag.origin.x + event.clientX - drag.startX, y: drag.origin.y + event.clientY - drag.startY });
    }
  };

  const stopDragging = (event: ReactPointerEvent<HTMLDivElement>) => {
    nodeDrag.current = null;
    paperDrag.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  };

  const resetView = () => {
    setScale(.78);
    setPan({ x: 0, y: 0 });
  };

  const visibleEdges = [...rootEdges, ...edges].filter((edge) => {
    return (edge.source === PROJECT_ID || visibleIds.has(edge.source)) && (edge.target === PROJECT_ID || visibleIds.has(edge.target));
  });

  return <section className="mindmap-view">
    <div className="view-heading">
      <div><span className="eyebrow">DEIN GESCHICHTEN-PAPIER</span><h1>Mindmap</h1><p>Ziehe Bubbles, verschiebe das Papier und zoome mit dem Mausrad.</p></div>
      <div className="mindmap-actions">
        <div className="search-field compact-search"><Search size={14} /><input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Knoten suchen …" /></div>
        <button className="icon-button" title="Herauszoomen" onClick={() => updateZoom(scale - .1)}><Minus size={15} /></button>
        <span className="paper-zoom">{Math.round(scale * 100)}%</span>
        <button className="icon-button" title="Hineinzoomen" onClick={() => updateZoom(scale + .1)}><Plus size={15} /></button>
        <button className="icon-button" title="Papier zentrieren" onClick={resetView}><Maximize2 size={15} /></button>
      </div>
    </div>
    <div className="mindmap-toolbar paper-toolbar"><Filter size={14} /><span>Nur anzeigen:</span>{['Alle', 'Charakter', 'Hinweis', 'Geheimnis', 'Ort', 'Kapitel'].map((item) => <button key={item} className={filter === item ? 'active' : ''} onClick={() => setFilter(item)}>{item}</button>)}</div>
    <div ref={boardRef} className={`mindmap-board paper-board ${paperDrag.current ? 'is-panning' : ''}`} onWheel={handleWheel} onPointerDown={handleBoardPointerDown} onPointerMove={handlePointerMove} onPointerUp={stopDragging} onPointerCancel={stopDragging}>
      <div className="paper-world" style={{ transform: `translate(${pan.x}px, ${pan.y}px) scale(${scale})` }}>
        <svg className="map-lines" width={PAPER_WIDTH} height={PAPER_HEIGHT} viewBox={`0 0 ${PAPER_WIDTH} ${PAPER_HEIGHT}`} aria-hidden="true">
          {visibleEdges.map((edge) => {
            const sourcePosition = getPosition(edge.source);
            const targetPosition = getPosition(edge.target);
            const sourceCentre = centreOf(sourcePosition, getSize(edge.source));
            const targetCentre = centreOf(targetPosition, getSize(edge.target));
            const source = getAnchorPoint(edge.source, targetCentre);
            const target = getAnchorPoint(edge.target, sourceCentre);
            const curve = Math.max(70, Math.hypot(target.x - source.x, target.y - source.y) * .18);
            const sourceDistance = Math.hypot(source.x - sourceCentre.x, source.y - sourceCentre.y) || 1;
            const targetDistance = Math.hypot(targetCentre.x - target.x, targetCentre.y - target.y) || 1;
            const sourceDirection = { x: (source.x - sourceCentre.x) / sourceDistance, y: (source.y - sourceCentre.y) / sourceDistance };
            const targetDirection = { x: (targetCentre.x - target.x) / targetDistance, y: (targetCentre.y - target.y) / targetDistance };
            const control1 = { x: source.x + sourceDirection.x * curve, y: source.y + sourceDirection.y * curve };
            const control2 = { x: target.x - targetDirection.x * curve, y: target.y - targetDirection.y * curve };
            const labelX = (source.x + target.x) / 2;
            const labelY = (source.y + target.y) / 2 - 8;
            const labelWidth = Math.max(76, edge.label.length * 7.2 + 20);
            const angle = Math.atan2(targetDirection.y, targetDirection.x);
            const arrowSize = 18;
            const arrowWidth = 8;
            const base = { x: target.x - targetDirection.x * arrowSize, y: target.y - targetDirection.y * arrowSize };
            const arrowPoints = `${target.x},${target.y} ${base.x + Math.sin(angle) * arrowWidth},${base.y - Math.cos(angle) * arrowWidth} ${base.x - Math.sin(angle) * arrowWidth},${base.y + Math.cos(angle) * arrowWidth}`;
            const sourceLabel = edge.source === PROJECT_ID ? 'Zugestellt' : nodes.find((node) => node.id === edge.source)?.label ?? edge.source;
            const targetLabel = edge.target === PROJECT_ID ? 'Zugestellt' : nodes.find((node) => node.id === edge.target)?.label ?? edge.target;
            return <g key={edge.id}><title>{sourceLabel} → {targetLabel}: {edge.label}</title><path className="mind-edge" d={`M ${source.x} ${source.y} C ${control1.x} ${control1.y} ${control2.x} ${control2.y} ${target.x} ${target.y}`} /><polygon className="mind-arrowhead" points={arrowPoints} /><g className="edge-label"><rect x={labelX - labelWidth / 2} y={labelY - 13} width={labelWidth} height="25" rx="12.5" /><text x={labelX} y={labelY + 5} textAnchor="middle">{edge.label}</text></g></g>;
          })}
        </svg>
        <div className="map-world">
          <button className={`map-node project-node ${selected === PROJECT_ID ? 'selected' : ''}`} style={{ left: PROJECT_POSITION.x, top: PROJECT_POSITION.y }} onPointerDown={(event) => handleNodePointerDown(event, PROJECT_ID)}><span className="node-type">PROJEKT</span><strong>Zugestellt</strong><small>Band 1 · Story Memory</small></button>
          {shown.map((node) => { const position = positions[node.id] ?? { x: node.x, y: node.y }; return <button key={node.id} className={`map-node ${node.status ?? ''} ${selected === node.id ? 'selected' : ''}`} style={{ left: position.x, top: position.y }} onPointerDown={(event) => handleNodePointerDown(event, node.id)}><span className="node-type">{node.type}</span><strong>{node.label}</strong></button>; })}
        </div>
      </div>
      <div className="map-direction"><span>→</span> Pfeil zeigt immer von der Quelle zum Ziel</div><div className="map-legend"><span><i className="legend-dot green" />Kanon</span><span><i className="legend-dot yellow" />Vermutung</span><span><i className="legend-dot purple" />Idee</span></div>
    </div>
    {activeLabel && <div className="map-detail"><div><span className="eyebrow">AUSGEWÄHLT</span><h2>{activeLabel}</h2><p>{activeType} · Bubble ziehen, um sie neu anzuordnen</p></div><button className="text-button">In Story Bible öffnen <ChevronRight size={14} /></button></div>}
  </section>;
}
