import sharedCatalog from './genreCatalog.shared.json';

export interface GenreCatalogEntry { id: string; name: string; englishName?: string; category: string; description: string; searchTerms: string[]; active: boolean; }

export const GENRE_CATALOG: GenreCatalogEntry[] = sharedCatalog.entries;
export const GENRE_CATALOG_VERSION = sharedCatalog.version;

export const genreById = (id: string | undefined) => GENRE_CATALOG.find((entry) => entry.id === id && entry.active);
export const genreCategories = [...new Set(GENRE_CATALOG.filter((entry) => entry.active).map((entry) => entry.category))];
export const isCatalogGenreId = (id: string | undefined): id is string => Boolean(id && genreById(id));
