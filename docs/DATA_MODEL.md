# Datenmodell

`migrations/001_initial.sql` definiert die lokale SQLite-Basis.

| Bereich | Tabellen |
| --- | --- |
| Projektstruktur | projects, books, chapters, scenes, scene_versions |
| Story Bible | story_entities, entity_relations, facts, character_knowledge |
| Dramaturgie | timeline_events, plot_threads, clues, secrets |
| Workflows | correction_results, analysis_jobs |
| Konfiguration | provider_settings, app_settings |

Alle fachlichen Tabellen besitzen stabile UUID-IDs sowie `created_at` und `updated_at`. Foreign Keys schützen Kapitel-/Szenenbeziehungen und Entitätsrelationen. Ein Status und eine Vertrauensstufe trennen bestätigten Kanon von Vorschlägen und Vermutungen.
