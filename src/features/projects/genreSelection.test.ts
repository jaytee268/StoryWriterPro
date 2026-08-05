import { describe, expect, it } from 'vitest';
import { filterGenres, groupGenres } from './genreSelection';

describe('gemeinsame Genre-Kataloglogik', () => {
  it('filtert nach Namen, Beschreibung und Suchbegriffen und gruppiert nach Kategorie', () => {
    const entries = filterGenres('krimi');
    expect(entries.length).toBeGreaterThan(0);
    expect(groupGenres(entries).every((group) => group.entries.every((entry) => entry.active))).toBe(true);
  });

  it('schließt das Hauptgenre aus der Nebengenre-Auswahl aus', () => {
    expect(filterGenres('', ['mystery']).some((entry) => entry.id === 'mystery')).toBe(false);
  });
});
