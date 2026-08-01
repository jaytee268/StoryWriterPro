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
        connection.execute("INSERT INTO scene_versions (id, scene_id, content, created_at, version_number, snapshot_json) VALUES (?1, ?2, ?3, ?4, 1, ?5)", params![uuid::Uuid::new_v4().to_string(), id, snapshot["content"].as_str().unwrap_or_default(), updated_at, snapshot.to_string()])?;
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
            3
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM pragma_table_info('scene_versions') WHERE name IN ('version_number', 'snapshot_json')", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            2
        );
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
}
