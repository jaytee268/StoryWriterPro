use crate::providers::codex::{
    self, AiProviderSettings, CodexCliCapabilities, CodexError, CodexRuntimeState,
    RunCodexTaskInput,
};
use crate::{
    database::DbState,
    models::{
        validate_character_memory_status, validate_character_significance,
        validate_character_voice_pattern_type, validate_dialogue_kind, validate_entity_status,
        validate_evidence_role, validate_knowledge_state, validate_lore_category,
        validate_lore_entity_type, validate_lore_importance, validate_lore_reveal_state,
        validate_lore_scope, validate_memory_kind, validate_memory_reliability,
        validate_participant_role, validate_proposal_action, validate_proposal_classification,
        validate_relation_type, validate_relationship_memory_type, validate_review_status,
        validate_scene_status, validate_scene_version_reason, validate_style_reference_category,
        validate_truthfulness, AddCharacterMemoryEvidenceInput, BibleProposal, BibleProposalInput,
        BibleUpdateRun, Book, Chapter, ChapterGenerationJob, ChapterGenerationPlan,
        ChapterGenerationReview, ChapterGenerationSection, ChapterPlanBeat,
        CharacterDialogueMemory, CharacterExperience, CharacterKnowledgeState,
        CharacterMemoryEvidence, CharacterMemoryProposal, CharacterMemoryProposalDraft,
        CharacterMemoryUpdateRun, CharacterProfile, CharacterSceneState, CharacterVoicePattern,
        CreateBibleUpdateRunInput, CreateChapterGenerationJobInput, CreateChapterInput,
        CreateCharacterMemoryUpdateRunInput, CreateLoreEntryInput, CreateProjectInput,
        CreateProjectStyleAnalysisRunInput, CreateSceneInput, CreateSceneVersionInput,
        CreateSourceReferenceInput, CreateStoryEntityInput, CreateStoryEntityRelationInput,
        CreateStyleReferenceInput, DatabaseInfo, DialogueMemoryParticipant, EditorPreferences,
        LoreEntry, LoreMetadata, ManuscriptImportInput, ManuscriptImportResult, NarrativeSummary,
        Project, ProjectStyle, ProjectStyleAnalysisRun, ProjectStyleObservation, ProviderStatus,
        RelationshipMemory, RestoreSceneVersionInput, ReviewBibleProposalInput,
        ReviewCharacterMemoryProposalInput, SaveChapterGenerationPlanInput,
        SaveChapterGenerationReviewInput, SaveChapterGenerationSectionInput,
        SaveCharacterDialogueMemoryInput, SaveCharacterExperienceInput,
        SaveCharacterKnowledgeStateInput, SaveCharacterProfileInput, SaveCharacterSceneStateInput,
        SaveCharacterVoicePatternInput, SaveLoreMetadataInput, SaveNarrativeSummaryInput,
        SaveProjectStyleInput, SaveProjectStyleObservationInput, SaveRelationshipMemoryInput,
        SaveStoryDirectionInput, SaveWritingPreferencesInput, Scene, SceneInput, SceneVersion,
        StoryDirection, StoryEntity, StoryEntityInput, StoryEntityRelation, StorySourceReference,
        StyleReference, UpdateChapterInput, UpdateStoryEntityInput, UpdateStyleReferenceInput,
        WorkspaceSnapshot, WritingPreferences,
    },
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use std::sync::Arc;
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

fn canonical_editor_text(content: &str) -> String {
    let mut output = String::with_capacity(content.len());
    let mut inside_tag = false;
    let mut tag = String::new();
    for character in content.chars() {
        if character == '<' {
            inside_tag = true;
            tag.clear();
            continue;
        }
        if inside_tag {
            if character == '>' {
                let normalized = tag.trim().to_ascii_lowercase();
                if normalized.starts_with("br")
                    || normalized.starts_with("/p")
                    || normalized.starts_with("/div")
                    || normalized.starts_with("/li")
                    || normalized.starts_with("/blockquote")
                {
                    output.push('\n');
                }
                inside_tag = false;
            } else {
                tag.push(character);
            }
            continue;
        }
        output.push(if character == '\u{00a0}' {
            ' '
        } else {
            character
        });
    }
    while output.ends_with('\n') {
        output.pop();
    }
    output
}

fn canonical_content_hash(content: &str) -> String {
    let mut hash = 2_166_136_261_u32;
    for character in canonical_editor_text(content).chars() {
        hash ^= character as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    format!("{hash:08x}")
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
    let current_hash = canonical_content_hash(&input.content);
    transaction
        .execute(
            "UPDATE narrative_summaries SET status='outdated', updated_at=?1 WHERE project_id=(SELECT books.project_id FROM books JOIN chapters ON chapters.book_id=books.id WHERE chapters.id=?2) AND scope_type='scene' AND scope_id=?3 AND content_hash<>?4 AND status='confirmed'",
            params![timestamp, input.chapter_id, input.id, current_hash],
        )
        .map_err(|error| sql_error("Zusammenfassung konnte nicht aktualisiert werden", error))?;
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

pub(crate) fn import_manuscript_in_db(
    db: &Connection,
    input: ManuscriptImportInput,
) -> Result<ManuscriptImportResult, String> {
    if input.chapters.is_empty() {
        return Err("Der Import enthält keine Kapitel.".into());
    }
    if input.chapters.len() > 1000 {
        return Err("Der Import enthält mehr als 1.000 Kapitel.".into());
    }
    for chapter in &input.chapters {
        required(&chapter.title, "Der Kapitelname")?;
        if chapter.title.chars().count() > 500 {
            return Err("Ein Kapitelname darf höchstens 500 Zeichen enthalten.".into());
        }
        if chapter.content.chars().count() > 20_000_000 {
            return Err("Der Text eines Kapitels ist zu groß für den sicheren Import.".into());
        }
    }
    let book_project: Option<String> = db
        .query_row(
            "SELECT project_id FROM books WHERE id=?1",
            params![input.book_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| sql_error("Der Zielband konnte nicht geprüft werden", error))?;
    if book_project.as_deref() != Some(input.project_id.as_str()) {
        return Err("Der Zielband gehört nicht zum ausgewählten Projekt.".into());
    }

    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Importtransaktion konnte nicht gestartet werden", error))?;
    let timestamp = now();
    let start_order: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(order_index), 0) FROM chapters WHERE book_id=?1",
            params![input.book_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Kapitelreihenfolge konnte nicht ermittelt werden", error))?;
    let mut chapters = Vec::with_capacity(input.chapters.len());
    let mut scenes = Vec::with_capacity(input.chapters.len());
    let mut version_ids = Vec::with_capacity(input.chapters.len());

    for (index, imported) in input.chapters.iter().enumerate() {
        let chapter_id = new_id();
        let scene_id = new_id();
        let chapter = Chapter {
            id: chapter_id.clone(),
            book_id: input.book_id.clone(),
            title: imported.title.trim().to_string(),
            order_index: start_order + index as i64 + 1,
            scenes: Vec::new(),
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
        };
        let scene = Scene {
            id: scene_id,
            chapter_id: chapter_id.clone(),
            title: "Kapiteltext".into(),
            order_index: 1,
            content: imported.content.clone(),
            pov: String::new(),
            location: String::new(),
            story_time: String::new(),
            status: "draft".into(),
            goal: String::new(),
            notes: String::new(),
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
        };
        transaction
            .execute(
                "INSERT INTO chapters (id, book_id, title, order_index, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![chapter.id, chapter.book_id, chapter.title, chapter.order_index, timestamp],
            )
            .map_err(|error| sql_error("Kapitel konnte nicht importiert werden", error))?;
        transaction
            .execute(
                "INSERT INTO scenes (id, chapter_id, title, order_index, content, pov, location, story_time, status, goal, notes, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                params![scene.id, scene.chapter_id, scene.title, scene.order_index, scene.content, scene.pov, scene.location, scene.story_time, scene.status, scene.goal, scene.notes, timestamp],
            )
            .map_err(|error| sql_error("Kapiteltext konnte nicht importiert werden", error))?;
        let (version_id, _) =
            insert_scene_version_in_transaction(&transaction, &scene, &timestamp, "before_import")?;
        version_ids.push((scene.id.clone(), version_id));
        scenes.push(scene.clone());
        chapters.push(Chapter {
            scenes: vec![scene],
            ..chapter
        });
    }
    transaction
        .execute(
            "UPDATE projects SET updated_at=?1 WHERE id=?2",
            params![timestamp, input.project_id],
        )
        .map_err(|error| sql_error("Projektzeitpunkt konnte nicht aktualisiert werden", error))?;
    transaction
        .commit()
        .map_err(|error| sql_error("Import konnte nicht abgeschlossen werden", error))?;

    let mut versions = Vec::with_capacity(version_ids.len());
    for (scene_id, version_id) in version_ids {
        let version = load_scene_versions(db, &scene_id)?
            .into_iter()
            .find(|item| item.id == version_id)
            .ok_or_else(|| {
                "Eine importierte Szenenversion konnte nicht geladen werden.".to_string()
            })?;
        versions.push(version);
    }
    Ok(ManuscriptImportResult {
        chapters,
        scenes,
        versions,
    })
}

#[tauri::command]
pub fn import_manuscript(
    state: State<'_, DbState>,
    input: ManuscriptImportInput,
) -> Result<ManuscriptImportResult, String> {
    let db = lock_db(&state)?;
    import_manuscript_in_db(&db, input)
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
        insert_source_reference_if_missing_tx(
            &transaction,
            &CreateSourceReferenceInput {
                project_id: input.project_id.clone(),
                entity_id: Some(id.clone()),
                proposal_id: None,
                chapter_id,
                scene_id: input.scene_id.clone().unwrap_or_default(),
                excerpt: input.excerpt.clone(),
                start_offset: None,
                end_offset: None,
            },
        )?;
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
        insert_source_reference_if_missing_tx(
            &transaction,
            &CreateSourceReferenceInput {
                project_id: input.project_id.clone(),
                entity_id: Some(input.id.clone()),
                proposal_id: None,
                chapter_id: chapter_id.to_string(),
                scene_id: scene_id.to_string(),
                excerpt: input.excerpt.clone(),
                start_offset: None,
                end_offset: None,
            },
        )?;
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

fn insert_source_reference_if_missing_tx(
    transaction: &rusqlite::Transaction<'_>,
    input: &CreateSourceReferenceInput,
) -> Result<String, String> {
    let existing = transaction
        .query_row(
            "SELECT id FROM story_source_references
             WHERE project_id=?1 AND entity_id IS ?2 AND chapter_id=?3 AND scene_id=?4
               AND excerpt=?5 AND start_offset IS ?6 AND end_offset IS ?7
             LIMIT 1",
            params![
                input.project_id,
                input.entity_id,
                input.chapter_id,
                input.scene_id,
                input.excerpt,
                input.start_offset,
                input.end_offset
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| sql_error("Vorhandene Quellen konnten nicht geprüft werden", error))?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = new_id();
    transaction
        .execute(
            "INSERT INTO story_source_references
             (id, project_id, entity_id, proposal_id, chapter_id, scene_id, excerpt, start_offset, end_offset, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                id,
                input.project_id,
                input.entity_id,
                input.proposal_id,
                input.chapter_id,
                input.scene_id,
                input.excerpt,
                input.start_offset,
                input.end_offset,
                now()
            ],
        )
        .map_err(|error| sql_error("Quellenreferenz konnte nicht gespeichert werden", error))?;
    Ok(id)
}

fn load_source_reference(db: &Connection, id: &str) -> Result<StorySourceReference, String> {
    db.query_row(
        "SELECT id, project_id, entity_id, proposal_id, chapter_id, scene_id, excerpt, start_offset, end_offset, created_at
         FROM story_source_references WHERE id=?1",
        params![id],
        source_from_row,
    )
    .map_err(|error| sql_error("Gespeicherte Quellenreferenz konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn create_source_reference(
    state: State<'_, DbState>,
    input: CreateSourceReferenceInput,
) -> Result<StorySourceReference, String> {
    let db = lock_db(&state)?;
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Quellenreferenz konnte nicht gespeichert werden", error))?;
    let id = insert_source_reference_if_missing_tx(&transaction, &input)?;
    transaction
        .commit()
        .map_err(|error| sql_error("Quellenreferenz konnte nicht abgeschlossen werden", error))?;
    load_source_reference(&db, &id)
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
        analyzed_content: row.get(6)?,
        status: row.get(7)?,
        created_at: row.get(8)?,
        completed_at: row.get(9)?,
        error_message: row.get(10)?,
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
    db.query_row("SELECT id, project_id, scene_id, scene_updated_at, content_hash, extractor_id, analyzed_content, status, created_at, completed_at, error_message FROM bible_update_runs WHERE id=?1", params![id], run_from_row).map_err(|error| sql_error("Bible-Update-Lauf konnte nicht geladen werden", error))
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
    db.execute("INSERT INTO bible_update_runs (id, project_id, scene_id, scene_updated_at, content_hash, extractor_id, analyzed_content, status, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,'pending',?8)", params![id, input.project_id, input.scene_id, input.scene_updated_at, input.content_hash, input.extractor_id, input.analyzed_content, timestamp]).map_err(|error| sql_error("Bible-Update-Lauf konnte nicht angelegt werden", error))?;
    load_run(&db, &id)
}

#[tauri::command]
pub fn list_bible_update_runs(
    state: State<'_, DbState>,
    project_id: String,
    scene_id: Option<String>,
) -> Result<Vec<BibleUpdateRun>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id, project_id, scene_id, scene_updated_at, content_hash, extractor_id, analyzed_content, status, created_at, completed_at, error_message FROM bible_update_runs WHERE project_id=?1 AND (?2 IS NULL OR scene_id=?2) ORDER BY created_at DESC").map_err(|error| sql_error("Bible-Update-Läufe konnten nicht geladen werden", error))?;
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
    if proposal.review_status != "pending" {
        return Err(
            "Dieser Vorschlag wurde bereits geprüft und kann nicht erneut geändert werden.".into(),
        );
    }
    let decision = input
        .decision
        .clone()
        .unwrap_or_else(|| match input.review_status.as_str() {
            "accepted" => "accept".into(),
            "edited" => "edit_accept".into(),
            "rejected" => "reject".into(),
            _ => "defer".into(),
        });
    if decision == "defer" {
        if input.review_status != "pending" {
            return Err(
                "Eine zurückgestellte Entscheidung muss den Review-Status pending behalten.".into(),
            );
        }
        return Ok(proposal);
    }
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
    validate_entity_type(&proposal.entity_type)?;
    validate_entity_status(&status)?;
    validate_proposal_classification(&classification)?;
    let is_rejected = decision == "reject" || input.review_status == "rejected";
    if is_rejected && input.review_status != "rejected" {
        return Err("Ein verworfener Vorschlag benötigt den Review-Status rejected.".into());
    }
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Review konnte nicht gestartet werden", error))?;
    let mut entity_id = proposal.target_entity_id.clone();
    let contradiction = proposal.classification == "possible_contradiction"
        || proposal.proposal_action == "mark_contradiction";
    let effective_decision = if contradiction && decision == "accept" {
        "mark_contradiction"
    } else {
        decision.as_str()
    };
    let effective_type = if effective_decision == "save_author_note" {
        "author_note"
    } else if effective_decision == "accept_retcon" {
        "retcon"
    } else {
        proposal.entity_type.as_str()
    };
    let (effective_status, author_confirmed, origin) = match effective_decision {
        "save_uncertain" => ("uncertain", false, "bible_update"),
        "save_author_note" => ("confirmed", true, "bible_update"),
        "accept_retcon" => ("retconned", true, "edited"),
        "mark_contradiction" => ("contradicted", false, "bible_update"),
        "keep_existing" => (status.as_str(), false, "bible_update"),
        "edit_accept" | "accept_new_value" => ("confirmed", true, "edited"),
        "accept" => ("confirmed", true, "bible_update"),
        "reject" => (status.as_str(), false, "bible_update"),
        _ => return Err(format!("Unbekannte Review-Aktion: {effective_decision}")),
    };

    if !is_rejected {
        let target_required = matches!(
            effective_decision,
            "keep_existing" | "mark_contradiction" | "accept_new_value"
        ) || proposal.proposal_action == "update_entity";
        if target_required && entity_id.is_none() {
            return Err("Der Vorschlag hat keinen Ziel-Eintrag.".into());
        }
        if effective_decision == "keep_existing" {
            // Keep the canonical value untouched, but retain this evidence as a
            // second source so the later decision remains auditable.
        } else if effective_decision == "mark_contradiction" {
            let target = entity_id
                .as_deref()
                .ok_or_else(|| "Der Widerspruch hat keinen Ziel-Eintrag.".to_string())?;
            let changed = transaction
                .execute("UPDATE story_entities SET status='contradicted', updated_at=?2 WHERE id=?1 AND project_id=?3", params![target, now(), proposal.project_id])
                .map_err(|error| sql_error("Widerspruch konnte nicht markiert werden", error))?;
            if changed == 0 {
                return Err("Der Ziel-Eintrag des Widerspruchs wurde nicht gefunden.".into());
            }
        } else {
            let target = if let Some(target) = entity_id.clone() {
                target
            } else {
                let (chapter, scene) = transaction
                    .query_row("SELECT chapters.title, scenes.title FROM scenes JOIN chapters ON chapters.id=scenes.chapter_id WHERE scenes.id=?1", params![proposal.scene_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
                    .map_err(|error| sql_error("Quellenszene konnte nicht geladen werden", error))?;
                let inserted = insert_entity_tx(
                    &transaction,
                    &proposal.project_id,
                    &name,
                    effective_type,
                    &description,
                    effective_status,
                    proposal.confidence,
                    &chapter,
                    &scene,
                    &proposal.evidence_excerpt,
                    author_confirmed,
                    &[],
                    origin,
                    &now(),
                )?;
                entity_id = Some(inserted.clone());
                inserted
            };
            // Existing targets are changed only by an explicit review action;
            // contradictions never overwrite the candidate value.
            if proposal.target_entity_id.is_some() {
                let changed = if proposal.proposal_action == "add_source" && effective_decision == "accept" {
                    transaction.execute("UPDATE story_entities SET status=?2, author_confirmed=?3, updated_at=?4, origin=?5 WHERE id=?1 AND project_id=?6", params![target, effective_status, author_confirmed, now(), origin, proposal.project_id])
                } else {
                    transaction.execute("UPDATE story_entities SET name=?2, entity_type=?3, description=?4, status=?5, confidence=?6, author_confirmed=?7, updated_at=?8, origin=?9 WHERE id=?1 AND project_id=?10", params![target, name, effective_type, description, effective_status, proposal.confidence, author_confirmed, now(), origin, proposal.project_id])
                }.map_err(|error| sql_error("Story-Bible-Eintrag konnte nicht geändert werden", error))?;
                if changed == 0 {
                    return Err("Der Ziel-Eintrag des Vorschlags wurde nicht gefunden.".into());
                }
            }
        }
        let chapter_id: String = transaction
            .query_row(
                "SELECT chapter_id FROM scenes WHERE id=?1",
                params![proposal.scene_id],
                |row| row.get(0),
            )
            .map_err(|error| sql_error("Quellenszene konnte nicht geladen werden", error))?;
        insert_source_reference_if_missing_tx(
            &transaction,
            &CreateSourceReferenceInput {
                project_id: proposal.project_id.clone(),
                entity_id: entity_id.clone(),
                proposal_id: Some(proposal.id.clone()),
                chapter_id,
                scene_id: proposal.scene_id.clone(),
                excerpt: proposal.evidence_excerpt.clone(),
                start_offset: proposal.start_offset,
                end_offset: proposal.end_offset,
            },
        )?;
    }
    transaction.execute("UPDATE bible_proposals SET target_entity_id=?2, candidate_name=?3, candidate_description=?4, candidate_status=?5, classification=?6, review_status=?7, reviewed_at=?8 WHERE id=?1", params![proposal.id, entity_id, name, description, effective_status, classification, input.review_status, now()]).map_err(|error| sql_error("Review-Status konnte nicht gespeichert werden", error))?;
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

fn load_ai_provider_settings(db: &Connection) -> Result<AiProviderSettings, String> {
    let value = db
        .query_row(
            "SELECT value_json FROM app_settings WHERE key='ai_provider_settings'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| {
            sql_error(
                "KI-Anbieter-Einstellungen konnten nicht geladen werden",
                error,
            )
        })?;
    match value {
        Some(json) => serde_json::from_str(&json)
            .map_err(|error| sql_error("KI-Anbieter-Einstellungen sind ungültig", error)),
        None => Ok(AiProviderSettings::default()),
    }
}

fn validate_ai_provider_settings(settings: &AiProviderSettings) -> Result<(), String> {
    if !matches!(
        settings.active_provider.as_str(),
        "local-prototype" | "codex-cli"
    ) {
        return Err("Unbekannter KI-Anbieter.".into());
    }
    if !(1..=900).contains(&settings.bible_update_timeout_seconds)
        || !(1..=900).contains(&settings.chat_timeout_seconds)
    {
        return Err("Timeouts müssen zwischen 1 und 900 Sekunden liegen.".into());
    }
    if settings
        .codex_binary_path
        .as_deref()
        .is_some_and(|path| path.len() > 1000 || path.contains('\0'))
    {
        return Err("Der Codex-Pfad ist ungültig.".into());
    }
    if settings
        .codex_model_override
        .as_deref()
        .is_some_and(|model| model.len() > 100 || model.starts_with('-'))
    {
        return Err("Die optionale Modellkennung ist ungültig.".into());
    }
    codex::validate_codex_privacy(settings).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_ai_provider_settings(state: State<'_, DbState>) -> Result<AiProviderSettings, String> {
    let db = lock_db(&state)?;
    load_ai_provider_settings(&db)
}

#[tauri::command]
pub fn save_ai_provider_settings(
    state: State<'_, DbState>,
    input: AiProviderSettings,
) -> Result<AiProviderSettings, String> {
    validate_ai_provider_settings(&input)?;
    let db = lock_db(&state)?;
    let json = serde_json::to_string(&input).map_err(|error| {
        sql_error(
            "KI-Anbieter-Einstellungen konnten nicht vorbereitet werden",
            error,
        )
    })?;
    let timestamp = now();
    db.execute("INSERT INTO app_settings (key, value_json, created_at, updated_at) VALUES ('ai_provider_settings', ?1, ?2, ?2) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json, updated_at=excluded.updated_at", params![json, timestamp]).map_err(|error| sql_error("KI-Anbieter-Einstellungen konnten nicht gespeichert werden", error))?;
    Ok(input)
}

#[tauri::command]
pub fn get_codex_provider_status(
    state: State<'_, DbState>,
) -> Result<CodexCliCapabilities, String> {
    let db = lock_db(&state)?;
    let settings = load_ai_provider_settings(&db)?;
    Ok(codex::codex_status(settings.codex_binary_path.as_deref()))
}

fn record_codex_audit(
    db: &Connection,
    input: &RunCodexTaskInput,
    status: &str,
    error_code: Option<&str>,
) -> Result<(), String> {
    let payload = codex::task_audit_payload(input, status, error_code).to_string();
    db.execute("INSERT INTO analysis_jobs (id, job_type, status, progress, payload_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6) ON CONFLICT(id) DO UPDATE SET status=excluded.status, progress=excluded.progress, payload_json=excluded.payload_json, updated_at=excluded.updated_at", params![input.task_id, format!("codex:{:?}", input.task_kind), status, if status == "completed" { 1.0 } else { 0.0 }, payload, now()]).map_err(|error| sql_error("Codex-Task-Audit konnte nicht gespeichert werden", error))?;
    Ok(())
}

#[tauri::command]
pub async fn run_codex_task(
    state: State<'_, DbState>,
    runtime: State<'_, Arc<CodexRuntimeState>>,
    input: RunCodexTaskInput,
) -> Result<codex::CodexTaskResult, String> {
    let settings = {
        let db = lock_db(&state)?;
        let settings = load_ai_provider_settings(&db)?;
        record_codex_audit(&db, &input, "running", None)?;
        settings
    };
    let runtime = runtime.inner().clone();
    let task_input = input.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        codex::run_task(runtime, task_input, settings)
    })
    .await
    .map_err(|error| {
        format!("CODEX_PROCESS_FAILED: Codex-Task konnte nicht ausgeführt werden: {error}")
    })?;
    let db = lock_db(&state)?;
    match &result {
        Ok(_) => record_codex_audit(&db, &input, "completed", None)?,
        Err(error) => record_codex_audit(&db, &input, "failed", Some(error.code))?,
    }
    result.map_err(|error: CodexError| error.to_string())
}

#[tauri::command]
pub fn cancel_codex_task(
    runtime: State<'_, Arc<CodexRuntimeState>>,
    task_id: String,
) -> Result<(), String> {
    codex::cancel_task(runtime.inner(), &task_id).map_err(|error| error.to_string())
}

fn project_entity_exists(
    db: &Connection,
    project_id: &str,
    entity_id: &str,
    expected_type: Option<&str>,
) -> Result<(), String> {
    let entity_type: Option<String> = db
        .query_row(
            "SELECT entity_type FROM story_entities WHERE id=?1 AND project_id=?2",
            params![entity_id, project_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| sql_error("Story-Bible-Eintrag konnte nicht geprüft werden", error))?;
    match entity_type {
        Some(value) if expected_type.is_none_or(|expected| expected == value) => Ok(()),
        Some(_) => Err("Der Eintrag besitzt nicht den erwarteten Typ.".into()),
        None => Err("Der Story-Bible-Eintrag wurde im Projekt nicht gefunden.".into()),
    }
}

fn lore_from_row(row: &rusqlite::Row<'_>) -> SqlResult<LoreMetadata> {
    Ok(LoreMetadata {
        entity_id: row.get(0)?,
        project_id: row.get(1)?,
        category: row.get(2)?,
        scope: row.get(3)?,
        reveal_state: row.get(4)?,
        importance: row.get(5)?,
        truth_statement: row.get(6)?,
        rules_text: row.get(7)?,
        exceptions_text: row.get(8)?,
        author_knowledge: row.get(9)?,
        reader_knowledge: row.get(10)?,
        reveal_plan: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}
fn profile_from_row(row: &rusqlite::Row<'_>) -> SqlResult<CharacterProfile> {
    Ok(CharacterProfile {
        entity_id: row.get(0)?,
        project_id: row.get(1)?,
        core_want: row.get(2)?,
        core_need: row.get(3)?,
        fears: row.get(4)?,
        false_belief: row.get(5)?,
        values: row.get(6)?,
        strengths: row.get(7)?,
        flaws: row.get(8)?,
        pressure_behavior: row.get(9)?,
        voice: row.get(10)?,
        backstory: row.get(11)?,
        arc_summary: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}
fn state_from_row(row: &rusqlite::Row<'_>) -> SqlResult<CharacterSceneState> {
    Ok(CharacterSceneState {
        id: row.get(0)?,
        project_id: row.get(1)?,
        character_entity_id: row.get(2)?,
        scene_id: row.get(3)?,
        emotional_state: row.get(4)?,
        physical_state: row.get(5)?,
        goal: row.get(6)?,
        conflict: row.get(7)?,
        knowledge_notes: row.get(8)?,
        relationship_state: row.get(9)?,
        change_note: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}
fn style_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProjectStyle> {
    Ok(ProjectStyle {
        project_id: row.get(0)?,
        narrative_pov: row.get(1)?,
        tense: row.get(2)?,
        sentence_style: row.get(3)?,
        dialogue_style: row.get(4)?,
        description_density: row.get(5)?,
        inner_monologue: row.get(6)?,
        preferred_patterns: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(7)?)
            .unwrap_or_default(),
        avoided_patterns: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(8)?)
            .unwrap_or_default(),
        notes: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}
fn reference_from_row(row: &rusqlite::Row<'_>) -> SqlResult<StyleReference> {
    Ok(StyleReference {
        id: row.get(0)?,
        project_id: row.get(1)?,
        chapter_id: row.get(2)?,
        scene_id: row.get(3)?,
        start_offset: row.get(4)?,
        end_offset: row.get(5)?,
        category: row.get(6)?,
        label: row.get(7)?,
        excerpt: row.get(8)?,
        notes: row.get(9)?,
        weight: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn relation_from_row(row: &rusqlite::Row<'_>) -> SqlResult<StoryEntityRelation> {
    Ok(StoryEntityRelation {
        id: row.get(0)?,
        project_id: row.get(1)?,
        source_entity_id: row.get(2)?,
        target_entity_id: row.get(3)?,
        relation_type: row.get(4)?,
        label: row.get(5)?,
        author_confirmed: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

#[tauri::command]
pub fn get_lore_metadata(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<LoreMetadata>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT entity_id, project_id, category, scope, reveal_state, importance, truth_statement, rules_text, exceptions_text, author_knowledge, reader_knowledge, reveal_plan, created_at, updated_at FROM lore_metadata WHERE project_id=?1 ORDER BY updated_at DESC").map_err(|error| sql_error("Lore konnte nicht geladen werden", error))?;
    let result = statement
        .query_map(params![project_id], lore_from_row)
        .map_err(|error| sql_error("Lore konnte nicht geladen werden", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| sql_error("Lore konnte nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn save_lore_metadata(
    state: State<'_, DbState>,
    input: SaveLoreMetadataInput,
) -> Result<LoreMetadata, String> {
    required(&input.entity_id, "Die Lore-Entität")?;
    validate_lore_category(&input.category)?;
    validate_lore_scope(&input.scope)?;
    validate_lore_reveal_state(&input.reveal_state)?;
    validate_lore_importance(&input.importance)?;
    let db = lock_db(&state)?;
    project_entity_exists(&db, &input.project_id, &input.entity_id, None)?;
    let timestamp = now();
    db.execute("INSERT INTO lore_metadata (entity_id, project_id, category, scope, reveal_state, importance, truth_statement, rules_text, exceptions_text, author_knowledge, reader_knowledge, reveal_plan, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13) ON CONFLICT(entity_id) DO UPDATE SET category=excluded.category, scope=excluded.scope, reveal_state=excluded.reveal_state, importance=excluded.importance, truth_statement=excluded.truth_statement, rules_text=excluded.rules_text, exceptions_text=excluded.exceptions_text, author_knowledge=excluded.author_knowledge, reader_knowledge=excluded.reader_knowledge, reveal_plan=excluded.reveal_plan, updated_at=excluded.updated_at", params![input.entity_id, input.project_id, input.category, input.scope, input.reveal_state, input.importance, input.truth_statement, input.rules_text, input.exceptions_text, input.author_knowledge, input.reader_knowledge, input.reveal_plan, timestamp]).map_err(|error| sql_error("Lore konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT entity_id, project_id, category, scope, reveal_state, importance, truth_statement, rules_text, exceptions_text, author_knowledge, reader_knowledge, reveal_plan, created_at, updated_at FROM lore_metadata WHERE entity_id=?1", params![input.entity_id], lore_from_row).map_err(|error| sql_error("Gespeicherte Lore konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn get_character_profile(
    state: State<'_, DbState>,
    entity_id: String,
) -> Result<Option<CharacterProfile>, String> {
    let db = lock_db(&state)?;
    db.query_row("SELECT entity_id, project_id, core_want, core_need, fears, false_belief, values_text, strengths, flaws, pressure_behavior, voice, backstory, arc_summary, created_at, updated_at FROM character_profiles WHERE entity_id=?1", params![entity_id], profile_from_row).optional().map_err(|error| sql_error("Charakterprofil konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn save_character_profile(
    state: State<'_, DbState>,
    input: SaveCharacterProfileInput,
) -> Result<CharacterProfile, String> {
    let db = lock_db(&state)?;
    project_entity_exists(&db, &input.project_id, &input.entity_id, Some("character"))?;
    let timestamp = now();
    db.execute("INSERT INTO character_profiles (entity_id, project_id, core_want, core_need, fears, false_belief, values_text, strengths, flaws, pressure_behavior, voice, backstory, arc_summary, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14) ON CONFLICT(entity_id) DO UPDATE SET core_want=excluded.core_want, core_need=excluded.core_need, fears=excluded.fears, false_belief=excluded.false_belief, values_text=excluded.values_text, strengths=excluded.strengths, flaws=excluded.flaws, pressure_behavior=excluded.pressure_behavior, voice=excluded.voice, backstory=excluded.backstory, arc_summary=excluded.arc_summary, updated_at=excluded.updated_at", params![input.entity_id, input.project_id, input.core_want, input.core_need, input.fears, input.false_belief, input.values, input.strengths, input.flaws, input.pressure_behavior, input.voice, input.backstory, input.arc_summary, timestamp]).map_err(|error| sql_error("Charakterprofil konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT entity_id, project_id, core_want, core_need, fears, false_belief, values_text, strengths, flaws, pressure_behavior, voice, backstory, arc_summary, created_at, updated_at FROM character_profiles WHERE entity_id=?1", params![input.entity_id], profile_from_row).map_err(|error| sql_error("Gespeichertes Charakterprofil konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn list_character_scene_states(
    state: State<'_, DbState>,
    project_id: String,
    scene_id: Option<String>,
    character_entity_id: Option<String>,
) -> Result<Vec<CharacterSceneState>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id, project_id, character_entity_id, scene_id, emotional_state, physical_state, goal, conflict, knowledge, relationship_state, change_note, created_at, updated_at FROM character_scene_states WHERE project_id=?1 AND (?2 IS NULL OR scene_id=?2) AND (?3 IS NULL OR character_entity_id=?3) ORDER BY updated_at DESC").map_err(|error| sql_error("Szenenzustände konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(
            params![project_id, scene_id, character_entity_id],
            state_from_row,
        )
        .map_err(|error| sql_error("Szenenzustände konnten nicht geladen werden", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| sql_error("Szenenzustände konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn save_character_scene_state(
    state: State<'_, DbState>,
    input: SaveCharacterSceneStateInput,
) -> Result<CharacterSceneState, String> {
    let db = lock_db(&state)?;
    project_entity_exists(
        &db,
        &input.project_id,
        &input.character_entity_id,
        Some("character"),
    )?;
    let scene_project: Option<String> = db.query_row("SELECT books.project_id FROM scenes JOIN chapters ON chapters.id=scenes.chapter_id JOIN books ON books.id=chapters.book_id WHERE scenes.id=?1", params![input.scene_id], |row| row.get(0)).optional().map_err(|error| sql_error("Szene konnte nicht geprüft werden", error))?;
    if scene_project.as_deref() != Some(input.project_id.as_str()) {
        return Err("Die Szene gehört nicht zu diesem Projekt.".into());
    }
    let id = input.id.clone().unwrap_or_else(new_id);
    let timestamp = now();
    db.execute("INSERT INTO character_scene_states (id, project_id, character_entity_id, scene_id, emotional_state, physical_state, goal, conflict, knowledge, relationship_state, change_note, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12) ON CONFLICT(character_entity_id, scene_id) DO UPDATE SET id=excluded.id, emotional_state=excluded.emotional_state, physical_state=excluded.physical_state, goal=excluded.goal, conflict=excluded.conflict, knowledge=excluded.knowledge, relationship_state=excluded.relationship_state, change_note=excluded.change_note, updated_at=excluded.updated_at", params![id, input.project_id, input.character_entity_id, input.scene_id, input.emotional_state, input.physical_state, input.goal, input.conflict, input.knowledge_notes, input.relationship_state, input.change_note, timestamp]).map_err(|error| sql_error("Szenenzustand konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT id, project_id, character_entity_id, scene_id, emotional_state, physical_state, goal, conflict, knowledge, relationship_state, change_note, created_at, updated_at FROM character_scene_states WHERE character_entity_id=?1 AND scene_id=?2", params![input.character_entity_id, input.scene_id], state_from_row).map_err(|error| sql_error("Gespeicherter Szenenzustand konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn get_project_style(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<Option<ProjectStyle>, String> {
    let db = lock_db(&state)?;
    db.query_row("SELECT project_id, narrative_pov, tense, sentence_style, dialogue_style, description_density, inner_monologue, preferred_patterns_json, avoided_patterns_json, notes, created_at, updated_at FROM project_styles WHERE project_id=?1", params![project_id], style_from_row).optional().map_err(|error| sql_error("Projektstil konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn save_project_style(
    state: State<'_, DbState>,
    input: SaveProjectStyleInput,
) -> Result<ProjectStyle, String> {
    let db = lock_db(&state)?;
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            params![input.project_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Projekt konnte nicht geprüft werden", error))?;
    if !exists {
        return Err("Das Projekt wurde nicht gefunden.".into());
    }
    let preferred =
        serde_json::to_string(&input.preferred_patterns).map_err(|error| error.to_string())?;
    let avoided =
        serde_json::to_string(&input.avoided_patterns).map_err(|error| error.to_string())?;
    let timestamp = now();
    db.execute("INSERT INTO project_styles (project_id, narrative_pov, tense, sentence_style, dialogue_style, description_density, inner_monologue, preferred_patterns_json, avoided_patterns_json, notes, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11) ON CONFLICT(project_id) DO UPDATE SET narrative_pov=excluded.narrative_pov, tense=excluded.tense, sentence_style=excluded.sentence_style, dialogue_style=excluded.dialogue_style, description_density=excluded.description_density, inner_monologue=excluded.inner_monologue, preferred_patterns_json=excluded.preferred_patterns_json, avoided_patterns_json=excluded.avoided_patterns_json, notes=excluded.notes, updated_at=excluded.updated_at", params![input.project_id, input.narrative_pov, input.tense, input.sentence_style, input.dialogue_style, input.description_density, input.inner_monologue, preferred, avoided, input.notes, timestamp]).map_err(|error| sql_error("Projektstil konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT project_id, narrative_pov, tense, sentence_style, dialogue_style, description_density, inner_monologue, preferred_patterns_json, avoided_patterns_json, notes, created_at, updated_at FROM project_styles WHERE project_id=?1", params![input.project_id], style_from_row).map_err(|error| sql_error("Gespeicherter Projektstil konnte nicht geladen werden", error))
}

fn style_analysis_run_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProjectStyleAnalysisRun> {
    Ok(ProjectStyleAnalysisRun {
        id: row.get(0)?,
        project_id: row.get(1)?,
        source_hash: row.get(2)?,
        provider_id: row.get(3)?,
        status: row.get(4)?,
        created_at: row.get(5)?,
        completed_at: row.get(6)?,
        error_message: row.get(7)?,
    })
}

fn style_observation_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProjectStyleObservation> {
    let evidence_json: String = row.get(7)?;
    Ok(ProjectStyleObservation {
        id: row.get(0)?,
        run_id: row.get(1)?,
        project_id: row.get(2)?,
        observation_type: row.get(3)?,
        observation_text: row.get(4)?,
        recommendation: row.get(5)?,
        confidence: row.get(6)?,
        evidence: serde_json::from_str(&evidence_json).unwrap_or_default(),
        review_status: row.get(8)?,
        reviewed_at: row.get(9)?,
        created_at: row.get(10)?,
    })
}

fn narrative_summary_from_row(row: &rusqlite::Row<'_>) -> SqlResult<NarrativeSummary> {
    Ok(NarrativeSummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        scope_type: row.get(2)?,
        scope_id: row.get(3)?,
        content_hash: row.get(4)?,
        summary: row.get(5)?,
        important_events: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
        open_threads: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
        character_changes: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
        status: row.get(9)?,
        author_confirmed: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn validate_style_observation_type(value: &str) -> Result<(), String> {
    if matches!(
        value,
        "narrative_pov"
            | "tense"
            | "sentence_rhythm"
            | "dialogue"
            | "description"
            | "inner_monologue"
            | "humor"
            | "pacing"
            | "vocabulary"
            | "transitions"
            | "tension"
            | "avoidance"
            | "character_voice_separation"
    ) {
        Ok(())
    } else {
        Err(format!("Unbekannter Stilbeobachtungstyp: {value}"))
    }
}

fn validate_style_analysis_status(value: &str) -> Result<(), String> {
    if matches!(value, "pending" | "running" | "completed" | "failed") {
        Ok(())
    } else {
        Err(format!("Ungültiger Stilanalyse-Status: {value}"))
    }
}

#[tauri::command]
pub fn create_project_style_analysis_run(
    state: State<'_, DbState>,
    input: CreateProjectStyleAnalysisRunInput,
) -> Result<ProjectStyleAnalysisRun, String> {
    required(&input.source_hash, "Der Stil-Hash")?;
    let db = lock_db(&state)?;
    project_from_db(&db, &input.project_id)?;
    let id = new_id();
    let stamp = now();
    db.execute("INSERT INTO project_style_analysis_runs(id,project_id,source_hash,provider_id,status,created_at) VALUES(?1,?2,?3,?4,'running',?5)", params![id, input.project_id, input.source_hash, input.provider_id, stamp]).map_err(|e| sql_error("Stilanalyse konnte nicht angelegt werden", e))?;
    db.query_row("SELECT id,project_id,source_hash,provider_id,status,created_at,completed_at,error_message FROM project_style_analysis_runs WHERE id=?1", params![id], style_analysis_run_from_row).map_err(|e| sql_error("Stilanalyse konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn complete_project_style_analysis_run(
    state: State<'_, DbState>,
    id: String,
) -> Result<ProjectStyleAnalysisRun, String> {
    let db = lock_db(&state)?;
    let changed = db.execute("UPDATE project_style_analysis_runs SET status='completed',completed_at=?1,error_message=NULL WHERE id=?2", params![now(), id]).map_err(|e| sql_error("Stilanalyse konnte nicht abgeschlossen werden", e))?;
    if changed == 0 {
        return Err("Stilanalyse nicht gefunden.".into());
    }
    db.query_row("SELECT id,project_id,source_hash,provider_id,status,created_at,completed_at,error_message FROM project_style_analysis_runs WHERE id=?1", params![id], style_analysis_run_from_row).map_err(|e| sql_error("Stilanalyse konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn fail_project_style_analysis_run(
    state: State<'_, DbState>,
    id: String,
    error_message: String,
) -> Result<ProjectStyleAnalysisRun, String> {
    let db = lock_db(&state)?;
    let changed = db.execute("UPDATE project_style_analysis_runs SET status='failed',completed_at=?1,error_message=?2 WHERE id=?3", params![now(), error_message.chars().take(1000).collect::<String>(), id]).map_err(|e| sql_error("Stilanalyse konnte nicht fehlgeschlagen markiert werden", e))?;
    if changed == 0 {
        return Err("Stilanalyse nicht gefunden.".into());
    }
    db.query_row("SELECT id,project_id,source_hash,provider_id,status,created_at,completed_at,error_message FROM project_style_analysis_runs WHERE id=?1", params![id], style_analysis_run_from_row).map_err(|e| sql_error("Stilanalyse konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn list_project_style_analysis_runs(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<ProjectStyleAnalysisRun>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id,project_id,source_hash,provider_id,status,created_at,completed_at,error_message FROM project_style_analysis_runs WHERE project_id=?1 ORDER BY created_at DESC").map_err(|e| sql_error("Stilanalysen konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(params![project_id], style_analysis_run_from_row)
        .map_err(|e| sql_error("Stilanalysen konnten nicht geladen werden", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error("Stilanalysen konnten nicht geladen werden", e));
    result
}

#[tauri::command]
pub fn save_project_style_observations(
    state: State<'_, DbState>,
    run_id: String,
    observations: Vec<SaveProjectStyleObservationInput>,
) -> Result<Vec<ProjectStyleObservation>, String> {
    if observations.len() > 100 {
        return Err("Zu viele Stilbeobachtungen.".into());
    }
    let db = lock_db(&state)?;
    let run: (String, String) = db
        .query_row(
            "SELECT project_id,status FROM project_style_analysis_runs WHERE id=?1",
            params![run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| sql_error("Stilanalyse konnte nicht geprüft werden", e))?;
    let tx = db
        .unchecked_transaction()
        .map_err(|e| sql_error("Stilbeobachtungen konnten nicht gestartet werden", e))?;
    for observation in observations {
        if observation.run_id != run_id || observation.project_id != run.0 {
            return Err("Stilbeobachtung gehört nicht zum Analyse-Lauf.".into());
        }
        validate_style_observation_type(&observation.observation_type)?;
        if !(0.0..=1.0).contains(&observation.confidence)
            || observation.observation_text.trim().is_empty()
            || observation.observation_text.chars().count() > 2000
            || observation.recommendation.chars().count() > 2000
        {
            return Err("Ungültige Stilbeobachtung.".into());
        }
        let review_status = observation.review_status.as_deref().unwrap_or("pending");
        if !matches!(
            review_status,
            "pending" | "accepted" | "edited" | "rejected"
        ) {
            return Err("Ungültiger Reviewstatus der Stilbeobachtung.".into());
        }
        let evidence = serde_json::to_string(&observation.evidence)
            .map_err(|e| sql_error("Stilbeleg konnte nicht serialisiert werden", e))?;
        tx.execute("INSERT INTO project_style_observations(id,run_id,project_id,observation_type,observation_text,recommendation,confidence,evidence_json,review_status,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", params![new_id(), observation.run_id, observation.project_id, observation.observation_type, observation.observation_text, observation.recommendation, observation.confidence, evidence, review_status, now()]).map_err(|e| sql_error("Stilbeobachtung konnte nicht gespeichert werden", e))?;
    }
    tx.commit()
        .map_err(|e| sql_error("Stilbeobachtungen konnten nicht gespeichert werden", e))?;
    list_project_style_observations_from_db(&db, &run.0, Some(&run_id))
}

fn list_project_style_observations_from_db(
    db: &Connection,
    project_id: &str,
    run_id: Option<&str>,
) -> Result<Vec<ProjectStyleObservation>, String> {
    let mut statement = db.prepare("SELECT id,run_id,project_id,observation_type,observation_text,recommendation,confidence,evidence_json,review_status,reviewed_at,created_at FROM project_style_observations WHERE project_id=?1 AND (?2 IS NULL OR run_id=?2) ORDER BY created_at ASC").map_err(|e| sql_error("Stilbeobachtungen konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(params![project_id, run_id], style_observation_from_row)
        .map_err(|e| sql_error("Stilbeobachtungen konnten nicht geladen werden", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error("Stilbeobachtungen konnten nicht geladen werden", e));
    result
}

#[tauri::command]
pub fn list_project_style_observations(
    state: State<'_, DbState>,
    project_id: String,
    run_id: Option<String>,
) -> Result<Vec<ProjectStyleObservation>, String> {
    let db = lock_db(&state)?;
    list_project_style_observations_from_db(&db, &project_id, run_id.as_deref())
}

#[tauri::command]
pub fn review_project_style_observation(
    state: State<'_, DbState>,
    id: String,
    review_status: String,
    observation_text: Option<String>,
    recommendation: Option<String>,
) -> Result<ProjectStyleObservation, String> {
    if !matches!(
        review_status.as_str(),
        "pending" | "accepted" | "edited" | "rejected"
    ) {
        return Err("Ungültiger Reviewstatus der Stilbeobachtung.".into());
    }
    let db = lock_db(&state)?;
    let changed = db.execute("UPDATE project_style_observations SET review_status=?1,observation_text=COALESCE(?2,observation_text),recommendation=COALESCE(?3,recommendation),reviewed_at=?4 WHERE id=?5", params![review_status, observation_text, recommendation, now(), id]).map_err(|e| sql_error("Stilbeobachtung konnte nicht reviewt werden", e))?;
    if changed == 0 {
        return Err("Stilbeobachtung nicht gefunden.".into());
    }
    db.query_row("SELECT id,run_id,project_id,observation_type,observation_text,recommendation,confidence,evidence_json,review_status,reviewed_at,created_at FROM project_style_observations WHERE id=?1", params![id], style_observation_from_row).map_err(|e| sql_error("Stilbeobachtung konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn list_narrative_summaries(
    state: State<'_, DbState>,
    project_id: String,
    scope_type: Option<String>,
    scope_id: Option<String>,
) -> Result<Vec<NarrativeSummary>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id,project_id,scope_type,scope_id,content_hash,summary,important_events_json,open_threads_json,character_changes_json,status,author_confirmed,created_at,updated_at FROM narrative_summaries WHERE project_id=?1 AND (?2 IS NULL OR scope_type=?2) AND (?3 IS NULL OR scope_id=?3) ORDER BY updated_at DESC").map_err(|e| sql_error("Zusammenfassungen konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(
            params![project_id, scope_type, scope_id],
            narrative_summary_from_row,
        )
        .map_err(|e| sql_error("Zusammenfassungen konnten nicht geladen werden", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error("Zusammenfassungen konnten nicht geladen werden", e));
    result
}

fn validate_summary_scope(value: &str) -> Result<(), String> {
    if matches!(value, "scene" | "chapter" | "book" | "project") {
        Ok(())
    } else {
        Err(format!("Ungültiger Zusammenfassungsbereich: {value}"))
    }
}
fn validate_summary_status(value: &str) -> Result<(), String> {
    if matches!(value, "proposed" | "confirmed" | "outdated" | "rejected") {
        Ok(())
    } else {
        Err(format!("Ungültiger Zusammenfassungsstatus: {value}"))
    }
}

#[tauri::command]
pub fn save_narrative_summary(
    state: State<'_, DbState>,
    input: SaveNarrativeSummaryInput,
) -> Result<NarrativeSummary, String> {
    validate_summary_scope(&input.scope_type)?;
    validate_summary_status(&input.status)?;
    required(&input.scope_id, "Die Zusammenfassungs-ID")?;
    required(&input.content_hash, "Der Zusammenfassungs-Hash")?;
    required(&input.summary, "Die Zusammenfassung")?;
    if input.summary.chars().count() > 20_000
        || input.important_events.len() > 100
        || input.open_threads.len() > 100
        || input.character_changes.len() > 100
    {
        return Err("Die Zusammenfassung ist zu groß.".into());
    }
    let db = lock_db(&state)?;
    project_from_db(&db, &input.project_id)?;
    let events = serde_json::to_string(&input.important_events)
        .map_err(|e| sql_error("Zusammenfassung konnte nicht serialisiert werden", e))?;
    let threads = serde_json::to_string(&input.open_threads)
        .map_err(|e| sql_error("Zusammenfassung konnte nicht serialisiert werden", e))?;
    let changes = serde_json::to_string(&input.character_changes)
        .map_err(|e| sql_error("Zusammenfassung konnte nicht serialisiert werden", e))?;
    let id = input.id.unwrap_or_else(new_id);
    let stamp = now();
    db.execute("INSERT INTO narrative_summaries(id,project_id,scope_type,scope_id,content_hash,summary,important_events_json,open_threads_json,character_changes_json,status,author_confirmed,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12) ON CONFLICT(project_id,scope_type,scope_id,content_hash) DO UPDATE SET summary=excluded.summary,important_events_json=excluded.important_events_json,open_threads_json=excluded.open_threads_json,character_changes_json=excluded.character_changes_json,status=excluded.status,author_confirmed=excluded.author_confirmed,updated_at=excluded.updated_at", params![id,input.project_id,input.scope_type,input.scope_id,input.content_hash,input.summary,events,threads,changes,input.status,input.author_confirmed,stamp]).map_err(|e| sql_error("Zusammenfassung konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,project_id,scope_type,scope_id,content_hash,summary,important_events_json,open_threads_json,character_changes_json,status,author_confirmed,created_at,updated_at FROM narrative_summaries WHERE project_id=?1 AND scope_type=?2 AND scope_id=?3 AND content_hash=?4", params![input.project_id,input.scope_type,input.scope_id,input.content_hash], narrative_summary_from_row).map_err(|e| sql_error("Gespeicherte Zusammenfassung konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn mark_narrative_summary_outdated(
    state: State<'_, DbState>,
    project_id: String,
    scope_type: String,
    scope_id: String,
    content_hash: String,
) -> Result<(), String> {
    validate_summary_scope(&scope_type)?;
    let db = lock_db(&state)?;
    db.execute("UPDATE narrative_summaries SET status='outdated',updated_at=?1 WHERE project_id=?2 AND scope_type=?3 AND scope_id=?4 AND content_hash<>?5 AND status='confirmed'", params![now(),project_id,scope_type,scope_id,content_hash]).map_err(|e| sql_error("Zusammenfassung konnte nicht veraltet markiert werden", e))?;
    Ok(())
}

#[tauri::command]
pub fn review_narrative_summary(
    state: State<'_, DbState>,
    id: String,
    status: String,
) -> Result<NarrativeSummary, String> {
    validate_summary_status(&status)?;
    let db = lock_db(&state)?;
    let changed = db.execute("UPDATE narrative_summaries SET status=?1,author_confirmed=?2,updated_at=?3 WHERE id=?4", params![status, if status == "confirmed" { 1 } else { 0 }, now(), id]).map_err(|e| sql_error("Zusammenfassung konnte nicht reviewt werden", e))?;
    if changed == 0 {
        return Err("Zusammenfassung nicht gefunden.".into());
    }
    db.query_row("SELECT id,project_id,scope_type,scope_id,content_hash,summary,important_events_json,open_threads_json,character_changes_json,status,author_confirmed,created_at,updated_at FROM narrative_summaries WHERE id=?1", params![id], narrative_summary_from_row).map_err(|e| sql_error("Zusammenfassung konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn list_style_references(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<StyleReference>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id, project_id, chapter_id, scene_id, start_offset, end_offset, category, label, excerpt, notes, weight, created_at, updated_at FROM style_references WHERE project_id=?1 ORDER BY created_at DESC").map_err(|error| sql_error("Stilreferenzen konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![project_id], reference_from_row)
        .map_err(|error| sql_error("Stilreferenzen konnten nicht geladen werden", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| sql_error("Stilreferenzen konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn create_style_reference(
    state: State<'_, DbState>,
    input: CreateStyleReferenceInput,
) -> Result<StyleReference, String> {
    required(&input.label, "Der Name der Stilreferenz")?;
    required(&input.excerpt, "Der Ausschnitt")?;
    validate_style_reference_category(&input.category)?;
    if !(0.1..=5.0).contains(&input.weight) {
        return Err("Das Gewicht muss zwischen 0,1 und 5,0 liegen.".into());
    }
    if input.label.chars().count() > 160
        || input.notes.chars().count() > 2000
        || input.excerpt.chars().count() > 10_000
    {
        return Err("Die Stilreferenz ist zu lang.".into());
    }
    match (input.start_offset, input.end_offset) {
        (Some(start), Some(end)) if start >= 0 && end > start => {}
        (None, None) => {}
        _ => return Err("Start- und Endoffset müssen gemeinsam angegeben werden.".into()),
    }
    let db = lock_db(&state)?;
    let scene_data: Option<(String, String)> = db.query_row("SELECT chapters.id, scenes.content FROM scenes JOIN chapters ON chapters.id=scenes.chapter_id JOIN books ON books.id=chapters.book_id WHERE scenes.id=?1 AND books.project_id=?2", params![input.scene_id, input.project_id], |row| Ok((row.get(0)?, row.get(1)?))).optional().map_err(|error| sql_error("Szene konnte nicht geprüft werden", error))?;
    let Some((scene_chapter_id, scene_content)) = scene_data else {
        return Err("Die Szene gehört nicht zu diesem Projekt.".into());
    };
    if let Some(chapter_id) = input.chapter_id.as_deref() {
        if chapter_id != scene_chapter_id {
            return Err("Kapitel und Szene passen nicht zusammen.".into());
        }
    }
    if let (Some(start), Some(end)) = (input.start_offset, input.end_offset) {
        let text = crate::services::plain_text::editor_content_to_plain_text(&scene_content);
        let chars: Vec<char> = text.chars().collect();
        if end as usize > chars.len()
            || input.excerpt
                != chars[start as usize..end as usize]
                    .iter()
                    .collect::<String>()
        {
            return Err("Die Stilreferenz passt nicht zum ausgewählten Szenentext.".into());
        }
    }
    let id = new_id();
    let timestamp = now();
    db.execute("INSERT INTO style_references (id, project_id, chapter_id, scene_id, start_offset, end_offset, category, label, excerpt, notes, weight, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12)", params![id, input.project_id, input.chapter_id.or(Some(scene_chapter_id)), input.scene_id, input.start_offset, input.end_offset, input.category, input.label, input.excerpt, input.notes, input.weight, timestamp]).map_err(|error| sql_error("Stilreferenz konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT id, project_id, chapter_id, scene_id, start_offset, end_offset, category, label, excerpt, notes, weight, created_at, updated_at FROM style_references WHERE id=?1", params![id], reference_from_row).map_err(|error| sql_error("Gespeicherte Stilreferenz konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn delete_style_reference(state: State<'_, DbState>, id: String) -> Result<(), String> {
    let db = lock_db(&state)?;
    db.execute("DELETE FROM style_references WHERE id=?1", params![id])
        .map_err(|error| sql_error("Stilreferenz konnte nicht entfernt werden", error))?;
    Ok(())
}

#[tauri::command]
pub fn update_style_reference(
    state: State<'_, DbState>,
    input: UpdateStyleReferenceInput,
) -> Result<StyleReference, String> {
    required(&input.label, "Der Name der Stilreferenz")?;
    validate_style_reference_category(&input.category)?;
    if !(0.1..=5.0).contains(&input.weight) {
        return Err("Das Gewicht muss zwischen 0,1 und 5,0 liegen.".into());
    }
    if input.label.chars().count() > 160 || input.notes.chars().count() > 2000 {
        return Err("Die Stilreferenz ist zu lang.".into());
    }
    let db = lock_db(&state)?;
    let changed = db.execute("UPDATE style_references SET label=?1, category=?2, notes=?3, weight=?4, updated_at=?5 WHERE id=?6 AND project_id=?7", params![input.label, input.category, input.notes, input.weight, now(), input.id, input.project_id]).map_err(|error| sql_error("Stilreferenz konnte nicht aktualisiert werden", error))?;
    if changed == 0 {
        return Err("Stilreferenz nicht gefunden.".into());
    }
    db.query_row("SELECT id, project_id, chapter_id, scene_id, start_offset, end_offset, category, label, excerpt, notes, weight, created_at, updated_at FROM style_references WHERE id=?1", params![input.id], reference_from_row).map_err(|error| sql_error("Aktualisierte Stilreferenz konnte nicht geladen werden", error))
}

fn load_lore_metadata(db: &Connection, entity_id: &str) -> Result<LoreMetadata, String> {
    db.query_row("SELECT entity_id, project_id, category, scope, reveal_state, importance, truth_statement, rules_text, exceptions_text, author_knowledge, reader_knowledge, reveal_plan, created_at, updated_at FROM lore_metadata WHERE entity_id=?1", params![entity_id], lore_from_row).map_err(|error| sql_error("Lore konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn create_lore_entry(
    state: State<'_, DbState>,
    input: CreateLoreEntryInput,
) -> Result<LoreEntry, String> {
    required(&input.name, "Der Lore-Name")?;
    validate_lore_entity_type(&input.entity_type)?;
    validate_entity_status(&input.status)?;
    validate_lore_category(&input.category)?;
    validate_lore_scope(&input.scope)?;
    validate_lore_reveal_state(&input.reveal_state)?;
    validate_lore_importance(&input.importance)?;
    if input.name.chars().count() > 240 || input.description.chars().count() > 10_000 {
        return Err("Der Lore-Eintrag ist zu lang.".into());
    }
    let db = lock_db(&state)?;
    let project_exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            params![input.project_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Projekt konnte nicht geprüft werden", error))?;
    if !project_exists {
        return Err("Das Projekt wurde nicht gefunden.".into());
    }
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Lore-Transaktion konnte nicht gestartet werden", error))?;
    let timestamp = now();
    let entity_id = insert_entity_tx(
        &transaction,
        &input.project_id,
        &input.name,
        &input.entity_type,
        &input.description,
        &input.status,
        if input.status == "confirmed" {
            1.0
        } else {
            0.7
        },
        "",
        "",
        "",
        true,
        &input.tags,
        "manual",
        &timestamp,
    )?;
    transaction.execute("INSERT INTO lore_metadata (entity_id, project_id, category, scope, reveal_state, importance, truth_statement, rules_text, exceptions_text, author_knowledge, reader_knowledge, reveal_plan, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13)", params![entity_id, input.project_id, input.category, input.scope, input.reveal_state, input.importance, input.truth_statement, input.rules_text, input.exceptions_text, input.author_knowledge, input.reader_knowledge, input.reveal_plan, timestamp]).map_err(|error| sql_error("Lore-Metadaten konnten nicht gespeichert werden", error))?;
    transaction
        .commit()
        .map_err(|error| sql_error("Lore-Transaktion konnte nicht abgeschlossen werden", error))?;
    Ok(LoreEntry {
        entity: load_entity(&db, &entity_id)?,
        metadata: load_lore_metadata(&db, &entity_id)?,
    })
}

#[tauri::command]
pub fn list_story_entity_relations(
    state: State<'_, DbState>,
    project_id: String,
    entity_id: Option<String>,
) -> Result<Vec<StoryEntityRelation>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id, project_id, source_entity_id, target_entity_id, relation_type, label, author_confirmed, created_at, updated_at FROM story_entity_relations WHERE project_id=?1 AND (?2 IS NULL OR source_entity_id=?2 OR target_entity_id=?2) ORDER BY updated_at DESC").map_err(|error| sql_error("Verbindungen konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![project_id, entity_id], relation_from_row)
        .map_err(|error| sql_error("Verbindungen konnten nicht geladen werden", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| sql_error("Verbindungen konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn create_story_entity_relation(
    state: State<'_, DbState>,
    input: CreateStoryEntityRelationInput,
) -> Result<StoryEntityRelation, String> {
    validate_relation_type(&input.relation_type)?;
    required(&input.source_entity_id, "Die Quellentität")?;
    required(&input.target_entity_id, "Die Zielentität")?;
    if input.source_entity_id == input.target_entity_id {
        return Err("Eine Entität kann nicht mit sich selbst verbunden werden.".into());
    }
    if input.label.chars().count() > 160 {
        return Err("Die Bezeichnung der Verbindung ist zu lang.".into());
    }
    let db = lock_db(&state)?;
    project_entity_exists(&db, &input.project_id, &input.source_entity_id, None)?;
    project_entity_exists(&db, &input.project_id, &input.target_entity_id, None)?;
    let id = new_id();
    let timestamp = now();
    db.execute("INSERT INTO story_entity_relations (id, project_id, source_entity_id, target_entity_id, relation_type, label, author_confirmed, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8)", params![id, input.project_id, input.source_entity_id, input.target_entity_id, input.relation_type, input.label, input.author_confirmed, timestamp]).map_err(|error| if error.to_string().contains("UNIQUE") { "Diese Verbindung existiert bereits.".to_string() } else { sql_error("Verbindung konnte nicht gespeichert werden", error) })?;
    db.query_row("SELECT id, project_id, source_entity_id, target_entity_id, relation_type, label, author_confirmed, created_at, updated_at FROM story_entity_relations WHERE id=?1", params![id], relation_from_row).map_err(|error| sql_error("Gespeicherte Verbindung konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn delete_story_entity_relation(state: State<'_, DbState>, id: String) -> Result<(), String> {
    let db = lock_db(&state)?;
    db.execute(
        "DELETE FROM story_entity_relations WHERE id=?1",
        params![id],
    )
    .map_err(|error| sql_error("Verbindung konnte nicht gelöscht werden", error))?;
    Ok(())
}

fn validate_character(db: &Connection, project_id: &str, character_id: &str) -> Result<(), String> {
    project_entity_exists(db, project_id, character_id, Some("character"))
}
fn validate_scene_project(db: &Connection, project_id: &str, scene_id: &str) -> Result<(), String> {
    let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM scenes JOIN chapters ON chapters.id=scenes.chapter_id JOIN books ON books.id=chapters.book_id WHERE scenes.id=?1 AND books.project_id=?2)", params![scene_id, project_id], |row| row.get(0)).map_err(|error| sql_error("Szene konnte nicht geprüft werden", error))?;
    if exists {
        Ok(())
    } else {
        Err("Die Szene gehört nicht zu diesem Projekt.".into())
    }
}
fn validate_memory_text(value: &str, label: &str, max: usize) -> Result<(), String> {
    required(value, label)?;
    if value.chars().count() > max {
        return Err(format!("{label} ist zu lang."));
    }
    Ok(())
}
fn validate_probability(value: f64, label: &str) -> Result<(), String> {
    if !(0.0..=1.0).contains(&value) {
        Err(format!("{label} muss zwischen 0 und 1 liegen."))
    } else {
        Ok(())
    }
}
fn payload_text(
    payload: &serde_json::Value,
    key: &str,
    label: &str,
    required_value: bool,
) -> Result<String, String> {
    let value = payload
        .get(key)
        .and_then(|item| item.as_str())
        .unwrap_or_default()
        .to_string();
    if required_value {
        required(&value, label)?;
    }
    if value.chars().count() > 4000 {
        return Err(format!("{label} ist zu lang."));
    }
    Ok(value)
}
fn validate_memory_payload_tx(
    tx: &rusqlite::Transaction<'_>,
    project_id: &str,
    kind: &str,
    payload: &serde_json::Value,
    subject: Option<&str>,
    related: Option<&str>,
) -> Result<(), String> {
    let subject_id = subject.or_else(|| {
        payload
            .get("subjectCharacterId")
            .and_then(|value| value.as_str())
    });
    if let Some(id) = subject_id {
        validate_character(tx, project_id, id)?;
    }
    if let Some(id) = related.or_else(|| {
        payload
            .get("relatedCharacterId")
            .and_then(|value| value.as_str())
    }) {
        validate_character(tx, project_id, id)?;
    }
    match kind {
        "voice_pattern" => {
            validate_memory_text(
                &payload_text(payload, "patternText", "Das Sprachmuster", true)?,
                "Das Sprachmuster",
                240,
            )?;
            validate_character_voice_pattern_type(
                payload
                    .get("patternType")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )?;
        }
        "experience" => {
            payload_text(payload, "title", "Der Erlebnistitel", true)?;
            validate_character_significance(
                payload
                    .get("significance")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )?;
            validate_memory_reliability(
                payload
                    .get("memoryReliability")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )?;
        }
        "dialogue_memory" => {
            payload_text(payload, "summary", "Die Dialogzusammenfassung", true)?;
            validate_dialogue_kind(
                payload
                    .get("dialogueKind")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )?;
            validate_character_significance(
                payload
                    .get("significance")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )?;
            validate_truthfulness(
                payload
                    .get("truthfulness")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )?;
            let participants = payload
                .get("participants")
                .and_then(|v| v.as_array())
                .ok_or("Dialogteilnehmer fehlen.")?;
            let speaker = subject_id.ok_or("Der Sprecher fehlt.")?;
            let mut speaker_count = 0;
            for participant in participants {
                let id = participant
                    .get("characterId")
                    .and_then(|v| v.as_str())
                    .ok_or("Ungültiger Dialogteilnehmer.")?;
                validate_character(tx, project_id, id)?;
                let role = participant
                    .get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                validate_participant_role(role)?;
                if id == speaker && role == "speaker" {
                    speaker_count += 1;
                }
            }
            if speaker_count != 1 {
                return Err("Der Sprecher muss genau einmal als speaker teilnehmen.".into());
            }
        }
        "relationship_memory" => {
            let other = related
                .or_else(|| payload.get("relatedCharacterId").and_then(|v| v.as_str()))
                .ok_or("Die zweite Figur fehlt.")?;
            normalize_pair(subject_id.ok_or("Die erste Figur fehlt.")?, other)?;
            validate_relationship_memory_type(
                payload
                    .get("memoryType")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )?;
            payload_text(payload, "title", "Der Beziehungstitel", true)?;
            payload_text(payload, "summary", "Die Beziehungszusammenfassung", true)?;
        }
        "knowledge_change" => {
            let fact = payload
                .get("factEntityId")
                .and_then(|v| v.as_str())
                .ok_or("Der Wissensfakt fehlt.")?;
            project_entity_exists(tx, project_id, fact, None)?;
            validate_knowledge_state(
                payload
                    .get("knowledgeState")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )?;
            validate_probability(
                payload
                    .get("certainty")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(-1.0),
                "Certainty",
            )?;
        }
        "profile_observation" => {
            payload_text(payload, "field", "Das Profilfeld", true)?;
            payload_text(payload, "observedBehavior", "Die Beobachtung", true)?;
        }
        "character_relation" => {
            return Err(
                "Character-Relation-Proposals benötigen eine bewusste manuelle Entscheidung."
                    .into(),
            );
        }
        _ => return Err("Unbekannter Character-Memory-Proposal-Typ.".into()),
    }
    Ok(())
}
fn normalize_pair(a: &str, b: &str) -> Result<(String, String), String> {
    if a == b {
        Err("Eine Figur kann keine Beziehungserinnerung mit sich selbst besitzen.".into())
    } else if a < b {
        Ok((a.to_string(), b.to_string()))
    } else {
        Ok((b.to_string(), a.to_string()))
    }
}

fn voice_from_row(row: &rusqlite::Row<'_>) -> SqlResult<CharacterVoicePattern> {
    Ok(CharacterVoicePattern {
        id: row.get(0)?,
        project_id: row.get(1)?,
        character_id: row.get(2)?,
        related_character_id: row.get(3)?,
        pattern_type: row.get(4)?,
        pattern_text: row.get(5)?,
        description: row.get(6)?,
        context_condition: row.get(7)?,
        confidence: row.get(8)?,
        status: row.get(9)?,
        author_confirmed: row.get(10)?,
        occurrence_count: row.get(11)?,
        first_observed_scene_id: row.get(12)?,
        last_observed_scene_id: row.get(13)?,
        retired_scene_id: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}
fn experience_from_row(row: &rusqlite::Row<'_>) -> SqlResult<CharacterExperience> {
    Ok(CharacterExperience {
        id: row.get(0)?,
        project_id: row.get(1)?,
        character_id: row.get(2)?,
        event_entity_id: row.get(3)?,
        scene_id: row.get(4)?,
        title: row.get(5)?,
        objective_summary: row.get(6)?,
        subjective_interpretation: row.get(7)?,
        emotional_impact: row.get(8)?,
        lasting_effect: row.get(9)?,
        significance: row.get(10)?,
        memory_reliability: row.get(11)?,
        status: row.get(12)?,
        author_confirmed: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}
fn participant_from_row(row: &rusqlite::Row<'_>) -> SqlResult<DialogueMemoryParticipant> {
    Ok(DialogueMemoryParticipant {
        dialogue_memory_id: row.get(0)?,
        character_id: row.get(1)?,
        role: row.get(2)?,
    })
}
fn dialogue_from_row(
    row: &rusqlite::Row<'_>,
    participants: Vec<DialogueMemoryParticipant>,
) -> SqlResult<CharacterDialogueMemory> {
    Ok(CharacterDialogueMemory {
        id: row.get(0)?,
        project_id: row.get(1)?,
        speaker_id: row.get(2)?,
        scene_id: row.get(3)?,
        dialogue_kind: row.get(4)?,
        topic: row.get(5)?,
        summary: row.get(6)?,
        exact_excerpt: row.get(7)?,
        emotional_tone: row.get(8)?,
        hidden_intent: row.get(9)?,
        significance: row.get(10)?,
        truthfulness: row.get(11)?,
        status: row.get(12)?,
        author_confirmed: row.get(13)?,
        participants,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}
fn relation_memory_from_row(row: &rusqlite::Row<'_>) -> SqlResult<RelationshipMemory> {
    Ok(RelationshipMemory {
        id: row.get(0)?,
        project_id: row.get(1)?,
        character_a_id: row.get(2)?,
        character_b_id: row.get(3)?,
        scene_id: row.get(4)?,
        memory_type: row.get(5)?,
        title: row.get(6)?,
        summary: row.get(7)?,
        private_meaning: row.get(8)?,
        relationship_effect: row.get(9)?,
        significance: row.get(10)?,
        status: row.get(11)?,
        author_confirmed: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}
fn knowledge_from_row(row: &rusqlite::Row<'_>) -> SqlResult<CharacterKnowledgeState> {
    Ok(CharacterKnowledgeState {
        id: row.get(0)?,
        project_id: row.get(1)?,
        character_id: row.get(2)?,
        fact_entity_id: row.get(3)?,
        knowledge_state: row.get(4)?,
        acquired_scene_id: row.get(5)?,
        changed_scene_id: row.get(6)?,
        effective_from_scene_id: row.get(7)?,
        effective_until_scene_id: row.get(8)?,
        source_character_id: row.get(9)?,
        certainty: row.get(10)?,
        notes: row.get(11)?,
        status: row.get(12)?,
        author_confirmed: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}
fn evidence_from_row(row: &rusqlite::Row<'_>) -> SqlResult<CharacterMemoryEvidence> {
    Ok(CharacterMemoryEvidence {
        id: row.get(0)?,
        project_id: row.get(1)?,
        memory_kind: row.get(2)?,
        memory_id: row.get(3)?,
        source_reference_id: row.get(4)?,
        evidence_role: row.get(5)?,
        created_at: row.get(6)?,
    })
}
fn memory_run_from_row(row: &rusqlite::Row<'_>) -> SqlResult<CharacterMemoryUpdateRun> {
    Ok(CharacterMemoryUpdateRun {
        id: row.get(0)?,
        project_id: row.get(1)?,
        scene_id: row.get(2)?,
        content_hash: row.get(3)?,
        extractor_id: row.get(4)?,
        analyzed_content: row.get(5)?,
        status: row.get(6)?,
        created_at: row.get(7)?,
        completed_at: row.get(8)?,
        error_message: row.get(9)?,
    })
}
fn memory_proposal_from_row(row: &rusqlite::Row<'_>) -> SqlResult<CharacterMemoryProposal> {
    let payload: String = row.get(8)?;
    Ok(CharacterMemoryProposal {
        id: row.get(0)?,
        run_id: row.get(1)?,
        project_id: row.get(2)?,
        scene_id: row.get(3)?,
        proposal_kind: row.get(4)?,
        subject_character_id: row.get(5)?,
        related_character_id: row.get(6)?,
        target_entity_id: row.get(7)?,
        payload: serde_json::from_str(&payload).unwrap_or_else(|_| serde_json::json!({})),
        classification: row.get(9)?,
        confidence: row.get(10)?,
        evidence_excerpt: row.get(11)?,
        start_offset: row.get(12)?,
        end_offset: row.get(13)?,
        reason: row.get(14)?,
        review_status: row.get(15)?,
        reviewed_at: row.get(16)?,
        analyzed_content_hash: row.get(17).unwrap_or_default(),
        accepted_memory_id: row.get(18).ok(),
        accepted_memory_kind: row.get(19).ok(),
        created_at: row.get(20)?,
    })
}

fn list_voice_db(
    db: &Connection,
    project_id: &str,
    character_id: Option<&str>,
) -> Result<Vec<CharacterVoicePattern>, String> {
    let mut statement = db.prepare("SELECT id,project_id,character_id,related_character_id,pattern_type,pattern_text,description,context_condition,confidence,status,author_confirmed,occurrence_count,first_observed_scene_id,last_observed_scene_id,retired_scene_id,created_at,updated_at FROM character_voice_patterns WHERE project_id=?1 AND (?2 IS NULL OR character_id=?2) ORDER BY occurrence_count DESC, updated_at DESC").map_err(|e| sql_error("Sprachmuster konnten nicht geladen werden", e))?;
    let rows = statement
        .query_map(params![project_id, character_id], voice_from_row)
        .map_err(|e| sql_error("Sprachmuster konnten nicht geladen werden", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error("Sprachmuster konnten nicht geladen werden", e));
    rows
}
fn list_experience_db(
    db: &Connection,
    project_id: &str,
    character_id: Option<&str>,
) -> Result<Vec<CharacterExperience>, String> {
    let mut statement = db.prepare("SELECT id,project_id,character_id,event_entity_id,scene_id,title,objective_summary,subjective_interpretation,emotional_impact,lasting_effect,significance,memory_reliability,status,author_confirmed,created_at,updated_at FROM character_experiences WHERE project_id=?1 AND (?2 IS NULL OR character_id=?2) ORDER BY created_at ASC").map_err(|e| sql_error("Erlebnisse konnten nicht geladen werden", e))?;
    let rows = statement
        .query_map(params![project_id, character_id], experience_from_row)
        .map_err(|e| sql_error("Erlebnisse konnten nicht geladen werden", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error("Erlebnisse konnten nicht geladen werden", e));
    rows
}

#[tauri::command]
pub fn list_character_voice_patterns(
    state: State<'_, DbState>,
    project_id: String,
    character_id: Option<String>,
) -> Result<Vec<CharacterVoicePattern>, String> {
    let db = lock_db(&state)?;
    list_voice_db(&db, &project_id, character_id.as_deref())
}
#[tauri::command]
pub fn save_character_voice_pattern(
    state: State<'_, DbState>,
    input: SaveCharacterVoicePatternInput,
) -> Result<CharacterVoicePattern, String> {
    let db = lock_db(&state)?;
    validate_character(&db, &input.project_id, &input.character_id)?;
    validate_character_voice_pattern_type(&input.pattern_type)?;
    validate_character_memory_status(&input.status)?;
    validate_probability(input.confidence, "Confidence")?;
    validate_memory_text(&input.pattern_text, "Das Sprachmuster", 240)?;
    if let Some(id) = &input.related_character_id {
        validate_character(&db, &input.project_id, id)?;
    }
    let id = input.id.clone().unwrap_or_else(new_id);
    let stamp = now();
    db.execute("INSERT INTO character_voice_patterns(id,project_id,character_id,related_character_id,pattern_type,pattern_text,description,context_condition,confidence,status,author_confirmed,occurrence_count,first_observed_scene_id,last_observed_scene_id,retired_scene_id,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,COALESCE((SELECT created_at FROM character_voice_patterns WHERE id=?1),?16),?16) ON CONFLICT(id) DO UPDATE SET related_character_id=excluded.related_character_id,pattern_type=excluded.pattern_type,pattern_text=excluded.pattern_text,description=excluded.description,context_condition=excluded.context_condition,confidence=excluded.confidence,status=excluded.status,author_confirmed=excluded.author_confirmed,occurrence_count=excluded.occurrence_count,first_observed_scene_id=excluded.first_observed_scene_id,last_observed_scene_id=excluded.last_observed_scene_id,retired_scene_id=excluded.retired_scene_id,updated_at=excluded.updated_at", params![id,input.project_id,input.character_id,input.related_character_id,input.pattern_type,input.pattern_text,input.description,input.context_condition,input.confidence,input.status,input.author_confirmed,input.occurrence_count.max(1),input.first_observed_scene_id,input.last_observed_scene_id,input.retired_scene_id,stamp]).map_err(|e| sql_error("Sprachmuster konnte nicht gespeichert werden",e))?;
    db.query_row("SELECT id,project_id,character_id,related_character_id,pattern_type,pattern_text,description,context_condition,confidence,status,author_confirmed,occurrence_count,first_observed_scene_id,last_observed_scene_id,retired_scene_id,created_at,updated_at FROM character_voice_patterns WHERE id=?1",params![id],voice_from_row).map_err(|e| sql_error("Sprachmuster konnte nicht geladen werden",e))
}
#[tauri::command]
pub fn delete_character_voice_pattern(
    state: State<'_, DbState>,
    project_id: String,
    id: String,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    let changed = db
        .execute(
            "DELETE FROM character_voice_patterns WHERE id=?1 AND project_id=?2",
            params![id, project_id],
        )
        .map_err(|e| sql_error("Sprachmuster konnte nicht gelöscht werden", e))?;
    if changed == 0 {
        return Err("Sprachmuster nicht gefunden.".into());
    }
    Ok(())
}

#[tauri::command]
pub fn list_character_experiences(
    state: State<'_, DbState>,
    project_id: String,
    character_id: Option<String>,
) -> Result<Vec<CharacterExperience>, String> {
    let db = lock_db(&state)?;
    list_experience_db(&db, &project_id, character_id.as_deref())
}
#[tauri::command]
pub fn save_character_experience(
    state: State<'_, DbState>,
    input: SaveCharacterExperienceInput,
) -> Result<CharacterExperience, String> {
    let db = lock_db(&state)?;
    validate_character(&db, &input.project_id, &input.character_id)?;
    validate_character_significance(&input.significance)?;
    validate_memory_reliability(&input.memory_reliability)?;
    validate_character_memory_status(&input.status)?;
    validate_memory_text(&input.title, "Der Erlebnistitel", 240)?;
    if let Some(scene) = &input.scene_id {
        validate_scene_project(&db, &input.project_id, scene)?;
    }
    if let Some(entity) = &input.event_entity_id {
        project_entity_exists(&db, &input.project_id, entity, None)?;
    }
    let id = input.id.clone().unwrap_or_else(new_id);
    let stamp = now();
    db.execute("INSERT INTO character_experiences(id,project_id,character_id,event_entity_id,scene_id,title,objective_summary,subjective_interpretation,emotional_impact,lasting_effect,significance,memory_reliability,status,author_confirmed,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,COALESCE((SELECT created_at FROM character_experiences WHERE id=?1),?15),?15) ON CONFLICT(id) DO UPDATE SET event_entity_id=excluded.event_entity_id,scene_id=excluded.scene_id,title=excluded.title,objective_summary=excluded.objective_summary,subjective_interpretation=excluded.subjective_interpretation,emotional_impact=excluded.emotional_impact,lasting_effect=excluded.lasting_effect,significance=excluded.significance,memory_reliability=excluded.memory_reliability,status=excluded.status,author_confirmed=excluded.author_confirmed,updated_at=excluded.updated_at",params![id,input.project_id,input.character_id,input.event_entity_id,input.scene_id,input.title,input.objective_summary,input.subjective_interpretation,input.emotional_impact,input.lasting_effect,input.significance,input.memory_reliability,input.status,input.author_confirmed,stamp]).map_err(|e| sql_error("Erlebnis konnte nicht gespeichert werden",e))?;
    db.query_row("SELECT id,project_id,character_id,event_entity_id,scene_id,title,objective_summary,subjective_interpretation,emotional_impact,lasting_effect,significance,memory_reliability,status,author_confirmed,created_at,updated_at FROM character_experiences WHERE id=?1",params![id],experience_from_row).map_err(|e| sql_error("Erlebnis konnte nicht geladen werden",e))
}
#[tauri::command]
pub fn delete_character_experience(
    state: State<'_, DbState>,
    project_id: String,
    id: String,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    let changed = db
        .execute(
            "DELETE FROM character_experiences WHERE id=?1 AND project_id=?2",
            params![id, project_id],
        )
        .map_err(|e| sql_error("Erlebnis konnte nicht gelöscht werden", e))?;
    if changed == 0 {
        return Err("Erlebnis nicht gefunden.".into());
    }
    Ok(())
}

fn load_participants(db: &Connection, id: &str) -> Result<Vec<DialogueMemoryParticipant>, String> {
    let mut statement=db.prepare("SELECT dialogue_memory_id,character_id,role FROM dialogue_memory_participants WHERE dialogue_memory_id=?1 ORDER BY role,character_id").map_err(|e|sql_error("Dialogteilnehmer konnten nicht geladen werden",e))?;
    let rows = statement
        .query_map(params![id], participant_from_row)
        .map_err(|e| sql_error("Dialogteilnehmer konnten nicht geladen werden", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error("Dialogteilnehmer konnten nicht geladen werden", e));
    rows
}
fn load_dialogue(db: &Connection, id: &str) -> Result<CharacterDialogueMemory, String> {
    let participants = load_participants(db, id)?;
    db.query_row("SELECT id,project_id,speaker_id,scene_id,dialogue_kind,topic,summary,exact_excerpt,emotional_tone,hidden_intent,significance,truthfulness,status,author_confirmed,created_at,updated_at FROM character_dialogue_memories WHERE id=?1",params![id],|row| dialogue_from_row(row,participants.clone())).map_err(|e|sql_error("Dialogerinnerung konnte nicht geladen werden",e))
}
#[tauri::command]
pub fn list_character_dialogue_memories(
    state: State<'_, DbState>,
    project_id: String,
    character_id: Option<String>,
) -> Result<Vec<CharacterDialogueMemory>, String> {
    let db = lock_db(&state)?;
    let ids: Vec<String> = if let Some(id) = character_id {
        validate_character(&db, &project_id, &id)?;
        let mut stmt=db.prepare("SELECT DISTINCT dmp.dialogue_memory_id FROM dialogue_memory_participants dmp JOIN character_dialogue_memories dm ON dm.id=dmp.dialogue_memory_id WHERE dmp.character_id=?1 AND dm.project_id=?2").map_err(|e|sql_error("Dialogerinnerungen konnten nicht geladen werden",e))?;
        let rows = stmt
            .query_map(params![id, project_id], |row| row.get(0))
            .map_err(|e| sql_error("Dialogerinnerungen konnten nicht geladen werden", e))?
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| sql_error("Dialogerinnerungen konnten nicht geladen werden", e));
        rows?
    } else {
        let mut stmt=db.prepare("SELECT id FROM character_dialogue_memories WHERE project_id=?1 ORDER BY created_at DESC").map_err(|e|sql_error("Dialogerinnerungen konnten nicht geladen werden",e))?;
        let rows = stmt
            .query_map(params![project_id], |row| row.get(0))
            .map_err(|e| sql_error("Dialogerinnerungen konnten nicht geladen werden", e))?
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| sql_error("Dialogerinnerungen konnten nicht geladen werden", e));
        rows?
    };
    ids.into_iter().map(|id| load_dialogue(&db, &id)).collect()
}
#[tauri::command]
pub fn save_character_dialogue_memory(
    state: State<'_, DbState>,
    input: SaveCharacterDialogueMemoryInput,
) -> Result<CharacterDialogueMemory, String> {
    let db = lock_db(&state)?;
    validate_character(&db, &input.project_id, &input.speaker_id)?;
    validate_scene_project(&db, &input.project_id, &input.scene_id)?;
    validate_dialogue_kind(&input.dialogue_kind)?;
    validate_character_significance(&input.significance)?;
    validate_truthfulness(&input.truthfulness)?;
    validate_character_memory_status(&input.status)?;
    validate_memory_text(&input.summary, "Die Dialogzusammenfassung", 4000)?;
    if input.exact_excerpt.chars().count() > 10000 {
        return Err("Das Dialogzitat ist zu lang.".into());
    }
    let mut participants = input.participants.clone();
    if !participants
        .iter()
        .any(|p| p.character_id == input.speaker_id && p.role == "speaker")
    {
        return Err(
            "Der Sprecher muss als Teilnehmer mit der Rolle speaker angegeben werden.".into(),
        );
    }
    for participant in &participants {
        validate_participant_role(&participant.role)?;
        validate_character(&db, &input.project_id, &participant.character_id)?;
    }
    participants.sort_by(|a, b| a.character_id.cmp(&b.character_id));
    let id = input.id.clone().unwrap_or_else(new_id);
    let stamp = now();
    let tx = db
        .unchecked_transaction()
        .map_err(|e| sql_error("Dialogtransaktion konnte nicht gestartet werden", e))?;
    tx.execute("INSERT INTO character_dialogue_memories(id,project_id,speaker_id,scene_id,dialogue_kind,topic,summary,exact_excerpt,emotional_tone,hidden_intent,significance,truthfulness,status,author_confirmed,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,COALESCE((SELECT created_at FROM character_dialogue_memories WHERE id=?1),?15),?15) ON CONFLICT(id) DO UPDATE SET speaker_id=excluded.speaker_id,scene_id=excluded.scene_id,dialogue_kind=excluded.dialogue_kind,topic=excluded.topic,summary=excluded.summary,exact_excerpt=excluded.exact_excerpt,emotional_tone=excluded.emotional_tone,hidden_intent=excluded.hidden_intent,significance=excluded.significance,truthfulness=excluded.truthfulness,status=excluded.status,author_confirmed=excluded.author_confirmed,updated_at=excluded.updated_at",params![id,input.project_id,input.speaker_id,input.scene_id,input.dialogue_kind,input.topic,input.summary,input.exact_excerpt,input.emotional_tone,input.hidden_intent,input.significance,input.truthfulness,input.status,input.author_confirmed,stamp]).map_err(|e|sql_error("Dialogerinnerung konnte nicht gespeichert werden",e))?;
    tx.execute(
        "DELETE FROM dialogue_memory_participants WHERE dialogue_memory_id=?1",
        params![id],
    )
    .map_err(|e| sql_error("Dialogteilnehmer konnten nicht aktualisiert werden", e))?;
    for p in participants {
        tx.execute("INSERT INTO dialogue_memory_participants(dialogue_memory_id,character_id,role) VALUES(?1,?2,?3)",params![id,p.character_id,p.role]).map_err(|e|sql_error("Dialogteilnehmer konnten nicht gespeichert werden",e))?;
    }
    tx.commit()
        .map_err(|e| sql_error("Dialogtransaktion konnte nicht abgeschlossen werden", e))?;
    load_dialogue(&db, &id)
}
#[tauri::command]
pub fn delete_character_dialogue_memory(
    state: State<'_, DbState>,
    project_id: String,
    id: String,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    let changed = db
        .execute(
            "DELETE FROM character_dialogue_memories WHERE id=?1 AND project_id=?2",
            params![id, project_id],
        )
        .map_err(|e| sql_error("Dialogerinnerung konnte nicht gelöscht werden", e))?;
    if changed == 0 {
        return Err("Dialogerinnerung nicht gefunden.".into());
    }
    Ok(())
}

#[tauri::command]
pub fn list_relationship_memories(
    state: State<'_, DbState>,
    project_id: String,
    character_id: Option<String>,
    related_character_id: Option<String>,
) -> Result<Vec<RelationshipMemory>, String> {
    let db = lock_db(&state)?;
    let mut stmt=db.prepare("SELECT id,project_id,character_a_id,character_b_id,scene_id,memory_type,title,summary,private_meaning,relationship_effect,significance,status,author_confirmed,created_at,updated_at FROM relationship_memories WHERE project_id=?1 AND (?2 IS NULL OR character_a_id=?2 OR character_b_id=?2) AND (?3 IS NULL OR character_a_id=?3 OR character_b_id=?3) ORDER BY created_at ASC").map_err(|e|sql_error("Beziehungserinnerungen konnten nicht geladen werden",e))?;
    let rows = stmt
        .query_map(
            params![project_id, character_id, related_character_id],
            relation_memory_from_row,
        )
        .map_err(|e| sql_error("Beziehungserinnerungen konnten nicht geladen werden", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error("Beziehungserinnerungen konnten nicht geladen werden", e));
    rows
}
#[tauri::command]
pub fn save_relationship_memory(
    state: State<'_, DbState>,
    input: SaveRelationshipMemoryInput,
) -> Result<RelationshipMemory, String> {
    let db = lock_db(&state)?;
    let (a, b) = normalize_pair(&input.character_a_id, &input.character_b_id)?;
    validate_character(&db, &input.project_id, &a)?;
    validate_character(&db, &input.project_id, &b)?;
    if let Some(scene) = &input.scene_id {
        validate_scene_project(&db, &input.project_id, scene)?;
    }
    validate_relationship_memory_type(&input.memory_type)?;
    validate_character_significance(&input.significance)?;
    validate_character_memory_status(&input.status)?;
    validate_memory_text(&input.title, "Der Beziehungstitel", 240)?;
    validate_memory_text(&input.summary, "Die Beziehungszusammenfassung", 4000)?;
    let id = input.id.clone().unwrap_or_else(new_id);
    let stamp = now();
    db.execute("INSERT INTO relationship_memories(id,project_id,character_a_id,character_b_id,scene_id,memory_type,title,summary,private_meaning,relationship_effect,significance,status,author_confirmed,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,COALESCE((SELECT created_at FROM relationship_memories WHERE id=?1),?14),?14) ON CONFLICT(id) DO UPDATE SET character_a_id=excluded.character_a_id,character_b_id=excluded.character_b_id,scene_id=excluded.scene_id,memory_type=excluded.memory_type,title=excluded.title,summary=excluded.summary,private_meaning=excluded.private_meaning,relationship_effect=excluded.relationship_effect,significance=excluded.significance,status=excluded.status,author_confirmed=excluded.author_confirmed,updated_at=excluded.updated_at",params![id,input.project_id,a,b,input.scene_id,input.memory_type,input.title,input.summary,input.private_meaning,input.relationship_effect,input.significance,input.status,input.author_confirmed,stamp]).map_err(|e|sql_error("Beziehungserinnerung konnte nicht gespeichert werden",e))?;
    db.query_row("SELECT id,project_id,character_a_id,character_b_id,scene_id,memory_type,title,summary,private_meaning,relationship_effect,significance,status,author_confirmed,created_at,updated_at FROM relationship_memories WHERE id=?1",params![id],relation_memory_from_row).map_err(|e|sql_error("Beziehungserinnerung konnte nicht geladen werden",e))
}
#[tauri::command]
pub fn delete_relationship_memory(
    state: State<'_, DbState>,
    project_id: String,
    id: String,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    let changed = db
        .execute(
            "DELETE FROM relationship_memories WHERE id=?1 AND project_id=?2",
            params![id, project_id],
        )
        .map_err(|e| sql_error("Beziehungserinnerung konnte nicht gelöscht werden", e))?;
    if changed == 0 {
        return Err("Beziehungserinnerung nicht gefunden.".into());
    }
    Ok(())
}

#[tauri::command]
pub fn list_character_knowledge_states(
    state: State<'_, DbState>,
    project_id: String,
    character_id: Option<String>,
) -> Result<Vec<CharacterKnowledgeState>, String> {
    let db = lock_db(&state)?;
    if let Some(id) = &character_id {
        validate_character(&db, &project_id, id)?;
    }
    let mut stmt=db.prepare("SELECT id,project_id,character_id,fact_entity_id,knowledge_state,acquired_scene_id,changed_scene_id,effective_from_scene_id,effective_until_scene_id,source_character_id,certainty,notes,status,author_confirmed,created_at,updated_at FROM character_knowledge_states WHERE project_id=?1 AND (?2 IS NULL OR character_id=?2) ORDER BY updated_at DESC").map_err(|e|sql_error("Wissensstände konnten nicht geladen werden",e))?;
    let rows = stmt
        .query_map(params![project_id, character_id], knowledge_from_row)
        .map_err(|e| sql_error("Wissensstände konnten nicht geladen werden", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error("Wissensstände konnten nicht geladen werden", e));
    rows
}
#[tauri::command]
pub fn list_character_knowledge_history(
    state: State<'_, DbState>,
    project_id: String,
    character_id: Option<String>,
) -> Result<Vec<CharacterKnowledgeState>, String> {
    let db = lock_db(&state)?;
    let mut stmt=db.prepare("SELECT id,project_id,character_id,fact_entity_id,knowledge_state,NULL,scene_id,effective_from_scene_id,effective_until_scene_id,NULL,certainty,'', 'confirmed',1,created_at,created_at FROM character_knowledge_history WHERE project_id=?1 AND (?2 IS NULL OR character_id=?2) ORDER BY created_at ASC").map_err(|e|sql_error("Wissenshistorie konnte nicht geladen werden",e))?;
    let rows = stmt
        .query_map(params![project_id, character_id], knowledge_from_row)
        .map_err(|e| sql_error("Wissenshistorie konnte nicht geladen werden", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error("Wissenshistorie konnte nicht geladen werden", e));
    rows
}

type PreviousKnowledgeState = (
    String,
    String,
    f64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

#[tauri::command]
pub fn save_character_knowledge_state(
    state: State<'_, DbState>,
    input: SaveCharacterKnowledgeStateInput,
) -> Result<CharacterKnowledgeState, String> {
    let db = lock_db(&state)?;
    validate_character(&db, &input.project_id, &input.character_id)?;
    project_entity_exists(&db, &input.project_id, &input.fact_entity_id, None)?;
    if let Some(id) = &input.acquired_scene_id {
        validate_scene_project(&db, &input.project_id, id)?;
    }
    if let Some(id) = &input.changed_scene_id {
        validate_scene_project(&db, &input.project_id, id)?;
    }
    if let Some(id) = &input.source_character_id {
        validate_character(&db, &input.project_id, id)?;
    }
    validate_knowledge_state(&input.knowledge_state)?;
    validate_character_memory_status(&input.status)?;
    validate_probability(input.certainty, "Certainty")?;
    let id = input.id.clone().unwrap_or_else(new_id);
    let previous: Option<PreviousKnowledgeState> = db.query_row("SELECT knowledge_state,project_id,certainty,effective_from_scene_id,effective_until_scene_id,acquired_scene_id,changed_scene_id FROM character_knowledge_states WHERE id=?1",params![id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?))).optional().map_err(|e|sql_error("Wissensstand konnte nicht geprüft werden",e))?;
    let transition_scene_id = input
        .changed_scene_id
        .clone()
        .or_else(|| input.acquired_scene_id.clone());
    let effective_from_scene_id = if previous.is_some() {
        transition_scene_id.clone()
    } else {
        input
            .effective_from_scene_id
            .clone()
            .or_else(|| transition_scene_id.clone())
    };
    let stamp = now();
    let tx = db
        .unchecked_transaction()
        .map_err(|e| sql_error("Wissensstandtransaktion konnte nicht gestartet werden", e))?;
    tx.execute("INSERT INTO character_knowledge_states(id,project_id,character_id,fact_entity_id,knowledge_state,acquired_scene_id,changed_scene_id,effective_from_scene_id,effective_until_scene_id,source_character_id,certainty,notes,status,author_confirmed,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,NULL,?9,?10,?11,?12,?13,COALESCE((SELECT created_at FROM character_knowledge_states WHERE id=?1),?14),?14) ON CONFLICT(id) DO UPDATE SET knowledge_state=excluded.knowledge_state,acquired_scene_id=excluded.acquired_scene_id,changed_scene_id=excluded.changed_scene_id,effective_from_scene_id=excluded.effective_from_scene_id,effective_until_scene_id=NULL,source_character_id=excluded.source_character_id,certainty=excluded.certainty,notes=excluded.notes,status=excluded.status,author_confirmed=excluded.author_confirmed,updated_at=excluded.updated_at",params![id,input.project_id,input.character_id,input.fact_entity_id,input.knowledge_state,input.acquired_scene_id,input.changed_scene_id,effective_from_scene_id,input.source_character_id,input.certainty,input.notes,input.status,input.author_confirmed,stamp]).map_err(|e|sql_error("Wissensstand konnte nicht gespeichert werden",e))?;
    if let Some((
        old_state,
        old_project,
        old_certainty,
        old_from,
        _old_until,
        old_acquired,
        old_changed,
    )) = previous
    {
        if old_state != input.knowledge_state
            || (old_certainty - input.certainty).abs() > f64::EPSILON
        {
            tx.execute("INSERT INTO character_knowledge_history(id,knowledge_state_id,project_id,character_id,fact_entity_id,knowledge_state,certainty,scene_id,effective_from_scene_id,effective_until_scene_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![new_id(),id,old_project,input.character_id,input.fact_entity_id,old_state,old_certainty,transition_scene_id,old_from.or(old_changed).or(old_acquired),transition_scene_id]).map_err(|e|sql_error("Wissenshistorie konnte nicht gespeichert werden",e))?;
        }
    }
    tx.commit().map_err(|e| {
        sql_error(
            "Wissensstandtransaktion konnte nicht abgeschlossen werden",
            e,
        )
    })?;
    db.query_row("SELECT id,project_id,character_id,fact_entity_id,knowledge_state,acquired_scene_id,changed_scene_id,effective_from_scene_id,effective_until_scene_id,source_character_id,certainty,notes,status,author_confirmed,created_at,updated_at FROM character_knowledge_states WHERE id=?1",params![id],knowledge_from_row).map_err(|e|sql_error("Gespeicherter Wissensstand konnte nicht geladen werden",e))
}
#[tauri::command]
pub fn delete_character_knowledge_state(
    state: State<'_, DbState>,
    project_id: String,
    id: String,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    let changed = db
        .execute(
            "DELETE FROM character_knowledge_states WHERE id=?1 AND project_id=?2",
            params![id, project_id],
        )
        .map_err(|e| sql_error("Wissensstand konnte nicht gelöscht werden", e))?;
    if changed == 0 {
        return Err("Wissensstand nicht gefunden.".into());
    }
    Ok(())
}

fn memory_exists(db: &Connection, kind: &str, id: &str) -> Result<bool, String> {
    let table = match kind {
        "voice_pattern" => "character_voice_patterns",
        "experience" => "character_experiences",
        "dialogue_memory" => "character_dialogue_memories",
        "relationship_memory" => "relationship_memories",
        "knowledge_state" => "character_knowledge_states",
        "profile_observation" => "character_profiles",
        _ => return Err("Ungültige Gedächtnisart.".into()),
    };
    let query = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)");
    db.query_row(&query, params![id], |row| row.get(0))
        .map_err(|e| sql_error("Gedächtniseintrag konnte nicht geprüft werden", e))
}
fn evidence_source_valid(db: &Connection, project_id: &str, source_id: &str) -> Result<(), String> {
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2)",
            params![source_id, project_id],
            |row| row.get(0),
        )
        .map_err(|e| sql_error("Quellenbeleg konnte nicht geprüft werden", e))?;
    if exists {
        Ok(())
    } else {
        Err("Der Quellenbeleg gehört nicht zu diesem Projekt.".into())
    }
}
#[tauri::command]
pub fn list_character_memory_evidence(
    state: State<'_, DbState>,
    project_id: String,
    memory_kind: String,
    memory_id: String,
) -> Result<Vec<CharacterMemoryEvidence>, String> {
    validate_memory_kind(&memory_kind)?;
    let db = lock_db(&state)?;
    if !memory_exists(&db, &memory_kind, &memory_id)? {
        return Err("Der Gedächtniseintrag wurde nicht gefunden.".into());
    }
    let mut stmt=db.prepare("SELECT id,project_id,memory_kind,memory_id,source_reference_id,evidence_role,created_at FROM character_memory_evidence WHERE project_id=?1 AND memory_kind=?2 AND memory_id=?3 ORDER BY created_at ASC").map_err(|e|sql_error("Gedächtnisbelege konnten nicht geladen werden",e))?;
    let rows = stmt
        .query_map(
            params![project_id, memory_kind, memory_id],
            evidence_from_row,
        )
        .map_err(|e| sql_error("Gedächtnisbelege konnten nicht geladen werden", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error("Gedächtnisbelege konnten nicht geladen werden", e));
    rows
}
#[tauri::command]
pub fn add_character_memory_evidence(
    state: State<'_, DbState>,
    input: AddCharacterMemoryEvidenceInput,
) -> Result<CharacterMemoryEvidence, String> {
    validate_memory_kind(&input.memory_kind)?;
    validate_evidence_role(&input.evidence_role)?;
    let db = lock_db(&state)?;
    if !memory_exists(&db, &input.memory_kind, &input.memory_id)? {
        return Err("Der Gedächtniseintrag wurde nicht gefunden.".into());
    }
    evidence_source_valid(&db, &input.project_id, &input.source_reference_id)?;
    let id = new_id();
    let stamp = now();
    db.execute("INSERT INTO character_memory_evidence(id,project_id,memory_kind,memory_id,source_reference_id,evidence_role,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(memory_kind,memory_id,source_reference_id) DO UPDATE SET evidence_role=excluded.evidence_role",params![id,input.project_id,input.memory_kind,input.memory_id,input.source_reference_id,input.evidence_role,stamp]).map_err(|e|sql_error("Gedächtnisbeleg konnte nicht gespeichert werden",e))?;
    db.query_row("SELECT id,project_id,memory_kind,memory_id,source_reference_id,evidence_role,created_at FROM character_memory_evidence WHERE memory_kind=?1 AND memory_id=?2 AND source_reference_id=?3",params![input.memory_kind,input.memory_id,input.source_reference_id],evidence_from_row).map_err(|e|sql_error("Gedächtnisbeleg konnte nicht geladen werden",e))
}
#[tauri::command]
pub fn delete_character_memory_evidence(
    state: State<'_, DbState>,
    project_id: String,
    id: String,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    let changed = db
        .execute(
            "DELETE FROM character_memory_evidence WHERE id=?1 AND project_id=?2",
            params![id, project_id],
        )
        .map_err(|e| sql_error("Gedächtnisbeleg konnte nicht gelöscht werden", e))?;
    if changed == 0 {
        return Err("Gedächtnisbeleg nicht gefunden.".into());
    }
    Ok(())
}

#[tauri::command]
pub fn create_character_memory_update_run(
    state: State<'_, DbState>,
    input: CreateCharacterMemoryUpdateRunInput,
) -> Result<CharacterMemoryUpdateRun, String> {
    let db = lock_db(&state)?;
    validate_scene_project(&db, &input.project_id, &input.scene_id)?;
    required(&input.content_hash, "Der Content-Hash")?;
    required(&input.extractor_id, "Der Extractor")?;
    if let Some(existing)=db.query_row("SELECT id,project_id,scene_id,content_hash,extractor_id,analyzed_content,status,created_at,completed_at,error_message FROM character_memory_update_runs WHERE project_id=?1 AND scene_id=?2 AND content_hash=?3 AND extractor_id=?4 AND status IN ('completed','reviewed') ORDER BY created_at DESC LIMIT 1",params![input.project_id,input.scene_id,input.content_hash,input.extractor_id],memory_run_from_row).optional().map_err(|e|sql_error("Character-Memory-Run konnte nicht geprüft werden",e))? { return Ok(existing); }
    let id = new_id();
    let stamp = now();
    db.execute("INSERT INTO character_memory_update_runs(id,project_id,scene_id,content_hash,extractor_id,analyzed_content,status,created_at) VALUES(?1,?2,?3,?4,?5,?6,'pending',?7)",params![id,input.project_id,input.scene_id,input.content_hash,input.extractor_id,input.analyzed_content,stamp]).map_err(|e|sql_error("Character-Memory-Run konnte nicht angelegt werden",e))?;
    db.query_row("SELECT id,project_id,scene_id,content_hash,extractor_id,analyzed_content,status,created_at,completed_at,error_message FROM character_memory_update_runs WHERE id=?1",params![id],memory_run_from_row).map_err(|e|sql_error("Character-Memory-Run konnte nicht geladen werden",e))
}
#[tauri::command]
pub fn list_character_memory_update_runs(
    state: State<'_, DbState>,
    project_id: String,
    scene_id: Option<String>,
) -> Result<Vec<CharacterMemoryUpdateRun>, String> {
    let db = lock_db(&state)?;
    let mut stmt=db.prepare("SELECT id,project_id,scene_id,content_hash,extractor_id,analyzed_content,status,created_at,completed_at,error_message FROM character_memory_update_runs WHERE project_id=?1 AND (?2 IS NULL OR scene_id=?2) ORDER BY created_at DESC").map_err(|e|sql_error("Character-Memory-Runs konnten nicht geladen werden",e))?;
    let rows = stmt
        .query_map(params![project_id, scene_id], memory_run_from_row)
        .map_err(|e| sql_error("Character-Memory-Runs konnten nicht geladen werden", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error("Character-Memory-Runs konnten nicht geladen werden", e));
    rows
}
#[tauri::command]
pub fn save_character_memory_proposals(
    state: State<'_, DbState>,
    run_id: String,
    proposals: Vec<CharacterMemoryProposalDraft>,
) -> Result<Vec<CharacterMemoryProposal>, String> {
    if proposals.len() > 100 {
        return Err("Maximal 100 Charaktergedächtnis-Vorschläge pro Lauf sind erlaubt.".into());
    }
    let db = lock_db(&state)?;
    let run: (String, String, String, String) = db
        .query_row(
            "SELECT project_id,scene_id,status,content_hash FROM character_memory_update_runs WHERE id=?1",
            params![run_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| sql_error("Character-Memory-Run wurde nicht gefunden", e))?;
    let tx = db
        .unchecked_transaction()
        .map_err(|e| sql_error("Proposal-Transaktion konnte nicht gestartet werden", e))?;
    for p in &proposals {
        validate_memory_kind(&p.proposal_kind)?;
        if !matches!(
            p.classification.as_str(),
            "observable" | "interpretation" | "author_decision_required" | "possible_contradiction"
        ) {
            return Err("Ungültige Character-Memory-Klassifikation.".into());
        }
        validate_probability(p.confidence, "Confidence")?;
        if p.evidence_excerpt.trim().is_empty() && p.classification != "author_decision_required" {
            return Err("Jeder belegte Vorschlag benötigt eine Evidence-Passage.".into());
        }
        if let Some(id) = &p.subject_character_id {
            validate_character(&tx, &run.0, id)?;
        }
        if let Some(id) = &p.related_character_id {
            validate_character(&tx, &run.0, id)?;
        }
        if let Some(id) = &p.target_entity_id {
            project_entity_exists(&tx, &run.0, id, None)?;
        }
        validate_memory_payload_tx(
            &tx,
            &run.0,
            &p.proposal_kind,
            &p.payload,
            p.subject_character_id.as_deref(),
            p.related_character_id.as_deref(),
        )?;
        let payload = serde_json::to_string(&p.payload)
            .map_err(|e| format!("Proposal-Payload ist ungültig: {e}"))?;
        let id = new_id();
        let analyzed_hash = if p.analyzed_content_hash.is_empty() {
            run.3.clone()
        } else {
            p.analyzed_content_hash.clone()
        };
        tx.execute("INSERT INTO character_memory_proposals(id,run_id,project_id,scene_id,proposal_kind,subject_character_id,related_character_id,target_entity_id,payload_json,classification,confidence,evidence_excerpt,start_offset,end_offset,reason,review_status,analyzed_content_hash,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,'pending',?17,?18)",params![id,run_id,run.0,run.1,p.proposal_kind,p.subject_character_id,p.related_character_id,p.target_entity_id,payload,p.classification,p.confidence,p.evidence_excerpt,p.start_offset,p.end_offset,p.reason,analyzed_hash,now()]).map_err(|e|sql_error("Character-Memory-Proposal konnte nicht gespeichert werden",e))?;
    }
    tx.execute(
        "UPDATE character_memory_update_runs SET status='completed',completed_at=?1 WHERE id=?2",
        params![now(), run_id],
    )
    .map_err(|e| sql_error("Character-Memory-Run konnte nicht abgeschlossen werden", e))?;
    tx.commit()
        .map_err(|e| sql_error("Proposal-Transaktion konnte nicht abgeschlossen werden", e))?;
    list_character_memory_proposals_from_db(&db, &run_id)
}
fn list_character_memory_proposals_from_db(
    db: &Connection,
    run_id: &str,
) -> Result<Vec<CharacterMemoryProposal>, String> {
    let mut stmt=db.prepare("SELECT id,run_id,project_id,scene_id,proposal_kind,subject_character_id,related_character_id,target_entity_id,payload_json,classification,confidence,evidence_excerpt,start_offset,end_offset,reason,review_status,reviewed_at,analyzed_content_hash,accepted_memory_id,accepted_memory_kind,created_at FROM character_memory_proposals WHERE run_id=?1 ORDER BY created_at ASC").map_err(|e|sql_error("Character-Memory-Proposals konnten nicht geladen werden",e))?;
    let rows = stmt
        .query_map(params![run_id], memory_proposal_from_row)
        .map_err(|e| sql_error("Character-Memory-Proposals konnten nicht geladen werden", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error("Character-Memory-Proposals konnten nicht geladen werden", e));
    rows
}
#[tauri::command]
pub fn list_character_memory_proposals(
    state: State<'_, DbState>,
    run_id: String,
) -> Result<Vec<CharacterMemoryProposal>, String> {
    let db = lock_db(&state)?;
    list_character_memory_proposals_from_db(&db, &run_id)
}
type CharacterMemoryReviewRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    Option<i64>,
    Option<i64>,
);

#[tauri::command]
pub fn review_character_memory_proposal(
    state: State<'_, DbState>,
    input: ReviewCharacterMemoryProposalInput,
) -> Result<CharacterMemoryProposal, String> {
    let db = lock_db(&state)?;
    let current: CharacterMemoryReviewRow = db.query_row("SELECT project_id,scene_id,proposal_kind,review_status,payload_json,subject_character_id,related_character_id,target_entity_id,evidence_excerpt,analyzed_content_hash,start_offset,end_offset FROM character_memory_proposals WHERE id=?1",params![input.proposal_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?))).map_err(|e|sql_error("Character-Memory-Proposal wurde nicht gefunden",e))?;
    if current.3 != "pending" {
        return Err("Dieser Character-Memory-Vorschlag wurde bereits geprüft.".into());
    }
    let status = input.review_status.clone();
    if !matches!(status.as_str(), "accepted" | "edited" | "rejected") {
        return Err("Ungültiger Review-Status.".into());
    }
    let tx = db
        .unchecked_transaction()
        .map_err(|e| sql_error("Review-Transaktion konnte nicht gestartet werden", e))?;
    let mut accepted: Option<(String, String)> = None;
    if status != "rejected" {
        let mut payload = input.payload.unwrap_or_else(|| {
            serde_json::from_str(&current.4).unwrap_or_else(|_| serde_json::json!({}))
        });
        if let Some(object) = payload.as_object_mut() {
            object.insert("subjectCharacterId".into(), serde_json::json!(current.5));
            object.insert("relatedCharacterId".into(), serde_json::json!(current.6));
            object.insert("targetEntityId".into(), serde_json::json!(current.7));
        }
        validate_memory_payload_tx(
            &tx,
            &current.0,
            &current.2,
            &payload,
            current.5.as_deref(),
            current.6.as_deref(),
        )?;
        accepted = Some(apply_memory_payload(
            &tx,
            &current.0,
            &current.1,
            &current.2,
            payload,
            &status,
            input.decision.as_deref(),
        )?);
        if !current.8.trim().is_empty() {
            let scene_data: (String, String) = tx.query_row("SELECT chapters.id,scenes.content FROM scenes JOIN chapters ON chapters.id=scenes.chapter_id WHERE scenes.id=?1", params![current.1], |row| Ok((row.get(0)?, row.get(1)?))).map_err(|e| sql_error("Quellenszene konnte nicht geladen werden", e))?;
            let text = crate::services::plain_text::editor_content_to_plain_text(&scene_data.1);
            let chars: Vec<char> = text.chars().collect();
            let (start, end) = match (current.10, current.11) {
                (Some(start), Some(end)) if start >= 0 && end > start && (end as usize) <= chars.len() && chars[start as usize..end as usize].iter().collect::<String>() == current.8 => (start, end),
                (None, None) => { let needle: Vec<char> = current.8.chars().collect(); let found = chars.windows(needle.len()).position(|window| window == needle.as_slice()).ok_or("Die Evidence-Passage wurde in der aktuellen Szene nicht gefunden. Bitte analysiere die Szene erneut.")? as i64; (found, found + needle.len() as i64) },
                _ => return Err("Die Evidence-Offsets passen nicht mehr zur aktuellen Szene. Bitte analysiere die Szene erneut.".into()),
            };
            let source = CreateSourceReferenceInput {
                project_id: current.0.clone(),
                entity_id: None,
                proposal_id: Some(input.proposal_id.clone()),
                chapter_id: scene_data.0,
                scene_id: current.1.clone(),
                excerpt: current.8.clone(),
                start_offset: Some(start),
                end_offset: Some(end),
            };
            let source_id = insert_source_reference_if_missing_tx(&tx, &source)?;
            if let Some((memory_id, memory_kind)) = &accepted {
                tx.execute("INSERT INTO character_memory_evidence(id,project_id,memory_kind,memory_id,source_reference_id,evidence_role,created_at) VALUES(?1,?2,?3,?4,?5,'primary',?6) ON CONFLICT(memory_kind,memory_id,source_reference_id) DO NOTHING", params![new_id(), current.0, memory_kind, memory_id, source_id, now()]).map_err(|e| sql_error("Gedächtnis-Evidence konnte nicht gespeichert werden", e))?;
            }
        }
    }
    tx.execute(
        "UPDATE character_memory_proposals SET review_status=?1,reviewed_at=?2,accepted_memory_id=?3,accepted_memory_kind=?4 WHERE id=?5",
        params![status, now(), accepted.as_ref().map(|value| value.0.clone()), accepted.as_ref().map(|value| value.1.clone()), input.proposal_id],
    )
    .map_err(|e| {
        sql_error(
            "Character-Memory-Proposal konnte nicht aktualisiert werden",
            e,
        )
    })?;
    tx.commit()
        .map_err(|e| sql_error("Review-Transaktion konnte nicht abgeschlossen werden", e))?;
    db.query_row("SELECT id,run_id,project_id,scene_id,proposal_kind,subject_character_id,related_character_id,target_entity_id,payload_json,classification,confidence,evidence_excerpt,start_offset,end_offset,reason,review_status,reviewed_at,analyzed_content_hash,accepted_memory_id,accepted_memory_kind,created_at FROM character_memory_proposals WHERE id=?1",params![input.proposal_id],memory_proposal_from_row).map_err(|e|sql_error("Geprüfter Character-Memory-Vorschlag konnte nicht geladen werden",e))
}
fn apply_memory_payload(
    tx: &rusqlite::Transaction<'_>,
    project: &str,
    scene: &str,
    kind: &str,
    payload: serde_json::Value,
    status: &str,
    decision: Option<&str>,
) -> Result<(String, String), String> {
    if status == "rejected" {
        return Err("Abgelehnte Vorschläge dürfen keine Produktdaten erzeugen.".into());
    }
    let text = |key: &str, fallback: &str| {
        payload
            .get(key)
            .and_then(|value| value.as_str())
            .unwrap_or(fallback)
            .to_string()
    };
    let stamp = now();
    let memory_status = if decision == Some("uncertain") {
        "uncertain"
    } else {
        "confirmed"
    };
    let author_confirmed = if memory_status == "confirmed" {
        1_i64
    } else {
        0_i64
    };
    let subject = payload
        .get("subjectCharacterId")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let id = if kind == "voice_pattern" {
        let pattern = payload
            .get("patternText")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let pattern_type = payload
            .get("patternType")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        tx.query_row("SELECT id FROM character_voice_patterns WHERE project_id=?1 AND character_id=?2 AND pattern_type=?3 AND LOWER(pattern_text)=LOWER(?4) AND related_character_id IS ?5 ORDER BY updated_at DESC LIMIT 1", params![project, subject, pattern_type, pattern, payload.get("relatedCharacterId").and_then(|value| value.as_str())], |row| row.get(0)).optional().map_err(|e| sql_error("Bestehende Sprachmuster konnten nicht geprüft werden", e))?.unwrap_or_else(new_id)
    } else {
        new_id()
    };
    match kind {
        "voice_pattern" => { let pattern = text("patternText", ""); if pattern.trim().is_empty() { return Err("Das Sprachmuster benötigt einen Text.".into()); } tx.execute("INSERT INTO character_voice_patterns(id,project_id,character_id,related_character_id,pattern_type,pattern_text,description,context_condition,confidence,status,author_confirmed,occurrence_count,first_observed_scene_id,last_observed_scene_id,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,1,?12,?12,?13,?13) ON CONFLICT(id) DO UPDATE SET occurrence_count=character_voice_patterns.occurrence_count+1,confidence=excluded.confidence,status=excluded.status,author_confirmed=excluded.author_confirmed,last_observed_scene_id=excluded.last_observed_scene_id,updated_at=excluded.updated_at", params![id,project,subject,payload.get("relatedCharacterId").and_then(|v|v.as_str()),text("patternType","signature_phrase"),pattern,text("description",""),text("contextCondition",""),payload.get("confidence").and_then(|v|v.as_f64()).unwrap_or(0.7),memory_status,author_confirmed,scene,stamp]).map_err(|e| sql_error("Sprachmuster konnte nicht übernommen werden",e))?; }
        "experience" => { let title=text("title",""); if title.trim().is_empty() { return Err("Das Erlebnis benötigt einen Titel.".into()); } tx.execute("INSERT INTO character_experiences(id,project_id,character_id,event_entity_id,scene_id,title,objective_summary,subjective_interpretation,emotional_impact,lasting_effect,significance,memory_reliability,status,author_confirmed,created_at,updated_at) VALUES(?1,?2,?3,NULL,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14)",params![id,project,subject,scene,title,text("objectiveSummary",""),text("subjectiveInterpretation",""),text("emotionalImpact",""),text("lastingEffect",""),text("significance","supporting"),text("memoryReliability","reliable"),memory_status,author_confirmed,stamp]).map_err(|e| sql_error("Erlebnis konnte nicht übernommen werden",e))?; }
        "relationship_memory" => { let related=payload.get("relatedCharacterId").and_then(|v|v.as_str()).unwrap_or_default(); let (a,b)=normalize_pair(subject,related)?; tx.execute("INSERT INTO relationship_memories(id,project_id,character_a_id,character_b_id,scene_id,memory_type,title,summary,private_meaning,relationship_effect,significance,status,author_confirmed,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",params![id,project,a,b,scene,text("memoryType","shared_memory"),text("title","Beziehungserinnerung"),text("summary",""),text("privateMeaning",""),text("relationshipEffect",""),text("significance","supporting"),memory_status,author_confirmed,stamp]).map_err(|e| sql_error("Beziehungserinnerung konnte nicht übernommen werden",e))?; }
        "knowledge_change" => { let fact=payload.get("factEntityId").and_then(|v|v.as_str()).unwrap_or_default(); if fact.is_empty() { return Err("Ein Wissensstand benötigt einen Fakt.".into()); } tx.execute("INSERT INTO character_knowledge_states(id,project_id,character_id,fact_entity_id,knowledge_state,acquired_scene_id,changed_scene_id,source_character_id,certainty,notes,status,author_confirmed,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?6,NULL,?7,?8,?9,?10,?11,?11)",params![id,project,subject,fact,text("knowledgeState","knows"),scene,payload.get("certainty").and_then(|v|v.as_f64()).unwrap_or(0.7),text("notes",""),memory_status,author_confirmed,stamp]).map_err(|e| sql_error("Wissensstand konnte nicht übernommen werden",e))?; }
        "dialogue_memory" => {
            let summary = text("summary", "");
            if summary.trim().is_empty() {
                return Err("Eine Dialogerinnerung benötigt eine Zusammenfassung.".into());
            }
            tx.execute("INSERT INTO character_dialogue_memories(id,project_id,speaker_id,scene_id,dialogue_kind,topic,summary,exact_excerpt,emotional_tone,hidden_intent,significance,truthfulness,status,author_confirmed,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15)",params![id,project,subject,scene,text("dialogueKind","statement"),text("topic",""),summary,text("exactExcerpt",""),text("emotionalTone",""),text("hiddenIntent",""),text("significance","supporting"),text("truthfulness","unknown"),memory_status,author_confirmed,stamp]).map_err(|e| sql_error("Dialogerinnerung konnte nicht übernommen werden",e))?;
            let participants = payload.get("participants").and_then(|value| value.as_array()).ok_or("Dialogteilnehmer fehlen.")?;
            let mut seen = std::collections::HashSet::new();
            let mut speaker_count = 0;
            for participant in participants {
                let character_id = participant.get("characterId").and_then(|value| value.as_str()).ok_or("Ungültiger Dialogteilnehmer.")?;
                let role = participant.get("role").and_then(|value| value.as_str()).ok_or("Ungültige Teilnehmerrolle.")?;
                validate_character(tx, project, character_id)?;
                validate_participant_role(role)?;
                if !seen.insert((character_id.to_string(), role.to_string())) {
                    return Err("Ein Dialogteilnehmer wurde doppelt angegeben.".into());
                }
                if character_id == subject && role == "speaker" { speaker_count += 1; }
                tx.execute("INSERT INTO dialogue_memory_participants(dialogue_memory_id,character_id,role) VALUES(?1,?2,?3)", params![id, character_id, role]).map_err(|e| sql_error("Dialogteilnehmer konnten nicht gespeichert werden",e))?;
            }
            if speaker_count != 1 { return Err("Der Sprecher muss genau einmal als speaker teilnehmen.".into()); }
        }
        "profile_observation" | "character_relation" => return Err("Dieser Vorschlagstyp benötigt die manuelle Charakterpflege und wurde nicht automatisch übernommen.".into()),
        _ => return Err("Unbekannter Character-Memory-Proposal-Typ.".into()),
    }
    Ok((id, kind.to_string()))
}
#[tauri::command]
pub fn complete_character_memory_review(
    state: State<'_, DbState>,
    run_id: String,
) -> Result<CharacterMemoryUpdateRun, String> {
    let db = lock_db(&state)?;
    let pending: i64=db.query_row("SELECT COUNT(*) FROM character_memory_proposals WHERE run_id=?1 AND review_status='pending'",params![run_id],|row|row.get(0)).map_err(|e|sql_error("Reviewstatus konnte nicht geprüft werden",e))?;
    if pending > 0 {
        return Err("Bitte prüfe zuerst alle Charaktergedächtnis-Vorschläge.".into());
    }
    db.execute("UPDATE character_memory_update_runs SET status='reviewed',completed_at=COALESCE(completed_at,?1) WHERE id=?2",params![now(),run_id]).map_err(|e|sql_error("Character-Memory-Run konnte nicht abgeschlossen werden",e))?;
    db.query_row("SELECT id,project_id,scene_id,content_hash,extractor_id,analyzed_content,status,created_at,completed_at,error_message FROM character_memory_update_runs WHERE id=?1",params![run_id],memory_run_from_row).map_err(|e|sql_error("Character-Memory-Run konnte nicht geladen werden",e))
}

fn json_array<T: serde::de::DeserializeOwned>(value: String) -> Result<Vec<T>, String> {
    serde_json::from_str(&value).map_err(|error| sql_error("Longform-Daten sind ungültig", error))
}

fn direction_from_row(row: &rusqlite::Row<'_>) -> SqlResult<StoryDirection> {
    Ok(StoryDirection {
        project_id: row.get(0)?,
        premise: row.get(1)?,
        current_story_phase: row.get(2)?,
        book_goal: row.get(3)?,
        planned_ending: row.get(4)?,
        ending_status: row.get(5)?,
        central_twist: row.get(6)?,
        thematic_goal: row.get(7)?,
        must_happen: json_array(row.get(8)?).unwrap_or_default(),
        must_not_happen: json_array(row.get(9)?).unwrap_or_default(),
        next_turning_point: row.get(10)?,
        reveal_constraints: json_array(row.get(11)?).unwrap_or_default(),
        author_notes: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn preferences_from_row(row: &rusqlite::Row<'_>) -> SqlResult<WritingPreferences> {
    Ok(WritingPreferences {
        project_id: row.get(0)?,
        words_per_page: row.get(1)?,
        preferred_section_words: row.get(2)?,
        maximum_section_words: row.get(3)?,
        default_scene_count: row.get(4)?,
        require_plan_confirmation: row.get::<_, i64>(5)? != 0,
        require_final_confirmation: row.get::<_, i64>(6)? != 0,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn validate_direction(input: &SaveStoryDirectionInput) -> Result<(), String> {
    if !matches!(input.ending_status.as_str(), "fixed" | "preferred" | "open") {
        return Err("Ungültiger Status des geplanten Endes.".into());
    }
    if input.premise.len() > 10_000 || input.planned_ending.len() > 10_000 {
        return Err("Story-Richtung ist zu lang.".into());
    }
    Ok(())
}

fn validate_preferences(input: &SaveWritingPreferencesInput) -> Result<(), String> {
    if !(150..=500).contains(&input.words_per_page)
        || !(400..=1500).contains(&input.preferred_section_words)
        || !(600..=2000).contains(&input.maximum_section_words)
        || !(1..=12).contains(&input.default_scene_count)
    {
        return Err("Schreibpräferenzen liegen außerhalb der erlaubten Grenzen.".into());
    }
    if input.maximum_section_words < input.preferred_section_words {
        return Err(
            "Die maximale Abschnittslänge muss mindestens der bevorzugten Länge entsprechen."
                .into(),
        );
    }
    Ok(())
}

#[tauri::command]
pub fn get_story_direction(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<Option<StoryDirection>, String> {
    let db = lock_db(&state)?;
    db.query_row("SELECT project_id,premise,current_story_phase,book_goal,planned_ending,ending_status,central_twist,thematic_goal,must_happen_json,must_not_happen_json,next_turning_point,reveal_constraints_json,author_notes,created_at,updated_at FROM project_story_direction WHERE project_id=?1", params![project_id], direction_from_row).optional().map_err(|error| sql_error("Story-Richtung konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn save_story_direction(
    state: State<'_, DbState>,
    input: SaveStoryDirectionInput,
) -> Result<StoryDirection, String> {
    validate_direction(&input)?;
    let db = lock_db(&state)?;
    let stamp = now();
    let project_exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            params![input.project_id],
            |row| row.get(0),
        )
        .map_err(|e| sql_error("Projekt konnte nicht geprüft werden", e))?;
    if !project_exists {
        return Err("Das Projekt wurde nicht gefunden.".into());
    }
    let must = serde_json::to_string(&input.must_happen)
        .map_err(|e| sql_error("Story-Richtung konnte nicht serialisiert werden", e))?;
    let must_not = serde_json::to_string(&input.must_not_happen)
        .map_err(|e| sql_error("Story-Richtung konnte nicht serialisiert werden", e))?;
    let reveal = serde_json::to_string(&input.reveal_constraints)
        .map_err(|e| sql_error("Enthüllungsgrenzen konnten nicht serialisiert werden", e))?;
    db.execute("INSERT INTO project_story_direction(project_id,premise,current_story_phase,book_goal,planned_ending,ending_status,central_twist,thematic_goal,must_happen_json,must_not_happen_json,next_turning_point,reveal_constraints_json,author_notes,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,COALESCE((SELECT created_at FROM project_story_direction WHERE project_id=?1),?14),?14) ON CONFLICT(project_id) DO UPDATE SET premise=excluded.premise,current_story_phase=excluded.current_story_phase,book_goal=excluded.book_goal,planned_ending=excluded.planned_ending,ending_status=excluded.ending_status,central_twist=excluded.central_twist,thematic_goal=excluded.thematic_goal,must_happen_json=excluded.must_happen_json,must_not_happen_json=excluded.must_not_happen_json,next_turning_point=excluded.next_turning_point,reveal_constraints_json=excluded.reveal_constraints_json,author_notes=excluded.author_notes,updated_at=excluded.updated_at", params![input.project_id,input.premise,input.current_story_phase,input.book_goal,input.planned_ending,input.ending_status,input.central_twist,input.thematic_goal,must,must_not,input.next_turning_point,reveal,input.author_notes,stamp]).map_err(|e| sql_error("Story-Richtung konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT project_id,premise,current_story_phase,book_goal,planned_ending,ending_status,central_twist,thematic_goal,must_happen_json,must_not_happen_json,next_turning_point,reveal_constraints_json,author_notes,created_at,updated_at FROM project_story_direction WHERE project_id=?1", params![input.project_id], direction_from_row).map_err(|e| sql_error("Story-Richtung konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn get_writing_preferences(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<WritingPreferences, String> {
    let db = lock_db(&state)?;
    if let Some(value) = db.query_row("SELECT project_id,words_per_page,preferred_section_words,maximum_section_words,default_scene_count,require_plan_confirmation,require_final_confirmation,created_at,updated_at FROM project_writing_preferences WHERE project_id=?1", params![project_id], preferences_from_row).optional().map_err(|e| sql_error("Schreibpräferenzen konnten nicht geladen werden", e))? { return Ok(value); }
    let stamp = now();
    db.execute("INSERT INTO project_writing_preferences(project_id,words_per_page,preferred_section_words,maximum_section_words,default_scene_count,require_plan_confirmation,require_final_confirmation,created_at,updated_at) VALUES(?1,250,850,1200,4,1,1,?2,?2)", params![project_id, stamp]).map_err(|e| sql_error("Standard-Schreibpräferenzen konnten nicht gespeichert werden", e))?;
    db.query_row("SELECT project_id,words_per_page,preferred_section_words,maximum_section_words,default_scene_count,require_plan_confirmation,require_final_confirmation,created_at,updated_at FROM project_writing_preferences WHERE project_id=?1", params![project_id], preferences_from_row).map_err(|e| sql_error("Standard-Schreibpräferenzen konnten nicht geladen werden", e))
}

#[tauri::command]
pub fn save_writing_preferences(
    state: State<'_, DbState>,
    input: SaveWritingPreferencesInput,
) -> Result<WritingPreferences, String> {
    validate_preferences(&input)?;
    let db = lock_db(&state)?;
    let stamp = now();
    db.execute("INSERT INTO project_writing_preferences(project_id,words_per_page,preferred_section_words,maximum_section_words,default_scene_count,require_plan_confirmation,require_final_confirmation,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,COALESCE((SELECT created_at FROM project_writing_preferences WHERE project_id=?1),?8),?8) ON CONFLICT(project_id) DO UPDATE SET words_per_page=excluded.words_per_page,preferred_section_words=excluded.preferred_section_words,maximum_section_words=excluded.maximum_section_words,default_scene_count=excluded.default_scene_count,require_plan_confirmation=excluded.require_plan_confirmation,require_final_confirmation=excluded.require_final_confirmation,updated_at=excluded.updated_at", params![input.project_id,input.words_per_page,input.preferred_section_words,input.maximum_section_words,input.default_scene_count,input.require_plan_confirmation as i64,input.require_final_confirmation as i64,stamp]).map_err(|e| sql_error("Schreibpräferenzen konnten nicht gespeichert werden", e))?;
    db.query_row("SELECT project_id,words_per_page,preferred_section_words,maximum_section_words,default_scene_count,require_plan_confirmation,require_final_confirmation,created_at,updated_at FROM project_writing_preferences WHERE project_id=?1", params![input.project_id], preferences_from_row).map_err(|e| sql_error("Schreibpräferenzen konnten nicht geladen werden", e))
}

fn job_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ChapterGenerationJob> {
    Ok(ChapterGenerationJob {
        id: row.get(0)?,
        project_id: row.get(1)?,
        target_book_id: row.get(2)?,
        target_after_chapter_id: row.get(3)?,
        requested_pages: row.get(4)?,
        target_words: row.get(5)?,
        requested_scene_count: row.get(6)?,
        user_instruction: row.get(7)?,
        status: row.get(8)?,
        active_provider: row.get(9)?,
        content_context_hash: row.get(10)?,
        context_override_accepted: row.get::<_, i64>(11)? != 0,
        last_resumed_at: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        completed_at: row.get(15)?,
        error_message: row.get(16)?,
    })
}

fn load_job(db: &Connection, id: &str) -> Result<ChapterGenerationJob, String> {
    db.query_row("SELECT id,project_id,target_book_id,target_after_chapter_id,requested_pages,target_words,requested_scene_count,user_instruction,status,active_provider,content_context_hash,context_override_accepted,last_resumed_at,created_at,updated_at,completed_at,error_message FROM chapter_generation_jobs WHERE id=?1", params![id], job_from_row).map_err(|e| sql_error("Schreibauftrag konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn create_chapter_generation_job(
    state: State<'_, DbState>,
    input: CreateChapterGenerationJobInput,
) -> Result<ChapterGenerationJob, String> {
    if input.target_words < 1 {
        return Err("Der Zielumfang muss größer als 0 sein.".into());
    }
    let db = lock_db(&state)?;
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM books WHERE id=?1 AND project_id=?2)",
            params![input.target_book_id, input.project_id],
            |row| row.get(0),
        )
        .map_err(|e| sql_error("Zielbuch konnte nicht geprüft werden", e))?;
    if !exists {
        return Err("Das Zielbuch wurde nicht gefunden.".into());
    }
    if let Some(chapter) = &input.target_after_chapter_id {
        let valid: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM chapters WHERE id=?1 AND book_id=?2)",
                params![chapter, input.target_book_id],
                |row| row.get(0),
            )
            .map_err(|e| sql_error("Kapitelposition konnte nicht geprüft werden", e))?;
        if !valid {
            return Err("Die Zielposition gehört nicht zum Zielbuch.".into());
        }
    }
    let id = new_id();
    let stamp = now();
    db.execute("INSERT INTO chapter_generation_jobs(id,project_id,target_book_id,target_after_chapter_id,requested_pages,target_words,requested_scene_count,user_instruction,status,active_provider,content_context_hash,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'preparing',?9,?10,?11,?11)", params![id,input.project_id,input.target_book_id,input.target_after_chapter_id,input.requested_pages,input.target_words,input.requested_scene_count,input.user_instruction,input.active_provider,input.content_context_hash,stamp]).map_err(|e| sql_error("Schreibauftrag konnte nicht gespeichert werden", e))?;
    load_job(&db, &id)
}

#[tauri::command]
pub fn list_chapter_generation_jobs(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<ChapterGenerationJob>, String> {
    let db = lock_db(&state)?;
    let mut statement=db.prepare("SELECT id,project_id,target_book_id,target_after_chapter_id,requested_pages,target_words,requested_scene_count,user_instruction,status,active_provider,content_context_hash,context_override_accepted,last_resumed_at,created_at,updated_at,completed_at,error_message FROM chapter_generation_jobs WHERE project_id=?1 ORDER BY updated_at DESC").map_err(|e|sql_error("Schreibaufträge konnten nicht geladen werden",e))?;
    let rows = statement
        .query_map(params![project_id], job_from_row)
        .map_err(|e| sql_error("Schreibaufträge konnten nicht geladen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Schreibaufträge konnten nicht geladen werden", e))?;
    Ok(rows)
}

#[tauri::command]
pub fn update_chapter_generation_job_status(
    state: State<'_, DbState>,
    job_id: String,
    status: String,
    error_message: Option<String>,
) -> Result<ChapterGenerationJob, String> {
    if !matches!(
        status.as_str(),
        "preparing"
            | "needs_input"
            | "planning"
            | "plan_ready"
            | "generating"
            | "reviewing"
            | "draft_ready"
            | "accepted"
            | "cancelled"
            | "failed"
    ) {
        return Err("Ungültiger Schreibauftragsstatus.".into());
    }
    let db = lock_db(&state)?;
    let stamp = now();
    db.execute("UPDATE chapter_generation_jobs SET status=?1,error_message=?2,updated_at=?3,completed_at=CASE WHEN ?1 IN ('accepted','cancelled','failed') THEN COALESCE(completed_at,?3) ELSE completed_at END WHERE id=?4",params![status,error_message,stamp,job_id]).map_err(|e|sql_error("Schreibauftragsstatus konnte nicht aktualisiert werden",e))?;
    load_job(&db, &job_id)
}

#[tauri::command]
pub fn accept_chapter_generation_context_override(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<ChapterGenerationJob, String> {
    let db = lock_db(&state)?;
    let changed = db.execute("UPDATE chapter_generation_jobs SET context_override_accepted=1,updated_at=?1,last_resumed_at=?1 WHERE id=?2", params![now(), job_id]).map_err(|e| sql_error("Kontextübernahme konnte nicht gespeichert werden", e))?;
    if changed == 0 {
        return Err("Schreibauftrag nicht gefunden.".into());
    }
    load_job(&db, &job_id)
}

fn plan_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ChapterGenerationPlan> {
    let new_information: Vec<String> = json_array(row.get(9)?).unwrap_or_default();
    let withheld_information: Vec<String> = json_array(row.get(10)?).unwrap_or_default();
    let beats: Vec<ChapterPlanBeat> =
        serde_json::from_str(&row.get::<_, String>(11)?).unwrap_or_default();
    Ok(ChapterGenerationPlan {
        id: row.get(0)?,
        job_id: row.get(1)?,
        chapter_title: row.get(2)?,
        chapter_goal: row.get(3)?,
        pov_character_id: row.get(4)?,
        starting_state: row.get(5)?,
        ending_state: row.get(6)?,
        chapter_summary: row.get(7)?,
        ending_connection: row.get(8)?,
        new_information,
        withheld_information,
        beats,
        review_status: row.get(12)?,
        reviewed_at: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

fn plan_select() -> &'static str {
    "SELECT id,job_id,chapter_title,chapter_goal,pov_character_id,starting_state,ending_state,chapter_summary,ending_connection,new_information,withheld_information,plan_json,review_status,reviewed_at,created_at,updated_at FROM chapter_generation_plans WHERE job_id=?1"
}

#[tauri::command]
pub fn get_chapter_generation_plan(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Option<ChapterGenerationPlan>, String> {
    let db = lock_db(&state)?;
    db.query_row(plan_select(), params![job_id], plan_from_row)
        .optional()
        .map_err(|e| sql_error("Kapitelplan konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn save_chapter_generation_plan(
    state: State<'_, DbState>,
    input: SaveChapterGenerationPlanInput,
) -> Result<ChapterGenerationPlan, String> {
    if input.chapter_title.trim().is_empty()
        || input.chapter_goal.trim().is_empty()
        || input.beats.is_empty()
    {
        return Err("Ein Kapitelplan benötigt Titel, Ziel und mindestens einen Beat.".into());
    }
    let db = lock_db(&state)?;
    let job = load_job(&db, &input.job_id)?;
    if let Some(pov) = &input.pov_character_id {
        let valid: bool=db.query_row("SELECT EXISTS(SELECT 1 FROM story_entities WHERE id=?1 AND project_id=?2 AND entity_type='character')",params![pov,job.project_id],|row|row.get(0)).map_err(|e|sql_error("POV-Figur konnte nicht geprüft werden",e))?;
        if !valid {
            return Err("Die POV-Figur gehört nicht zum Projekt.".into());
        }
    }
    let plan_id = new_id();
    let stamp = now();
    let new_info = serde_json::to_string(&input.new_information)
        .map_err(|e| sql_error("Plan konnte nicht serialisiert werden", e))?;
    let withheld = serde_json::to_string(&input.withheld_information)
        .map_err(|e| sql_error("Plan konnte nicht serialisiert werden", e))?;
    let beats = serde_json::to_string(&input.beats)
        .map_err(|e| sql_error("Planbeats konnten nicht serialisiert werden", e))?;
    db.execute("INSERT INTO chapter_generation_plans(id,job_id,chapter_title,chapter_goal,pov_character_id,starting_state,ending_state,chapter_summary,ending_connection,new_information,withheld_information,plan_json,review_status,reviewed_at,created_at,updated_at) VALUES(COALESCE((SELECT id FROM chapter_generation_plans WHERE job_id=?1),?2),?1,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,CASE WHEN ?13='accepted' THEN ?14 ELSE NULL END,COALESCE((SELECT created_at FROM chapter_generation_plans WHERE job_id=?1),?14),?14) ON CONFLICT(job_id) DO UPDATE SET chapter_title=excluded.chapter_title,chapter_goal=excluded.chapter_goal,pov_character_id=excluded.pov_character_id,starting_state=excluded.starting_state,ending_state=excluded.ending_state,chapter_summary=excluded.chapter_summary,ending_connection=excluded.ending_connection,new_information=excluded.new_information,withheld_information=excluded.withheld_information,plan_json=excluded.plan_json,review_status=excluded.review_status,reviewed_at=excluded.reviewed_at,updated_at=excluded.updated_at",params![input.job_id,plan_id,input.chapter_title,input.chapter_goal,input.pov_character_id,input.starting_state,input.ending_state,input.chapter_summary,input.ending_connection,new_info,withheld,beats,input.review_status,stamp]).map_err(|e|sql_error("Kapitelplan konnte nicht gespeichert werden",e))?;
    db.execute(
        "UPDATE chapter_generation_jobs SET status='plan_ready',updated_at=?1 WHERE id=?2",
        params![stamp, input.job_id],
    )
    .map_err(|e| sql_error("Schreibauftrag konnte nicht aktualisiert werden", e))?;
    db.query_row(plan_select(), params![input.job_id], plan_from_row)
        .map_err(|e| sql_error("Gespeicherter Kapitelplan konnte nicht geladen werden", e))
}

fn section_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ChapterGenerationSection> {
    Ok(ChapterGenerationSection {
        id: row.get(0)?,
        job_id: row.get(1)?,
        plan_beat_id: row.get(2)?,
        order_index: row.get(3)?,
        target_words: row.get(4)?,
        actual_words: row.get(5)?,
        content: row.get(6)?,
        continuation_summary: row.get(7)?,
        continuity_state: serde_json::from_str(&row.get::<_, String>(8)?).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                8,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Ungültiger Continuity State",
                )),
            )
        })?,
        status: row.get(9)?,
        provider_id: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

#[tauri::command]
pub fn list_chapter_generation_sections(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<ChapterGenerationSection>, String> {
    let db = lock_db(&state)?;
    let mut s=db.prepare("SELECT id,job_id,plan_beat_id,order_index,target_words,actual_words,content,continuation_summary,continuity_state_json,status,provider_id,created_at,updated_at FROM chapter_generation_sections WHERE job_id=?1 ORDER BY order_index").map_err(|e|sql_error("Kapitelabschnitte konnten nicht geladen werden",e))?;
    let rows = s
        .query_map(params![job_id], section_from_row)
        .map_err(|e| sql_error("Kapitelabschnitte konnten nicht geladen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Kapitelabschnitte konnten nicht geladen werden", e))?;
    Ok(rows)
}

#[tauri::command]
pub fn save_chapter_generation_section(
    state: State<'_, DbState>,
    input: SaveChapterGenerationSectionInput,
) -> Result<ChapterGenerationSection, String> {
    if input.order_index < 0 || input.target_words < 1 {
        return Err("Ungültiger Abschnitt.".into());
    }
    let db = lock_db(&state)?;
    let _ = load_job(&db, &input.job_id)?;
    let stamp = now();
    let id = new_id();
    let actual = input.content.split_whitespace().count() as i64;
    let state_json = serde_json::to_string(&input.continuity_state)
        .map_err(|e| sql_error("Continuity State konnte nicht serialisiert werden", e))?;
    db.execute("INSERT INTO chapter_generation_sections(id,job_id,plan_beat_id,order_index,target_words,actual_words,content,continuation_summary,continuity_state_json,status,provider_id,created_at,updated_at) VALUES(COALESCE((SELECT id FROM chapter_generation_sections WHERE job_id=?1 AND order_index=?2),?3),?1,?4,?2,?5,?6,?7,?8,?9,?10,?11,COALESCE((SELECT created_at FROM chapter_generation_sections WHERE job_id=?1 AND order_index=?2),?12),?12) ON CONFLICT(job_id,order_index) DO UPDATE SET plan_beat_id=excluded.plan_beat_id,target_words=excluded.target_words,actual_words=excluded.actual_words,content=excluded.content,continuation_summary=excluded.continuation_summary,continuity_state_json=excluded.continuity_state_json,status=excluded.status,provider_id=excluded.provider_id,updated_at=excluded.updated_at",params![input.job_id,input.order_index,id,input.plan_beat_id,input.target_words,actual,input.content,input.continuation_summary,state_json,input.status,input.provider_id,stamp]).map_err(|e|sql_error("Kapitelabschnitt konnte nicht gespeichert werden",e))?;
    db.query_row("SELECT id,job_id,plan_beat_id,order_index,target_words,actual_words,content,continuation_summary,continuity_state_json,status,provider_id,created_at,updated_at FROM chapter_generation_sections WHERE job_id=?1 AND order_index=?2",params![input.job_id,input.order_index],section_from_row).map_err(|e|sql_error("Gespeicherter Kapitelabschnitt konnte nicht geladen werden",e))
}

fn review_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ChapterGenerationReview> {
    Ok(ChapterGenerationReview {
        id: row.get(0)?,
        job_id: row.get(1)?,
        section_id: row.get(2)?,
        review_scope: row.get(3)?,
        issue_type: row.get(4)?,
        severity: row.get(5)?,
        title: row.get(6)?,
        description: row.get(7)?,
        related_entity_ids: json_array(row.get(8)?).unwrap_or_default(),
        related_source_ids: json_array(row.get(9)?).unwrap_or_default(),
        suggested_action: row.get(10)?,
        status: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

#[tauri::command]
pub fn list_chapter_generation_reviews(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<ChapterGenerationReview>, String> {
    let db = lock_db(&state)?;
    let mut s=db.prepare("SELECT id,job_id,section_id,review_scope,issue_type,severity,title,description,related_entity_ids_json,related_source_ids_json,suggested_action,status,created_at,updated_at FROM chapter_generation_reviews WHERE job_id=?1 ORDER BY created_at").map_err(|e|sql_error("Kapitelprüfungen konnten nicht geladen werden",e))?;
    let rows = s
        .query_map(params![job_id], review_from_row)
        .map_err(|e| sql_error("Kapitelprüfungen konnten nicht geladen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Kapitelprüfungen konnten nicht geladen werden", e))?;
    Ok(rows)
}

#[tauri::command]
pub fn save_chapter_generation_reviews(
    state: State<'_, DbState>,
    job_id: String,
    reviews: Vec<SaveChapterGenerationReviewInput>,
) -> Result<Vec<ChapterGenerationReview>, String> {
    let db = lock_db(&state)?;
    let _ = load_job(&db, &job_id)?;
    let tx = db
        .unchecked_transaction()
        .map_err(|e| sql_error("Kapitelprüfung konnte nicht gestartet werden", e))?;
    for review in reviews {
        if !matches!(review.review_scope.as_str(), "section" | "chapter")
            || !matches!(review.severity.as_str(), "info" | "warning" | "blocking")
            || review.title.chars().count() > 300
            || review.description.chars().count() > 4000
        {
            return Err("Ungültige Kapitelprüfung.".into());
        }
        let id = new_id();
        let related_entities = serde_json::to_string(&review.related_entity_ids)
            .map_err(|e| sql_error("Prüfungsbezug konnte nicht serialisiert werden", e))?;
        let related_sources = serde_json::to_string(&review.related_source_ids)
            .map_err(|e| sql_error("Prüfungsquellen konnten nicht serialisiert werden", e))?;
        tx.execute("INSERT INTO chapter_generation_reviews(id,job_id,section_id,review_scope,issue_type,severity,title,description,related_entity_ids_json,related_source_ids_json,suggested_action,status,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13)", params![id, job_id, review.section_id, review.review_scope, review.issue_type, review.severity, review.title, review.description, related_entities, related_sources, review.suggested_action, review.status, now()]).map_err(|e| sql_error("Kapitelprüfung konnte nicht gespeichert werden", e))?;
    }
    tx.commit()
        .map_err(|e| sql_error("Kapitelprüfung konnte nicht abgeschlossen werden", e))?;
    let mut stmt = db.prepare("SELECT id,job_id,section_id,review_scope,issue_type,severity,title,description,related_entity_ids_json,related_source_ids_json,suggested_action,status,created_at,updated_at FROM chapter_generation_reviews WHERE job_id=?1 ORDER BY created_at").map_err(|e| sql_error("Kapitelprüfungen konnten nicht geladen werden", e))?;
    let result = stmt
        .query_map(params![job_id], review_from_row)
        .map_err(|e| sql_error("Kapitelprüfungen konnten nicht geladen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Kapitelprüfungen konnten nicht geladen werden", e));
    result
}

#[tauri::command]
pub fn delete_chapter_generation_reviews_for_section(
    state: State<'_, DbState>,
    job_id: String,
    section_id: String,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    db.execute(
        "DELETE FROM chapter_generation_reviews WHERE job_id=?1 AND section_id=?2",
        params![job_id, section_id],
    )
    .map_err(|e| sql_error("Kapitelprüfungen konnten nicht gelöscht werden", e))?;
    Ok(())
}

#[tauri::command]
pub fn update_chapter_generation_review_status(
    state: State<'_, DbState>,
    id: String,
    status: String,
) -> Result<ChapterGenerationReview, String> {
    if !matches!(
        status.as_str(),
        "open" | "accepted" | "exception" | "resolved"
    ) {
        return Err("Ungültiger Prüfstatus.".into());
    }
    let db = lock_db(&state)?;
    db.execute(
        "UPDATE chapter_generation_reviews SET status=?1,updated_at=?2 WHERE id=?3",
        params![status, now(), id],
    )
    .map_err(|e| sql_error("Prüfstatus konnte nicht aktualisiert werden", e))?;
    db.query_row("SELECT id,job_id,section_id,review_scope,issue_type,severity,title,description,related_entity_ids_json,related_source_ids_json,suggested_action,status,created_at,updated_at FROM chapter_generation_reviews WHERE id=?1", params![id], review_from_row).map_err(|e| sql_error("Kapitelprüfung konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn accept_chapter_generation_job(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<ChapterGenerationJob, String> {
    let db = lock_db(&state)?;
    let job = load_job(&db, &job_id)?;
    if job.status != "draft_ready" {
        return Err("Der Entwurf ist noch nicht zur Übernahme bereit.".into());
    }
    let plan = db
        .query_row(plan_select(), params![job_id], plan_from_row)
        .map_err(|e| sql_error("Kapitelplan fehlt", e))?;
    let sections = list_chapter_generation_sections_from_db(&db, &job.id)?;
    if sections.is_empty() {
        return Err("Der Entwurf enthält noch keine Abschnitte.".into());
    }
    if plan.review_status != "accepted"
        || sections
            .iter()
            .any(|section| section.content.trim().is_empty())
    {
        return Err("Plan und alle Abschnitte müssen vor der Übernahme bestätigt sein.".into());
    }
    let blocking: i64 = db.query_row("SELECT COUNT(*) FROM chapter_generation_reviews WHERE job_id=?1 AND severity='blocking' AND status='open'", params![job_id], |row| row.get(0)).map_err(|e| sql_error("Kapitelprüfungen konnten nicht geprüft werden", e))?;
    if blocking > 0 {
        return Err("Offene blockierende Kapitelprüfung verhindert die Übernahme.".into());
    }
    let tx = db
        .unchecked_transaction()
        .map_err(|e| sql_error("Übernahmetransaktion konnte nicht gestartet werden", e))?;
    let title = plan.chapter_title.trim();
    let order: i64 = if let Some(after_id) = &job.target_after_chapter_id {
        let after_order: i64 = tx
            .query_row(
                "SELECT order_index FROM chapters WHERE id=?1 AND book_id=?2",
                params![after_id, job.target_book_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| sql_error("Kapitelposition konnte nicht ermittelt werden", e))?
            .ok_or("Das Zielkapitel wurde seit Beginn des Entwurfs verschoben oder gelöscht.")?;
        tx.execute("UPDATE chapters SET order_index=order_index+1,updated_at=?1 WHERE book_id=?2 AND order_index>?3", params![now(), job.target_book_id, after_order]).map_err(|e| sql_error("Kapitelreihenfolge konnte nicht aktualisiert werden", e))?;
        after_order + 1
    } else {
        tx.query_row(
            "SELECT COALESCE(MAX(order_index),0)+1 FROM chapters WHERE book_id=?1",
            params![job.target_book_id],
            |r| r.get(0),
        )
        .map_err(|e| sql_error("Kapitelposition konnte nicht ermittelt werden", e))?
    };
    let chapter_id = new_id();
    tx.execute(
        "INSERT INTO chapters(id,book_id,title,order_index) VALUES(?1,?2,?3,?4)",
        params![chapter_id, job.target_book_id, title, order],
    )
    .map_err(|e| sql_error("Kapitel konnte nicht übernommen werden", e))?;
    for (index, section) in sections.iter().enumerate() {
        let scene_id = new_id();
        let beat = plan.beats.get(index);
        let title = beat
            .map(|item| item.title.clone())
            .unwrap_or_else(|| format!("Szene {}", index + 1));
        let pov = beat
            .and_then(|item| item.pov_character_id.clone())
            .or_else(|| plan.pov_character_id.clone())
            .unwrap_or_default();
        let location = beat
            .and_then(|item| item.location.clone())
            .unwrap_or_default();
        tx.execute("INSERT INTO scenes(id,chapter_id,title,order_index,content,pov,location,story_time,status,goal,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,'','draft',?8,'')",params![scene_id,chapter_id,title,index as i64+1,section.content,pov,location,plan.chapter_goal]).map_err(|e|sql_error("Szene konnte nicht übernommen werden",e))?;
        let scene = Scene {
            id: scene_id.clone(),
            chapter_id: chapter_id.clone(),
            title,
            order_index: index as i64 + 1,
            content: section.content.clone(),
            pov,
            location,
            story_time: String::new(),
            status: "draft".into(),
            goal: plan.chapter_goal.clone(),
            notes: String::new(),
            created_at: now(),
            updated_at: now(),
        };
        insert_scene_version_in_transaction(&tx, &scene, &now(), "manual")?;
    }
    tx.execute("UPDATE chapter_generation_jobs SET status='accepted',completed_at=?1,updated_at=?1 WHERE id=?2",params![now(),job.id]).map_err(|e|sql_error("Schreibauftrag konnte nicht abgeschlossen werden",e))?;
    tx.commit()
        .map_err(|e| sql_error("Kapitelübernahme konnte nicht abgeschlossen werden", e))?;
    load_job(&db, &job.id)
}

fn list_chapter_generation_sections_from_db(
    db: &Connection,
    job_id: &str,
) -> Result<Vec<ChapterGenerationSection>, String> {
    let mut s=db.prepare("SELECT id,job_id,plan_beat_id,order_index,target_words,actual_words,content,continuation_summary,continuity_state_json,status,provider_id,created_at,updated_at FROM chapter_generation_sections WHERE job_id=?1 ORDER BY order_index").map_err(|e|sql_error("Kapitelabschnitte konnten nicht geladen werden",e))?;
    let rows = s
        .query_map(params![job_id], section_from_row)
        .map_err(|e| sql_error("Kapitelabschnitte konnten nicht geladen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Kapitelabschnitte konnten nicht geladen werden", e))?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{
        database_path_for_test, has_column, initialize_connection, seed_if_empty,
    };
    use crate::models::ManuscriptImportChapterInput;
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
    fn manuscript_import_creates_one_text_scene_and_initial_version_per_chapter() {
        let (path, db) = connection("manuscript-import");
        let result = import_manuscript_in_db(
            &db,
            ManuscriptImportInput {
                project_id: "project-zugestellt".into(),
                book_id: "book-1".into(),
                chapters: vec![
                    ManuscriptImportChapterInput {
                        title: "Kapitel 1".into(),
                        content: "Der Anfang.".into(),
                    },
                    ManuscriptImportChapterInput {
                        title: "Kapitel 2".into(),
                        content: "Die Fortsetzung.".into(),
                    },
                ],
            },
        )
        .unwrap();
        assert_eq!(result.chapters.len(), 2);
        assert!(result
            .chapters
            .iter()
            .all(|chapter| chapter.scenes.len() == 1));
        assert_eq!(result.scenes.len(), 2);
        assert_eq!(result.versions.len(), 2);
        assert!(result
            .versions
            .iter()
            .all(|version| version.reason == "before_import"));
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM scenes WHERE chapter_id IN (?1, ?2)",
                params![result.chapters[0].id, result.chapters[1].id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            2
        );
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn manuscript_import_rolls_back_when_one_chapter_is_invalid() {
        let (path, db) = connection("manuscript-import-rollback");
        let before: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM chapters WHERE book_id='book-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let error = import_manuscript_in_db(
            &db,
            ManuscriptImportInput {
                project_id: "project-zugestellt".into(),
                book_id: "book-1".into(),
                chapters: vec![
                    ManuscriptImportChapterInput {
                        title: "Gültig".into(),
                        content: "Text".into(),
                    },
                    ManuscriptImportChapterInput {
                        title: "   ".into(),
                        content: "Ungültig".into(),
                    },
                ],
            },
        )
        .unwrap_err();
        assert!(error.contains("Kapitelname"));
        let after: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM chapters WHERE book_id='book-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(before, after);
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
    fn scene_update_marks_confirmed_summary_outdated_when_content_changes() {
        let (path, db) = connection("summary-outdated");
        let mut input = scene_input_from_scene(&load_scene(&db, "scene-1").unwrap());
        input.content = "Marek blieb stehen.".into();
        update_scene_in_db(&db, input.clone()).unwrap();
        db.execute(
            "INSERT INTO narrative_summaries(id,project_id,scope_type,scope_id,content_hash,summary,status,author_confirmed) VALUES('summary-outdated','project-zugestellt','scene','scene-1',?1,'Alt','confirmed',1)",
            params![canonical_content_hash(&input.content)],
        )
        .unwrap();
        input.content = "<p>Marek lief weiter.</p>".into();
        update_scene_in_db(&db, input).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT status FROM narrative_summaries WHERE id='summary-outdated'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "outdated"
        );
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

    fn insert_review_fixture(
        db: &Connection,
        id: &str,
        classification: &str,
        action: &str,
        target_entity_id: Option<&str>,
    ) {
        db.execute("INSERT INTO bible_update_runs (id, project_id, scene_id, scene_updated_at, content_hash, extractor_id, analyzed_content, status) VALUES (?1, 'project-zugestellt', 'scene-3', 'now', ?2, 'test', 'Mareks Augen waren grün.', 'completed')", params![format!("run-{id}"), format!("hash-{id}")]).unwrap();
        db.execute("INSERT INTO bible_proposals (id, run_id, project_id, scene_id, target_entity_id, proposal_action, entity_type, candidate_name, candidate_description, candidate_status, confidence, classification, evidence_excerpt, start_offset, end_offset, reason, review_status) VALUES (?1, ?2, 'project-zugestellt', 'scene-3', ?3, ?4, 'fact', 'Augenfarbe', 'Mareks Augen waren grün.', 'proposed', 0.95, ?5, 'Mareks Augen waren grün.', 0, 25, 'Testvorschlag', 'pending')", params![id, format!("run-{id}"), target_entity_id, action, classification]).unwrap();
    }

    #[test]
    fn accepted_fact_is_confirmed_and_idempotent() {
        let (path, db) = connection("review-idempotent");
        insert_review_fixture(
            &db,
            "proposal-fact",
            "observable_fact",
            "create_entity",
            None,
        );
        let accepted = review_bible_proposal_in_db(
            &db,
            ReviewBibleProposalInput {
                proposal_id: "proposal-fact".into(),
                review_status: "accepted".into(),
                decision: Some("accept".into()),
                candidate_name: None,
                candidate_description: None,
                candidate_status: None,
                classification: None,
            },
        )
        .unwrap();
        let entity_id = accepted.target_entity_id.clone().unwrap();
        let entity = load_entity(&db, &entity_id).unwrap();
        assert_eq!(entity.status, "confirmed");
        assert!(entity.author_confirmed);
        assert_eq!(entity.origin, "bible_update");
        let second = review_bible_proposal_in_db(
            &db,
            ReviewBibleProposalInput {
                proposal_id: "proposal-fact".into(),
                review_status: "accepted".into(),
                decision: Some("accept".into()),
                candidate_name: None,
                candidate_description: None,
                candidate_status: None,
                classification: None,
            },
        );
        assert!(second.is_err());
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM story_entities WHERE id=?1",
                params![entity_id],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM story_source_references WHERE proposal_id='proposal-fact'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn review_decisions_keep_fact_guess_note_and_rejection_distinct() {
        let (path, db) = connection("review-decisions");
        insert_review_fixture(
            &db,
            "proposal-guess",
            "interpretation",
            "create_entity",
            None,
        );
        insert_review_fixture(
            &db,
            "proposal-note",
            "author_note",
            "create_author_note",
            None,
        );
        insert_review_fixture(
            &db,
            "proposal-reject",
            "observable_fact",
            "create_entity",
            None,
        );
        let guess = review_bible_proposal_in_db(
            &db,
            ReviewBibleProposalInput {
                proposal_id: "proposal-guess".into(),
                review_status: "accepted".into(),
                decision: Some("save_uncertain".into()),
                candidate_name: None,
                candidate_description: None,
                candidate_status: None,
                classification: None,
            },
        )
        .unwrap();
        let note = review_bible_proposal_in_db(
            &db,
            ReviewBibleProposalInput {
                proposal_id: "proposal-note".into(),
                review_status: "accepted".into(),
                decision: Some("save_author_note".into()),
                candidate_name: None,
                candidate_description: None,
                candidate_status: None,
                classification: None,
            },
        )
        .unwrap();
        review_bible_proposal_in_db(
            &db,
            ReviewBibleProposalInput {
                proposal_id: "proposal-reject".into(),
                review_status: "rejected".into(),
                decision: Some("reject".into()),
                candidate_name: None,
                candidate_description: None,
                candidate_status: None,
                classification: None,
            },
        )
        .unwrap();
        let guess_entity = load_entity(&db, &guess.target_entity_id.unwrap()).unwrap();
        let note_entity = load_entity(&db, &note.target_entity_id.unwrap()).unwrap();
        assert_eq!(
            (guess_entity.status.as_str(), guess_entity.author_confirmed),
            ("uncertain", false)
        );
        assert_eq!(
            (
                note_entity.entity_type.as_str(),
                note_entity.status.as_str(),
                note_entity.author_confirmed
            ),
            ("author_note", "confirmed", true)
        );
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM story_entities WHERE name='Augenfarbe'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM story_source_references WHERE proposal_id='proposal-reject'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn source_references_are_deduplicated_by_structured_location_and_excerpt() {
        let (path, db) = connection("source-dedupe");
        let input = CreateSourceReferenceInput {
            project_id: "project-zugestellt".into(),
            entity_id: Some("entity-marek".into()),
            proposal_id: None,
            chapter_id: "chapter-3".into(),
            scene_id: "scene-3".into(),
            excerpt: "Marek".into(),
            start_offset: Some(0),
            end_offset: Some(5),
        };
        for _ in 0..2 {
            let tx = db.unchecked_transaction().unwrap();
            insert_source_reference_if_missing_tx(&tx, &input).unwrap();
            tx.commit().unwrap();
        }
        let second = CreateSourceReferenceInput {
            excerpt: "Marek sah".into(),
            ..input
        };
        let tx = db.unchecked_transaction().unwrap();
        insert_source_reference_if_missing_tx(&tx, &second).unwrap();
        tx.commit().unwrap();
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM story_source_references WHERE entity_id='entity-marek'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn analysis_snapshot_is_stored_and_reused_for_same_hash() {
        let (path, db) = connection("analysis-snapshot");
        db.execute("INSERT INTO bible_update_runs (id, project_id, scene_id, scene_updated_at, content_hash, extractor_id, analyzed_content, status) VALUES ('run-snapshot', 'project-zugestellt', 'scene-3', 'now', 'same', 'test', 'Alter Text', 'completed')", []).unwrap();
        let run = load_run(&db, "run-snapshot").unwrap();
        assert_eq!(run.analyzed_content, "Alter Text");
        assert_eq!(db.query_row("SELECT COUNT(*) FROM bible_update_runs WHERE scene_id='scene-3' AND content_hash='same'", [], |row| row.get::<_, i64>(0)).unwrap(), 1);
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn partially_prepared_story_bible_migration_is_resumed() {
        let path = std::env::temp_dir().join(format!(
            "storymemory-partial-{}.sqlite3",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = Connection::open(&path).unwrap();
        db.execute_batch(include_str!("../../../migrations/001_initial.sql"))
            .unwrap();
        db.execute_batch("CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP); INSERT INTO schema_migrations (version) VALUES (1),(2),(3),(4),(5); ALTER TABLE story_entities ADD COLUMN project_id TEXT REFERENCES projects(id) ON DELETE CASCADE; ALTER TABLE scene_versions ADD COLUMN version_number INTEGER NOT NULL DEFAULT 0; ALTER TABLE scene_versions ADD COLUMN snapshot_json TEXT NOT NULL DEFAULT ''; ALTER TABLE scene_versions ADD COLUMN reason TEXT NOT NULL DEFAULT 'manual'; ALTER TABLE story_entities ADD COLUMN origin TEXT NOT NULL DEFAULT 'manual'; ALTER TABLE story_entities ADD COLUMN tags_json TEXT NOT NULL DEFAULT '[]';").unwrap();
        initialize_connection(&db).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            13
        );
        assert!(has_column(&db, "bible_update_runs", "analyzed_content").unwrap());
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn lore_character_and_style_foundations_migrate_and_survive_reopen() {
        let (path, db) = connection("foundations");
        for table in [
            "lore_metadata",
            "character_profiles",
            "character_scene_states",
            "project_styles",
            "style_references",
        ] {
            assert!(db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                    params![table],
                    |row| row.get::<_, bool>(0)
                )
                .unwrap());
        }
        db.execute("INSERT INTO lore_metadata (entity_id, project_id, truth_scope, truth_statement) VALUES ('entity-marek','project-zugestellt','world_truth','Die Paketnummer kann sich verändern.')", []).unwrap();
        db.execute("INSERT INTO character_profiles (entity_id, project_id, core_want, fears) VALUES ('entity-marek','project-zugestellt','Die Wahrheit finden','Dass Lena ihn verlässt.')", []).unwrap();
        db.execute("INSERT INTO character_scene_states (id, project_id, character_entity_id, scene_id, emotional_state, goal) VALUES ('state-test','project-zugestellt','entity-marek','scene-3','angespannt','Zeit gewinnen')", []).unwrap();
        db.execute("INSERT INTO project_styles (project_id, narrative_pov, tense, preferred_patterns_json) VALUES ('project-zugestellt','personale 3. Person','Präteritum','[\"kurze Sätze\"]')", []).unwrap();
        db.execute("INSERT INTO style_references (id, project_id, scene_id, label, excerpt) VALUES ('style-test','project-zugestellt','scene-3','Spannung','Marek sah auf den Aufkleber.')", []).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT truth_statement FROM lore_metadata WHERE entity_id='entity-marek'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "Die Paketnummer kann sich verändern."
        );
        drop(db);
        let reopened = database_path_for_test(&path).unwrap();
        assert_eq!(
            reopened
                .query_row(
                    "SELECT COUNT(*) FROM character_scene_states WHERE id='state-test'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            reopened
                .query_row(
                    "SELECT COUNT(*) FROM style_references WHERE id='style-test'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        drop(reopened);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn character_memory_schema_and_payload_are_source_ready() {
        let (path, db) = connection("character-memory");
        for table in [
            "character_voice_patterns",
            "character_experiences",
            "character_dialogue_memories",
            "dialogue_memory_participants",
            "relationship_memories",
            "character_knowledge_states",
            "character_knowledge_history",
            "character_memory_evidence",
            "character_memory_update_runs",
            "character_memory_proposals",
        ] {
            assert!(db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                    params![table],
                    |row| row.get::<_, bool>(0)
                )
                .unwrap());
        }
        let tx = db.unchecked_transaction().unwrap();
        apply_memory_payload(&tx, "project-zugestellt", "scene-3", "experience", serde_json::json!({"subjectCharacterId":"entity-marek","title":"Das Paket","objectiveSummary":"Marek sieht die abweichende Nummer.","subjectiveInterpretation":"Er zweifelt zunächst an sich.","emotionalImpact":"Verunsicherung","lastingEffect":"Mehr Vorsicht"}), "accepted", None).unwrap();
        tx.commit().unwrap();
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM character_experiences WHERE project_id='project-zugestellt'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn relationship_memory_rejects_self_relation_and_knowledge_history_is_additive() {
        let (path, db) = connection("character-memory-validation");
        assert!(normalize_pair("entity-marek", "entity-marek").is_err());
        db.execute("INSERT INTO character_knowledge_states (id,project_id,character_id,fact_entity_id,knowledge_state,certainty) VALUES ('knowledge-test','project-zugestellt','entity-marek','entity-package','suspects',0.4)", []).unwrap();
        db.execute("INSERT INTO character_knowledge_history (id,knowledge_state_id,project_id,character_id,fact_entity_id,knowledge_state,certainty) VALUES ('history-test','knowledge-test','project-zugestellt','entity-marek','entity-package','unknown',0.2)", []).unwrap();
        assert_eq!(db.query_row("SELECT COUNT(*) FROM character_knowledge_history WHERE knowledge_state_id='knowledge-test'", [], |row| row.get::<_, i64>(0)).unwrap(), 1);
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn knowledge_intervals_keep_old_state_until_transition_scene() {
        let (path, db) = connection("knowledge-intervals");
        db.execute("INSERT INTO character_knowledge_states (id,project_id,character_id,fact_entity_id,knowledge_state,acquired_scene_id,changed_scene_id,effective_from_scene_id,certainty) VALUES ('knowledge-interval','project-zugestellt','entity-marek','entity-package','knows','scene-1','scene-3','scene-3',1.0)", []).unwrap();
        db.execute("INSERT INTO character_knowledge_history (id,knowledge_state_id,project_id,character_id,fact_entity_id,knowledge_state,certainty,scene_id,effective_from_scene_id,effective_until_scene_id) VALUES ('history-interval','knowledge-interval','project-zugestellt','entity-marek','entity-package','suspects',0.4,'scene-3','scene-1','scene-3')", []).unwrap();
        let interval: (String, String) = db.query_row("SELECT effective_from_scene_id,effective_until_scene_id FROM character_knowledge_history WHERE id='history-interval'", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!(interval, ("scene-1".into(), "scene-3".into()));
        assert_eq!(db.query_row("SELECT effective_from_scene_id FROM character_knowledge_states WHERE id='knowledge-interval'", [], |row| row.get::<_, String>(0)).unwrap(), "scene-3");
        drop(db);
        let _ = fs::remove_file(path);
    }
}
