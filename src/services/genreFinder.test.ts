import { beforeEach, describe, expect, it } from 'vitest';
import { GENRE_CATALOG } from '../data/genreCatalog';
import { BrowserDemoRepository } from './storyRepository';

const store = new Map<string, string>();
globalThis.localStorage = { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value), removeItem: (key: string) => store.delete(key), clear: () => store.clear(), key: () => null, length: 0 } as Storage;

describe('optionaler Manuskript-Genre Finder', () => {
  beforeEach(() => store.clear());

  it('liefert einen ausreichend breiten statischen Katalog und speichert manuelle Auswahl', async () => {
    expect(GENRE_CATALOG.length).toBeGreaterThanOrEqual(60);
    expect(GENRE_CATALOG.every((entry) => /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(entry.id))).toBe(true);
    expect(GENRE_CATALOG.find((entry) => entry.id === 'fantasy')?.category).toBe('Fantasy');
    expect(GENRE_CATALOG.filter((entry) => ['space-opera', 'cyberpunk', 'near-future', 'speculative-fiction', 'techno-thriller'].includes(entry.id)).every((entry) => entry.category === 'Science-Fiction')).toBe(true);
    expect(GENRE_CATALOG.find((entry) => entry.id === 'magical-realism')?.category).toBe('Gegenwart und Gesellschaft');
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const book = workspace.books[0]!;
    const saved = await repository.saveBookGenres({ bookId: book.id, projectId: workspace.project.id, primaryGenreId: 'mystery', secondaryGenreIds: ['historical-mystery'], customGenreNames: ['Eigene Mischung'], genreSource: 'manual', genreAuthorConfirmed: true });
    expect(saved).toMatchObject({ primaryGenreId: 'mystery', secondaryGenreIds: ['historical-mystery'], genreSource: 'manual', genreAuthorConfirmed: true });
    await expect(repository.saveBookGenres({ bookId: book.id, projectId: workspace.project.id, primaryGenreId: 'not-in-catalog', secondaryGenreIds: [], customGenreNames: [], genreSource: 'manual', genreAuthorConfirmed: false })).rejects.toThrow('Unbekannte Hauptgenre-ID');
  });

  it('überschreibt ein manuell bestätigtes Genre nicht durch AI-Erkennung', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const book = workspace.books[0]!;
    await repository.saveBookGenres({ bookId: book.id, projectId: workspace.project.id, primaryGenreId: 'crime', secondaryGenreIds: [], customGenreNames: [], genreSource: 'manual', genreAuthorConfirmed: true });
    const unchanged = await repository.saveBookGenres({ bookId: book.id, projectId: workspace.project.id, primaryGenreId: 'fantasy', secondaryGenreIds: [], customGenreNames: [], genreSource: 'ai_detected', genreAuthorConfirmed: false, genreConfidence: 0.9 });
    expect(unchanged).toMatchObject({ primaryGenreId: 'crime', genreSource: 'manual', genreAuthorConfirmed: true });
  });

  it('bewahrt die vollständigen AI-Signale und lässt den Vorschlag offen', async () => {
    const repository = new BrowserDemoRepository();
    const workspace = await repository.loadWorkspace();
    const book = workspace.books[0]!;
    const saved = await repository.saveBookGenres({ bookId: book.id, projectId: workspace.project.id, primaryGenreId: 'science-fiction', secondaryGenreIds: ['cyberpunk'], customGenreNames: [], genreSource: 'ai_detected', genreConfidence: 0.74, genreReason: 'Technische Zukunft und Machtkonflikt.', genreAuthorConfirmed: false, genreSupportingSignals: ['autonome Systeme'], genreContradictingSignals: ['Gegenwartsnahe Nebenhandlung'], genreAlternatives: [{ genreId: 'techno-thriller', reason: 'Technik als Spannungsträger' }], genreAudienceNotes: ['Erwachsene Leserschaft'], genreWarnings: ['Noch nicht bestätigt'], genrePromptVersion: 'storymemory-genre-v1' });
    expect(saved.genreAuthorConfirmed).toBe(false);
    expect(saved.genreSupportingSignals).toEqual(['autonome Systeme']);
    expect(saved.genreAlternatives?.[0]?.genreId).toBe('techno-thriller');
  });
});
