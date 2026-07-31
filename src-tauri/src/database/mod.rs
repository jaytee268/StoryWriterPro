use rusqlite::{params, Connection, Result};
use std::{fs, path::PathBuf, sync::Mutex};
use tauri::{AppHandle, Manager};

pub struct DbState {
    pub connection: Mutex<Connection>,
}

impl DbState {
    pub fn open(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let data_dir: PathBuf = app.path().app_data_dir()?;
        fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("storymemory.sqlite3");
        let connection = Connection::open(db_path)?;
        connection.execute_batch(include_str!("../../../migrations/001_initial.sql"))?;
        seed(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
}

fn seed(connection: &Connection) -> Result<()> {
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))?;
    if count == 0 {
        connection.execute(
            "INSERT INTO projects (id, title, author, description) VALUES (?1, ?2, ?3, ?4)",
            params![
                "project-zugestellt",
                "Zugestellt",
                "Mara König",
                "Lokales Demo-Projekt"
            ],
        )?;
        connection.execute(
            "INSERT INTO books (id, project_id, title, volume) VALUES (?1, ?2, ?3, ?4)",
            params!["book-1", "project-zugestellt", "Zugestellt", 1],
        )?;
        for (chapter_id, chapter_title, order_index, scene_id, scene_title) in [
            (
                "chapter-1",
                "Kapitel 1 – Zugestellt",
                1,
                "scene-1",
                "Die Zustellung",
            ),
            (
                "chapter-2",
                "Kapitel 2 – Das Foto",
                2,
                "scene-2",
                "Silbergelatine",
            ),
            (
                "chapter-3",
                "Kapitel 3 – Abweichung",
                3,
                "scene-3",
                "Die zweite Nummer",
            ),
        ] {
            connection.execute(
                "INSERT INTO chapters (id, book_id, title, order_index) VALUES (?1, 'book-1', ?2, ?3)",
                params![chapter_id, chapter_title, order_index],
            )?;
            connection.execute(
                "INSERT INTO scenes (id, chapter_id, title, order_index, pov, location, story_time, status) VALUES (?1, ?2, ?3, 1, 'Marek', '', '', 'draft')",
                params![scene_id, chapter_id, scene_title],
            )?;
        }
    }
    Ok(())
}
