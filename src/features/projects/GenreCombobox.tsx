import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { Check, ChevronDown, Search, X } from 'lucide-react';
import { GENRE_CATALOG, type GenreCatalogEntry } from '../../data/genreCatalog';
import { filterGenres, groupGenres } from './genreSelection';

interface Props {
  value: string;
  onChange: (value: string) => void;
  label?: string;
  placeholder?: string;
}

export function GenreCombobox({ value, onChange, label = 'Hauptgenre', placeholder = 'Genre auswählen – optional' }: Props) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const listId = useId();
  const labelId = useId();
  const selected = GENRE_CATALOG.find((entry) => entry.id === value && entry.active);
  const entries = filterGenres(query);
  const options: Array<GenreCatalogEntry | undefined> = [undefined, ...entries];
  const groups = groupGenres(entries);
  const activeOption = options[activeIndex];

  useEffect(() => {
    if (!open) return undefined;
    const onPointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', onPointerDown);
    searchRef.current?.focus();
    return () => document.removeEventListener('pointerdown', onPointerDown);
  }, [open]);

  const openPicker = () => {
    setOpen(true);
    setQuery('');
    setActiveIndex(value ? Math.max(0, entries.findIndex((entry) => entry.id === value) + 1) : 0);
  };

  const choose = (nextValue: string) => {
    onChange(nextValue);
    setOpen(false);
    setQuery('');
  };

  const move = (direction: 1 | -1) => {
    if (!open) {
      openPicker();
      return;
    }
    setActiveIndex((current) => (current + direction + options.length) % options.length);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === 'Escape') { event.preventDefault(); setOpen(false); return; }
    if (event.key === 'ArrowDown') { event.preventDefault(); move(1); return; }
    if (event.key === 'ArrowUp') { event.preventDefault(); move(-1); return; }
    if (event.key === 'Enter' && open) { event.preventDefault(); choose(activeOption?.id ?? ''); }
  };

  return <div className="genre-picker" ref={rootRef}>
    <span className="field-label-text" id={labelId}>{label}</span>
    <div className="genre-trigger-row">
      <button type="button" className={`genre-combobox-trigger ${open ? 'open' : ''}`} role="combobox" aria-haspopup="listbox" aria-expanded={open} aria-controls={listId} aria-labelledby={labelId} aria-activedescendant={open ? `${listId}-option-${activeOption?.id ?? 'none'}` : undefined} onClick={() => open ? setOpen(false) : openPicker()} onKeyDown={onKeyDown}>
        <span className={selected ? 'genre-selected-label' : 'genre-placeholder'}>{selected?.name ?? placeholder}</span><ChevronDown size={16} aria-hidden="true" />
      </button>
      {selected && <button type="button" className="genre-trigger-clear" aria-label="Hauptgenre entfernen" onClick={() => choose('')}><X size={14} /></button>}
    </div>
    {open && <div className="genre-picker-menu">
      <div className="genre-picker-search"><Search size={15} aria-hidden="true" /><input ref={searchRef} value={query} onChange={(event) => { setQuery(event.target.value); setActiveIndex(0); }} onKeyDown={onKeyDown} role="searchbox" aria-label="Genre suchen" placeholder="Genre suchen …" /></div>
      <div className="genre-picker-options" id={listId} role="listbox" aria-labelledby={labelId}>
        <div id={`${listId}-option-none`} role="option" aria-selected={!value} className={`genre-picker-option ${!value && activeIndex === 0 ? 'active' : ''}`} onMouseEnter={() => setActiveIndex(0)} onClick={() => choose('')}><span>Kein Hauptgenre</span>{!value && <Check size={15} aria-hidden="true" />}</div>
        {groups.map((group) => <div className="genre-picker-group" key={group.category}><div className="genre-picker-category">{group.category}</div>{group.entries.map((entry) => { const index = options.findIndex((option) => option?.id === entry.id); return <div id={`${listId}-option-${entry.id}`} role="option" aria-selected={entry.id === value} className={`genre-picker-option ${index === activeIndex ? 'active' : ''}`} key={entry.id} onMouseEnter={() => setActiveIndex(index)} onClick={() => choose(entry.id)}><span>{entry.name}</span>{entry.id === value && <Check size={15} aria-hidden="true" />}</div>; })}</div>)}
        {!entries.length && <p className="genre-picker-empty">Kein passendes Genre gefunden.</p>}
      </div>
      {activeOption ? <p className="genre-picker-description">{activeOption.description}</p> : <p className="genre-picker-description">Ein Hauptgenre ist optional und kann später ergänzt werden.</p>}
    </div>}
  </div>;
}
