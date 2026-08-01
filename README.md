# StoryMemory

StoryMemory ist ein lokaler Tauri-Desktop-Prototyp für Romane und Buchreihen. Der Autor schreibt selbst; die App verwaltet Manuskript, Story Bible, Quellen, Timeline, Mindmap und vorbereitete Analyse-Workflows.

## Voraussetzungen

- macOS, Windows oder Linux mit Tauri-Systemvoraussetzungen
- Node.js 22+ und npm 10+
- Rust stable, Cargo und Rustup
- Git

## Installation und Start

```bash
npm install
npm run tauri dev
```

Der Browser-Preview ist mit `npm run dev` möglich. Für die echte Desktop-App immer `npm run tauri dev` verwenden.

## Build und Qualität

```bash
npm run typecheck
npm run lint
npm test
npm run build
npm run format:rust
npm run clippy
npm run tauri build
```

Auf diesem macOS-Arbeitsvolume erzeugt der Rust-Build sonst AppleDouble-Nebenartefakte. Für einen reproduzierbaren Build genügt:

```bash
CARGO_TARGET_DIR=/private/tmp/storymemory-target npm run tauri build -- --bundles app
```

Das erzeugt `StoryMemory.app`. Der DMG-Schritt kann auf diesem Volume beim Nachbereiten scheitern; der App-Build selbst ist davon nicht betroffen.

## Lokale Datenhaltung

Die kanonische Datenhaltung läuft in SQLite im Rust/Tauri-Layer. Die Datenbank wird bei der ersten App-Ausführung im Tauri-App-Datenverzeichnis unter `storymemory.sqlite3` angelegt. Die versionierten Migrationen liegen in `migrations/001_initial.sql` bis `migrations/005_normalize_scene_version_numbers.sql`. Der Browser-Preview nutzt zusätzlich einen kleinen `localStorage`-Fallback, damit die UI ohne Desktop-Runtime navigierbar bleibt.

Die Datenbank enthält Projekte, Bücher, Kapitel, Szenen, Versionen, Story-Entitäten, Relationen, Fakten, Figurenwissen, Timeline-Ereignisse, Handlungsstränge, Hinweise, Geheimnisse, Korrekturresultate, Analysejobs, Provider-Einstellungen und App-Einstellungen. Provider-Tokens werden nicht in SQLite gespeichert. Story-Bible-Altlasten werden in Migration 004 nur bei genau einem vorhandenen Projekt automatisch zugeordnet; mehrdeutige Zeilen bleiben sichtbar unzugeordnet und werden nicht geraten.

## Architektur

```text
React/TypeScript UI
  ├─ features: editor, chat, story-bible, timeline, mindmap, projects
  ├─ services: localStore, providerBridge, correctionService
  └─ zustand: View-/Panelzustand
Tauri Commands
  ├─ Rust services/providers/imports
  └─ rusqlite + migrations
```

Die UI kennt keine SQL-Abfragen. Sie verwendet Service-Funktionen, die im Desktop-Modus Tauri-Commands aufrufen und im Preview lokal persistieren.

## Provider-System

`AiProvider` ist die interne austauschbare Schnittstelle. Aktiv ist der `MockProvider`. Für Codex CLI, Claude CLI, Grok Build, Gemini CLI und lokale Modelle existieren Platzhalter-Adapter mit bewusst nicht implementierter Prozessausführung. Später werden ausschließlich offiziell dokumentierte lokale Clients, ACP, JSON-RPC oder CLI-Modi verwendet. Keine Cookies, privaten Sessiontokens oder Web-UI-Automatisierung.

## Rechtschreibung und LanguageTool

`CorrectionService` unterscheidet spelling, grammar, punctuation, capitalization und whitespace. Stilumschreibung, Satzumstellung, Inhaltsänderung und Synonymersetzung sind ausgeschlossen. `MockCorrectionService` liefert lokale Beispieldiffs; `LocalLanguageToolProvider` meldet derzeit verständlich, wenn kein lokaler Server erreichbar ist. Ein späterer Adapter kann ausschließlich einen lokal betriebenen LanguageTool-Server ansprechen.

## Aktuelle Funktionen

- Dashboard mit Beispielprojekt „Zugestellt“
- navigierbarer Manuskripteditor mit Kapitel-/Szenenbaum, Autosave und Metadaten
- lokale SQLite-Migration und Rust-Commands für Projekte, Szenen und Story-Bible-Einträge
- getrennte Szenen-Historie: Autosave erzeugt keine Version pro Tastaturpause; bewusste Versionen werden lokal mit Grund gespeichert
- Chat-Prototyp mit Mock-Antworten und Quellenchips
- Story-Bible-Liste, Suche, Filter, Status, Vertrauen und Quelldetail
- Timeline mit Spuren, Filtern, Zoom, Auswahl und Detailpanel
- interaktive Mindmap mit Zoom, Knotenauswahl, Filtern und Beziehungen
- Bible-Update-Review, Importdialog und Deep-Research-Jobdialog als lokale Mock-Workflows
- Correction-Diff-Service, Chunking-Grundlage, Datenschutz-Hinweis

## Noch nicht implementiert

Echte KI-Provider, DOCX/EPUB-Import, vollständige Rich-Text-Formatierung, semantische Suche, echte LanguageTool-Kommunikation, robuste Drag-and-drop-Reihenfolge, Produktions-Backups, signierte Updates, Cloud-Sync, Konten und Kollaboration.

## Nächste Schritte

1. mehrdeutige alte Story-Bible-Einträge mit einem manuellen Zuordnungsdialog absichern.
2. echten lokalen LanguageTool-Healthcheck und Diff-Review ergänzen.
3. Tauri-Provider-Prozessrunner mit Freigabe-Snapshot und strukturierten JSON-Patches bauen.
4. automatische Checkpoints nur als deutlich langsamen, optionalen Mechanismus ergänzen.

Weitere Architektur- und Modellnotizen stehen unter `docs/`.
