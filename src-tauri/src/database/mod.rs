use rusqlite::{params, Connection, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Manager};

pub struct DbState {
    pub connection: Mutex<Connection>,
    pub path: PathBuf,
}

pub(crate) fn has_column(connection: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>>>()?;
    Ok(columns.iter().any(|name| name == column))
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    if !has_column(connection, table, column)? {
        connection.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}

fn ensure_story_bible_review_schema(connection: &Connection) -> Result<()> {
    // Migration 006 used ALTER TABLE directly. Checking the schema first makes
    // a partially applied upgrade safe to resume on the next app start.
    ensure_column(
        connection,
        "story_entities",
        "origin",
        "TEXT NOT NULL DEFAULT 'manual'",
    )?;
    ensure_column(
        connection,
        "story_entities",
        "tags_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS bible_update_runs (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
            scene_updated_at TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            extractor_id TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('pending', 'running', 'completed', 'failed', 'reviewed')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            completed_at TEXT,
            error_message TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_bible_update_runs_scene_hash
          ON bible_update_runs(scene_id, content_hash, status);
        CREATE TABLE IF NOT EXISTS bible_proposals (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL REFERENCES bible_update_runs(id) ON DELETE CASCADE,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
            target_entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL,
            proposal_action TEXT NOT NULL CHECK(proposal_action IN ('create_entity', 'update_entity', 'add_source', 'mark_contradiction', 'create_open_question', 'create_author_note')),
            entity_type TEXT NOT NULL,
            candidate_name TEXT NOT NULL,
            candidate_description TEXT NOT NULL,
            candidate_status TEXT NOT NULL,
            confidence REAL NOT NULL,
            classification TEXT NOT NULL CHECK(classification IN ('observable_fact', 'interpretation', 'open_question', 'possible_contradiction', 'author_note')),
            evidence_excerpt TEXT NOT NULL,
            start_offset INTEGER,
            end_offset INTEGER,
            reason TEXT NOT NULL,
            review_status TEXT NOT NULL CHECK(review_status IN ('pending', 'accepted', 'edited', 'rejected')),
            reviewed_at TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_bible_proposals_run_status
          ON bible_proposals(run_id, review_status);
        CREATE TABLE IF NOT EXISTS story_source_references (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            entity_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL,
            proposal_id TEXT REFERENCES bible_proposals(id) ON DELETE SET NULL,
            chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
            scene_id TEXT NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
            excerpt TEXT NOT NULL DEFAULT '',
            start_offset INTEGER,
            end_offset INTEGER,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_story_sources_entity ON story_source_references(entity_id);
        CREATE INDEX IF NOT EXISTS idx_story_sources_scene ON story_source_references(scene_id);",
    )?;
    Ok(())
}

fn ensure_lore_completion_schema(connection: &Connection) -> Result<()> {
    ensure_column(
        connection,
        "lore_metadata",
        "category",
        "TEXT NOT NULL DEFAULT 'objective_truth'",
    )?;
    ensure_column(
        connection,
        "lore_metadata",
        "scope",
        "TEXT NOT NULL DEFAULT 'book'",
    )?;
    ensure_column(
        connection,
        "lore_metadata",
        "reveal_state",
        "TEXT NOT NULL DEFAULT 'author_only'",
    )?;
    ensure_column(
        connection,
        "lore_metadata",
        "importance",
        "TEXT NOT NULL DEFAULT 'supporting'",
    )?;
    ensure_column(
        connection,
        "style_references",
        "chapter_id",
        "TEXT REFERENCES chapters(id) ON DELETE CASCADE",
    )?;
    ensure_column(connection, "style_references", "start_offset", "INTEGER")?;
    ensure_column(connection, "style_references", "end_offset", "INTEGER")?;
    ensure_column(
        connection,
        "style_references",
        "category",
        "TEXT NOT NULL DEFAULT 'general'",
    )?;
    ensure_column(
        connection,
        "style_references",
        "weight",
        "REAL NOT NULL DEFAULT 1.0",
    )?;
    connection.execute_batch(include_str!(
        "../../../migrations/009_complete_lore_links_and_style_references.sql"
    ))?;
    connection.execute_batch(
        "UPDATE lore_metadata SET category=CASE truth_scope WHEN 'planned_reveal' THEN 'mystery' ELSE 'objective_truth' END, reveal_state=CASE truth_scope WHEN 'reader_revealed' THEN 'reader_revealed' WHEN 'planned_reveal' THEN 'foreshadowed' ELSE 'author_only' END WHERE category IS NULL OR category='objective_truth' AND truth_scope <> 'world_truth';
         UPDATE style_references SET chapter_id=(SELECT chapter_id FROM scenes WHERE scenes.id=style_references.scene_id) WHERE chapter_id IS NULL;",
    )?;
    Ok(())
}

fn ensure_character_memory_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(include_str!(
        "../../../migrations/010_character_memory_graph.sql"
    ))
}

impl DbState {
    pub fn open(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let data_dir = app.path().app_data_dir()?;
        fs::create_dir_all(&data_dir)?;
        let path = data_dir.join("storymemory.sqlite3");
        let connection = Connection::open(&path)?;
        initialize_connection(&connection)?;
        seed_if_empty(&connection)?;
        ensure_initial_scene_versions(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            path,
        })
    }
}

pub fn initialize_connection(connection: &Connection) -> Result<()> {
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    connection.execute_batch(include_str!("../../../migrations/001_initial.sql"))?;
    connection.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);")?;
    let has_initial: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 1",
        [],
        |row| row.get(0),
    )?;
    if has_initial == 0 {
        connection.execute("INSERT INTO schema_migrations (version) VALUES (1)", [])?;
    }
    let has_workspace: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 2",
        [],
        |row| row.get(0),
    )?;
    if has_workspace == 0 {
        connection.execute_batch(include_str!(
            "../../../migrations/002_workspace_indexes.sql"
        ))?;
        connection.execute("INSERT INTO schema_migrations (version) VALUES (2)", [])?;
    }
    let has_scene_versions: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 3",
        [],
        |row| row.get(0),
    )?;
    if has_scene_versions == 0 {
        connection.execute_batch(include_str!(
            "../../../migrations/003_scene_version_snapshots.sql"
        ))?;
        connection.execute("INSERT INTO schema_migrations (version) VALUES (3)", [])?;
    }
    let has_data_safety: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 4",
        [],
        |row| row.get(0),
    )?;
    if has_data_safety == 0 {
        connection.execute_batch(include_str!("../../../migrations/004_data_safety.sql"))?;
        connection.execute("INSERT INTO schema_migrations (version) VALUES (4)", [])?;
    }
    let has_normalized_versions: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 5",
        [],
        |row| row.get(0),
    )?;
    if has_normalized_versions == 0 {
        connection.execute_batch(include_str!(
            "../../../migrations/005_normalize_scene_version_numbers.sql"
        ))?;
        connection.execute("INSERT INTO schema_migrations (version) VALUES (5)", [])?;
    }
    let has_story_bible_review: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 6",
        [],
        |row| row.get(0),
    )?;
    ensure_story_bible_review_schema(connection)?;
    if has_story_bible_review == 0 {
        connection.execute("INSERT INTO schema_migrations (version) VALUES (6)", [])?;
    }
    let has_analysis_snapshots: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 7",
        [],
        |row| row.get(0),
    )?;
    if has_analysis_snapshots == 0 {
        ensure_column(
            connection,
            "bible_update_runs",
            "analyzed_content",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        connection.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_bible_update_runs_scene_extractor_created
             ON bible_update_runs(scene_id, extractor_id, created_at DESC);",
        )?;
        connection.execute("INSERT INTO schema_migrations (version) VALUES (7)", [])?;
    }
    let has_lore_foundations: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 8",
        [],
        |row| row.get(0),
    )?;
    if has_lore_foundations == 0 {
        connection.execute_batch(include_str!(
            "../../../migrations/008_lore_character_style_foundations.sql"
        ))?;
        connection.execute("INSERT INTO schema_migrations (version) VALUES (8)", [])?;
    }
    let has_lore_completion: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 9",
        [],
        |row| row.get(0),
    )?;
    if has_lore_completion == 0 {
        ensure_lore_completion_schema(connection)?;
        connection.execute("INSERT INTO schema_migrations (version) VALUES (9)", [])?;
    } else {
        // A partially applied migration can be resumed safely.
        ensure_lore_completion_schema(connection)?;
    }
    let has_character_memory: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 10",
        [],
        |row| row.get(0),
    )?;
    ensure_character_memory_schema(connection)?;
    if has_character_memory == 0 {
        connection.execute("INSERT INTO schema_migrations (version) VALUES (10)", [])?;
    }
    let has_longform = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 11",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    connection.execute_batch(include_str!(
        "../../../migrations/011_longform_chapter_drafting.sql"
    ))?;
    if has_longform == 0 {
        connection.execute("INSERT INTO schema_migrations (version) VALUES (11)", [])?;
    }
    let has_character_longform = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 12",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    // Migration 012 is additive.  Column checks make a partially applied
    // upgrade resumable even on SQLite versions without ADD COLUMN IF NOT EXISTS.
    ensure_column(
        connection,
        "character_voice_patterns",
        "first_observed_scene_id",
        "TEXT REFERENCES scenes(id) ON DELETE SET NULL",
    )?;
    ensure_column(
        connection,
        "character_voice_patterns",
        "last_observed_scene_id",
        "TEXT REFERENCES scenes(id) ON DELETE SET NULL",
    )?;
    ensure_column(
        connection,
        "character_voice_patterns",
        "retired_scene_id",
        "TEXT REFERENCES scenes(id) ON DELETE SET NULL",
    )?;
    ensure_column(
        connection,
        "character_memory_proposals",
        "analyzed_content_hash",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "character_memory_proposals",
        "accepted_memory_id",
        "TEXT",
    )?;
    ensure_column(
        connection,
        "character_memory_proposals",
        "accepted_memory_kind",
        "TEXT",
    )?;
    ensure_column(
        connection,
        "character_memory_update_runs",
        "analyzed_content",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "chapter_generation_jobs",
        "context_override_accepted",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        connection,
        "chapter_generation_jobs",
        "last_resumed_at",
        "TEXT",
    )?;
    if has_character_longform == 0 {
        connection.execute("INSERT INTO schema_migrations (version) VALUES (12)", [])?;
    }
    let has_knowledge_intervals: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 13",
        [],
        |row| row.get(0),
    )?;
    // Migration 013 is intentionally resumable. The interval columns make
    // the historical meaning explicit without rewriting existing knowledge.
    ensure_column(
        connection,
        "character_knowledge_states",
        "effective_from_scene_id",
        "TEXT REFERENCES scenes(id) ON DELETE SET NULL",
    )?;
    ensure_column(
        connection,
        "character_knowledge_states",
        "effective_until_scene_id",
        "TEXT REFERENCES scenes(id) ON DELETE SET NULL",
    )?;
    ensure_column(
        connection,
        "character_knowledge_history",
        "effective_from_scene_id",
        "TEXT REFERENCES scenes(id) ON DELETE SET NULL",
    )?;
    ensure_column(
        connection,
        "character_knowledge_history",
        "effective_until_scene_id",
        "TEXT REFERENCES scenes(id) ON DELETE SET NULL",
    )?;
    connection.execute(
        "UPDATE character_knowledge_states SET effective_from_scene_id=COALESCE(effective_from_scene_id, changed_scene_id, acquired_scene_id) WHERE effective_from_scene_id IS NULL",
        [],
    )?;
    connection.execute(
        "UPDATE character_knowledge_history SET effective_until_scene_id=COALESCE(effective_until_scene_id, scene_id) WHERE effective_until_scene_id IS NULL",
        [],
    )?;
    if has_knowledge_intervals == 0 {
        connection.execute("INSERT INTO schema_migrations (version) VALUES (13)", [])?;
    }
    let has_continuity_graph: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 14",
        [],
        |row| row.get(0),
    )?;
    connection.execute_batch(include_str!(
        "../../../migrations/014_continuity_rule_graph.sql"
    ))?;
    // If a desktop upgrade was interrupted after creating the tables, the
    // additive JSON columns are still completed on the next startup.
    ensure_column(
        connection,
        "project_rules",
        "source_reference_ids_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_column(
        connection,
        "project_rule_proposals",
        "source_reference_ids_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    if has_continuity_graph == 0 {
        connection.execute("INSERT INTO schema_migrations (version) VALUES (14)", [])?;
    }
    let has_incremental_review: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 15",
        [],
        |row| row.get(0),
    )?;
    connection.execute_batch(include_str!(
        "../../../migrations/015_incremental_continuity_review.sql"
    ))?;
    if has_incremental_review == 0 {
        connection.execute("INSERT INTO schema_migrations (version) VALUES (15)", [])?;
    }
    let has_ai_continuity_evidence: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 16",
        [],
        |row| row.get(0),
    )?;
    ensure_column(
        connection,
        "continuity_review_findings",
        "counter_evidence_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_column(
        connection,
        "continuity_review_findings",
        "confidence",
        "REAL NOT NULL DEFAULT 0.5 CHECK(confidence BETWEEN 0 AND 1)",
    )?;
    connection.execute_batch(include_str!(
        "../../../migrations/016_ai_continuity_evidence.sql"
    ))?;
    if has_ai_continuity_evidence == 0 {
        connection.execute("INSERT INTO schema_migrations (version) VALUES (16)", [])?;
    }
    let has_resumable_manuscript_analysis: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 17",
        [],
        |row| row.get(0),
    )?;
    if has_resumable_manuscript_analysis == 0 {
        connection.execute_batch(include_str!(
            "../../../migrations/017_resumable_manuscript_analysis.sql"
        ))?;
        connection.execute("INSERT INTO schema_migrations (version) VALUES (17)", [])?;
    }
    let has_continuity_sources_temporal_context: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = 18",
        [],
        |row| row.get(0),
    )?;
    if has_continuity_sources_temporal_context == 0 {
        connection.execute_batch(include_str!(
            "../../../migrations/018_continuity_sources_temporal_context.sql"
        ))?;
        connection.execute("INSERT INTO schema_migrations (version) VALUES (18)", [])?;
    }
    Ok(())
}

