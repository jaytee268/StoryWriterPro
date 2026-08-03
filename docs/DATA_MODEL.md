# Datenmodell

`migrations/001_initial.sql` definiert die lokale SQLite-Basis.

| Bereich | Tabellen |
| --- | --- |
| Projektstruktur | projects, books, chapters, scenes, scene_versions |
| Story Bible | story_entities, story_source_references, entity_relations, facts, character_knowledge |
| Bible-Review | bible_update_runs, bible_proposals |
| Dramaturgie | timeline_events, plot_threads, clues, secrets |
| Workflows | correction_results, analysis_jobs |
| Konfiguration | provider_settings, app_settings |

Alle fachlichen Tabellen besitzen stabile UUID-IDs sowie `created_at` und `updated_at`. Foreign Keys schützen Kapitel-/Szenenbeziehungen und Entitätsrelationen. Ein Status und eine Vertrauensstufe trennen bestätigten Kanon von Vorschlägen und Vermutungen.

## Migrationen und Story-Bible-Zuordnung

`002_workspace_indexes.sql` ergänzt `story_entities.project_id`. `004_data_safety.sql` backfillt verwaiste Altzeilen nur dann, wenn genau ein Projekt vorhanden ist. Bei mehreren Projekten bleibt `project_id` bewusst `NULL`, bis der Autor die Zuordnung eindeutig festlegt; die App ordnet solche Daten nicht stillschweigend einem Projekt zu. Neue Einträge werden in den Rust-Commands nur mit einem existierenden Projekt akzeptiert. Ein nachträgliches `NOT NULL` ist in SQLite ein Tabellenumbau und wird deshalb erst nach einer gesicherten Datenbereinigung geplant.

## Szenen-Versionen

`update_scene` speichert nur den aktuellen Szenenstand und aktualisiert Kapitel-/Projektzeitpunkte. Es erzeugt keine historische Kopie. `create_scene_version` legt eine bewusste Vollversion mit `reason` (`manual`, `before_correction`, `before_ai_change`, `before_import` oder `automatic_checkpoint`) an. Die beim ersten Öffnen erzeugten Baselines sind als `automatic_checkpoint` markiert.

## Bible Update und Quellen

Migration 006 ergänzt `bible_update_runs` und `bible_proposals`. Ein Run enthält Szene-Zeitpunkt, stabilen Content-Hash und Extractor-ID. Identische abgeschlossene Hashes werden wiederverwendet. Vorschläge bleiben bis zur Review-Aktion `pending`; erst `accepted` oder `edited` erzeugt beziehungsweise ändert einen Story-Bible-Eintrag. `story_source_references` verknüpft Eintrag, Proposal, Kapitel und Szene über IDs und speichert zusätzlich den belegenden Ausschnitt und optionale Offsets.
