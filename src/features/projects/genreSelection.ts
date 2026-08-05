import { GENRE_CATALOG, type GenreCatalogEntry } from '../../data/genreCatalog';

export interface GenreGroup {
  category: string;
  entries: GenreCatalogEntry[];
}

export function filterGenres(query: string, excludedIds: string[] = []): GenreCatalogEntry[] {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const excluded = new Set(excludedIds);

  return GENRE_CATALOG
    .filter((entry) => entry.active && !excluded.has(entry.id))
    .filter((entry) => !normalizedQuery || `${entry.name} ${entry.englishName ?? ''} ${entry.description} ${entry.searchTerms.join(' ')}`.toLocaleLowerCase().includes(normalizedQuery));
}

export function groupGenres(entries: GenreCatalogEntry[]): GenreGroup[] {
  const groups = new Map<string, GenreCatalogEntry[]>();
  entries.forEach((entry) => groups.set(entry.category, [...(groups.get(entry.category) ?? []), entry]));
  return [...groups.entries()].sort(([first], [second]) => first.localeCompare(second)).map(([category, groupedEntries]) => ({ category, entries: groupedEntries }));
}
