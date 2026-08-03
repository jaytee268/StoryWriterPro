use crate::{
    database::DbState,
    models::{
        validate_entity_status, validate_proposal_action, validate_proposal_classification,
        validate_review_status, validate_scene_status, validate_scene_version_reason,
        BibleProposal, BibleProposalInput, BibleUpdateRun, Book, Chapter,
        CreateBibleUpdateRunInput, CreateChapterInput, CreateProjectInput, CreateSceneInput,
        CreateSceneVersionInput, CreateSourceReferenceInput, CreateStoryEntityInput, DatabaseInfo,
        EditorPreferences, Project, ProviderStatus, RestoreSceneVersionInput,
        ReviewBibleProposalInput, Scene, SceneInput, SceneVersion, StoryEntity, StoryEntityInput,
        StorySourceReference, UpdateChapterInput, UpdateStoryEntityInput, WorkspaceSnapshot,
    },
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use tauri::State;

fn lock_db<'a>(
    state: &'a State<'a, DbState>,
) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    state
        .connection
        .lock()
        .map_err(|error| format!("SQLite-Verbindung konnte nicht gesperrt werden: {error}"))
}
fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn required(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{label} darf nicht leer sein."))
    } else {
        Ok(())
    }
}
fn sql_error(context: &str, error: impl std::fmt::Display) -> String {
    format!("{context}: {error}")
}

fn project_from_db(db: &Connection, project_id: &str) -> Result<Project, String> {
    db.query_row(
        "SELECT id, title, author, description, created_at, updated_at FROM projects WHERE id=?1",
        params![project_id],
        |row| {
            Ok(Project {
                id: row.get(0)?,
                title: row.get(1)?,
                author: row.get(2)?,
                description: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                word_count: 0,
                open_warnings: 0,
                bible_progress: 0,
            })
        },
    )
    .map_err(|error| sql_error("Projekt konnte nicht geladen werden", error))
}

