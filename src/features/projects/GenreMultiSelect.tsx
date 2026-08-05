import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { Check, ChevronDown, Search, X } from 'lucide-react';
import { GENRE_CATALOG } from '../../data/genreCatalog';
import { filterGenres, groupGenres } from './genreSelection';

interface Props {
  value: string[];
  onChange: (value: string[]) => void;
  excludeId?: string;
  label?: string;
  placeholder?: string;
}

export function GenreMultiSelect({ value, onChange, excludeId, label = 'Weitere Genres', placeholder = 'Nebengenres hinzufügen – optional' }: Props) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const listId = useId();
  const labelId = useId();
  const entries = filterGenres(query, excludeId ? [excludeId] : []);
  const groups = groupGenres(entries);
  const selectedEntries = value.map((id) => GENRE_CATALOG.find((entry) => entry.id === id)).filter((entry): entry is NonNullable<typeof entry> => Boolean(entry));
  const activeEntry = entries[activeIndex];

  useEffect(() => {
    if (!open) return undefined;
    const onPointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', onPointerDown);
    searchRef.current?.focus();
    return () => document.removeEventListener('pointerdown', onPointerDown);
  }, [open]);

  const openPicker = () => { setOpen(true); setQuery(''); setActiveIndex(0); };
  const toggle = (id: string) => onChange(value.includes(id) ? value.filter((current) => current !== id) : [...value, id]);
  const move = (direction: 1 | -1) => {
    if (!open) { openPicker(); return; }
    if (entries.length) setActiveIndex((current) => (current + direction + entries.length) % entries.length);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === 'Escape') { event.preventDefault(); setOpen(false); return; }
    if (event.key === 'ArrowDown') { event.preventDefault(); move(1); return; }
    if (event.key === 'ArrowUp') { event.preventDefault(); move(-1); return; }
    if (event.key === 'Enter' && open && activeEntry) { event.preventDefault(); toggle(activeEntry.id); }
  };

  return <div className="genre-picker" ref={rootRef}>
    <span className="field-label-text" id={labelId}>{label}</span>
    <button type="button" className={`genre-combobox-trigger ${open ? 'open' : ''}`} role="combobox" aria-haspopup="listbox" aria-expanded={open} aria-controls={listId} aria-labelledby={labelId} aria-activedescendant={open && activeEntry ? `${listId}-option-${activeEntry.id}` : undefined} onClick={() => open ? setOpen(false) : openPicker()} onKeyDown={onKeyDown}>
      <span className={value.length ? 'genre-selected-label' : 'genre-placeholder'}>{value.length ? `${value.length} ausgewählt` : placeholder}</span><ChevronDown size={16} aria-hidden="true" />
    </button>
    {selectedEntries.length > 0 && <div className="genre-chips" aria-label="Ausgewählte Nebengenres">{selectedEntries.map((entry) => <span className="genre-chip" key={entry.id}>{entry.name}<button type="button" aria-label={`${entry.name} entfernen`} onClick={() => toggle(entry.id)}><X size={12} /></button></span>)}</div>}
    {open && <div className="genre-picker-menu">
      <div className="genre-picker-search"><Search size={15} aria-hidden="true" /><input ref={searchRef} value={query} onChange={(event) => { setQuery(event.target.value); setActiveIndex(0); }} onKeyDown={onKeyDown} role="searchbox" aria-label="Weitere Genres suchen" placeholder="Weitere Genres suchen …" /></div>
      <div className="genre-picker-options" id={listId} role="listbox" aria-multiselectable="true" aria-labelledby={labelId}>{groups.map((group) => <div className="genre-picker-group" key={group.category}><div className="genre-picker-category">{group.category}</div>{group.entries.map((entry) => { const index = entries.findIndex((option) => option.id === entry.id); const isSelected = value.includes(entry.id); return <div id={`${listId}-option-${entry.id}`} role="option" aria-selected={isSelected} className={`genre-picker-option ${index === activeIndex ? 'active' : ''}`} key={entry.id} onMouseEnter={() => setActiveIndex(index)} onClick={() => toggle(entry.id)}><span>{entry.name}</span>{isSelected && <Check size={15} aria-hidden="true" />}</div>; })}</div>)}{!entries.length && <p className="genre-picker-empty">Kein passendes Genre gefunden.</p>}</div>
      <p className="genre-picker-description">Mehrere Nebengenres sind möglich. Das Hauptgenre wird hier nicht angeboten.</p>
    </div>}
  </div>;
}
