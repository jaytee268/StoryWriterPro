# Architektur

StoryMemory ist eine lokale Desktop-App. Tauri 2 stellt das Fenster und die IPC-Grenze bereit. React/TypeScript rendert die Oberfläche. Rust kapselt Dateisystem, SQLite, Migrationen und spätere lokale Prozess-Bridges.

Der Frontend-Fluss ist `Feature → Service → Tauri Command`. Komponenten führen keine SQL-Abfragen aus. `localStore` hält im Browser-Preview einen lokalen Fallback und delegiert in der Desktop-Runtime an Rust.

Die kanonische Datenhaltung bleibt relational. Flexible Analysepayloads werden als versioniertes JSON in Analyse- und Korrekturtabellen gespeichert. Der Originaltext bleibt die höchste Beweisquelle; strukturierte Entitäten und Quellen zeigen nur nachvollziehbare Ableitungen.

## Sicherheitsgrenzen

- Provider-Prozesse erhalten später nur einen schreibgeschützten, temporären Projektsnapshot.
- Vorschläge werden validiert und erst nach Autor-Bestätigung persistiert.
- Keine Provider-Zugangsdaten oder Sessiontokens in SQLite.
- Keine Browser-Cookies und keine Web-UI-Automatisierung.