pub fn seed_if_empty(connection: &Connection) -> Result<()> {
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))?;
    if count != 0 {
        return Ok(());
    }
    let transaction = connection.unchecked_transaction()?;
    transaction.execute("INSERT INTO projects (id, title, author, description) VALUES (?1, ?2, ?3, ?4)", params!["project-zugestellt", "Zugestellt", "Mara König", "Ein psychologischer Mystery-Roman über Erinnerung, Schuld und eine Paketnummer, die nicht existieren dürfte."])?;
    transaction.execute(
        "INSERT INTO books (id, project_id, title, volume) VALUES (?1, ?2, ?3, ?4)",
        params!["book-1", "project-zugestellt", "Zugestellt", 1],
    )?;
    let scenes = [
        ("chapter-1", "Kapitel 1 – Zugestellt", 1, "scene-1", "Die Zustellung", "Der Karton stand vor der Tür, als Marek nach Hause kam. Kein Klingeln. Keine Nachricht. Nur sein Name, sauber gedruckt, und eine Paketnummer, die er sofort erkannte.\n\nEr hob den Deckel nicht an. Noch nicht. Im Treppenhaus roch es nach nassem Stein und dem Parfüm der Nachbarin aus dem dritten Stock.", "Mareks Wohnung", "Montag, 18:40", "Marek", "Marek erhält den Gegenstand, den er längst vergessen wollte.", "Die Spannung entsteht aus dem Zögern vor dem Öffnen."),
        ("chapter-2", "Kapitel 2 – Das Foto", 2, "scene-2", "Silbergelatine", "Auf dem Foto war die Wohnung leer. Nicht verlassen, sondern leer, als hätte jemand die Wände aus der Wirklichkeit geschnitten. Lena stand am Rand des Bildes und sah in eine Richtung, die Marek nicht sehen konnte.", "Küchentisch", "Dienstag, 08:15", "Marek", "Das Foto verbindet Lena mit der unmöglichen Paketnummer.", "Hinweis auf die Simulation nur über Bildfehler."),
        ("chapter-3", "Kapitel 3 – Abweichung", 3, "scene-3", "Die zweite Nummer", "„Du hast die Nummer geändert“, sagte Lena.\n\nMarek sah auf den Aufkleber. Die Zahlen waren dieselben wie gestern. Nur die letzte Stelle schien sich gegen das Licht zu wehren.\n\n„Ich weiß nicht, wovon du sprichst.“\n\nDas war die erste Lüge, die er ihr erzählte.", "Café Meridian", "Dienstag, 13:20", "Marek", "Marek wird mit einer Tatsache konfrontiert, die er noch nicht akzeptieren kann.", "Szene 2 aus dem Konzept: Marek weiß nach dieser Szene von der Änderung."),
    ];
    for (
        chapter_id,
        title,
        order_index,
        scene_id,
        scene_title,
        content,
        location,
        story_time,
        pov,
        goal,
        notes,
    ) in scenes
    {
        transaction.execute(
            "INSERT INTO chapters (id, book_id, title, order_index) VALUES (?1, 'book-1', ?2, ?3)",
            params![chapter_id, title, order_index],
        )?;
        transaction.execute("INSERT INTO scenes (id, chapter_id, title, order_index, content, pov, location, story_time, status, goal, notes) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, 'draft', ?8, ?9)", params![scene_id, chapter_id, scene_title, content, pov, location, story_time, goal, notes])?;
    }
    let entities = [
        ("entity-marek", "Marek", "character", "Vermeidet Entscheidungen, wenn sie eine alte Schuld sichtbar machen könnten. Beobachtet mehr, als er zugibt.", "confirmed", 0.96, "Band 1 · Kapitel 1", "Kapitel 1", "Die Zustellung", 1),
        ("entity-lena", "Lena", "character", "Kennt die Abweichung der Paketnummer und testet, wie weit Marek die gemeinsame Vergangenheit verdrängt.", "confirmed", 0.91, "Band 1 · Kapitel 3", "Kapitel 3", "Die zweite Nummer", 1),
        ("entity-package", "Veränderte Paketnummer", "clue", "Die letzte Stelle verändert sich abhängig vom Blickwinkel. Sie ist ein Hinweis auf die Simulation.", "proposed", 0.78, "Band 1 · Kapitel 2", "Kapitel 2", "Silbergelatine", 0),
        ("entity-simulation", "Die Simulation", "secret", "Eine mögliche Erklärung für die widersprüchlichen Räume und Erinnerungen. Noch nicht als Kanon bestätigt.", "uncertain", 0.64, "Band 1 · Kapitel 2", "Kapitel 2", "Silbergelatine", 0),
    ];
    for (
        id,
        name,
        entity_type,
        description,
        status,
        confidence,
        source,
        chapter,
        scene,
        author_confirmed,
    ) in entities
    {
        transaction.execute("INSERT INTO story_entities (id, project_id, name, entity_type, description, status, confidence, source, chapter, scene, author_confirmed) VALUES (?1, 'project-zugestellt', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)", params![id, name, entity_type, description, status, confidence, source, chapter, scene, author_confirmed])?;
    }
    transaction.commit()
}

