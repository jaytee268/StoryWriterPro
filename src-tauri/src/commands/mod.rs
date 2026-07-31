use crate::{
    database::DbState,
    models::{ProviderStatus, SceneInput, StoryEntityInput},
};
use chrono::Utc;
use rusqlite::params;
use tauri::State;

#[tauri::command]
pub fn get_dashboard_snapshot(state: State<'_, DbState>) -> Result<serde_json::Value, String> {
    let db = state.connection.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.prepare("SELECT id, title, author, description, updated_at FROM projects ORDER BY updated_at DESC").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| Ok(serde_json::json!({ "id": row.get::<_, String>(0)?, "title": row.get::<_, String>(1)?, "author": row.get::<_, String>(2)?, "description": row.get::<_, String>(3)?, "updatedAt": row.get::<_, String>(4)? }))).map_err(|e| e.to_string())?;
    let projects: Result<Vec<_>, _> = rows.collect();
    Ok(serde_json::json!({ "projects": projects.map_err(|e| e.to_string())? }))
}

#[tauri::command]
pub fn list_story_entities(state: State<'_, DbState>) -> Result<Vec<serde_json::Value>, String> {
    let db = state.connection.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.prepare("SELECT id, name, entity_type, description, status, confidence, source, chapter, scene, author_confirmed, updated_at FROM story_entities ORDER BY updated_at DESC").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| Ok(serde_json::json!({ "id": row.get::<_, String>(0)?, "name": row.get::<_, String>(1)?, "type": row.get::<_, String>(2)?, "description": row.get::<_, String>(3)?, "status": row.get::<_, String>(4)?, "confidence": row.get::<_, f64>(5)?, "source": row.get::<_, String>(6)?, "chapter": row.get::<_, String>(7)?, "scene": row.get::<_, String>(8)?, "authorConfirmed": row.get::<_, bool>(9)?, "updatedAt": row.get::<_, String>(10)? }))).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_scene(state: State<'_, DbState>, scene: SceneInput) -> Result<(), String> {
    let db = state.connection.lock().map_err(|e| e.to_string())?;
    db.execute("INSERT INTO scenes (id, chapter_id, title, content, pov, location, story_time, status, goal, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) ON CONFLICT(id) DO UPDATE SET title=excluded.title, content=excluded.content, pov=excluded.pov, location=excluded.location, story_time=excluded.story_time, status=excluded.status, goal=excluded.goal, notes=excluded.notes, updated_at=CURRENT_TIMESTAMP", params![scene.id, scene.chapter_id, scene.title, scene.content, scene.pov, scene.location, scene.story_time, scene.status, scene.goal, scene.notes]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn create_chapter(state: State<'_, DbState>, title: String) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let db = state.connection.lock().map_err(|e| e.to_string())?;
    db.execute("INSERT INTO chapters (id, book_id, title, order_index) VALUES (?1, 'book-1', ?2, (SELECT COALESCE(MAX(order_index), 0) + 1 FROM chapters))", params![id, title]).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn create_scene(
    state: State<'_, DbState>,
    chapter_id: String,
    title: String,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let db = state.connection.lock().map_err(|e| e.to_string())?;
    db.execute("INSERT INTO scenes (id, chapter_id, title, content, pov, location, story_time, status, goal, notes) VALUES (?1, ?2, ?3, '', '', '', '', 'draft', '', '')", params![id, chapter_id, title]).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn save_story_entity(
    state: State<'_, DbState>,
    entity: StoryEntityInput,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let db = state.connection.lock().map_err(|e| e.to_string())?;
    db.execute("INSERT INTO story_entities (id, name, entity_type, description, status, confidence, source, chapter, scene, author_confirmed, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) ON CONFLICT(id) DO UPDATE SET name=excluded.name, description=excluded.description, status=excluded.status, confidence=excluded.confidence, updated_at=excluded.updated_at", params![entity.id, entity.name, entity.entity_type, entity.description, entity.status, entity.confidence, entity.source, entity.chapter, entity.scene, entity.author_confirmed, now]).map_err(|e| e.to_string())?;
    Ok(())
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