fn book_from_row(row: &rusqlite::Row<'_>) -> SqlResult<Book> {
    Ok(Book {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        volume: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn scene_from_row(row: &rusqlite::Row<'_>) -> SqlResult<Scene> {
    Ok(Scene {
        id: row.get(0)?,
        chapter_id: row.get(1)?,
        title: row.get(2)?,
        order_index: row.get(3)?,
        content: row.get(4)?,
        pov: row.get(5)?,
        location: row.get(6)?,
        story_time: row.get(7)?,
        status: row.get(8)?,
        goal: row.get(9)?,
        notes: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn load_scene(db: &Connection, scene_id: &str) -> Result<Scene, String> {
    db.query_row("SELECT id, chapter_id, title, order_index, content, pov, location, story_time, status, goal, notes, created_at, updated_at FROM scenes WHERE id=?1", params![scene_id], scene_from_row).map_err(|error| sql_error("Szene konnte nicht geladen werden", error))
}

fn scene_input_from_scene(scene: &Scene) -> SceneInput {
    SceneInput {
        id: scene.id.clone(),
        chapter_id: scene.chapter_id.clone(),
        title: scene.title.clone(),
        order_index: scene.order_index,
        content: scene.content.clone(),
        pov: scene.pov.clone(),
        location: scene.location.clone(),
        story_time: scene.story_time.clone(),
        status: scene.status.clone(),
        goal: scene.goal.clone(),
        notes: scene.notes.clone(),
    }
}

fn update_scene_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    input: &SceneInput,
    timestamp: &str,
) -> Result<(), String> {
    let changed = transaction
        .execute("UPDATE scenes SET chapter_id=?2, title=?3, order_index=?4, content=?5, pov=?6, location=?7, story_time=?8, status=?9, goal=?10, notes=?11, updated_at=?12 WHERE id=?1", params![input.id, input.chapter_id, input.title.trim(), input.order_index, input.content, input.pov, input.location, input.story_time, input.status, input.goal, input.notes, timestamp])
        .map_err(|error| sql_error("Szene konnte nicht gespeichert werden", error))?;
    if changed == 0 {
        return Err("Die Szene wurde nicht gefunden.".into());
    }
    transaction
        .execute(
            "UPDATE chapters SET updated_at=?1 WHERE id=?2",
            params![timestamp, input.chapter_id],
        )
        .map_err(|error| sql_error("Kapitelzeitpunkt konnte nicht aktualisiert werden", error))?;
    transaction
        .execute(
            "UPDATE projects SET updated_at=?1 WHERE id=(SELECT books.project_id FROM books JOIN chapters ON chapters.book_id=books.id WHERE chapters.id=?2)",
            params![timestamp, input.chapter_id],
        )
        .map_err(|error| sql_error("Projektzeitpunkt konnte nicht aktualisiert werden", error))?;
    Ok(())
}

fn insert_scene_version_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    scene: &Scene,
    timestamp: &str,
    reason: &str,
) -> Result<(String, i64), String> {
    validate_scene_version_reason(reason)?;
    let input = scene_input_from_scene(scene);
    let snapshot_json = serde_json::to_string(&input)
        .map_err(|error| sql_error("Szenenversion konnte nicht vorbereitet werden", error))?;
    let id = new_id();
    let version_number: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(version_number), 0) + 1 FROM scene_versions WHERE scene_id=?1",
            params![scene.id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Versionsnummer konnte nicht ermittelt werden", error))?;
    transaction
        .execute(
            "INSERT INTO scene_versions (id, scene_id, content, reason, created_at, version_number, snapshot_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, scene.id, scene.content, reason, timestamp, version_number, snapshot_json],
        )
        .map_err(|error| sql_error("Szenenversion konnte nicht gespeichert werden", error))?;
    Ok((id, version_number))
}

fn create_scene_version_in_db(
    db: &Connection,
    scene_id: &str,
    reason: &str,
) -> Result<SceneVersion, String> {
    validate_scene_version_reason(reason)?;
    let scene = load_scene(db, scene_id)?;
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Versionsspeicherung konnte nicht gestartet werden", error))?;
    let timestamp = now();
    let (version_id, _) =
        insert_scene_version_in_transaction(&transaction, &scene, &timestamp, reason)?;
    transaction.commit().map_err(|error| {
        sql_error(
            "Versionsspeicherung konnte nicht abgeschlossen werden",
            error,
        )
    })?;
    load_scene_versions(db, scene_id)?
        .into_iter()
        .find(|version| version.id == version_id)
        .ok_or_else(|| "Die gespeicherte Version konnte nicht geladen werden.".into())
}

fn load_scene_versions(db: &Connection, scene_id: &str) -> Result<Vec<SceneVersion>, String> {
    let current = load_scene(db, scene_id)?;
    let mut statement = db
        .prepare("SELECT id, scene_id, content, reason, created_at, version_number, snapshot_json FROM scene_versions WHERE scene_id=?1 ORDER BY created_at DESC, version_number DESC")
        .map_err(|error| sql_error("Verlauf konnte nicht geladen werden", error))?;
    let rows = statement
        .query_map(params![scene_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|error| sql_error("Verlauf konnte nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Verlauf konnte nicht geladen werden", error))?;
    let total = rows.len();
    rows.into_iter()
        .enumerate()
        .map(
            |(
                index,
                (id, version_scene_id, content, reason, created_at, version_number, snapshot_json),
            )| {
                let mut scene = current.clone();
                if let Ok(snapshot) = serde_json::from_str::<SceneInput>(&snapshot_json) {
                    scene.id = snapshot.id;
                    scene.chapter_id = snapshot.chapter_id;
                    scene.title = snapshot.title;
                    scene.order_index = snapshot.order_index;
                    scene.pov = snapshot.pov;
                    scene.location = snapshot.location;
                    scene.story_time = snapshot.story_time;
                    scene.status = snapshot.status;
                    scene.goal = snapshot.goal;
                    scene.notes = snapshot.notes;
                }
                scene.content = content.clone();
                scene.updated_at = created_at.clone();
                Ok(SceneVersion {
                    id,
                    scene_id: version_scene_id,
                    version_number: if version_number > 0 {
                        version_number
                    } else {
                        (total - index) as i64
                    },
                    content,
                    reason,
                    created_at,
                    scene,
                })
            },
        )
        .collect()
}

fn load_chapter(db: &Connection, chapter_id: &str) -> Result<Chapter, String> {
    let mut chapter = db.query_row("SELECT id, book_id, title, order_index, created_at, updated_at FROM chapters WHERE id=?1", params![chapter_id], |row| Ok(Chapter { id: row.get(0)?, book_id: row.get(1)?, title: row.get(2)?, order_index: row.get(3)?, scenes: Vec::new(), created_at: row.get(4)?, updated_at: row.get(5)? })).map_err(|error| sql_error("Kapitel konnte nicht geladen werden", error))?;
    let mut statement = db.prepare("SELECT id, chapter_id, title, order_index, content, pov, location, story_time, status, goal, notes, created_at, updated_at FROM scenes WHERE chapter_id=?1 ORDER BY order_index, created_at").map_err(|error| sql_error("Szenen konnten nicht geladen werden", error))?;
    chapter.scenes = statement
        .query_map(params![chapter_id], scene_from_row)
        .map_err(|error| sql_error("Szenen konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Szenen konnten nicht geladen werden", error))?;
    Ok(chapter)
}

fn load_books(db: &Connection, project_id: &str) -> Result<Vec<Book>, String> {
    let mut statement = db.prepare("SELECT id, project_id, title, volume, created_at, updated_at FROM books WHERE project_id=?1 ORDER BY volume, created_at").map_err(|error| sql_error("Bücher konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![project_id], book_from_row)
        .map_err(|error| sql_error("Bücher konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Bücher konnten nicht geladen werden", error));
    result
}

fn load_chapters(db: &Connection, books: &[Book]) -> Result<Vec<Chapter>, String> {
    let mut chapters = Vec::new();
    for book in books {
        let mut statement = db
            .prepare("SELECT id FROM chapters WHERE book_id=?1 ORDER BY order_index, created_at")
            .map_err(|error| sql_error("Kapitel konnten nicht geladen werden", error))?;
        let ids = statement
            .query_map(params![book.id], |row| row.get::<_, String>(0))
            .map_err(|error| sql_error("Kapitel konnten nicht geladen werden", error))?
            .collect::<SqlResult<Vec<_>>>()
            .map_err(|error| sql_error("Kapitel konnten nicht geladen werden", error))?;
        for id in ids {
            chapters.push(load_chapter(db, &id)?);
        }
    }
    Ok(chapters)
}

fn entity_from_row(row: &rusqlite::Row<'_>) -> SqlResult<StoryEntity> {
    let tags_json: String = row.get(14)?;
    Ok(StoryEntity {
        id: row.get(0)?,
        project_id: row.get(1)?,
        name: row.get(2)?,
        entity_type: row.get(3)?,
        description: row.get(4)?,
        status: row.get(5)?,
        confidence: row.get(6)?,
        source: row.get(7)?,
        chapter: row.get(8)?,
        scene: row.get(9)?,
        author_confirmed: row.get::<_, i64>(10)? != 0,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        origin: row.get(13)?,
    })
}

fn load_entities(db: &Connection, project_id: &str) -> Result<Vec<StoryEntity>, String> {
    let mut statement = db.prepare("SELECT id, project_id, name, entity_type, description, status, confidence, source, chapter, scene, author_confirmed, created_at, updated_at, origin, tags_json FROM story_entities WHERE project_id=?1 ORDER BY updated_at DESC").map_err(|error| sql_error("Story-Bible-Einträge konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![project_id], entity_from_row)
        .map_err(|error| sql_error("Story-Bible-Einträge konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Story-Bible-Einträge konnten nicht geladen werden", error));
    result
}

fn word_count(chapters: &[Chapter]) -> i64 {
    chapters
        .iter()
        .flat_map(|chapter| chapter.scenes.iter())
        .map(|scene| scene.content.split_whitespace().count() as i64)
        .sum()
}

#[tauri::command]
pub fn load_workspace(state: State<'_, DbState>) -> Result<WorkspaceSnapshot, String> {
    let db = lock_db(&state)?;
    let project_id: String = db
        .query_row(
            "SELECT id FROM projects ORDER BY updated_at DESC, created_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| sql_error("Workspace konnte nicht geladen werden", error))?
        .ok_or_else(|| "Keine lokale StoryMemory-Datenbank mit Projekt gefunden.".to_string())?;
    let books = load_books(&db, &project_id)?;
    let chapters = load_chapters(&db, &books)?;
    let entities = load_entities(&db, &project_id)?;
    let mut project = project_from_db(&db, &project_id)?;
    project.word_count = word_count(&chapters);
    project.open_warnings = entities
        .iter()
        .filter(|entity| entity.status == "contradicted")
        .count() as i64;
    project.bible_progress = if entities.is_empty() {
        0
    } else {
        ((entities
            .iter()
            .filter(|entity| entity.status == "confirmed")
            .count() as f64
            / entities.len() as f64)
            * 100.0)
            .round() as i64
    };
    Ok(WorkspaceSnapshot {
        project,
        books,
        chapters,
        entities,
    })
}

pub(crate) fn create_project_in_db(
    db: &Connection,
    input: CreateProjectInput,
) -> Result<Project, String> {
    required(&input.title, "Der Projekttitel")?;
    required(&input.author, "Der Autorenname")?;
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Projekttransaktion konnte nicht gestartet werden", error))?;
    let project_id = new_id();
    let book_id = new_id();
    let timestamp = now();
    let description = if input.description.trim().is_empty() {
        "Neues lokales StoryMemory-Projekt"
    } else {
        input.description.as_str()
    };
    let book_title = if input.volume_title.trim().is_empty() {
        input.title.as_str()
    } else {
        input.volume_title.as_str()
    };
    transaction.execute("INSERT INTO projects (id, title, author, description, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)", params![project_id, input.title.trim(), input.author.trim(), description, timestamp]).map_err(|error| sql_error("Projekt konnte nicht gespeichert werden", error))?;
    transaction.execute("INSERT INTO books (id, project_id, title, volume, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)", params![book_id, project_id, book_title, input.volume.max(1), timestamp]).map_err(|error| sql_error("Band konnte nicht gespeichert werden", error))?;
    transaction.commit().map_err(|error| {
        sql_error(
            "Projekttransaktion konnte nicht abgeschlossen werden",
            error,
        )
    })?;
    project_from_db(db, &project_id)
}

#[tauri::command]
pub fn create_project(
    state: State<'_, DbState>,
    input: CreateProjectInput,
) -> Result<Project, String> {
    let db = lock_db(&state)?;
    create_project_in_db(&db, input)
}

pub(crate) fn create_chapter_in_db(
    db: &Connection,
    input: CreateChapterInput,
) -> Result<Chapter, String> {
    required(&input.title, "Der Kapitelname")?;
    let timestamp = now();
    let book_exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM books WHERE id=?1)",
            params![input.book_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Buch konnte nicht geprüft werden", error))?;
    if !book_exists {
        return Err("Das ausgewählte Buch wurde nicht gefunden.".into());
    }
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Kapiteltransaktion konnte nicht gestartet werden", error))?;
    let id = new_id();
    transaction
        .execute("INSERT INTO chapters (id, book_id, title, order_index, created_at, updated_at) VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(order_index), 0) + 1 FROM chapters WHERE book_id=?2), ?4, ?4)", params![id, input.book_id, input.title.trim(), timestamp])
        .map_err(|error| sql_error("Kapitel konnte nicht gespeichert werden", error))?;
    transaction.commit().map_err(|error| {
        sql_error(
            "Kapiteltransaktion konnte nicht abgeschlossen werden",
            error,
        )
    })?;
    load_chapter(db, &id)
}

#[tauri::command]
pub fn create_chapter(
    state: State<'_, DbState>,
    input: CreateChapterInput,
) -> Result<Chapter, String> {
    let db = lock_db(&state)?;
    create_chapter_in_db(&db, input)
}

pub(crate) fn update_chapter_in_db(
    db: &Connection,
    input: UpdateChapterInput,
) -> Result<Chapter, String> {
    required(&input.title, "Der Kapitelname")?;
    let timestamp = now();
    let changed = db
        .execute(
            "UPDATE chapters SET title=?2, updated_at=?3 WHERE id=?1",
            params![input.id, input.title.trim(), timestamp],
        )
        .map_err(|error| sql_error("Kapitel konnte nicht aktualisiert werden", error))?;
    if changed == 0 {
        return Err("Das Kapitel wurde nicht gefunden.".into());
    }
    load_chapter(db, &input.id)
}

#[tauri::command]
pub fn update_chapter(
    state: State<'_, DbState>,
    input: UpdateChapterInput,
) -> Result<Chapter, String> {
    let db = lock_db(&state)?;
    update_chapter_in_db(&db, input)
}

pub(crate) fn create_scene_in_db(
    db: &Connection,
    input: CreateSceneInput,
) -> Result<Scene, String> {
    required(&input.title, "Der Szenenname")?;
    let chapter_exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM chapters WHERE id=?1)",
            params![input.chapter_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Kapitel konnte nicht geprüft werden", error))?;
    if !chapter_exists {
        return Err("Das ausgewählte Kapitel wurde nicht gefunden.".into());
    }
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Szenentransaktion konnte nicht gestartet werden", error))?;
    let id = new_id();
    transaction.execute("INSERT INTO scenes (id, chapter_id, title, order_index, content, pov, location, story_time, status, goal, notes) VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(order_index), 0) + 1 FROM scenes WHERE chapter_id=?2), '', '', '', '', 'draft', '', '')", params![id, input.chapter_id, input.title.trim()]).map_err(|error| sql_error("Szene konnte nicht gespeichert werden", error))?;
    transaction
        .commit()
        .map_err(|error| sql_error("Szenentransaktion konnte nicht abgeschlossen werden", error))?;
    load_scene(db, &id)
}

#[tauri::command]
pub fn create_scene(state: State<'_, DbState>, input: CreateSceneInput) -> Result<Scene, String> {
    let db = lock_db(&state)?;
    create_scene_in_db(&db, input)
}

pub(crate) fn update_scene_in_db(db: &Connection, input: SceneInput) -> Result<Scene, String> {
    required(&input.title, "Der Szenenname")?;
    validate_scene_status(&input.status)?;
    let chapter_exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM chapters WHERE id=?1)",
            params![input.chapter_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Kapitel konnte nicht geprüft werden", error))?;
    if !chapter_exists {
        return Err("Das Kapitel der Szene wurde nicht gefunden.".into());
    }
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Speichertransaktion konnte nicht gestartet werden", error))?;
    let timestamp = now();
    update_scene_in_transaction(&transaction, &input, &timestamp)?;
    transaction.commit().map_err(|error| {
        sql_error(
            "Speichertransaktion konnte nicht abgeschlossen werden",
            error,
        )
    })?;
    load_scene(db, &input.id)
}

#[tauri::command]
pub fn update_scene(state: State<'_, DbState>, input: SceneInput) -> Result<Scene, String> {
    let db = lock_db(&state)?;
    update_scene_in_db(&db, input)
}

#[tauri::command]
pub fn create_scene_version(
    state: State<'_, DbState>,
    input: CreateSceneVersionInput,
) -> Result<SceneVersion, String> {
    let db = lock_db(&state)?;
    create_scene_version_in_db(&db, &input.scene_id, &input.reason)
}

#[tauri::command]
pub fn list_scene_versions(
    state: State<'_, DbState>,
    scene_id: String,
) -> Result<Vec<SceneVersion>, String> {
    let db = lock_db(&state)?;
    load_scene_versions(&db, &scene_id)
}

#[tauri::command]
pub fn restore_scene_version(
    state: State<'_, DbState>,
    input: RestoreSceneVersionInput,
) -> Result<Scene, String> {
    let db = lock_db(&state)?;
    let current = load_scene(&db, &input.scene_id)?;
    let (version_scene_id, content, snapshot_json): (String, String, String) = db
        .query_row(
            "SELECT scene_id, content, snapshot_json FROM scene_versions WHERE id=?1 AND scene_id=?2",
            params![input.version_id, input.scene_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| sql_error("Version konnte nicht geladen werden", error))?;
    if version_scene_id != input.scene_id {
        return Err("Die Version gehört nicht zu dieser Szene.".into());
    }
    let mut restored = scene_input_from_scene(&current);
    restored.content = content;
    if let Ok(snapshot) = serde_json::from_str::<SceneInput>(&snapshot_json) {
        restored = snapshot;
    }
    restored.id = input.scene_id.clone();
    validate_scene_status(&restored.status)?;
    required(&restored.title, "Der Szenenname")?;
    let chapter_exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM chapters WHERE id=?1)",
            params![restored.chapter_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Kapitel konnte nicht geprüft werden", error))?;
    if !chapter_exists {
        return Err("Das Kapitel der gespeicherten Version wurde nicht gefunden.".into());
    }
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Wiederherstellung konnte nicht gestartet werden", error))?;
    let timestamp = now();
    insert_scene_version_in_transaction(&transaction, &current, &timestamp, "manual")?;
    update_scene_in_transaction(&transaction, &restored, &timestamp)?;
    transaction
        .commit()
        .map_err(|error| sql_error("Wiederherstellung konnte nicht abgeschlossen werden", error))?;
    load_scene(&db, &input.scene_id)
}

#[tauri::command]
pub fn get_editor_preferences(state: State<'_, DbState>) -> Result<EditorPreferences, String> {
    let db = lock_db(&state)?;
    let value: Option<String> = db
        .query_row(
            "SELECT value_json FROM app_settings WHERE key='editor_preferences'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| sql_error("Editor-Einstellungen konnten nicht geladen werden", error))?;
    value
        .map(|json| {
            serde_json::from_str(&json)
                .map_err(|error| sql_error("Editor-Einstellungen sind ungültig", error))
        })
        .unwrap_or_else(|| {
            Ok(EditorPreferences {
                font_family: "serif".into(),
                font_size: 18,
                line_height: 1.95,
            })
        })
}

#[tauri::command]
pub fn save_editor_preferences(
    state: State<'_, DbState>,
    mut input: EditorPreferences,
) -> Result<EditorPreferences, String> {
    if !matches!(input.font_family.as_str(), "serif" | "sans" | "typewriter") {
        return Err("Ungültige Manuskript-Schriftart.".into());
    }
    input.font_size = input.font_size.clamp(14, 28);
    input.line_height = input.line_height.clamp(1.3, 2.5);
    let db = lock_db(&state)?;
    let json = serde_json::to_string(&input).map_err(|error| {
        sql_error(
            "Editor-Einstellungen konnten nicht vorbereitet werden",
            error,
        )
    })?;
    let timestamp = now();
    db.execute("INSERT INTO app_settings (key, value_json, created_at, updated_at) VALUES ('editor_preferences', ?1, ?2, ?2) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json, updated_at=excluded.updated_at", params![json, timestamp])
        .map_err(|error| sql_error("Editor-Einstellungen konnten nicht gespeichert werden", error))?;
    Ok(input)
}

fn entity_query() -> &'static str {
    "SELECT id, project_id, name, entity_type, description, status, confidence, source, chapter, scene, author_confirmed, created_at, updated_at, origin, tags_json FROM story_entities WHERE id=?1"
}

fn load_entity(db: &Connection, id: &str) -> Result<StoryEntity, String> {
    db.query_row(entity_query(), params![id], entity_from_row)
        .map_err(|error| sql_error("Story-Bible-Eintrag konnte nicht geladen werden", error))
}

fn validate_entity_type(value: &str) -> Result<(), String> {
    match value {
        "character" | "relationship" | "place" | "organization" | "world_rule" | "object"
        | "event" | "fact" | "clue" | "secret" | "plot_thread" | "retcon" | "author_note" => Ok(()),
        _ => Err(format!("Ungültiger Story-Bible-Typ: {value}")),
    }
}

fn reference_location(
    db: &Connection,
    chapter_id: Option<&str>,
    scene_id: Option<&str>,
) -> Result<(String, String, String), String> {
    match (chapter_id, scene_id) {
        (Some(chapter_id), Some(scene_id)) => db
            .query_row(
                "SELECT chapters.id, chapters.title, scenes.title FROM scenes JOIN chapters ON chapters.id=scenes.chapter_id WHERE scenes.id=?1 AND chapters.id=?2",
                params![scene_id, chapter_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|error| sql_error("Kapitel und Szene der Quelle passen nicht zusammen", error)),
        (None, None) => Ok((String::new(), String::new(), String::new())),
        _ => Err("Eine Quelle benötigt sowohl Kapitel als auch Szene.".into()),
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_entity_tx(
    transaction: &rusqlite::Transaction<'_>,
    project_id: &str,
    name: &str,
    entity_type: &str,
    description: &str,
    status: &str,
    confidence: f64,
    chapter: &str,
    scene: &str,
    excerpt: &str,
    author_confirmed: bool,
    tags: &[String],
    origin: &str,
    timestamp: &str,
) -> Result<String, String> {
    let id = new_id();
    let tags_json = serde_json::to_string(tags)
        .map_err(|error| sql_error("Tags konnten nicht vorbereitet werden", error))?;
    transaction
        .execute(
            "INSERT INTO story_entities (id, project_id, name, entity_type, description, status, confidence, source, chapter, scene, author_confirmed, created_at, updated_at, origin, tags_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, ?13, ?14)",
            params![id, project_id, name.trim(), entity_type, description, status, confidence.clamp(0.0, 1.0), excerpt, chapter, scene, author_confirmed, timestamp, origin, tags_json],
        )
        .map_err(|error| sql_error("Story-Bible-Eintrag konnte nicht gespeichert werden", error))?;
    Ok(id)
}

pub(crate) fn create_story_entity_in_db(
    db: &Connection,
    input: CreateStoryEntityInput,
) -> Result<StoryEntity, String> {
    required(&input.name, "Der Eintragsname")?;
    validate_entity_type(&input.entity_type)?;
    validate_entity_status(&input.status)?;
    let (chapter_id, chapter, scene) =
        reference_location(db, input.chapter_id.as_deref(), input.scene_id.as_deref())?;
    let project_exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            params![input.project_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Projekt konnte nicht geprüft werden", error))?;
    if !project_exists {
        return Err("Das Projekt des Story-Bible-Eintrags wurde nicht gefunden.".into());
    }
    let transaction = db.unchecked_transaction().map_err(|error| {
        sql_error(
            "Story-Bible-Speicherung konnte nicht gestartet werden",
            error,
        )
    })?;
    let timestamp = now();
    let id = insert_entity_tx(
        &transaction,
        &input.project_id,
        &input.name,
        &input.entity_type,
        &input.description,
        &input.status,
        input.confidence,
        &chapter,
        &scene,
        &input.excerpt,
        input.author_confirmed,
        &input.tags,
        "manual",
        &timestamp,
    )?;
    if !chapter_id.is_empty() {
        transaction.execute("INSERT INTO story_source_references (id, project_id, entity_id, chapter_id, scene_id, excerpt, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![new_id(), input.project_id, id, chapter_id, input.scene_id, input.excerpt, timestamp]).map_err(|error| sql_error("Quelle konnte nicht gespeichert werden", error))?;
    }
    transaction.commit().map_err(|error| {
        sql_error(
            "Story-Bible-Speicherung konnte nicht abgeschlossen werden",
            error,
        )
    })?;
    load_entity(db, &id)
}

pub(crate) fn update_story_entity_in_db(
    db: &Connection,
    input: UpdateStoryEntityInput,
) -> Result<StoryEntity, String> {
    required(&input.name, "Der Eintragsname")?;
    validate_entity_type(&input.entity_type)?;
    validate_entity_status(&input.status)?;
    let current = load_entity(db, &input.id)?;
    if current.project_id != input.project_id {
        return Err("Der Eintrag gehört nicht zu diesem Projekt.".into());
    }
    let (_chapter_id, chapter, scene) =
        reference_location(db, input.chapter_id.as_deref(), input.scene_id.as_deref())?;
    let transaction = db.unchecked_transaction().map_err(|error| {
        sql_error(
            "Story-Bible-Speicherung konnte nicht gestartet werden",
            error,
        )
    })?;
    let timestamp = now();
    let tags_json = serde_json::to_string(&input.tags)
        .map_err(|error| sql_error("Tags konnten nicht vorbereitet werden", error))?;
    transaction.execute("UPDATE story_entities SET name=?2, entity_type=?3, description=?4, status=?5, confidence=?6, source=?7, chapter=?8, scene=?9, author_confirmed=?10, updated_at=?11, tags_json=?12 WHERE id=?1", params![input.id, input.name.trim(), input.entity_type, input.description, input.status, input.confidence.clamp(0.0, 1.0), input.excerpt, chapter, scene, input.author_confirmed, timestamp, tags_json]).map_err(|error| sql_error("Story-Bible-Eintrag konnte nicht aktualisiert werden", error))?;
    if let (Some(chapter_id), Some(scene_id)) =
        (input.chapter_id.as_deref(), input.scene_id.as_deref())
    {
        transaction.execute("INSERT INTO story_source_references (id, project_id, entity_id, chapter_id, scene_id, excerpt, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![new_id(), input.project_id, input.id, chapter_id, scene_id, input.excerpt, timestamp]).map_err(|error| sql_error("Quelle konnte nicht gespeichert werden", error))?;
    }
    transaction.commit().map_err(|error| {
        sql_error(
            "Story-Bible-Speicherung konnte nicht abgeschlossen werden",
            error,
        )
    })?;
    let _ = current;
    load_entity(db, &input.id)
}

#[tauri::command]
pub fn create_story_entity(
    state: State<'_, DbState>,
    input: CreateStoryEntityInput,
) -> Result<StoryEntity, String> {
    let db = lock_db(&state)?;
    create_story_entity_in_db(&db, input)
}

#[tauri::command]
pub fn update_story_entity(
    state: State<'_, DbState>,
    input: UpdateStoryEntityInput,
) -> Result<StoryEntity, String> {
    let db = lock_db(&state)?;
    update_story_entity_in_db(&db, input)
}

#[tauri::command]
pub fn archive_story_entity(state: State<'_, DbState>, id: String) -> Result<StoryEntity, String> {
    let db = lock_db(&state)?;
    let timestamp = now();
    let changed = db
        .execute(
            "UPDATE story_entities SET status='archived', updated_at=?2 WHERE id=?1",
            params![id, timestamp],
        )
        .map_err(|error| sql_error("Eintrag konnte nicht archiviert werden", error))?;
    if changed == 0 {
        return Err("Der Story-Bible-Eintrag wurde nicht gefunden.".into());
    }
    load_entity(&db, &id)
}

#[tauri::command]
pub fn get_story_entity(state: State<'_, DbState>, id: String) -> Result<StoryEntity, String> {
    let db = lock_db(&state)?;
    load_entity(&db, &id)
}

fn source_from_row(row: &rusqlite::Row<'_>) -> SqlResult<StorySourceReference> {
    Ok(StorySourceReference {
        id: row.get(0)?,
        project_id: row.get(1)?,
        entity_id: row.get(2)?,
        proposal_id: row.get(3)?,
        chapter_id: row.get(4)?,
        scene_id: row.get(5)?,
        excerpt: row.get(6)?,
        start_offset: row.get(7)?,
        end_offset: row.get(8)?,
        created_at: row.get(9)?,
    })
}

#[tauri::command]
pub fn create_source_reference(
    state: State<'_, DbState>,
    input: CreateSourceReferenceInput,
) -> Result<StorySourceReference, String> {
    let db = lock_db(&state)?;
    let timestamp = now();
    let id = new_id();
    db.execute("INSERT INTO story_source_references (id, project_id, entity_id, proposal_id, chapter_id, scene_id, excerpt, start_offset, end_offset, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", params![id, input.project_id, input.entity_id, input.proposal_id, input.chapter_id, input.scene_id, input.excerpt, input.start_offset, input.end_offset, timestamp]).map_err(|error| sql_error("Quellenreferenz konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT id, project_id, entity_id, proposal_id, chapter_id, scene_id, excerpt, start_offset, end_offset, created_at FROM story_source_references WHERE id=?1", params![id], source_from_row).map_err(|error| sql_error("Gespeicherte Quellenreferenz konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn list_source_references(
    state: State<'_, DbState>,
    project_id: String,
    entity_id: Option<String>,
) -> Result<Vec<StorySourceReference>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id, project_id, entity_id, proposal_id, chapter_id, scene_id, excerpt, start_offset, end_offset, created_at FROM story_source_references WHERE project_id=?1 AND (?2 IS NULL OR entity_id=?2) ORDER BY created_at DESC").map_err(|error| sql_error("Quellen konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![project_id, entity_id], source_from_row)
        .map_err(|error| sql_error("Quellen konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Quellen konnten nicht geladen werden", error));
    result
}

fn run_from_row(row: &rusqlite::Row<'_>) -> SqlResult<BibleUpdateRun> {
    Ok(BibleUpdateRun {
        id: row.get(0)?,
        project_id: row.get(1)?,
        scene_id: row.get(2)?,
        scene_updated_at: row.get(3)?,
        content_hash: row.get(4)?,
        extractor_id: row.get(5)?,
        status: row.get(6)?,
        created_at: row.get(7)?,
        completed_at: row.get(8)?,
        error_message: row.get(9)?,
    })
}

fn proposal_from_row(row: &rusqlite::Row<'_>) -> SqlResult<BibleProposal> {
    Ok(BibleProposal {
        id: row.get(0)?,
        run_id: row.get(1)?,
        project_id: row.get(2)?,
        scene_id: row.get(3)?,
        target_entity_id: row.get(4)?,
        proposal_action: row.get(5)?,
        entity_type: row.get(6)?,
        candidate_name: row.get(7)?,
        candidate_description: row.get(8)?,
        candidate_status: row.get(9)?,
        confidence: row.get(10)?,
        classification: row.get(11)?,
        evidence_excerpt: row.get(12)?,
        start_offset: row.get(13)?,
        end_offset: row.get(14)?,
        reason: row.get(15)?,
        review_status: row.get(16)?,
        reviewed_at: row.get(17)?,
        created_at: row.get(18)?,
    })
}

fn load_run(db: &Connection, id: &str) -> Result<BibleUpdateRun, String> {
    db.query_row("SELECT id, project_id, scene_id, scene_updated_at, content_hash, extractor_id, status, created_at, completed_at, error_message FROM bible_update_runs WHERE id=?1", params![id], run_from_row).map_err(|error| sql_error("Bible-Update-Lauf konnte nicht geladen werden", error))
}

fn load_proposal(db: &Connection, id: &str) -> Result<BibleProposal, String> {
    db.query_row("SELECT id, run_id, project_id, scene_id, target_entity_id, proposal_action, entity_type, candidate_name, candidate_description, candidate_status, confidence, classification, evidence_excerpt, start_offset, end_offset, reason, review_status, reviewed_at, created_at FROM bible_proposals WHERE id=?1", params![id], proposal_from_row).map_err(|error| sql_error("Bible-Vorschlag konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn create_bible_update_run(
    state: State<'_, DbState>,
    input: CreateBibleUpdateRunInput,
) -> Result<BibleUpdateRun, String> {
    let db = lock_db(&state)?;
    let scene_project: String = db.query_row("SELECT books.project_id FROM scenes JOIN chapters ON chapters.id=scenes.chapter_id JOIN books ON books.id=chapters.book_id WHERE scenes.id=?1", params![input.scene_id], |row| row.get(0)).map_err(|error| sql_error("Szene konnte nicht geprüft werden", error))?;
    if scene_project != input.project_id {
        return Err("Die Szene gehört nicht zu diesem Projekt.".into());
    }
    if !input.force {
        if let Some(existing) = db.query_row("SELECT id FROM bible_update_runs WHERE project_id=?1 AND scene_id=?2 AND content_hash=?3 AND extractor_id=?4 AND status IN ('completed','reviewed') ORDER BY created_at DESC LIMIT 1", params![input.project_id, input.scene_id, input.content_hash, input.extractor_id], |row| row.get::<_, String>(0)).optional().map_err(|error| sql_error("Vorherige Bible-Updates konnten nicht geprüft werden", error))? {
            return load_run(&db, &existing);
        }
    }
    let id = new_id();
    let timestamp = now();
    db.execute("INSERT INTO bible_update_runs (id, project_id, scene_id, scene_updated_at, content_hash, extractor_id, status, created_at) VALUES (?1,?2,?3,?4,?5,?6,'pending',?7)", params![id, input.project_id, input.scene_id, input.scene_updated_at, input.content_hash, input.extractor_id, timestamp]).map_err(|error| sql_error("Bible-Update-Lauf konnte nicht angelegt werden", error))?;
    load_run(&db, &id)
}

#[tauri::command]
pub fn list_bible_update_runs(
    state: State<'_, DbState>,
    project_id: String,
    scene_id: Option<String>,
) -> Result<Vec<BibleUpdateRun>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id, project_id, scene_id, scene_updated_at, content_hash, extractor_id, status, created_at, completed_at, error_message FROM bible_update_runs WHERE project_id=?1 AND (?2 IS NULL OR scene_id=?2) ORDER BY created_at DESC").map_err(|error| sql_error("Bible-Update-Läufe konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![project_id, scene_id], run_from_row)
        .map_err(|error| sql_error("Bible-Update-Läufe konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Bible-Update-Läufe konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn list_bible_proposals(
    state: State<'_, DbState>,
    run_id: String,
) -> Result<Vec<BibleProposal>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id, run_id, project_id, scene_id, target_entity_id, proposal_action, entity_type, candidate_name, candidate_description, candidate_status, confidence, classification, evidence_excerpt, start_offset, end_offset, reason, review_status, reviewed_at, created_at FROM bible_proposals WHERE run_id=?1 ORDER BY created_at, id").map_err(|error| sql_error("Bible-Vorschläge konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![run_id], proposal_from_row)
        .map_err(|error| sql_error("Bible-Vorschläge konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Bible-Vorschläge konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn save_bible_proposals(
    state: State<'_, DbState>,
    run_id: String,
    proposals: Vec<BibleProposalInput>,
) -> Result<Vec<BibleProposal>, String> {
    let db = lock_db(&state)?;
    let run = load_run(&db, &run_id)?;
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Vorschläge konnten nicht gespeichert werden", error))?;
    for input in proposals {
        if input.run_id != run_id
            || input.project_id != run.project_id
            || input.scene_id != run.scene_id
        {
            return Err("Der Vorschlag gehört nicht zum ausgewählten Lauf.".into());
        }
        validate_proposal_action(&input.proposal_action)?;
        validate_proposal_classification(&input.classification)?;
        validate_entity_type(&input.entity_type)?;
        validate_entity_status(&input.candidate_status)?;
        let id = input.id.unwrap_or_else(new_id);
        transaction.execute("INSERT OR REPLACE INTO bible_proposals (id,run_id,project_id,scene_id,target_entity_id,proposal_action,entity_type,candidate_name,candidate_description,candidate_status,confidence,classification,evidence_excerpt,start_offset,end_offset,reason,review_status,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,'pending',COALESCE((SELECT created_at FROM bible_proposals WHERE id=?1),?17))", params![id, input.run_id, input.project_id, input.scene_id, input.target_entity_id, input.proposal_action, input.entity_type, input.candidate_name, input.candidate_description, input.candidate_status, input.confidence.clamp(0.0, 1.0), input.classification, input.evidence_excerpt, input.start_offset, input.end_offset, input.reason, now()]).map_err(|error| sql_error("Bible-Vorschlag konnte nicht gespeichert werden", error))?;
    }
    let timestamp = now();
    transaction.execute("UPDATE bible_update_runs SET status='completed', completed_at=?, error_message=NULL WHERE id=?", params![timestamp, run_id]).map_err(|error| sql_error("Bible-Update-Lauf konnte nicht abgeschlossen werden", error))?;
    transaction
        .commit()
        .map_err(|error| sql_error("Bible-Update-Lauf konnte nicht abgeschlossen werden", error))?;
    let mut statement = db.prepare("SELECT id, run_id, project_id, scene_id, target_entity_id, proposal_action, entity_type, candidate_name, candidate_description, candidate_status, confidence, classification, evidence_excerpt, start_offset, end_offset, reason, review_status, reviewed_at, created_at FROM bible_proposals WHERE run_id=?1 ORDER BY created_at, id").map_err(|error| sql_error("Bible-Vorschläge konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![run_id], proposal_from_row)
        .map_err(|error| sql_error("Bible-Vorschläge konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Bible-Vorschläge konnten nicht geladen werden", error));
    result
}

pub(crate) fn review_bible_proposal_in_db(
    db: &Connection,
    input: ReviewBibleProposalInput,
) -> Result<BibleProposal, String> {
    validate_review_status(&input.review_status)?;
    let proposal = load_proposal(db, &input.proposal_id)?;
    let name = input
        .candidate_name
        .unwrap_or(proposal.candidate_name.clone());
    let description = input
        .candidate_description
        .unwrap_or(proposal.candidate_description.clone());
    let status = input
        .candidate_status
        .unwrap_or(proposal.candidate_status.clone());
    let classification = input
        .classification
        .unwrap_or(proposal.classification.clone());
    validate_entity_status(&status)?;
    validate_proposal_classification(&classification)?;
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Review konnte nicht gestartet werden", error))?;
    let mut entity_id = proposal.target_entity_id.clone();
    if input.review_status != "rejected" {
        let action = proposal.proposal_action.as_str();
        if matches!(
            action,
            "create_entity" | "create_open_question" | "create_author_note"
        ) || (action == "add_source" && entity_id.is_none())
        {
            let effective_status =
                if classification == "interpretation" || classification == "open_question" {
                    "uncertain"
                } else {
                    status.as_str()
                };
            entity_id = Some(insert_entity_tx(
                &transaction,
                &proposal.project_id,
                &name,
                &proposal.entity_type,
                &description,
                effective_status,
                proposal.confidence,
                "",
                "",
                &proposal.evidence_excerpt,
                false,
                &[],
                "bible_update",
                &now(),
            )?);
        } else if action == "update_entity" {
            let target = entity_id
                .clone()
                .ok_or_else(|| "Der Vorschlag hat keinen Ziel-Eintrag.".to_string())?;
            let changed = transaction.execute("UPDATE story_entities SET name=?2, description=?3, status=?4, confidence=?5, updated_at=?6, origin='bible_update' WHERE id=?1 AND project_id=?7", params![target, name, description, status, proposal.confidence, now(), proposal.project_id]).map_err(|error| sql_error("Story-Bible-Eintrag konnte nicht geändert werden", error))?;
            if changed == 0 {
                return Err("Der Ziel-Eintrag des Vorschlags wurde nicht gefunden.".into());
            }
        } else if action == "mark_contradiction" {
            if let Some(target) = entity_id.as_deref() {
                transaction.execute("UPDATE story_entities SET status='contradicted', updated_at=?2 WHERE id=?1", params![target, now()]).map_err(|error| sql_error("Widerspruch konnte nicht markiert werden", error))?;
            }
        }
        let chapter_id: String = transaction
            .query_row(
                "SELECT chapter_id FROM scenes WHERE id=?1",
                params![proposal.scene_id],
                |row| row.get(0),
            )
            .map_err(|error| sql_error("Quellenszene konnte nicht geladen werden", error))?;
        transaction.execute("INSERT INTO story_source_references (id, project_id, entity_id, proposal_id, chapter_id, scene_id, excerpt, start_offset, end_offset, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", params![new_id(), proposal.project_id, entity_id, proposal.id, chapter_id, proposal.scene_id, proposal.evidence_excerpt, proposal.start_offset, proposal.end_offset, now()]).map_err(|error| sql_error("Quellenreferenz konnte nicht gespeichert werden", error))?;
    }
    transaction.execute("UPDATE bible_proposals SET candidate_name=?2, candidate_description=?3, candidate_status=?4, classification=?5, review_status=?6, reviewed_at=?7 WHERE id=?1", params![proposal.id, name, description, status, classification, input.review_status, now()]).map_err(|error| sql_error("Review-Status konnte nicht gespeichert werden", error))?;
    transaction
        .commit()
        .map_err(|error| sql_error("Review konnte nicht abgeschlossen werden", error))?;
    load_proposal(db, &proposal.id)
}

#[tauri::command]
pub fn review_bible_proposal(
    state: State<'_, DbState>,
    input: ReviewBibleProposalInput,
) -> Result<BibleProposal, String> {
    let db = lock_db(&state)?;
    review_bible_proposal_in_db(&db, input)
}

#[tauri::command]
pub fn complete_bible_review(
    state: State<'_, DbState>,
    run_id: String,
) -> Result<BibleUpdateRun, String> {
    let db = lock_db(&state)?;
    let pending: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM bible_proposals WHERE run_id=?1 AND review_status='pending'",
            params![run_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Review-Status konnte nicht geprüft werden", error))?;
    if pending > 0 {
        return Err("Bitte prüfe zuerst alle offenen Vorschläge.".into());
    }
    db.execute(
        "UPDATE bible_update_runs SET status='reviewed' WHERE id=?1",
        params![run_id],
    )
    .map_err(|error| sql_error("Bible-Review konnte nicht abgeschlossen werden", error))?;
    load_run(&db, &run_id)
}

#[tauri::command]
pub fn save_story_entity(
    state: State<'_, DbState>,
    input: StoryEntityInput,
) -> Result<StoryEntity, String> {
    required(&input.name, "Der Eintragsname")?;
    validate_entity_status(&input.status)?;
    let db = lock_db(&state)?;
    let project_exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            params![input.project_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Projekt konnte nicht geprüft werden", error))?;
    if !project_exists {
        return Err("Das Projekt des Story-Bible-Eintrags wurde nicht gefunden.".into());
    }
    let timestamp = now();
    let tags_json = serde_json::to_string(&input.tags)
        .map_err(|error| sql_error("Tags konnten nicht vorbereitet werden", error))?;
    db.execute("INSERT INTO story_entities (id, project_id, name, entity_type, description, status, confidence, source, chapter, scene, author_confirmed, updated_at, origin, tags_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'manual', ?13) ON CONFLICT(id) DO UPDATE SET project_id=excluded.project_id, name=excluded.name, entity_type=excluded.entity_type, description=excluded.description, status=excluded.status, confidence=excluded.confidence, source=excluded.source, chapter=excluded.chapter, scene=excluded.scene, author_confirmed=excluded.author_confirmed, updated_at=excluded.updated_at, tags_json=excluded.tags_json", params![input.id, input.project_id, input.name.trim(), input.entity_type, input.description, input.status, input.confidence.clamp(0.0, 1.0), input.source, input.chapter, input.scene, input.author_confirmed, timestamp, tags_json]).map_err(|error| sql_error("Story-Bible-Eintrag konnte nicht gespeichert werden", error))?;
    db.query_row(entity_query(), params![input.id], entity_from_row)
        .map_err(|error| {
            sql_error(
                "Gespeicherter Story-Bible-Eintrag konnte nicht geladen werden",
                error,
            )
        })
}

#[tauri::command]
pub fn list_story_entities(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<StoryEntity>, String> {
    let db = lock_db(&state)?;
    load_entities(&db, &project_id)
}

#[tauri::command]
pub fn database_info(state: State<'_, DbState>) -> Result<DatabaseInfo, String> {
    let _db = lock_db(&state)?;
    Ok(DatabaseInfo {
        path: state.path.display().to_string(),
        connected: true,
        engine: "sqlite".into(),
        detail: "Lokale SQLite-Datenbank der StoryMemory-Desktop-App".into(),
    })
}

#[tauri::command]
pub fn check_local_languagetool() -> ProviderStatus {
    ProviderStatus {
        id: "language-tool-local".into(),
        available: false,
        label: "Nicht erreichbar".into(),
        detail: "Kein lokaler LanguageTool-Server gefunden".into(),
    }
}

#[tauri::command]
pub fn provider_status() -> Vec<ProviderStatus> {
    vec![
        ProviderStatus {
            id: "mock".into(),
            available: true,
            label: "Bereit".into(),
            detail: "Lokaler Mock Provider".into(),
        },
        ProviderStatus {
            id: "codex-cli".into(),
            available: false,
            label: "Nicht verbunden".into(),
            detail: "Offizieller CLI-Client noch nicht konfiguriert".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{database_path_for_test, seed_if_empty};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn connection(name: &str) -> (std::path::PathBuf, Connection) {
        let path = std::env::temp_dir().join(format!(
            "storymemory-command-{name}-{}.sqlite3",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = database_path_for_test(&path).unwrap();
        seed_if_empty(&db).unwrap();
        (path, db)
    }

    #[test]
    fn project_is_returned_after_insert() {
        let (path, db) = connection("project");
        let input = CreateProjectInput {
            title: "Neu".into(),
            author: "Ada".into(),
            description: String::new(),
            volume_title: String::new(),
            volume: 1,
        };
        let tx = db.unchecked_transaction().unwrap();
        let id = new_id();
        tx.execute(
            "INSERT INTO projects (id,title,author,description) VALUES (?1,?2,?3,?4)",
            params![id, input.title, input.author, "Test"],
        )
        .unwrap();
        tx.commit().unwrap();
        assert_eq!(project_from_db(&db, &id).unwrap().title, "Neu");
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn project_service_creates_project_and_first_book_transactionally() {
        let (path, db) = connection("project-service");
        let project = create_project_in_db(
            &db,
            CreateProjectInput {
                title: "Transaktion".into(),
                author: "Ada".into(),
                description: String::new(),
                volume_title: "Band Eins".into(),
                volume: 1,
            },
        )
        .unwrap();
        let book: (String, String) = db
            .query_row(
                "SELECT id, project_id FROM books WHERE project_id=?1",
                params![project.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(!project.id.is_empty());
        assert!(!book.0.is_empty());
        assert_eq!(book.1, project.id);
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn chapter_and_scene_services_return_backend_ids_and_foreign_keys() {
        let (path, db) = connection("service-ids");
        let chapter = create_chapter_in_db(
            &db,
            CreateChapterInput {
                book_id: "book-1".into(),
                title: "Service-Kapitel".into(),
            },
        )
        .unwrap();
        let scene = create_scene_in_db(
            &db,
            CreateSceneInput {
                chapter_id: chapter.id.clone(),
                title: "Service-Szene".into(),
            },
        )
        .unwrap();
        assert!(!chapter.id.is_empty());
        assert!(!scene.id.is_empty());
        assert_eq!(chapter.book_id, "book-1");
        assert_eq!(scene.chapter_id, chapter.id);
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn chapter_has_correct_book_reference() {
        let (path, db) = connection("chapter");
        let id = new_id();
        db.execute(
            "INSERT INTO chapters (id,book_id,title,order_index) VALUES (?1,'book-1','Neu',4)",
            params![id],
        )
        .unwrap();
        assert_eq!(
            db.query_row(
                "SELECT book_id FROM chapters WHERE id=?1",
                params![id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "book-1"
        );
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn scene_has_correct_chapter_reference() {
        let (path, db) = connection("scene");
        let id = new_id();
        db.execute(
            "INSERT INTO scenes (id,chapter_id,title) VALUES (?1,'chapter-1','Neu')",
            params![id],
        )
        .unwrap();
        assert_eq!(
            db.query_row(
                "SELECT chapter_id FROM scenes WHERE id=?1",
                params![id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "chapter-1"
        );
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn workspace_load_has_project_chapters_and_entities() {
        let (path, db) = connection("workspace");
        let books = load_books(&db, "project-zugestellt").unwrap();
        let chapters = load_chapters(&db, &books).unwrap();
        let entities = load_entities(&db, "project-zugestellt").unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(chapters.len(), 3);
        assert!(!entities.is_empty());
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn scene_versions_keep_full_scene_snapshot() {
        let (path, db) = connection("versions");
        let input = SceneInput {
            id: "scene-1".into(),
            chapter_id: "chapter-1".into(),
            title: "Versionierte Szene".into(),
            order_index: 1,
            content: "Älterer Stand\nmit Zeilenumbruch".into(),
            pov: "Lena".into(),
            location: "Café".into(),
            story_time: "Dienstag".into(),
            status: "revised".into(),
            goal: "Ein Ziel".into(),
            notes: "Eine Notiz".into(),
        };
        update_scene_in_db(&db, input.clone()).unwrap();
        let version = create_scene_version_in_db(&db, "scene-1", "manual").unwrap();
        let versions = load_scene_versions(&db, "scene-1").unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].scene.pov, "Lena");
        assert_eq!(versions[0].scene.content, input.content);
        assert_eq!(versions[0].version_number, 1);
        assert_eq!(version.reason, "manual");
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn autosave_updates_do_not_create_historical_versions() {
        let (path, db) = connection("autosave-versions");
        let mut input = scene_input_from_scene(&load_scene(&db, "scene-1").unwrap());
        for index in 0..20 {
            input.content = format!("Fassung {index}");
            update_scene_in_db(&db, input.clone()).unwrap();
        }
        assert!(load_scene_versions(&db, "scene-1").unwrap().is_empty());
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn created_version_survives_database_reopen_and_keeps_scene_fk() {
        let path = std::env::temp_dir().join(format!(
            "storymemory-command-version-reopen-{}.sqlite3",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let db = database_path_for_test(&path).unwrap();
            seed_if_empty(&db).unwrap();
            let version = create_scene_version_in_db(&db, "scene-1", "manual").unwrap();
            assert_eq!(version.scene_id, "scene-1");
        }
        let reopened = database_path_for_test(&path).unwrap();
        let versions = load_scene_versions(&reopened, "scene-1").unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].content, versions[0].scene.content);
        drop(reopened);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn update_scene_service_rejects_missing_scene_and_invalid_status() {
        let (path, db) = connection("scene-errors");
        let mut missing = scene_input_from_scene(&load_scene(&db, "scene-1").unwrap());
        missing.id = "not-there".into();
        assert!(update_scene_in_db(&db, missing).is_err());
        let mut invalid = scene_input_from_scene(&load_scene(&db, "scene-1").unwrap());
        invalid.status = "unknown".into();
        assert!(update_scene_in_db(&db, invalid).is_err());
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn story_entity_service_creates_updates_archives_and_keeps_source() {
        let (path, db) = connection("story-entity-crud");
        let created = create_story_entity_in_db(
            &db,
            CreateStoryEntityInput {
                project_id: "project-zugestellt".into(),
                name: "Neue Spur".into(),
                entity_type: "clue".into(),
                description: "Eine beobachtbare Spur.".into(),
                status: "proposed".into(),
                confidence: 0.7,
                chapter_id: Some("chapter-3".into()),
                scene_id: Some("scene-3".into()),
                excerpt: "die letzte Stelle".into(),
                author_confirmed: false,
                tags: vec!["Test".into()],
            },
        )
        .unwrap();
        assert_eq!(created.origin, "manual");
        assert_eq!(created.tags, vec!["Test"]);
        let source_count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM story_source_references WHERE entity_id=?1",
                params![created.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(source_count, 1);
        let updated = update_story_entity_in_db(
            &db,
            UpdateStoryEntityInput {
                id: created.id.clone(),
                project_id: created.project_id.clone(),
                name: "Bearbeitete Spur".into(),
                entity_type: created.entity_type.clone(),
                description: created.description.clone(),
                status: "confirmed".into(),
                confidence: 0.9,
                chapter_id: Some("chapter-3".into()),
                scene_id: Some("scene-3".into()),
                excerpt: created.source.clone(),
                author_confirmed: true,
                tags: vec!["Kanon".into()],
            },
        )
        .unwrap();
        assert_eq!(updated.name, "Bearbeitete Spur");
        db.execute(
            "UPDATE story_entities SET status='archived' WHERE id=?1",
            params![updated.id],
        )
        .unwrap();
        assert_eq!(load_entity(&db, &updated.id).unwrap().status, "archived");
        drop(db);
        let _ = fs::remove_file(path);
    }
}