pub fn ensure_initial_scene_versions(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("SELECT id, chapter_id, title, order_index, content, pov, location, story_time, status, goal, notes, updated_at FROM scenes WHERE NOT EXISTS (SELECT 1 FROM scene_versions WHERE scene_versions.scene_id = scenes.id)")?;
    let scenes = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
            ))
        })?
        .collect::<Result<Vec<_>>>()?;
    for (
        id,
        chapter_id,
        title,
        order_index,
        content,
        pov,
        location,
        story_time,
        status,
        goal,
        notes,
        updated_at,
    ) in scenes
    {
        let snapshot = serde_json::json!({
            "id": id,
            "chapterId": chapter_id,
            "title": title,
            "orderIndex": order_index,
            "content": content,
            "pov": pov,
            "location": location,
            "storyTime": story_time,
            "status": status,
            "goal": goal,
            "notes": notes,
        });
        connection.execute("INSERT INTO scene_versions (id, scene_id, content, created_at, version_number, snapshot_json, reason) VALUES (?1, ?2, ?3, ?4, 1, ?5, 'automatic_checkpoint')", params![uuid::Uuid::new_v4().to_string(), id, snapshot["content"].as_str().unwrap_or_default(), updated_at, snapshot.to_string()])?;
    }
    Ok(())
}

pub fn database_path_for_test(path: &Path) -> Result<Connection> {
    let connection = Connection::open(path)?;
    initialize_connection(&connection)?;
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "storymemory-{name}-{}.sqlite3",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn migration_runs_with_foreign_keys() {
        let path = temp_path("migration");
        let connection = database_path_for_test(&path).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            18
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM pragma_table_info('scene_versions') WHERE name IN ('version_number', 'snapshot_json', 'reason')", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            3
        );
        for table in [
            "continuity_review_settings",
            "continuity_review_runs",
            "continuity_review_run_statuses",
            "continuity_review_findings",
            "plot_thread_lifecycle",
            "plot_thread_lifecycle_proposals",
            "manuscript_analysis_jobs",
            "manuscript_analysis_units",
            "manuscript_analysis_draft_ledger",
        ] {
            assert_eq!(
                connection
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                        [table],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap(),
                1,
                "missing table {table}"
            );
        }
        drop(connection);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn resumable_analysis_schema_persists_unicode_positions_and_terminal_states() {
        let path = temp_path("resumable-analysis-schema");
        let connection = database_path_for_test(&path).unwrap();
        let job_sql: String = connection
            .query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='manuscript_analysis_jobs'", [], |row| row.get(0))
            .unwrap();
        let unit_sql: String = connection
            .query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='manuscript_analysis_units'", [], |row| row.get(0))
            .unwrap();
        let run_sql: String = connection
            .query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='continuity_review_run_statuses'", [], |row| row.get(0))
            .unwrap();
        assert!(job_sql.contains("page_markers_json"));
        assert!(job_sql.contains("'paused'"));
        assert!(job_sql.contains("'cancelled'"));
        assert!(unit_sql.contains("start_offset"));
        assert!(unit_sql.contains("content_hash"));
        assert!(run_sql.contains("'pending'"));
        assert!(run_sql.contains("'failed'"));
        assert!(run_sql.contains("'cancelled'"));
        drop(connection);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn longform_schema_migrates_after_character_memory() {
        let path = temp_path("longform-schema");
        let connection = database_path_for_test(&path).unwrap();
        for table in [
            "project_story_direction",
            "project_writing_preferences",
            "narrative_summaries",
            "project_style_analysis_runs",
            "project_style_observations",
            "chapter_generation_jobs",
            "chapter_generation_assumptions",
            "chapter_generation_plans",
            "chapter_generation_sections",
            "chapter_generation_reviews",
        ] {
            assert!(connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                    params![table],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap());
        }
        for (table, column) in [
            ("character_voice_patterns", "first_observed_scene_id"),
            ("character_voice_patterns", "last_observed_scene_id"),
            ("character_memory_proposals", "accepted_memory_id"),
            ("character_memory_update_runs", "analyzed_content"),
            ("chapter_generation_jobs", "context_override_accepted"),
            ("character_knowledge_states", "effective_from_scene_id"),
            ("character_knowledge_history", "effective_until_scene_id"),
        ] {
            assert!(has_column(&connection, table, column).unwrap());
        }
        drop(connection);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn seed_is_only_created_once() {
        let path = temp_path("seed");
        let connection = database_path_for_test(&path).unwrap();
        seed_if_empty(&connection).unwrap();
        seed_if_empty(&connection).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM projects", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM scenes", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            3
        );
        drop(connection);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn foreign_keys_reject_unknown_chapter() {
        let path = temp_path("fk");
        let connection = database_path_for_test(&path).unwrap();
        let result = connection.execute(
            "INSERT INTO scenes (id, chapter_id, title) VALUES ('bad', 'missing', 'Fehler')",
            [],
        );
        assert!(result.is_err());
        drop(connection);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn unicode_quotes_and_line_breaks_round_trip() {
        let path = temp_path("unicode");
        let connection = database_path_for_test(&path).unwrap();
        seed_if_empty(&connection).unwrap();
        let text = "Äpfel „sicher“\n\nZeile zwei.";
        connection
            .execute(
                "UPDATE scenes SET content=?1 WHERE id='scene-1'",
                params![text],
            )
            .unwrap();
        assert_eq!(
            connection
                .query_row("SELECT content FROM scenes WHERE id='scene-1'", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
            text
        );
        drop(connection);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn scene_update_round_trips_all_writing_fields() {
        let path = temp_path("scene-update");
        let connection = database_path_for_test(&path).unwrap();
        seed_if_empty(&connection).unwrap();
        connection.execute("UPDATE scenes SET content=?1, pov=?2, location=?3, story_time=?4, status=?5, goal=?6, notes=?7 WHERE id='scene-1'", params!["Neuer Text", "Lena", "Café", "Dienstag", "revised", "Ziel", "Notiz"]).unwrap();
        let values: (String, String, String, String, String, String, String) = connection.query_row("SELECT content, pov, location, story_time, status, goal, notes FROM scenes WHERE id='scene-1'", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))).unwrap();
        assert_eq!(
            values,
            (
                "Neuer Text".into(),
                "Lena".into(),
                "Café".into(),
                "Dienstag".into(),
                "revised".into(),
                "Ziel".into(),
                "Notiz".into()
            )
        );
        drop(connection);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn data_survives_database_close_and_reopen() {
        let path = temp_path("reopen");
        {
            let connection = database_path_for_test(&path).unwrap();
            seed_if_empty(&connection).unwrap();
            connection
                .execute(
                    "UPDATE scenes SET content='Bleibt erhalten' WHERE id='scene-3'",
                    [],
                )
                .unwrap();
        }
        let reopened = database_path_for_test(&path).unwrap();
        assert_eq!(
            reopened
                .query_row("SELECT content FROM scenes WHERE id='scene-3'", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
            "Bleibt erhalten"
        );
        drop(reopened);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn legacy_story_entities_are_backfilled_only_when_unambiguous() {
        let path = temp_path("legacy-entities");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(include_str!("../../../migrations/001_initial.sql"))
                .unwrap();
            connection
                .execute_batch("CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP); INSERT INTO schema_migrations (version) VALUES (1);")
                .unwrap();
            connection
                .execute(
                    "INSERT INTO projects (id, title, author) VALUES ('legacy-project', 'Alt', 'Autor')",
                    [],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO story_entities (id, name, entity_type) VALUES ('legacy-entity', 'Alte Figur', 'character')",
                    [],
                )
                .unwrap();
            initialize_connection(&connection).unwrap();
            assert_eq!(
                connection
                    .query_row(
                        "SELECT project_id FROM story_entities WHERE id='legacy-entity'",
                        [],
                        |row| row.get::<_, String>(0),
                    )
                    .unwrap(),
                "legacy-project"
            );
            assert_eq!(
                connection
                    .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                18
            );
            // Running startup migrations again must not change the assignment
            // or fail on the ALTER TABLE statement.
            initialize_connection(&connection).unwrap();
            assert_eq!(
                connection
                    .query_row(
                        "SELECT project_id FROM story_entities WHERE id='legacy-entity'",
                        [],
                        |row| row.get::<_, String>(0),
                    )
                    .unwrap(),
                "legacy-project"
            );
        }
        let _ = fs::remove_file(path);
    }

    #[test]
    fn ambiguous_legacy_story_entities_are_not_assigned() {
        let path = temp_path("ambiguous-entities");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(include_str!("../../../migrations/001_initial.sql"))
            .unwrap();
        connection
            .execute_batch("CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP); INSERT INTO schema_migrations (version) VALUES (1);")
            .unwrap();
        connection
            .execute_batch("INSERT INTO projects (id, title, author) VALUES ('p1', 'Eins', 'Autor'), ('p2', 'Zwei', 'Autor'); INSERT INTO story_entities (id, name, entity_type) VALUES ('orphan', 'Unklar', 'character');")
            .unwrap();
        initialize_connection(&connection).unwrap();
        let project_id: Option<String> = connection
            .query_row(
                "SELECT project_id FROM story_entities WHERE id='orphan'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(project_id.is_none());
        drop(connection);
        let _ = fs::remove_file(path);
    }
}
