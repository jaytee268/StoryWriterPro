use crate::providers::codex::{
    self, AiProviderSettings, CodexCliCapabilities, CodexError, CodexRuntimeState,
    RunCodexTaskInput,
};
use crate::{
    database::DbState,
    models::{
        validate_character_memory_status, validate_character_significance,
        validate_character_voice_pattern_type, validate_continuity_state_kind,
        validate_continuity_state_status, validate_dialogue_kind, validate_entity_status,
        validate_evidence_role, validate_knowledge_state, validate_lore_category,
        validate_lore_entity_type, validate_lore_importance, validate_lore_reveal_state,
        validate_lore_scope, validate_memory_kind, validate_memory_reliability,
        validate_participant_role, validate_project_rule_scope, validate_project_rule_status,
        validate_proposal_action, validate_proposal_classification, validate_relation_type,
        validate_relationship_memory_type, validate_review_status, validate_rule_proposal_status,
        validate_scene_status, validate_scene_version_reason, validate_style_reference_category,
        validate_truthfulness, AcceptChapterGenerationJobInput, AddCharacterMemoryEvidenceInput,
        ApplyContinuityFindingDecisionInput, BibleProposal, BibleProposalInput, BibleUpdateRun,
        Book, Chapter, ChapterGenerationDraftLedgerEntry, ChapterGenerationJob,
        ChapterGenerationPlan, ChapterGenerationReview, ChapterGenerationSection, ChapterPlanBeat,
        CharacterDialogueMemory, CharacterExperience, CharacterKnowledgeState,
        CharacterMemoryEvidence, CharacterMemoryProposal, CharacterMemoryProposalDraft,
        CharacterMemoryUpdateRun, CharacterProfile, CharacterSceneState, CharacterVoicePattern,
        ContinuityCanonChangeAudit, ContinuityFindingDecision, ContinuityReviewFinding,
        ContinuityReviewRun, ContinuityReviewSettings, ContinuityStateLedgerEntry,
        CreateBibleUpdateRunInput, CreateChapterGenerationJobInput, CreateChapterInput,
        CreateCharacterMemoryUpdateRunInput, CreateLoreCrafterRunInput, CreateLoreEntryInput,
        CreateManuscriptAnalysisJobInput, CreateManuscriptStructureRunInput, CreateProjectInput,
        CreateProjectSourceDocumentInput, CreateProjectSourceReferenceInput,
        CreateProjectStyleAnalysisRunInput, CreateSceneInput, CreateSceneVersionInput,
        CreateSourceReferenceInput, CreateStoryEntityInput, CreateStoryEntityRelationInput,
        CreateStyleReferenceInput, DatabaseInfo, DialogueMemoryParticipant, EditorPreferences,
        LoreCrafterClarification, LoreCrafterRun, LoreCrafterSourceReference, LoreEntry,
        LoreMetadata, LoreSheetDraft, LoreSheetItem, ManuscriptAnalysisArtifact,
        ManuscriptAnalysisCompletionReport, ManuscriptAnalysisDraftLedgerEntry,
        ManuscriptAnalysisJob, ManuscriptAnalysisPageMarker, ManuscriptAnalysisPhaseResult,
        ManuscriptAnalysisReviewAudit, ManuscriptAnalysisUnit, ManuscriptImportInput,
        ManuscriptImportResult, ManuscriptPosition, ManuscriptStructureProposal,
        ManuscriptStructureRun, MaterializeProvisionalEntityInput, MindmapLayout, NarrativeSummary,
        PersistentTimelineEvent, PlotThreadLifecycle, PlotThreadLifecycleProposal, Project,
        ProjectOnboardingState, ProjectRule, ProjectRuleProposal, ProjectSourceDocument,
        ProjectStyle, ProjectStyleAnalysisRun, ProjectStyleObservation, ProviderStatus,
        ProvisionalEntity, ProvisionalEntityMention, ProvisionalEvent, ProvisionalMergeProposal,
        ProvisionalRelation, ReconcileContinuityTextCorrectionInput, RelationshipMemory,
        RestoreSceneVersionInput, ReviewBibleProposalInput, ReviewCharacterMemoryProposalInput,
        SaveChapterGenerationDraftLedgerInput, SaveChapterGenerationPlanInput,
        SaveChapterGenerationReviewInput, SaveChapterGenerationSectionInput,
        SaveCharacterDialogueMemoryInput, SaveCharacterExperienceInput,
        SaveCharacterKnowledgeStateInput, SaveCharacterProfileInput, SaveCharacterSceneStateInput,
        SaveCharacterVoicePatternInput, SaveContinuityFindingInput, SaveContinuityReviewInput,
        SaveContinuityReviewRunStatusInput, SaveContinuityStateInput,
        SaveLoreCrafterClarificationInput, SaveLoreCrafterSourceInput, SaveLoreMetadataInput,
        SaveLoreSheetDraftInput, SaveLoreSheetItemInput, SaveManuscriptAnalysisArtifactInput,
        SaveManuscriptAnalysisCompletionReportInput, SaveManuscriptAnalysisDraftLedgerInput,
        SaveManuscriptAnalysisPhaseResultInput, SaveManuscriptAnalysisReviewAuditInput,
        SaveManuscriptStructureProposalInput, SaveMindmapLayoutInput, SaveNarrativeSummaryInput,
        SavePersistentTimelineEventInput, SavePlotThreadLifecycleInput,
        SavePlotThreadLifecycleProposalInput, SaveProjectOnboardingStateInput,
        SaveProjectRuleInput, SaveProjectRuleProposalInput, SaveProjectStyleInput,
        SaveProjectStyleObservationInput, SaveProvisionalEntityInput, SaveProvisionalEventInput,
        SaveProvisionalMentionInput, SaveProvisionalMergeProposalInput,
        SaveProvisionalRelationInput, SaveRelationshipMemoryInput, SaveStoryDirectionInput,
        SaveStoryGraphEdgeInput, SaveWritingPreferencesInput, Scene, SceneInput, SceneVersion,
        StoryDirection, StoryEntity, StoryEntityInput, StoryEntityRelation, StoryGraphEdge,
        StorySourceReference, StyleReference, UpdateChapterInput, UpdateLoreCrafterRunInput,
        UpdateManuscriptAnalysisJobInput, UpdateManuscriptAnalysisUnitInput,
        UpdateStoryEntityInput, UpdateStyleReferenceInput, WorkspaceSnapshot, WritingPreferences,
    },
};
use chrono::Utc;
use rusqlite::{params, types::Type, Connection, OptionalExtension, Result as SqlResult};
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
        "SELECT id, title, author, description, created_at, updated_at,
                COALESCE((SELECT status FROM project_workflow_state WHERE project_id=projects.id), 'active'),
                (SELECT last_opened_at FROM project_workflow_state WHERE project_id=projects.id)
         FROM projects WHERE id=?1",
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
                status: row.get(6)?,
                last_opened_at: row.get(7)?,
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
        is_implicit: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn load_scene(db: &Connection, scene_id: &str) -> Result<Scene, String> {
    db.query_row("SELECT id, chapter_id, title, order_index, content, pov, location, story_time, status, goal, notes, is_implicit, created_at, updated_at FROM scenes WHERE id=?1", params![scene_id], scene_from_row).map_err(|error| sql_error("Szene konnte nicht geladen werden", error))
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
        is_implicit: scene.is_implicit,
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
    let mut statement = db.prepare("SELECT id, chapter_id, title, order_index, content, pov, location, story_time, status, goal, notes, is_implicit, created_at, updated_at FROM scenes WHERE chapter_id=?1 ORDER BY order_index, created_at").map_err(|error| sql_error("Szenen konnten nicht geladen werden", error))?;
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
pub fn create_lore_crafter_run(
    state: State<'_, DbState>,
    input: CreateLoreCrafterRunInput,
) -> Result<LoreCrafterRun, String> {
    required(&input.original_text, "Die Lore-Notizen")?;
    let db = lock_db(&state)?;
    project_from_db(&db, &input.project_id)?;
    if canonical_content_hash(&input.original_text) != input.content_hash {
        return Err("Der Content-Hash der Lore-Notizen ist veraltet.".into());
    }
    if let Some(existing) = db
        .query_row(
            "SELECT id, project_id, original_text, content_hash, provider_id, prompt_version, status, understanding_summary, analysis_json, confirmation_text, created_at, updated_at, completed_at, error_code, error_message FROM lore_crafter_runs WHERE project_id=?1 AND content_hash=?2 AND provider_id=?3 AND status NOT IN ('failed','cancelled') ORDER BY updated_at DESC LIMIT 1",
            params![input.project_id, input.content_hash, input.provider_id],
            lore_crafter_run_from_row,
        )
        .optional()
        .map_err(|error| sql_error("Vorhandene Lore-Crafter-Läufe konnten nicht geprüft werden", error))?
    {
        return Ok(existing);
    }
    let id = new_id();
    let timestamp = now();
    db.execute(
        "INSERT INTO lore_crafter_runs (id, project_id, original_text, content_hash, provider_id, prompt_version, status, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,'pending',?7,?7)",
        params![id, input.project_id, input.original_text, input.content_hash, input.provider_id, input.prompt_version, timestamp],
    ).map_err(|error| sql_error("Lore-Crafter-Lauf konnte nicht angelegt werden", error))?;
    load_lore_crafter_run(&db, &id)
}

fn lore_crafter_run_from_row(row: &rusqlite::Row<'_>) -> SqlResult<LoreCrafterRun> {
    let analysis: Option<String> = row.get(8)?;
    Ok(LoreCrafterRun {
        id: row.get(0)?,
        project_id: row.get(1)?,
        original_text: row.get(2)?,
        content_hash: row.get(3)?,
        provider_id: row.get(4)?,
        prompt_version: row.get(5)?,
        status: row.get(6)?,
        understanding_summary: row.get(7)?,
        analysis: analysis.and_then(|value| serde_json::from_str(&value).ok()),
        confirmation_text: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        completed_at: row.get(12)?,
        error_code: row.get(13)?,
        error_message: row.get(14)?,
    })
}

fn load_lore_crafter_run(db: &Connection, id: &str) -> Result<LoreCrafterRun, String> {
    db.query_row("SELECT id, project_id, original_text, content_hash, provider_id, prompt_version, status, understanding_summary, analysis_json, confirmation_text, created_at, updated_at, completed_at, error_code, error_message FROM lore_crafter_runs WHERE id=?1", params![id], lore_crafter_run_from_row).map_err(|error| sql_error("Lore-Crafter-Lauf konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn list_lore_crafter_runs(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<LoreCrafterRun>, String> {
    let db = lock_db(&state)?;
    project_from_db(&db, &project_id)?;
    let mut statement = db.prepare("SELECT id, project_id, original_text, content_hash, provider_id, prompt_version, status, understanding_summary, analysis_json, confirmation_text, created_at, updated_at, completed_at, error_code, error_message FROM lore_crafter_runs WHERE project_id=?1 ORDER BY updated_at DESC").map_err(|error| sql_error("Lore-Crafter-Läufe konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![project_id], lore_crafter_run_from_row)
        .map_err(|error| sql_error("Lore-Crafter-Läufe konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Lore-Crafter-Läufe konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn get_lore_crafter_run(
    state: State<'_, DbState>,
    id: String,
) -> Result<LoreCrafterRun, String> {
    let db = lock_db(&state)?;
    load_lore_crafter_run(&db, &id)
}

#[tauri::command]
pub fn update_lore_crafter_run(
    state: State<'_, DbState>,
    input: UpdateLoreCrafterRunInput,
) -> Result<LoreCrafterRun, String> {
    if !matches!(
        input.status.as_str(),
        "pending" | "running" | "awaiting_review" | "completed" | "failed" | "cancelled"
    ) {
        return Err("Ungültiger Lore-Crafter-Status.".into());
    }
    let db = lock_db(&state)?;
    let current = load_lore_crafter_run(&db, &input.id)?;
    let analysis = input
        .analysis
        .map(|value| serde_json::to_string(&value))
        .transpose()
        .map_err(|error| {
            sql_error(
                "Lore-Crafter-Analyse konnte nicht gespeichert werden",
                error,
            )
        })?;
    db.execute("UPDATE lore_crafter_runs SET status=?2, understanding_summary=COALESCE(?3, understanding_summary), analysis_json=COALESCE(?4, analysis_json), confirmation_text=COALESCE(?5, confirmation_text), updated_at=?6, completed_at=?7, error_code=?8, error_message=?9 WHERE id=?1", params![input.id, input.status, input.understanding_summary, analysis, input.confirmation_text, now(), input.completed_at, input.error_code, input.error_message]).map_err(|error| sql_error("Lore-Crafter-Lauf konnte nicht aktualisiert werden", error))?;
    let _ = current;
    load_lore_crafter_run(&db, &input.id)
}

fn lore_clarification_from_row(row: &rusqlite::Row<'_>) -> SqlResult<LoreCrafterClarification> {
    Ok(LoreCrafterClarification {
        id: row.get(0)?,
        run_id: row.get(1)?,
        project_id: row.get(2)?,
        question: row.get(3)?,
        answer: row.get(4)?,
        status: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

#[tauri::command]
pub fn list_lore_crafter_clarifications(
    state: State<'_, DbState>,
    run_id: String,
) -> Result<Vec<LoreCrafterClarification>, String> {
    let db = lock_db(&state)?;
    let run = load_lore_crafter_run(&db, &run_id)?;
    let mut statement = db.prepare("SELECT id, run_id, project_id, question, answer, status, created_at, updated_at FROM lore_crafter_clarifications WHERE run_id=?1 AND project_id=?2 ORDER BY created_at").map_err(|error| sql_error("Lore-Crafter-Rückfragen konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![run_id, run.project_id], lore_clarification_from_row)
        .map_err(|error| {
            sql_error(
                "Lore-Crafter-Rückfragen konnten nicht geladen werden",
                error,
            )
        })?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| {
            sql_error(
                "Lore-Crafter-Rückfragen konnten nicht geladen werden",
                error,
            )
        });
    result
}

#[tauri::command]
pub fn save_lore_crafter_clarifications(
    state: State<'_, DbState>,
    run_id: String,
    inputs: Vec<SaveLoreCrafterClarificationInput>,
) -> Result<Vec<LoreCrafterClarification>, String> {
    let db = lock_db(&state)?;
    let run = load_lore_crafter_run(&db, &run_id)?;
    if inputs.iter().any(|input| {
        input.run_id != run_id
            || input.project_id != run.project_id
            || !matches!(
                input.status.as_deref().unwrap_or("open"),
                "open" | "answered" | "skipped"
            )
    }) {
        return Err("Eine Lore-Crafter-Rückfrage ist ungültig oder projektfremd.".into());
    }
    let transaction = db.unchecked_transaction().map_err(|error| {
        sql_error(
            "Rückfragen-Transaktion konnte nicht gestartet werden",
            error,
        )
    })?;
    let timestamp = now();
    for input in inputs {
        let id = input.id.clone().unwrap_or_else(new_id);
        let id_belongs_to_run: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM lore_crafter_clarifications WHERE id=?1 AND run_id=?2 AND project_id=?3)",
                params![id, run_id, run.project_id],
                |row| row.get(0),
            )
            .map_err(|error| sql_error("Lore-Crafter-Rückfrage konnte nicht geprüft werden", error))?;
        let id_exists_elsewhere: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM lore_crafter_clarifications WHERE id=?1)",
                params![id],
                |row| row.get(0),
            )
            .map_err(|error| {
                sql_error("Lore-Crafter-Rückfrage konnte nicht geprüft werden", error)
            })?;
        if input.id.is_some() && (!id_belongs_to_run || !id_exists_elsewhere) {
            return Err("Die Lore-Crafter-Rückfrage gehört nicht zu diesem Projektlauf.".into());
        }
        transaction.execute("INSERT INTO lore_crafter_clarifications (id, run_id, project_id, question, answer, status, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?7) ON CONFLICT(id) DO UPDATE SET question=excluded.question, answer=excluded.answer, status=excluded.status, updated_at=excluded.updated_at", params![id, input.run_id, input.project_id, input.question, input.answer, input.status.unwrap_or_else(|| "open".into()), timestamp]).map_err(|error| sql_error("Lore-Crafter-Rückfrage konnte nicht gespeichert werden", error))?;
    }
    transaction.commit().map_err(|error| {
        sql_error(
            "Rückfragen-Transaktion konnte nicht abgeschlossen werden",
            error,
        )
    })?;
    let mut statement = db.prepare("SELECT id, run_id, project_id, question, answer, status, created_at, updated_at FROM lore_crafter_clarifications WHERE run_id=?1 AND project_id=?2 ORDER BY created_at").map_err(|error| sql_error("Gespeicherte Lore-Crafter-Rückfragen konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![run_id, run.project_id], lore_clarification_from_row)
        .map_err(|error| {
            sql_error(
                "Gespeicherte Lore-Crafter-Rückfragen konnten nicht geladen werden",
                error,
            )
        })?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| {
            sql_error(
                "Gespeicherte Lore-Crafter-Rückfragen konnten nicht geladen werden",
                error,
            )
        });
    result
}

fn lore_source_from_row(row: &rusqlite::Row<'_>) -> SqlResult<LoreCrafterSourceReference> {
    Ok(LoreCrafterSourceReference {
        id: row.get(0)?,
        run_id: row.get(1)?,
        project_id: row.get(2)?,
        excerpt: row.get(3)?,
        start_offset: row.get(4)?,
        end_offset: row.get(5)?,
        source_document_id: row.get(6)?,
        source_reference_id: row.get(7)?,
        created_at: row.get(8)?,
    })
}

#[tauri::command]
pub fn save_lore_crafter_source(
    state: State<'_, DbState>,
    input: SaveLoreCrafterSourceInput,
) -> Result<LoreCrafterSourceReference, String> {
    let db = lock_db(&state)?;
    let run = load_lore_crafter_run(&db, &input.run_id)?;
    if run.project_id != input.project_id {
        return Err("Die Lore-Crafter-Quelle gehört nicht zum Projekt.".into());
    }
    let chars: Vec<char> = run.original_text.chars().collect();
    if input.start_offset < 0
        || input.end_offset <= input.start_offset
        || input.end_offset as usize > chars.len()
        || chars[input.start_offset as usize..input.end_offset as usize]
            .iter()
            .collect::<String>()
            != input.excerpt
    {
        return Err(
            "Die Lore-Quelle passt nicht zu den Unicode-Positionen des Originaltexts.".into(),
        );
    }
    if let Some(existing) = db.query_row("SELECT id, run_id, project_id, excerpt, start_offset, end_offset, source_document_id, source_reference_id, created_at FROM lore_crafter_sources WHERE run_id=?1 AND excerpt=?2 AND start_offset=?3 AND end_offset=?4 LIMIT 1", params![input.run_id, input.excerpt, input.start_offset, input.end_offset], lore_source_from_row).optional().map_err(|error| sql_error("Lore-Crafter-Quelle konnte nicht geprüft werden", error))? { return Ok(existing); }
    let run_hash: String = db
        .query_row(
            "SELECT content_hash FROM lore_crafter_runs WHERE id=?1",
            params![input.run_id],
            |row| row.get(0),
        )
        .map_err(|e| sql_error("Lore-Crafter-Hash konnte nicht geladen werden", e))?;
    let document_id: String = db.query_row("SELECT id FROM project_source_documents WHERE project_id=?1 AND content_hash=?2 AND source_kind='lore_crafter' ORDER BY updated_at DESC LIMIT 1", params![input.project_id, run_hash], |row| row.get(0)).optional().map_err(|e| sql_error("Lore-Crafter-Quelldokument konnte nicht geprüft werden", e))?.unwrap_or_else(new_id);
    let stamp = now();
    db.execute("INSERT INTO project_source_documents(id,project_id,source_kind,title,content,content_hash,origin_id,created_at,updated_at) VALUES(?1,?2,'lore_crafter','Lore-Crafter-Originaltext',(SELECT original_text FROM lore_crafter_runs WHERE id=?3),?4,?3,?5,?5) ON CONFLICT(id) DO NOTHING", params![document_id,input.project_id,input.run_id,run_hash,stamp]).map_err(|e| sql_error("Lore-Crafter-Quelldokument konnte nicht gespeichert werden", e))?;
    let reference_id = new_id();
    db.execute("INSERT INTO project_source_references(id,project_id,source_document_id,excerpt,start_offset,end_offset,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![reference_id,input.project_id,document_id,input.excerpt,input.start_offset,input.end_offset,stamp]).map_err(|e| sql_error("Lore-Crafter-Quellenreferenz konnte nicht gespeichert werden", e))?;
    let id = new_id();
    db.execute("INSERT INTO lore_crafter_sources (id, run_id, project_id, excerpt, start_offset, end_offset, source_document_id, source_reference_id, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![id, input.run_id, input.project_id, input.excerpt, input.start_offset, input.end_offset, document_id, reference_id, stamp]).map_err(|error| sql_error("Lore-Crafter-Quelle konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT id, run_id, project_id, excerpt, start_offset, end_offset, source_document_id, source_reference_id, created_at FROM lore_crafter_sources WHERE id=?1", params![id], lore_source_from_row).map_err(|error| sql_error("Gespeicherte Lore-Crafter-Quelle konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn list_lore_crafter_sources(
    state: State<'_, DbState>,
    run_id: String,
) -> Result<Vec<LoreCrafterSourceReference>, String> {
    let db = lock_db(&state)?;
    let run = load_lore_crafter_run(&db, &run_id)?;
    let mut statement = db.prepare("SELECT id, run_id, project_id, excerpt, start_offset, end_offset, source_document_id, source_reference_id, created_at FROM lore_crafter_sources WHERE run_id=?1 AND project_id=?2 ORDER BY start_offset").map_err(|error| sql_error("Lore-Crafter-Quellen konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![run_id, run.project_id], lore_source_from_row)
        .map_err(|error| sql_error("Lore-Crafter-Quellen konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Lore-Crafter-Quellen konnten nicht geladen werden", error));
    result
}

fn json_vec(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".into())
}
fn vec_from_json(value: &str) -> Vec<String> {
    serde_json::from_str(value).unwrap_or_default()
}
fn lore_sheet_from_row(row: &rusqlite::Row<'_>) -> SqlResult<LoreSheetDraft> {
    Ok(LoreSheetDraft {
        id: row.get(0)?,
        run_id: row.get(1)?,
        project_id: row.get(2)?,
        content_hash: row.get(3)?,
        title: row.get(4)?,
        premise: row.get(5)?,
        categories: vec_from_json(&row.get::<_, String>(6)?),
        world_rules: vec_from_json(&row.get::<_, String>(7)?),
        prerequisites: vec_from_json(&row.get::<_, String>(8)?),
        effects: vec_from_json(&row.get::<_, String>(9)?),
        limitations: vec_from_json(&row.get::<_, String>(10)?),
        costs: vec_from_json(&row.get::<_, String>(11)?),
        exceptions: vec_from_json(&row.get::<_, String>(12)?),
        terminology: vec_from_json(&row.get::<_, String>(13)?),
        organizations: vec_from_json(&row.get::<_, String>(14)?),
        locations: vec_from_json(&row.get::<_, String>(15)?),
        historical_events: vec_from_json(&row.get::<_, String>(16)?),
        known_aspects: vec_from_json(&row.get::<_, String>(17)?),
        unknown_aspects: vec_from_json(&row.get::<_, String>(18)?),
        rule_connections: vec_from_json(&row.get::<_, String>(19)?),
        open_questions: vec_from_json(&row.get::<_, String>(20)?),
        status: row.get(21)?,
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
    })
}

#[tauri::command]
pub fn save_lore_sheet_draft(
    state: State<'_, DbState>,
    input: SaveLoreSheetDraftInput,
) -> Result<LoreSheetDraft, String> {
    let db = lock_db(&state)?;
    let run = load_lore_crafter_run(&db, &input.run_id)?;
    if run.project_id != input.project_id || run.content_hash != input.content_hash {
        return Err(
            "Der Lore-Sheet-Entwurf gehört nicht zum Projekt oder basiert auf veralteten Notizen."
                .into(),
        );
    }
    if !matches!(input.status.as_str(), "proposed" | "reviewed" | "rejected") {
        return Err("Ungültiger Lore-Sheet-Status.".into());
    }
    let id = input.id.clone().unwrap_or_else(new_id);
    if let Some(existing_id) = &input.id {
        let existing = db
            .query_row(
                "SELECT run_id, project_id FROM lore_sheet_drafts WHERE id=?1",
                params![existing_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| sql_error("Lore-Sheet-Entwurf konnte nicht geprüft werden", error))?;
        if existing.is_some_and(|(run_id, project_id)| {
            run_id != input.run_id || project_id != input.project_id
        }) {
            return Err("Der Lore-Sheet-Entwurf gehört nicht zu diesem Projektlauf.".into());
        }
    }
    let stamp = now();
    let json = [
        &input.categories,
        &input.world_rules,
        &input.prerequisites,
        &input.effects,
        &input.limitations,
        &input.costs,
        &input.exceptions,
        &input.terminology,
        &input.organizations,
        &input.locations,
        &input.historical_events,
        &input.known_aspects,
        &input.unknown_aspects,
        &input.rule_connections,
        &input.open_questions,
    ]
    .map(|items| json_vec(items));
    db.execute("INSERT INTO lore_sheet_drafts (id, run_id, project_id, content_hash, sheet_json, status, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?7) ON CONFLICT(id) DO UPDATE SET content_hash=excluded.content_hash, sheet_json=excluded.sheet_json, status=excluded.status, updated_at=excluded.updated_at", params![id, input.run_id, input.project_id, input.content_hash, serde_json::json!({"title":input.title,"premise":input.premise,"categories":json[0],"worldRules":json[1],"prerequisites":json[2],"effects":json[3],"limitations":json[4],"costs":json[5],"exceptions":json[6],"terminology":json[7],"organizations":json[8],"locations":json[9],"historicalEvents":json[10],"knownAspects":json[11],"unknownAspects":json[12],"ruleConnections":json[13],"openQuestions":json[14]}).to_string(), input.status, stamp]).map_err(|error| sql_error("Lore-Sheet-Entwurf konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT id, run_id, project_id, content_hash, json_extract(sheet_json,'$.title'), json_extract(sheet_json,'$.premise'), json_extract(sheet_json,'$.categories'), json_extract(sheet_json,'$.worldRules'), json_extract(sheet_json,'$.prerequisites'), json_extract(sheet_json,'$.effects'), json_extract(sheet_json,'$.limitations'), json_extract(sheet_json,'$.costs'), json_extract(sheet_json,'$.exceptions'), json_extract(sheet_json,'$.terminology'), json_extract(sheet_json,'$.organizations'), json_extract(sheet_json,'$.locations'), json_extract(sheet_json,'$.historicalEvents'), json_extract(sheet_json,'$.knownAspects'), json_extract(sheet_json,'$.unknownAspects'), json_extract(sheet_json,'$.ruleConnections'), json_extract(sheet_json,'$.openQuestions'), status, created_at, updated_at FROM lore_sheet_drafts WHERE id=?1", params![id], lore_sheet_from_row).map_err(|error| sql_error("Gespeicherter Lore-Sheet-Entwurf konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn get_lore_sheet_draft(
    state: State<'_, DbState>,
    run_id: String,
) -> Result<Option<LoreSheetDraft>, String> {
    let db = lock_db(&state)?;
    let run = load_lore_crafter_run(&db, &run_id)?;
    db.query_row("SELECT id, run_id, project_id, content_hash, json_extract(sheet_json,'$.title'), json_extract(sheet_json,'$.premise'), json_extract(sheet_json,'$.categories'), json_extract(sheet_json,'$.worldRules'), json_extract(sheet_json,'$.prerequisites'), json_extract(sheet_json,'$.effects'), json_extract(sheet_json,'$.limitations'), json_extract(sheet_json,'$.costs'), json_extract(sheet_json,'$.exceptions'), json_extract(sheet_json,'$.terminology'), json_extract(sheet_json,'$.organizations'), json_extract(sheet_json,'$.locations'), json_extract(sheet_json,'$.historicalEvents'), json_extract(sheet_json,'$.knownAspects'), json_extract(sheet_json,'$.unknownAspects'), json_extract(sheet_json,'$.ruleConnections'), json_extract(sheet_json,'$.openQuestions'), status, created_at, updated_at FROM lore_sheet_drafts WHERE run_id=?1 AND project_id=?2 ORDER BY updated_at DESC LIMIT 1", params![run_id, run.project_id], lore_sheet_from_row).optional().map_err(|error| sql_error("Lore-Sheet-Entwurf konnte nicht geladen werden", error))
}

fn lore_item_from_row(row: &rusqlite::Row<'_>) -> SqlResult<LoreSheetItem> {
    Ok(LoreSheetItem {
        id: row.get(0)?,
        draft_id: row.get(1)?,
        run_id: row.get(2)?,
        project_id: row.get(3)?,
        item_type: row.get(4)?,
        title: row.get(5)?,
        content: row.get(6)?,
        confidence: row.get(7)?,
        source_reference_id: row.get(8)?,
        target_entity_id: row.get(9)?,
        target_rule_id: row.get(10)?,
        structured: row
            .get::<_, Option<String>>(11)?
            .and_then(|value| serde_json::from_str(&value).ok()),
        status: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

#[tauri::command]
pub fn save_lore_sheet_items(
    state: State<'_, DbState>,
    draft_id: String,
    inputs: Vec<SaveLoreSheetItemInput>,
) -> Result<Vec<LoreSheetItem>, String> {
    let db = lock_db(&state)?;
    let draft = db
        .query_row(
            "SELECT run_id, project_id FROM lore_sheet_drafts WHERE id=?1",
            params![draft_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|error| sql_error("Lore-Sheet-Entwurf konnte nicht geladen werden", error))?;
    if inputs.iter().any(|input| {
        input.draft_id != draft_id
            || input.run_id != draft.0
            || input.project_id != draft.1
            || !matches!(
                input.status.as_deref().unwrap_or("proposed"),
                "proposed" | "accepted" | "rejected" | "uncertain" | "merged"
            )
            || !(0.0..=1.0).contains(&input.confidence)
    }) {
        return Err("Ein Lore-Sheet-Eintrag ist ungültig oder projektfremd.".into());
    }
    for input in &inputs {
        if let Some(source_id) = &input.source_reference_id {
            let valid: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM project_source_references WHERE id=?1 AND project_id=?2) OR EXISTS(SELECT 1 FROM lore_crafter_sources WHERE id=?1 AND run_id=?3 AND project_id=?2)", params![source_id, draft.1, draft.0], |row| row.get(0)).map_err(|error| sql_error("Lore-Sheet-Quelle konnte nicht geprüft werden", error))?;
            if !valid {
                return Err("Eine Lore-Sheet-Quelle gehört nicht zum Projekt.".into());
            }
        }
        if let Some(existing_id) = &input.id {
            let existing = db
                .query_row(
                    "SELECT draft_id, run_id, project_id FROM lore_sheet_items WHERE id=?1",
                    params![existing_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(|error| {
                    sql_error("Lore-Sheet-Eintrag konnte nicht geprüft werden", error)
                })?;
            if existing.is_some_and(|(existing_draft, existing_run, existing_project)| {
                existing_draft != draft_id || existing_run != draft.0 || existing_project != draft.1
            }) {
                return Err("Der Lore-Sheet-Eintrag gehört nicht zu diesem Projektlauf.".into());
            }
        }
    }
    let transaction = db.unchecked_transaction().map_err(|error| {
        sql_error(
            "Lore-Sheet-Einträge konnten nicht gespeichert werden",
            error,
        )
    })?;
    let stamp = now();
    for input in inputs {
        let id = input.id.unwrap_or_else(new_id);
        transaction.execute("INSERT INTO lore_sheet_items (id,draft_id,run_id,project_id,item_type,title,content,confidence,source_reference_id,target_entity_id,target_rule_id,structured_json,status,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14) ON CONFLICT(id) DO UPDATE SET title=excluded.title, content=excluded.content, confidence=excluded.confidence, source_reference_id=excluded.source_reference_id, target_entity_id=excluded.target_entity_id, target_rule_id=excluded.target_rule_id, structured_json=excluded.structured_json, status=excluded.status, updated_at=excluded.updated_at", params![id, input.draft_id, input.run_id, input.project_id, input.item_type, input.title, input.content, input.confidence, input.source_reference_id, input.target_entity_id, input.target_rule_id, input.structured.map(|value| value.to_string()), input.status.unwrap_or_else(|| "proposed".into()), stamp]).map_err(|error| sql_error("Lore-Sheet-Eintrag konnte nicht gespeichert werden", error))?;
    }
    transaction.commit().map_err(|error| {
        sql_error(
            "Lore-Sheet-Einträge konnten nicht abgeschlossen werden",
            error,
        )
    })?;
    list_lore_sheet_items_for_db(&db, &draft_id, &draft.1)
}

fn list_lore_sheet_items_for_db(
    db: &Connection,
    draft_id: &str,
    project_id: &str,
) -> Result<Vec<LoreSheetItem>, String> {
    let mut statement = db.prepare("SELECT id,draft_id,run_id,project_id,item_type,title,content,confidence,source_reference_id,target_entity_id,target_rule_id,structured_json,status,created_at,updated_at FROM lore_sheet_items WHERE draft_id=?1 AND project_id=?2 ORDER BY created_at").map_err(|error| sql_error("Lore-Sheet-Einträge konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![draft_id, project_id], lore_item_from_row)
        .map_err(|error| sql_error("Lore-Sheet-Einträge konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Lore-Sheet-Einträge konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn list_lore_sheet_items(
    state: State<'_, DbState>,
    draft_id: String,
) -> Result<Vec<LoreSheetItem>, String> {
    let db = lock_db(&state)?;
    let project_id: String = db
        .query_row(
            "SELECT project_id FROM lore_sheet_drafts WHERE id=?1",
            params![draft_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Lore-Sheet-Entwurf konnte nicht geladen werden", error))?;
    list_lore_sheet_items_for_db(&db, &draft_id, &project_id)
}

#[tauri::command]
pub fn review_lore_sheet_item(
    state: State<'_, DbState>,
    project_id: String,
    id: String,
    status: String,
) -> Result<LoreSheetItem, String> {
    if !matches!(
        status.as_str(),
        "proposed" | "accepted" | "rejected" | "uncertain" | "merged"
    ) {
        return Err("Ungültiger Lore-Sheet-Reviewstatus.".into());
    }
    let db = lock_db(&state)?;
    let changed = db
        .execute(
            "UPDATE lore_sheet_items SET status=?3, updated_at=?4 WHERE id=?1 AND project_id=?2",
            params![id, project_id, status, now()],
        )
        .map_err(|error| sql_error("Lore-Sheet-Eintrag konnte nicht geprüft werden", error))?;
    if changed == 0 {
        return Err(
            "Der Lore-Sheet-Eintrag gehört nicht zum Projekt oder wurde nicht gefunden.".into(),
        );
    }
    db.query_row("SELECT id,draft_id,run_id,project_id,item_type,title,content,confidence,source_reference_id,target_entity_id,target_rule_id,structured_json,status,created_at,updated_at FROM lore_sheet_items WHERE id=?1 AND project_id=?2", params![id, project_id], lore_item_from_row).map_err(|error| sql_error("Lore-Sheet-Eintrag konnte nicht geladen werden", error))
}
fn load_workspace_for_project(
    db: &Connection,
    project_id: &str,
) -> Result<WorkspaceSnapshot, String> {
    let project_status: String = db
        .query_row(
            "SELECT COALESCE(status, 'active') FROM project_workflow_state WHERE project_id=?1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| sql_error("Projektstatus konnte nicht geladen werden", error))?
        .unwrap_or_else(|| "active".into());
    if project_status == "archived" {
        return Err("Das ausgewählte Projekt ist archiviert.".into());
    }
    let books = load_books(db, project_id)?;
    let chapters = load_chapters(db, &books)?;
    let entities = load_entities(db, project_id)?;
    let mut project = project_from_db(db, project_id)?;
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

#[tauri::command]
pub fn load_workspace(state: State<'_, DbState>) -> Result<WorkspaceSnapshot, String> {
    let db = lock_db(&state)?;
    let project_id: String = db
        .query_row(
            "SELECT project_id FROM project_workflow_state WHERE status='active' ORDER BY last_opened_at DESC, updated_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| sql_error("Workspace konnte nicht geladen werden", error))?
        .ok_or_else(|| "Keine aktive StoryMemory-Projekt gefunden.".to_string())?;
    db.execute(
        "UPDATE project_workflow_state SET last_opened_at=?2, updated_at=?2 WHERE project_id=?1",
        params![project_id, now()],
    )
    .map_err(|error| sql_error("Letztes Projekt konnte nicht gespeichert werden", error))?;
    load_workspace_for_project(&db, &project_id)
}

#[tauri::command]
pub fn load_project_workspace(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<WorkspaceSnapshot, String> {
    let db = lock_db(&state)?;
    let changed = db.execute("UPDATE project_workflow_state SET last_opened_at=?2, updated_at=?2 WHERE project_id=?1 AND status='active'", params![project_id, now()]).map_err(|error| sql_error("Projekt konnte nicht geöffnet werden", error))?;
    if changed == 0 {
        return Err("Das Projekt wurde nicht gefunden oder ist archiviert.".into());
    }
    load_workspace_for_project(&db, &project_id)
}

#[tauri::command]
pub fn list_projects(state: State<'_, DbState>) -> Result<Vec<Project>, String> {
    let db = lock_db(&state)?;
    let mut statement = db
        .prepare("SELECT id FROM projects ORDER BY COALESCE((SELECT last_opened_at FROM project_workflow_state WHERE project_id=projects.id), updated_at) DESC, created_at DESC")
        .map_err(|error| sql_error("Projekte konnten nicht geladen werden", error))?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| sql_error("Projektliste konnte nicht gelesen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Projektliste konnte nicht gelesen werden", error))?;
    ids.into_iter()
        .map(|id| {
            let books = load_books(&db, &id)?;
            let chapters = load_chapters(&db, &books)?;
            let entities = load_entities(&db, &id)?;
            let mut project = project_from_db(&db, &id)?;
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
            Ok(project)
        })
        .collect()
}

#[tauri::command]
pub fn archive_project(state: State<'_, DbState>, project_id: String) -> Result<Project, String> {
    let db = lock_db(&state)?;
    let changed = db.execute("UPDATE project_workflow_state SET status='archived', updated_at=?2 WHERE project_id=?1", params![project_id, now()]).map_err(|error| sql_error("Projekt konnte nicht archiviert werden", error))?;
    if changed == 0 {
        return Err("Das Projekt wurde nicht gefunden.".into());
    }
    project_from_db(&db, &project_id)
}

fn onboarding_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProjectOnboardingState> {
    Ok(ProjectOnboardingState {
        project_id: row.get(0)?,
        current_step: row.get(1)?,
        completed_steps: serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or_default(),
        skipped_steps: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
        language: row.get(4)?,
        genre: row.get(5)?,
        lore_crafter_run_id: row.get(6)?,
        import_id: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

#[tauri::command]
pub fn get_project_onboarding_state(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<ProjectOnboardingState, String> {
    let db = lock_db(&state)?;
    project_from_db(&db, &project_id)?;
    db.query_row(
        "SELECT project_id,current_step,completed_steps_json,skipped_steps_json,language,genre,lore_crafter_run_id,import_id,updated_at FROM project_onboarding_state WHERE project_id=?1",
        params![project_id],
        onboarding_from_row,
    )
    .map_err(|error| sql_error("Onboardingstatus konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn save_project_onboarding_state(
    state: State<'_, DbState>,
    input: SaveProjectOnboardingStateInput,
) -> Result<ProjectOnboardingState, String> {
    if !matches!(
        input.current_step.as_str(),
        "project" | "lore" | "manuscript" | "summary" | "completed"
    ) {
        return Err("Ungültiger Onboarding-Schritt.".into());
    }
    let db = lock_db(&state)?;
    project_from_db(&db, &input.project_id)?;
    let timestamp = now();
    db.execute(
        "INSERT INTO project_onboarding_state(project_id,current_step,completed_steps_json,skipped_steps_json,language,genre,lore_crafter_run_id,import_id,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(project_id) DO UPDATE SET current_step=excluded.current_step,completed_steps_json=excluded.completed_steps_json,skipped_steps_json=excluded.skipped_steps_json,language=excluded.language,genre=excluded.genre,lore_crafter_run_id=excluded.lore_crafter_run_id,import_id=excluded.import_id,updated_at=excluded.updated_at",
        params![
            input.project_id,
            input.current_step,
            serde_json::to_string(&input.completed_steps).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&input.skipped_steps).unwrap_or_else(|_| "[]".into()),
            input.language,
            input.genre,
            input.lore_crafter_run_id,
            input.import_id,
            timestamp,
        ],
    )
    .map_err(|error| sql_error("Onboardingstatus konnte nicht gespeichert werden", error))?;
    db.query_row(
        "SELECT project_id,current_step,completed_steps_json,skipped_steps_json,language,genre,lore_crafter_run_id,import_id,updated_at FROM project_onboarding_state WHERE project_id=?1",
        params![input.project_id],
        onboarding_from_row,
    )
    .map_err(|error| sql_error("Onboardingstatus konnte nicht geladen werden", error))
}

pub(crate) fn create_project_in_db(
    db: &Connection,
    input: CreateProjectInput,
) -> Result<Project, String> {
    required(&input.title, "Der Projekttitel")?;
    // The author is optional during onboarding; an empty value is retained and
    // can be filled in later without blocking project creation.
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
    transaction.execute("INSERT INTO project_workflow_state(project_id,status,last_opened_at,updated_at) VALUES(?1,'active',?2,?2)", params![project_id, timestamp]).map_err(|error| sql_error("Projektstatus konnte nicht gespeichert werden", error))?;
    transaction.execute("INSERT INTO project_onboarding_state(project_id,current_step,completed_steps_json,skipped_steps_json) VALUES(?1,'project','[]','[]')", params![project_id]).map_err(|error| sql_error("Onboardingstatus konnte nicht gespeichert werden", error))?;
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
            is_implicit: true,
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
                "INSERT INTO scenes (id, chapter_id, title, order_index, content, pov, location, story_time, status, goal, notes, is_implicit, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, ?12, ?12)",
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
        | "event" | "fact" | "clue" | "secret" | "plot_thread" | "retcon" | "author_note"
        | "open_question" => Ok(()),
        _ => Err(format!("Ungültiger Story-Bible-Typ: {value}")),
    }
}

fn validate_entity_origin(value: &str) -> Result<(), String> {
    if matches!(value, "manual" | "bible_update" | "edited" | "lore_crafter") {
        Ok(())
    } else {
        Err(format!("Ungültige Eintragsherkunft: {value}"))
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
    let origin = input.origin.as_deref().unwrap_or("manual");
    validate_entity_origin(origin)?;
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
        origin,
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
        source_document_id: row.get(6)?,
        excerpt: row.get(7)?,
        start_offset: row.get(8)?,
        end_offset: row.get(9)?,
        created_at: row.get(10)?,
    })
}

fn sync_manuscript_artifact(
    db: &Connection,
    artifact_type: &str,
    artifact_id: &str,
    status: &str,
) -> Result<(), String> {
    db.execute(
        "UPDATE manuscript_analysis_artifacts SET review_status=?1, explicitly_skipped=0, updated_at=?2 WHERE artifact_type=?3 AND artifact_id=?4",
        params![status, now(), artifact_type, artifact_id],
    )
    .map_err(|error| sql_error("Analyseartefakt konnte nicht aktualisiert werden", error))?;
    Ok(())
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
    let manuscript = db.query_row(
        "SELECT id, project_id, entity_id, proposal_id, chapter_id, scene_id, NULL AS source_document_id, excerpt, start_offset, end_offset, created_at
         FROM story_source_references WHERE id=?1",
        params![id], source_from_row,
    ).optional().map_err(|error| sql_error("Gespeicherte Quellenreferenz konnte nicht geladen werden", error))?;
    if let Some(source) = manuscript {
        return Ok(source);
    }
    db.query_row(
        "SELECT id, project_id, entity_id, proposal_id, chapter_id, scene_id, source_document_id, excerpt, start_offset, end_offset, created_at
         FROM project_source_references WHERE id=?1",
        params![id], source_from_row,
    ).map_err(|error| sql_error("Gespeicherte Quellenreferenz konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn create_source_reference(
    state: State<'_, DbState>,
    input: CreateSourceReferenceInput,
) -> Result<StorySourceReference, String> {
    let db = lock_db(&state)?;
    validate_scene_project(&db, &input.project_id, &input.scene_id)?;
    let chapter_matches: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM scenes WHERE id=?1 AND chapter_id=?2)",
            params![input.scene_id, input.chapter_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Kapitel und Szene konnten nicht geprüft werden", error))?;
    if !chapter_matches {
        return Err("Kapitel und Szene der Quelle passen nicht zusammen.".into());
    }
    if let Some(entity_id) = &input.entity_id {
        project_entity_exists(&db, &input.project_id, entity_id, None)?;
    }
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
    let mut statement = db.prepare("SELECT id, project_id, entity_id, proposal_id, chapter_id, scene_id, NULL AS source_document_id, excerpt, start_offset, end_offset, created_at FROM story_source_references WHERE project_id=?1 AND (?2 IS NULL OR entity_id=?2) UNION ALL SELECT id, project_id, entity_id, proposal_id, chapter_id, scene_id, source_document_id, excerpt, start_offset, end_offset, created_at FROM project_source_references WHERE project_id=?1 AND (?2 IS NULL OR entity_id=?2) ORDER BY created_at DESC").map_err(|error| sql_error("Quellen konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![project_id, entity_id], source_from_row)
        .map_err(|error| sql_error("Quellen konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Quellen konnten nicht geladen werden", error));
    result
}

fn project_source_document_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProjectSourceDocument> {
    Ok(ProjectSourceDocument {
        id: row.get(0)?,
        project_id: row.get(1)?,
        source_kind: row.get(2)?,
        title: row.get(3)?,
        content: row.get(4)?,
        content_hash: row.get(5)?,
        origin_id: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

#[tauri::command]
pub fn create_project_source_document(
    state: State<'_, DbState>,
    input: CreateProjectSourceDocumentInput,
) -> Result<ProjectSourceDocument, String> {
    if !matches!(
        input.source_kind.as_str(),
        "lore_crafter" | "research" | "author_note" | "external_text"
    ) || input.content.chars().count() > 200_000
    {
        return Err("Ungültiges oder zu großes Quelldokument.".into());
    }
    let db = lock_db(&state)?;
    project_from_db(&db, &input.project_id)?;
    let stamp = now();
    let existing: Option<(String, String)> = db.query_row("SELECT id,content FROM project_source_documents WHERE project_id=?1 AND content_hash=?2 AND source_kind=?3 ORDER BY updated_at DESC LIMIT 1", params![input.project_id, input.content_hash, input.source_kind], |row| Ok((row.get(0)?, row.get(1)?))).optional().map_err(|e| sql_error("Quelldokument konnte nicht geprüft werden", e))?;
    if let Some((_, existing_content)) = &existing {
        if existing_content != &input.content {
            return Err(
                "Ein unveränderliches Quelldokument mit diesem Hash enthält bereits anderen Text."
                    .into(),
            );
        }
    }
    let id = existing.map(|value| value.0).unwrap_or_else(new_id);
    db.execute("INSERT INTO project_source_documents(id,project_id,source_kind,title,content,content_hash,origin_id,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,COALESCE((SELECT created_at FROM project_source_documents WHERE id=?1),?8),?8) ON CONFLICT(id) DO UPDATE SET title=excluded.title,content=excluded.content,content_hash=excluded.content_hash,origin_id=excluded.origin_id,updated_at=excluded.updated_at", params![id,input.project_id,input.source_kind,input.title,input.content,input.content_hash,input.origin_id,stamp]).map_err(|e| sql_error("Quelldokument konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,project_id,source_kind,title,content,content_hash,origin_id,created_at,updated_at FROM project_source_documents WHERE id=?1", params![id], project_source_document_from_row).map_err(|e| sql_error("Quelldokument konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn list_project_source_documents(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<ProjectSourceDocument>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id,project_id,source_kind,title,content,content_hash,origin_id,created_at,updated_at FROM project_source_documents WHERE project_id=?1 ORDER BY updated_at DESC").map_err(|e| sql_error("Quelldokumente konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(params![project_id], project_source_document_from_row)
        .map_err(|e| sql_error("Quelldokumente konnten nicht gelesen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Quelldokumente konnten nicht gelesen werden", e));
    result
}

#[tauri::command]
pub fn create_project_source_reference(
    state: State<'_, DbState>,
    input: CreateProjectSourceReferenceInput,
) -> Result<StorySourceReference, String> {
    let db = lock_db(&state)?;
    let document: ProjectSourceDocument = db.query_row("SELECT id,project_id,source_kind,title,content,content_hash,origin_id,created_at,updated_at FROM project_source_documents WHERE id=?1 AND project_id=?2", params![input.source_document_id, input.project_id], project_source_document_from_row).map_err(|e| sql_error("Quelldokument konnte nicht geprüft werden", e))?;
    let chars: Vec<char> = document.content.chars().collect();
    if input.start_offset < 0
        || input.end_offset <= input.start_offset
        || input.end_offset as usize > chars.len()
        || chars[input.start_offset as usize..input.end_offset as usize]
            .iter()
            .collect::<String>()
            != input.excerpt
    {
        return Err("Die Quelle passt nicht zu den Unicode-Positionen des Quelldokuments.".into());
    }
    if let Some(entity_id) = &input.entity_id {
        project_entity_exists(&db, &input.project_id, entity_id, None)?;
    }
    let id = new_id();
    let stamp = now();
    db.execute("INSERT INTO project_source_references(id,project_id,source_document_id,entity_id,proposal_id,excerpt,start_offset,end_offset,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![id,input.project_id,input.source_document_id,input.entity_id,input.proposal_id,input.excerpt,input.start_offset,input.end_offset,stamp]).map_err(|e| sql_error("Projektweite Quellenreferenz konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,project_id,entity_id,proposal_id,chapter_id,scene_id,source_document_id,excerpt,start_offset,end_offset,created_at FROM project_source_references WHERE id=?1", params![id], |row| Ok(StorySourceReference { id: row.get(0)?, project_id: row.get(1)?, entity_id: row.get(2)?, proposal_id: row.get(3)?, chapter_id: row.get(4)?, scene_id: row.get(5)?, source_document_id: row.get(6)?, excerpt: row.get(7)?, start_offset: row.get(8)?, end_offset: row.get(9)?, created_at: row.get(10)? })).map_err(|e| sql_error("Projektweite Quellenreferenz konnte nicht geladen werden", e))
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
    sync_manuscript_artifact(
        db,
        "bible_proposal",
        &proposal.id,
        if input.review_status == "rejected" {
            "rejected"
        } else if input.review_status == "accepted" || input.review_status == "edited" {
            "confirmed"
        } else {
            "uncertain"
        },
    )?;
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
    sync_manuscript_artifact(
        &db,
        "narrative_summary",
        &id,
        if status == "rejected" {
            "rejected"
        } else if status == "confirmed" {
            "confirmed"
        } else {
            "uncertain"
        },
    )?;
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

fn json_strings(value: &str) -> Vec<String> {
    serde_json::from_str(value).unwrap_or_default()
}

fn project_rule_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProjectRule> {
    Ok(ProjectRule {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        statement: row.get(3)?,
        scope: row.get(4)?,
        prerequisites: json_strings(&row.get::<_, String>(5)?),
        effects: json_strings(&row.get::<_, String>(6)?),
        exceptions: json_strings(&row.get::<_, String>(7)?),
        connected_lore_ids: json_strings(&row.get::<_, String>(8)?),
        source_reference_ids: json_strings(&row.get::<_, String>(9)?),
        status: row.get(10)?,
        confidence: row.get(11)?,
        author_confirmed: row.get(12)?,
        origin: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

fn validate_rule_input(db: &Connection, input: &SaveProjectRuleInput) -> Result<(), String> {
    required(&input.title, "Der Regel-Titel")?;
    required(&input.statement, "Die Regelaussage")?;
    if input.title.chars().count() > 200 || input.statement.chars().count() > 4000 {
        return Err("Die Regel ist zu lang.".into());
    }
    validate_project_rule_scope(&input.scope)?;
    validate_project_rule_status(&input.status)?;
    validate_probability(input.confidence, "Die Regel-Sicherheit")?;
    if !matches!(input.origin.as_str(), "manual" | "bible_update" | "edited") {
        return Err("Ungültige Regelherkunft.".into());
    }
    for id in &input.connected_lore_ids {
        let confirmed_lore: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM story_entities e JOIN lore_metadata l ON l.entity_id=e.id WHERE e.id=?1 AND e.project_id=?2 AND e.status='confirmed' AND e.author_confirmed=1)",
                params![id, input.project_id],
                |row| row.get(0),
            )
            .map_err(|error| sql_error("Verbundene Lore konnte nicht geprüft werden", error))?;
        if !confirmed_lore {
            return Err("Verbundene Lore muss zuerst als bestätigte Aussage vorliegen.".into());
        }
    }
    for id in &input.source_reference_ids {
        let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2 UNION ALL SELECT 1 FROM project_source_references WHERE id=?1 AND project_id=?2)", params![id, input.project_id], |row| row.get(0)).map_err(|error| sql_error("Regelquelle konnte nicht geprüft werden", error))?;
        if !exists {
            return Err("Eine Regelquelle gehört nicht zum Projekt.".into());
        }
    }
    Ok(())
}

fn insert_project_rule_tx(
    transaction: &rusqlite::Transaction<'_>,
    input: &SaveProjectRuleInput,
    id: &str,
    created_at: &str,
) -> Result<(), String> {
    let values = [
        &input.prerequisites,
        &input.effects,
        &input.exceptions,
        &input.connected_lore_ids,
        &input.source_reference_ids,
    ];
    let json: Vec<String> = values
        .iter()
        .map(|value| serde_json::to_string(value).unwrap_or_else(|_| "[]".into()))
        .collect();
    transaction.execute("INSERT INTO project_rules (id, project_id, title, statement, scope, prerequisites_json, effects_json, exceptions_json, connected_lore_ids_json, source_reference_ids_json, status, confidence, author_confirmed, origin, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15) ON CONFLICT(id) DO UPDATE SET title=excluded.title, statement=excluded.statement, scope=excluded.scope, prerequisites_json=excluded.prerequisites_json, effects_json=excluded.effects_json, exceptions_json=excluded.exceptions_json, connected_lore_ids_json=excluded.connected_lore_ids_json, source_reference_ids_json=excluded.source_reference_ids_json, status=excluded.status, confidence=excluded.confidence, author_confirmed=excluded.author_confirmed, origin=excluded.origin, updated_at=excluded.updated_at", params![id, input.project_id, input.title, input.statement, input.scope, json[0], json[1], json[2], json[3], json[4], input.status, input.confidence, input.author_confirmed, input.origin, created_at]).map_err(|error| sql_error("Projektregel konnte nicht gespeichert werden", error))?;
    Ok(())
}

#[tauri::command]
pub fn list_project_rules(
    state: State<'_, DbState>,
    project_id: String,
    active_only: bool,
) -> Result<Vec<ProjectRule>, String> {
    let db = lock_db(&state)?;
    let query = "SELECT id, project_id, title, statement, scope, prerequisites_json, effects_json, exceptions_json, connected_lore_ids_json, source_reference_ids_json, status, confidence, author_confirmed, origin, created_at, updated_at FROM project_rules WHERE project_id=?1 AND (?2=0 OR (status='confirmed' AND author_confirmed=1)) ORDER BY updated_at DESC";
    let result = db
        .prepare(query)
        .map_err(|error| sql_error("Projektregeln konnten nicht geladen werden", error))?
        .query_map(params![project_id, active_only], project_rule_from_row)
        .map_err(|error| sql_error("Projektregeln konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Projektregeln konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn save_project_rule(
    state: State<'_, DbState>,
    input: SaveProjectRuleInput,
) -> Result<ProjectRule, String> {
    let db = lock_db(&state)?;
    validate_rule_input(&db, &input)?;
    let id = input.id.clone().unwrap_or_else(new_id);
    let stamp = now();
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Regeltransaktion konnte nicht gestartet werden", error))?;
    insert_project_rule_tx(&transaction, &input, &id, &stamp)?;
    transaction
        .commit()
        .map_err(|error| sql_error("Regeltransaktion konnte nicht abgeschlossen werden", error))?;
    db.query_row("SELECT id, project_id, title, statement, scope, prerequisites_json, effects_json, exceptions_json, connected_lore_ids_json, source_reference_ids_json, status, confidence, author_confirmed, origin, created_at, updated_at FROM project_rules WHERE id=?1", params![id], project_rule_from_row).map_err(|error| sql_error("Gespeicherte Projektregel konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn delete_project_rule(
    state: State<'_, DbState>,
    project_id: String,
    id: String,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    db.execute(
        "UPDATE project_rules SET status='retired', updated_at=?3 WHERE project_id=?1 AND id=?2",
        params![project_id, id, now()],
    )
    .map_err(|error| sql_error("Projektregel konnte nicht archiviert werden", error))?;
    Ok(())
}

fn rule_proposal_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProjectRuleProposal> {
    Ok(ProjectRuleProposal {
        id: row.get(0)?,
        project_id: row.get(1)?,
        target_rule_id: row.get(2)?,
        title: row.get(3)?,
        statement: row.get(4)?,
        scope: row.get(5)?,
        prerequisites: json_strings(&row.get::<_, String>(6)?),
        effects: json_strings(&row.get::<_, String>(7)?),
        exceptions: json_strings(&row.get::<_, String>(8)?),
        connected_lore_ids: json_strings(&row.get::<_, String>(9)?),
        source_reference_ids: json_strings(&row.get::<_, String>(10)?),
        evidence_excerpt: row.get(11)?,
        chapter_id: row.get(12)?,
        scene_id: row.get(13)?,
        start_offset: row.get(14)?,
        end_offset: row.get(15)?,
        confidence: row.get(16)?,
        reason: row.get(17)?,
        review_status: row.get(18)?,
        reviewed_at: row.get(19)?,
        created_at: row.get(20)?,
    })
}

#[tauri::command]
pub fn list_project_rule_proposals(
    state: State<'_, DbState>,
    project_id: String,
    review_status: Option<String>,
) -> Result<Vec<ProjectRuleProposal>, String> {
    let db = lock_db(&state)?;
    if let Some(status) = &review_status {
        validate_rule_proposal_status(status)?;
    }
    let result = db.prepare("SELECT id, project_id, target_rule_id, title, statement, scope, prerequisites_json, effects_json, exceptions_json, connected_lore_ids_json, source_reference_ids_json, evidence_excerpt, chapter_id, scene_id, start_offset, end_offset, confidence, reason, review_status, reviewed_at, created_at FROM project_rule_proposals WHERE project_id=?1 AND (?2 IS NULL OR review_status=?2) ORDER BY created_at DESC").map_err(|error| sql_error("Regelvorschläge konnten nicht geladen werden", error))?.query_map(params![project_id, review_status], rule_proposal_from_row).map_err(|error| sql_error("Regelvorschläge konnten nicht geladen werden", error))?.collect::<SqlResult<Vec<_>>>().map_err(|error| sql_error("Regelvorschläge konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn save_project_rule_proposal(
    state: State<'_, DbState>,
    input: SaveProjectRuleProposalInput,
) -> Result<ProjectRuleProposal, String> {
    let db = lock_db(&state)?;
    validate_project_rule_scope(&input.scope)?;
    validate_probability(input.confidence, "Die Regel-Sicherheit")?;
    if input.title.chars().count() > 200 || input.statement.chars().count() > 4000 {
        return Err("Der Regelvorschlag ist zu lang.".into());
    }
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
    for id in &input.connected_lore_ids {
        project_entity_exists(&db, &input.project_id, id, None)?;
    }
    for id in &input.source_reference_ids {
        let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2 UNION ALL SELECT 1 FROM project_source_references WHERE id=?1 AND project_id=?2)", params![id, input.project_id], |row| row.get(0)).map_err(|error| sql_error("Regelquelle konnte nicht geprüft werden", error))?;
        if !exists {
            return Err("Eine Regelquelle gehört nicht zum Projekt.".into());
        }
    }
    if let Some(scene_id) = &input.scene_id {
        validate_scene_project(&db, &input.project_id, scene_id)?;
    }
    let id = input.id.clone().unwrap_or_else(new_id);
    let status = input
        .review_status
        .clone()
        .unwrap_or_else(|| "pending".into());
    validate_rule_proposal_status(&status)?;
    let json = [
        &input.prerequisites,
        &input.effects,
        &input.exceptions,
        &input.connected_lore_ids,
        &input.source_reference_ids,
    ]
    .iter()
    .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".into()))
    .collect::<Vec<_>>();
    db.execute("INSERT INTO project_rule_proposals (id, project_id, target_rule_id, title, statement, scope, prerequisites_json, effects_json, exceptions_json, connected_lore_ids_json, source_reference_ids_json, evidence_excerpt, chapter_id, scene_id, start_offset, end_offset, confidence, reason, review_status, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20) ON CONFLICT(id) DO UPDATE SET title=excluded.title, statement=excluded.statement, review_status=excluded.review_status", params![id, input.project_id, input.target_rule_id, input.title, input.statement, input.scope, json[0], json[1], json[2], json[3], json[4], input.evidence_excerpt, input.chapter_id, input.scene_id, input.start_offset, input.end_offset, input.confidence, input.reason, status, now()]).map_err(|error| sql_error("Regelvorschlag konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT id, project_id, target_rule_id, title, statement, scope, prerequisites_json, effects_json, exceptions_json, connected_lore_ids_json, source_reference_ids_json, evidence_excerpt, chapter_id, scene_id, start_offset, end_offset, confidence, reason, review_status, reviewed_at, created_at FROM project_rule_proposals WHERE id=?1", params![id], rule_proposal_from_row).map_err(|error| sql_error("Gespeicherter Regelvorschlag konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn review_project_rule_proposal(
    state: State<'_, DbState>,
    id: String,
    review_status: String,
    input: Option<SaveProjectRuleInput>,
) -> Result<ProjectRuleProposal, String> {
    validate_rule_proposal_status(&review_status)?;
    if review_status == "pending" {
        return Err("Ein Regelvorschlag kann nicht auf ausstehend geprüft werden.".into());
    }
    let db = lock_db(&state)?;
    let proposal = db.query_row("SELECT id, project_id, target_rule_id, title, statement, scope, prerequisites_json, effects_json, exceptions_json, connected_lore_ids_json, source_reference_ids_json, evidence_excerpt, chapter_id, scene_id, start_offset, end_offset, confidence, reason, review_status, reviewed_at, created_at FROM project_rule_proposals WHERE id=?1", params![id], rule_proposal_from_row).map_err(|error| sql_error("Regelvorschlag konnte nicht geladen werden", error))?;
    if proposal.review_status != "pending" {
        return Err("Dieser Regelvorschlag wurde bereits geprüft.".into());
    }
    let transaction = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Regelreview konnte nicht gestartet werden", error))?;
    if review_status != "rejected" {
        let mut rule_input = input.unwrap_or(SaveProjectRuleInput {
            id: proposal.target_rule_id.clone(),
            project_id: proposal.project_id.clone(),
            title: proposal.title.clone(),
            statement: proposal.statement.clone(),
            scope: proposal.scope.clone(),
            prerequisites: proposal.prerequisites.clone(),
            effects: proposal.effects.clone(),
            exceptions: proposal.exceptions.clone(),
            connected_lore_ids: proposal.connected_lore_ids.clone(),
            source_reference_ids: proposal.source_reference_ids.clone(),
            status: "confirmed".into(),
            confidence: proposal.confidence,
            author_confirmed: true,
            origin: if review_status == "edited" {
                "edited".into()
            } else {
                "bible_update".into()
            },
        });
        validate_rule_input(&db, &rule_input)?;
        if rule_input.source_reference_ids.is_empty()
            && !proposal.evidence_excerpt.trim().is_empty()
            && proposal.chapter_id.is_some()
            && proposal.scene_id.is_some()
        {
            let source_id = insert_source_reference_if_missing_tx(
                &transaction,
                &CreateSourceReferenceInput {
                    project_id: proposal.project_id.clone(),
                    entity_id: None,
                    proposal_id: Some(proposal.id.clone()),
                    chapter_id: proposal.chapter_id.clone().unwrap_or_default(),
                    scene_id: proposal.scene_id.clone().unwrap_or_default(),
                    excerpt: proposal.evidence_excerpt.clone(),
                    start_offset: proposal.start_offset,
                    end_offset: proposal.end_offset,
                },
            )?;
            rule_input.source_reference_ids.push(source_id);
        }
        insert_project_rule_tx(
            &transaction,
            &rule_input,
            &rule_input.id.clone().unwrap_or_else(new_id),
            &now(),
        )?;
    }
    transaction
        .execute(
            "UPDATE project_rule_proposals SET review_status=?2, reviewed_at=?3 WHERE id=?1",
            params![id, review_status, now()],
        )
        .map_err(|error| sql_error("Regelreview konnte nicht gespeichert werden", error))?;
    transaction
        .commit()
        .map_err(|error| sql_error("Regelreview konnte nicht abgeschlossen werden", error))?;
    sync_manuscript_artifact(
        &db,
        "project_rule_proposal",
        &id,
        if review_status == "rejected" {
            "rejected"
        } else {
            "confirmed"
        },
    )?;
    db.query_row("SELECT id, project_id, target_rule_id, title, statement, scope, prerequisites_json, effects_json, exceptions_json, connected_lore_ids_json, source_reference_ids_json, evidence_excerpt, chapter_id, scene_id, start_offset, end_offset, confidence, reason, review_status, reviewed_at, created_at FROM project_rule_proposals WHERE id=?1", params![id], rule_proposal_from_row).map_err(|error| sql_error("Regelreview konnte nicht geladen werden", error))
}

fn ledger_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ContinuityStateLedgerEntry> {
    Ok(ContinuityStateLedgerEntry {
        id: row.get(0)?,
        project_id: row.get(1)?,
        entity_id: row.get(2)?,
        related_entity_id: row.get(3)?,
        state_kind: row.get(4)?,
        previous_state: row.get(5)?,
        new_state: row.get(6)?,
        reason: row.get(7)?,
        evidence_excerpt: row.get(8)?,
        chapter_id: row.get(9)?,
        scene_id: row.get(10)?,
        start_offset: row.get(11)?,
        end_offset: row.get(12)?,
        source_reference_id: row.get(13)?,
        status: row.get(14)?,
        confidence: row.get(15)?,
        author_confirmed: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

fn validate_continuity_input(
    db: &Connection,
    input: &SaveContinuityStateInput,
) -> Result<(), String> {
    required(&input.new_state, "Der neue Zustand")?;
    if input.new_state.chars().count() > 4000 || input.previous_state.chars().count() > 4000 {
        return Err("Der Zustandswert ist zu lang.".into());
    }
    validate_continuity_state_kind(&input.state_kind)?;
    validate_continuity_state_status(&input.status)?;
    validate_probability(input.confidence, "Die Zustands-Sicherheit")?;
    project_entity_exists(db, &input.project_id, &input.entity_id, None)?;
    if let Some(id) = &input.related_entity_id {
        project_entity_exists(db, &input.project_id, id, None)?;
    }
    if let (Some(chapter_id), Some(scene_id)) = (&input.chapter_id, &input.scene_id) {
        let matches: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM scenes WHERE id=?1 AND chapter_id=?2)",
                params![scene_id, chapter_id],
                |row| row.get(0),
            )
            .map_err(|error| sql_error("Kapitel und Szene konnten nicht geprüft werden", error))?;
        if !matches {
            return Err("Kapitel und Szene passen nicht zusammen.".into());
        }
        validate_scene_project(db, &input.project_id, scene_id)?;
    }
    if let Some(source_id) = &input.source_reference_id {
        let ok: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2 UNION ALL SELECT 1 FROM project_source_references WHERE id=?1 AND project_id=?2)", params![source_id, input.project_id], |row| row.get(0)).map_err(|error| sql_error("Quelle konnte nicht geprüft werden", error))?;
        if !ok {
            return Err("Die Quelle gehört nicht zum Projekt.".into());
        }
    }
    Ok(())
}

#[tauri::command]
pub fn list_continuity_state_ledger(
    state: State<'_, DbState>,
    project_id: String,
    entity_id: Option<String>,
    state_kind: Option<String>,
) -> Result<Vec<ContinuityStateLedgerEntry>, String> {
    let db = lock_db(&state)?;
    if let Some(kind) = &state_kind {
        validate_continuity_state_kind(kind)?;
    }
    let result = db.prepare("SELECT id, project_id, entity_id, related_entity_id, state_kind, previous_state, new_state, reason, evidence_excerpt, chapter_id, scene_id, start_offset, end_offset, source_reference_id, status, confidence, author_confirmed, created_at, updated_at FROM continuity_state_ledger WHERE project_id=?1 AND (?2 IS NULL OR entity_id=?2) AND (?3 IS NULL OR state_kind=?3) ORDER BY created_at DESC").map_err(|error| sql_error("Zustandsverlauf konnte nicht geladen werden", error))?.query_map(params![project_id, entity_id, state_kind], ledger_from_row).map_err(|error| sql_error("Zustandsverlauf konnte nicht geladen werden", error))?.collect::<SqlResult<Vec<_>>>().map_err(|error| sql_error("Zustandsverlauf konnte nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn save_continuity_state_entry(
    state: State<'_, DbState>,
    input: SaveContinuityStateInput,
) -> Result<ContinuityStateLedgerEntry, String> {
    let db = lock_db(&state)?;
    validate_continuity_input(&db, &input)?;
    if input.id.is_some()
        && input.status == "confirmed"
        && (!input.author_confirmed || input.source_reference_id.is_none())
    {
        return Err(
            "Ein bestätigter Zustand benötigt Autorbestätigung und eine Source Reference.".into(),
        );
    }
    let id = input.id.clone().unwrap_or_else(new_id);
    let stamp = now();
    db.execute("INSERT INTO continuity_state_ledger (id, project_id, entity_id, related_entity_id, state_kind, previous_state, new_state, reason, evidence_excerpt, chapter_id, scene_id, start_offset, end_offset, source_reference_id, status, confidence, author_confirmed, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?18) ON CONFLICT(id) DO UPDATE SET previous_state=excluded.previous_state, new_state=excluded.new_state, reason=excluded.reason, evidence_excerpt=excluded.evidence_excerpt, chapter_id=excluded.chapter_id, scene_id=excluded.scene_id, start_offset=excluded.start_offset, end_offset=excluded.end_offset, source_reference_id=excluded.source_reference_id, status=excluded.status, confidence=excluded.confidence, author_confirmed=excluded.author_confirmed, updated_at=excluded.updated_at", params![id, input.project_id, input.entity_id, input.related_entity_id, input.state_kind, input.previous_state, input.new_state, input.reason, input.evidence_excerpt, input.chapter_id, input.scene_id, input.start_offset, input.end_offset, input.source_reference_id, input.status, input.confidence, input.author_confirmed, stamp]).map_err(|error| sql_error("Zustandsänderung konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT id, project_id, entity_id, related_entity_id, state_kind, previous_state, new_state, reason, evidence_excerpt, chapter_id, scene_id, start_offset, end_offset, source_reference_id, status, confidence, author_confirmed, created_at, updated_at FROM continuity_state_ledger WHERE id=?1", params![id], ledger_from_row).map_err(|error| sql_error("Gespeicherte Zustandsänderung konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn get_state_at_position(
    state: State<'_, DbState>,
    project_id: String,
    entity_id: String,
    state_kind: String,
    target_position: ManuscriptPosition,
) -> Result<Option<ContinuityStateLedgerEntry>, String> {
    validate_continuity_state_kind(&state_kind)?;
    let db = lock_db(&state)?;
    validate_scene_project(&db, &project_id, &target_position.scene_id)?;
    let target: (i64, i64) = db.query_row("SELECT chapters.order_index, scenes.order_index FROM scenes JOIN chapters ON chapters.id=scenes.chapter_id WHERE scenes.id=?1 AND chapters.id=?2", params![target_position.scene_id, target_position.chapter_id], |row| Ok((row.get(0)?, row.get(1)?))).map_err(|error| sql_error("Zielposition konnte nicht geprüft werden", error))?;
    let mut statement = db.prepare("SELECT l.id, l.project_id, l.entity_id, l.related_entity_id, l.state_kind, l.previous_state, l.new_state, l.reason, l.evidence_excerpt, l.chapter_id, l.scene_id, l.start_offset, l.end_offset, l.source_reference_id, l.status, l.confidence, l.author_confirmed, l.created_at, l.updated_at, COALESCE(c.order_index, -1), COALESCE(s.order_index, -1) FROM continuity_state_ledger l LEFT JOIN chapters c ON c.id=l.chapter_id LEFT JOIN scenes s ON s.id=l.scene_id WHERE l.project_id=?1 AND l.entity_id=?2 AND l.state_kind=?3 AND l.status='confirmed' AND l.author_confirmed=1 ORDER BY c.order_index DESC, s.order_index DESC, COALESCE(l.start_offset, -1) DESC").map_err(|error| sql_error("Zustandsverlauf konnte nicht geladen werden", error))?;
    let rows = statement
        .query_map(params![project_id, entity_id, state_kind], |row| {
            let entry = ledger_from_row(row)?;
            let chapter_order: i64 = row.get(17)?;
            let scene_order: i64 = row.get(18)?;
            Ok((entry, chapter_order, scene_order))
        })
        .map_err(|error| sql_error("Zustandsverlauf konnte nicht geladen werden", error))?;
    for row in rows {
        let (entry, chapter_order, scene_order) =
            row.map_err(|error| sql_error("Zustand konnte nicht gelesen werden", error))?;
        let before = chapter_order < target.0
            || (chapter_order == target.0
                && (scene_order < target.1
                    || (scene_order == target.1
                        && entry.start_offset.unwrap_or(-1)
                            <= target_position.offset.unwrap_or(i64::MAX))));
        if before {
            return Ok(Some(entry));
        }
    }
    Ok(None)
}

fn continuity_source_kind_valid(value: &str) -> bool {
    matches!(
        value,
        "word_threshold" | "page_marker" | "bible_update" | "longform_section" | "manual"
    )
}

fn continuity_finding_type_valid(value: &str) -> bool {
    matches!(
        value,
        "critical_contradiction"
            | "probable_contradiction"
            | "missing_explanation"
            | "character_deviation"
            | "lore_compatible_anomaly"
            | "possible_intentional_exception"
            | "insufficient_evidence"
    )
}

fn continuity_severity_valid(value: &str) -> bool {
    matches!(value, "info" | "warning" | "critical")
}

fn continuity_review_status_valid(value: &str) -> bool {
    matches!(
        value,
        "open" | "accepted" | "dismissed" | "resolved" | "deferred"
    )
}

fn lifecycle_status_valid(value: &str) -> bool {
    matches!(
        value,
        "open" | "closure_candidate" | "partially_resolved" | "resolved" | "reopened" | "abandoned"
    )
}

fn continuity_settings_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ContinuityReviewSettings> {
    Ok(ContinuityReviewSettings {
        project_id: row.get(0)?,
        word_threshold: row.get(1)?,
        updated_at: row.get(2)?,
    })
}

fn continuity_run_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ContinuityReviewRun> {
    Ok(ContinuityReviewRun {
        id: row.get(0)?,
        project_id: row.get(1)?,
        chapter_id: row.get(2)?,
        scene_id: row.get(3)?,
        source_kind: row.get(4)?,
        content_hash: row.get(5)?,
        start_offset: row.get(6)?,
        end_offset: row.get(7)?,
        provider_id: row.get(8)?,
        status: row.get(9)?,
        created_at: row.get(10)?,
        completed_at: row.get(11)?,
        error_message: row.get(12)?,
    })
}

fn continuity_finding_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ContinuityReviewFinding> {
    Ok(ContinuityReviewFinding {
        id: row.get(0)?,
        run_id: row.get(1)?,
        project_id: row.get(2)?,
        chapter_id: row.get(3)?,
        scene_id: row.get(4)?,
        finding_type: row.get(5)?,
        severity: row.get(6)?,
        subject_entity_id: row.get(7)?,
        related_entity_ids: json_strings(&row.get::<_, String>(8)?),
        related_state_ids: json_strings(&row.get::<_, String>(9)?),
        related_rule_ids: json_strings(&row.get::<_, String>(10)?),
        objective_conflict: row.get(11)?,
        lore_explanations: json_strings(&row.get::<_, String>(12)?),
        evidence_excerpt: row.get(13)?,
        source_reference_id: row.get(14)?,
        counter_evidence_excerpts: json_strings(&row.get::<_, String>(15)?),
        counter_evidence: serde_json::from_str(&row.get::<_, String>(16)?).unwrap_or_default(),
        confidence: row.get(17)?,
        start_offset: row.get(18)?,
        end_offset: row.get(19)?,
        reason: row.get(20)?,
        review_status: row.get(21)?,
        user_decision: row.get(22)?,
        created_at: row.get(23)?,
        updated_at: row.get(24)?,
    })
}

fn lifecycle_from_row(row: &rusqlite::Row<'_>) -> SqlResult<PlotThreadLifecycle> {
    Ok(PlotThreadLifecycle {
        id: row.get(0)?,
        project_id: row.get(1)?,
        entity_id: row.get(2)?,
        lifecycle_status: row.get(3)?,
        last_source_reference_id: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn lifecycle_proposal_from_row(row: &rusqlite::Row<'_>) -> SqlResult<PlotThreadLifecycleProposal> {
    Ok(PlotThreadLifecycleProposal {
        id: row.get(0)?,
        run_id: row.get(1)?,
        project_id: row.get(2)?,
        entity_id: row.get(3)?,
        proposed_status: row.get(4)?,
        evidence_excerpt: row.get(5)?,
        source_reference_id: row.get(6)?,
        start_offset: row.get(7)?,
        end_offset: row.get(8)?,
        reason: row.get(9)?,
        confidence: row.get(10)?,
        review_status: row.get(11)?,
        reviewed_at: row.get(12)?,
        created_at: row.get(13)?,
    })
}

#[tauri::command]
pub fn get_continuity_review_settings(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<ContinuityReviewSettings, String> {
    let db = lock_db(&state)?;
    db.query_row("SELECT project_id, word_threshold, updated_at FROM continuity_review_settings WHERE project_id=?1", params![project_id], continuity_settings_from_row).optional().map_err(|error| sql_error("Prüfeinstellungen konnten nicht geladen werden", error)).map(|value| value.unwrap_or(ContinuityReviewSettings { project_id, word_threshold: 300, updated_at: now() }))
}

#[tauri::command]
pub fn save_continuity_review_settings(
    state: State<'_, DbState>,
    project_id: String,
    word_threshold: i64,
) -> Result<ContinuityReviewSettings, String> {
    if !(50..=5000).contains(&word_threshold) {
        return Err("Die Prüfschwelle muss zwischen 50 und 5000 Wörtern liegen.".into());
    }
    let db = lock_db(&state)?;
    let stamp = now();
    db.execute("INSERT INTO continuity_review_settings (project_id, word_threshold, updated_at) VALUES (?1,?2,?3) ON CONFLICT(project_id) DO UPDATE SET word_threshold=excluded.word_threshold, updated_at=excluded.updated_at", params![project_id, word_threshold, stamp]).map_err(|error| sql_error("Prüfeinstellungen konnten nicht gespeichert werden", error))?;
    db.query_row("SELECT project_id, word_threshold, updated_at FROM continuity_review_settings WHERE project_id=?1", params![project_id], continuity_settings_from_row).map_err(|error| sql_error("Gespeicherte Prüfeinstellungen konnten nicht geladen werden", error))
}

#[tauri::command]
pub fn create_continuity_review_run(
    state: State<'_, DbState>,
    input: SaveContinuityReviewInput,
) -> Result<ContinuityReviewRun, String> {
    if !continuity_source_kind_valid(&input.source_kind) {
        return Err("Unbekannte Quelle der Kontinuitätsprüfung.".into());
    }
    required(&input.content_hash, "Der Prüfinhalt")?;
    let db = lock_db(&state)?;
    if let Some(chapter_id) = &input.chapter_id {
        validate_chapter_project(&db, &input.project_id, chapter_id)?;
    }
    if let Some(scene_id) = &input.scene_id {
        validate_scene_project(&db, &input.project_id, scene_id)?;
    }
    let id = new_id();
    let stamp = now();
    db.execute("INSERT INTO continuity_review_runs (id, project_id, chapter_id, scene_id, source_kind, content_hash, start_offset, end_offset, provider_id, status, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'pending',?10)", params![id, input.project_id, input.chapter_id, input.scene_id, input.source_kind, input.content_hash, input.start_offset, input.end_offset, input.provider_id.unwrap_or_else(|| "local-continuity-review".into()), stamp]).map_err(|error| sql_error("Kontinuitätsprüfung konnte nicht gespeichert werden", error))?;
    db.execute("INSERT INTO continuity_review_run_statuses (run_id, status, updated_at) VALUES (?1,'pending',?2)", params![id, stamp]).map_err(|error| sql_error("Status der Kontinuitätsprüfung konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT r.id, r.project_id, r.chapter_id, r.scene_id, r.source_kind, r.content_hash, r.start_offset, r.end_offset, r.provider_id, COALESCE(s.status, r.status), r.created_at, COALESCE(s.completed_at, r.completed_at), COALESCE(s.error_message, r.error_message) FROM continuity_review_runs r LEFT JOIN continuity_review_run_statuses s ON s.run_id=r.id WHERE r.id=?1", params![id], continuity_run_from_row).map_err(|error| sql_error("Kontinuitätsprüfung konnte nicht geladen werden", error))
}

fn continuity_run_status_valid(value: &str) -> bool {
    matches!(
        value,
        "pending" | "running" | "completed" | "failed" | "cancelled" | "reviewed"
    )
}

#[tauri::command]
pub fn update_continuity_review_run_status(
    state: State<'_, DbState>,
    input: SaveContinuityReviewRunStatusInput,
) -> Result<ContinuityReviewRun, String> {
    if !continuity_run_status_valid(&input.status) {
        return Err("Ungültiger Status der Kontinuitätsprüfung.".into());
    }
    let db = lock_db(&state)?;
    let completed_at = if matches!(
        input.status.as_str(),
        "completed" | "failed" | "cancelled" | "reviewed"
    ) {
        Some(input.completed_at.unwrap_or_else(now))
    } else {
        None
    };
    let legacy_status = if input.status == "cancelled" {
        "failed"
    } else {
        input.status.as_str()
    };
    db.execute("UPDATE continuity_review_runs SET status=?2, error_message=?3, completed_at=?4 WHERE id=?1", params![input.id, legacy_status, input.error_message, completed_at]).map_err(|error| sql_error("Status der Kontinuitätsprüfung konnte nicht gespeichert werden", error))?;
    db.execute("INSERT INTO continuity_review_run_statuses (run_id, status, completed_at, error_message, updated_at) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(run_id) DO UPDATE SET status=excluded.status, completed_at=excluded.completed_at, error_message=excluded.error_message, updated_at=excluded.updated_at", params![input.id, input.status, completed_at, input.error_message, now()]).map_err(|error| sql_error("Status der Kontinuitätsprüfung konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT r.id, r.project_id, r.chapter_id, r.scene_id, r.source_kind, r.content_hash, r.start_offset, r.end_offset, r.provider_id, COALESCE(s.status, r.status), r.created_at, COALESCE(s.completed_at, r.completed_at), COALESCE(s.error_message, r.error_message) FROM continuity_review_runs r LEFT JOIN continuity_review_run_statuses s ON s.run_id=r.id WHERE r.id=?1", params![input.id], continuity_run_from_row).map_err(|error| sql_error("Kontinuitätsprüfung konnte nicht geladen werden", error))
}

fn manuscript_analysis_job_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ManuscriptAnalysisJob> {
    let phase_progress_json: String = row.get(6)?;
    let phase_errors_json: String = row.get(7)?;
    let page_markers_json: String = row.get(14)?;
    let phase_progress =
        serde_json::from_str(&phase_progress_json).unwrap_or_else(|_| serde_json::json!({}));
    let phase_errors =
        serde_json::from_str(&phase_errors_json).unwrap_or_else(|_| serde_json::json!({}));
    let page_markers = serde_json::from_str::<Vec<ManuscriptAnalysisPageMarker>>(
        &page_markers_json,
    )
    .map_err(|error| rusqlite::Error::FromSqlConversionFailure(14, Type::Text, Box::new(error)))?;
    Ok(ManuscriptAnalysisJob {
        id: row.get(0)?,
        project_id: row.get(1)?,
        book_id: row.get(2)?,
        import_reference: row.get(3)?,
        status: row.get(4)?,
        current_phase: row.get(5)?,
        phase_progress,
        phase_errors,
        total_units: row.get(8)?,
        completed_units: row.get(9)?,
        failed_units: row.get(10)?,
        current_unit_id: row.get(11)?,
        last_successful_unit_id: row.get(12)?,
        provider_id: row.get(13)?,
        page_markers,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
        completed_at: row.get(17)?,
        error_message: row.get(18)?,
    })
}

fn manuscript_analysis_unit_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ManuscriptAnalysisUnit> {
    Ok(ManuscriptAnalysisUnit {
        id: row.get(0)?,
        job_id: row.get(1)?,
        project_id: row.get(2)?,
        chapter_id: row.get(3)?,
        scene_id: row.get(4)?,
        order_index: row.get(5)?,
        page_number: row.get(6)?,
        start_offset: row.get(7)?,
        end_offset: row.get(8)?,
        content: row.get(9)?,
        content_hash: row.get(10)?,
        status: row.get(11)?,
        retry_count: row.get(12)?,
        continuity_run_id: row.get(13)?,
        requested_provider: row.get(14)?,
        actual_provider: row.get(15)?,
        prompt_version: row.get(16)?,
        input_hash: row.get(17)?,
        output_hash: row.get(18)?,
        error_code: row.get(19)?,
        error_message: row.get(20)?,
        created_at: row.get(21)?,
        updated_at: row.get(22)?,
        completed_at: row.get(23)?,
    })
}

fn manuscript_analysis_draft_from_row(
    row: &rusqlite::Row<'_>,
) -> SqlResult<ManuscriptAnalysisDraftLedgerEntry> {
    Ok(ManuscriptAnalysisDraftLedgerEntry {
        id: row.get(0)?,
        job_id: row.get(1)?,
        unit_id: row.get(2)?,
        project_id: row.get(3)?,
        entity_id: row.get(4)?,
        related_entity_id: row.get(5)?,
        state_kind: row.get(6)?,
        previous_state: row.get(7)?,
        new_state: row.get(8)?,
        chapter_id: row.get(9)?,
        scene_id: row.get(10)?,
        start_offset: row.get(11)?,
        end_offset: row.get(12)?,
        source_excerpt: row.get(13)?,
        source_reference_id: row.get(14)?,
        confidence: row.get(15)?,
        status: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

fn manuscript_analysis_job_status_valid(value: &str) -> bool {
    matches!(
        value,
        "pending"
            | "running"
            | "paused"
            | "awaiting_structure_review"
            | "awaiting_user_review"
            | "completed"
            | "failed"
            | "cancelled"
    )
}

fn manuscript_analysis_unit_status_valid(value: &str) -> bool {
    matches!(
        value,
        "pending" | "running" | "completed" | "failed" | "stale" | "skipped"
    )
}

fn manuscript_analysis_draft_status_valid(value: &str) -> bool {
    matches!(
        value,
        "proposed" | "confirmed" | "rejected" | "uncertain" | "superseded"
    )
}

#[tauri::command]
pub fn create_manuscript_analysis_job(
    state: State<'_, DbState>,
    input: CreateManuscriptAnalysisJobInput,
) -> Result<ManuscriptAnalysisJob, String> {
    required(&input.project_id, "Das Projekt")?;
    required(&input.book_id, "Das Buch")?;
    required(&input.import_reference, "Die Importreferenz")?;
    let db = lock_db(&state)?;
    validate_book_project(&db, &input.project_id, &input.book_id)?;
    if !input.new_version {
        if let Some(existing) = db.query_row("SELECT id, project_id, book_id, import_reference, status, current_phase, phase_progress_json, phase_errors_json, total_units, completed_units, failed_units, current_unit_id, last_successful_unit_id, provider_id, page_markers_json, created_at, updated_at, completed_at, error_message FROM manuscript_analysis_jobs WHERE project_id=?1 AND import_reference=?2", params![input.project_id, input.import_reference], manuscript_analysis_job_from_row).optional().map_err(|error| sql_error("Manuskriptanalysejob konnte nicht geprüft werden", error))? {
        return Ok(existing);
    }
    }
    let tx = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Manuskriptanalysejob konnte nicht angelegt werden", error))?;
    let job_id = new_id();
    let stamp = now();
    let page_markers_json = serde_json::to_string(&input.page_markers)
        .map_err(|error| format!("Seitenmarker konnten nicht serialisiert werden: {error}"))?;
    tx.execute("INSERT INTO manuscript_analysis_jobs (id, project_id, book_id, import_reference, status, current_phase, phase_progress_json, phase_errors_json, total_units, provider_id, page_markers_json, created_at, updated_at) VALUES (?1,?2,?3,?4,'pending','structure','{}','{}',?5,?6,?7,?8,?8)", params![job_id, input.project_id, input.book_id, input.import_reference, input.units.len() as i64, input.provider_id, page_markers_json, stamp]).map_err(|error| sql_error("Manuskriptanalysejob konnte nicht gespeichert werden", error))?;
    for unit in &input.units {
        if unit.start_offset < 0
            || unit.end_offset < unit.start_offset
            || unit.content.chars().count() == 0
        {
            return Err(
                "Eine Prüfeinheit hat ungültige Unicode-Positionen oder keinen Inhalt.".into(),
            );
        }
        validate_scene_project(&tx, &input.project_id, &unit.scene_id)?;
        let chapter_matches: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM scenes WHERE id=?1 AND chapter_id=?2)",
                params![unit.scene_id, unit.chapter_id],
                |row| row.get(0),
            )
            .map_err(|error| sql_error("Kapitel und Szene konnten nicht geprüft werden", error))?;
        if !chapter_matches {
            return Err("Kapitel und Szene einer Prüfeinheit passen nicht zusammen.".into());
        }
        tx.execute("INSERT INTO manuscript_analysis_units (id, job_id, project_id, chapter_id, scene_id, order_index, page_number, start_offset, end_offset, content, content_hash, status, retry_count, requested_provider, prompt_version, input_hash, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'pending',0,?12,'manuscript-analysis-v1',?11,?13,?13)", params![unit.id.clone().unwrap_or_else(new_id), job_id, input.project_id, unit.chapter_id, unit.scene_id, unit.order_index, unit.page_number, unit.start_offset, unit.end_offset, unit.content, unit.content_hash, input.provider_id, stamp]).map_err(|error| sql_error("Manuskriptprüfeinheit konnte nicht gespeichert werden", error))?;
    }
    tx.commit().map_err(|error| {
        sql_error(
            "Manuskriptanalysejob konnte nicht abgeschlossen werden",
            error,
        )
    })?;
    db.query_row("SELECT id, project_id, book_id, import_reference, status, current_phase, phase_progress_json, phase_errors_json, total_units, completed_units, failed_units, current_unit_id, last_successful_unit_id, provider_id, page_markers_json, created_at, updated_at, completed_at, error_message FROM manuscript_analysis_jobs WHERE id=?1", params![job_id], manuscript_analysis_job_from_row).map_err(|error| sql_error("Manuskriptanalysejob konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn list_manuscript_analysis_jobs(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<ManuscriptAnalysisJob>, String> {
    let db = lock_db(&state)?;
    let result = db.prepare("SELECT id, project_id, book_id, import_reference, status, current_phase, phase_progress_json, phase_errors_json, total_units, completed_units, failed_units, current_unit_id, last_successful_unit_id, provider_id, page_markers_json, created_at, updated_at, completed_at, error_message FROM manuscript_analysis_jobs WHERE project_id=?1 ORDER BY updated_at DESC").map_err(|error| sql_error("Manuskriptanalysejobs konnten nicht geladen werden", error))?.query_map(params![project_id], manuscript_analysis_job_from_row).map_err(|error| sql_error("Manuskriptanalysejobs konnten nicht gelesen werden", error))?.collect::<SqlResult<Vec<_>>>().map_err(|error| sql_error("Manuskriptanalysejobs konnten nicht gelesen werden", error));
    result
}

#[tauri::command]
pub fn get_manuscript_analysis_job(
    state: State<'_, DbState>,
    id: String,
) -> Result<ManuscriptAnalysisJob, String> {
    let db = lock_db(&state)?;
    db.query_row("SELECT id, project_id, book_id, import_reference, status, current_phase, phase_progress_json, phase_errors_json, total_units, completed_units, failed_units, current_unit_id, last_successful_unit_id, provider_id, page_markers_json, created_at, updated_at, completed_at, error_message FROM manuscript_analysis_jobs WHERE id=?1", params![id], manuscript_analysis_job_from_row).map_err(|error| sql_error("Manuskriptanalysejob konnte nicht geladen werden", error))
}

fn load_manuscript_analysis_job(
    db: &Connection,
    id: &str,
) -> Result<ManuscriptAnalysisJob, String> {
    db.query_row("SELECT id, project_id, book_id, import_reference, status, current_phase, phase_progress_json, phase_errors_json, total_units, completed_units, failed_units, current_unit_id, last_successful_unit_id, provider_id, page_markers_json, created_at, updated_at, completed_at, error_message FROM manuscript_analysis_jobs WHERE id=?1", params![id], manuscript_analysis_job_from_row).map_err(|error| sql_error("Manuskriptanalysejob konnte nicht geladen werden", error))
}

fn load_manuscript_analysis_job_for_project(
    db: &Connection,
    id: &str,
    project_id: &str,
) -> Result<ManuscriptAnalysisJob, String> {
    let job = load_manuscript_analysis_job(db, id)?;
    if job.project_id != project_id {
        return Err("Der Analysejob gehört nicht zum Projekt.".into());
    }
    Ok(job)
}

#[tauri::command]
pub fn list_manuscript_analysis_units(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<ManuscriptAnalysisUnit>, String> {
    let db = lock_db(&state)?;
    let result = db.prepare("SELECT id, job_id, project_id, chapter_id, scene_id, order_index, page_number, start_offset, end_offset, content, content_hash, status, retry_count, continuity_run_id, requested_provider, actual_provider, prompt_version, input_hash, output_hash, error_code, error_message, created_at, updated_at, completed_at FROM manuscript_analysis_units WHERE job_id=?1 ORDER BY order_index ASC").map_err(|error| sql_error("Manuskriptprüfeinheiten konnten nicht geladen werden", error))?.query_map(params![job_id], manuscript_analysis_unit_from_row).map_err(|error| sql_error("Manuskriptprüfeinheiten konnten nicht gelesen werden", error))?.collect::<SqlResult<Vec<_>>>().map_err(|error| sql_error("Manuskriptprüfeinheiten konnten nicht gelesen werden", error));
    result
}

#[tauri::command]
pub fn update_manuscript_analysis_job(
    state: State<'_, DbState>,
    input: UpdateManuscriptAnalysisJobInput,
) -> Result<ManuscriptAnalysisJob, String> {
    if !manuscript_analysis_job_status_valid(&input.status) {
        return Err("Ungültiger Status des Manuskriptanalysejobs.".into());
    }
    let db = lock_db(&state)?;
    let completed_at = if matches!(input.status.as_str(), "completed" | "failed" | "cancelled") {
        Some(input.completed_at.unwrap_or_else(now))
    } else {
        None
    };
    let phase_progress = input
        .phase_progress
        .as_ref()
        .map(serde_json::Value::to_string);
    let phase_errors = input
        .phase_errors
        .as_ref()
        .map(serde_json::Value::to_string);
    db.execute("UPDATE manuscript_analysis_jobs SET status=?2, current_phase=COALESCE(?3,current_phase), phase_progress_json=COALESCE(?4,phase_progress_json), phase_errors_json=COALESCE(?5,phase_errors_json), current_unit_id=?6, last_successful_unit_id=COALESCE(?7,last_successful_unit_id), error_message=?8, completed_at=?9, completed_units=(SELECT COUNT(*) FROM manuscript_analysis_units WHERE job_id=?1 AND status IN ('completed','skipped')), failed_units=(SELECT COUNT(*) FROM manuscript_analysis_units WHERE job_id=?1 AND status='failed'), updated_at=?10 WHERE id=?1", params![input.id, input.status, input.current_phase, phase_progress, phase_errors, input.current_unit_id, input.last_successful_unit_id, input.error_message, completed_at, now()]).map_err(|error| sql_error("Manuskriptanalysejob konnte nicht aktualisiert werden", error))?;
    db.query_row("SELECT id, project_id, book_id, import_reference, status, current_phase, phase_progress_json, phase_errors_json, total_units, completed_units, failed_units, current_unit_id, last_successful_unit_id, provider_id, page_markers_json, created_at, updated_at, completed_at, error_message FROM manuscript_analysis_jobs WHERE id=?1", params![input.id], manuscript_analysis_job_from_row).map_err(|error| sql_error("Manuskriptanalysejob konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn update_manuscript_analysis_unit(
    state: State<'_, DbState>,
    input: UpdateManuscriptAnalysisUnitInput,
) -> Result<ManuscriptAnalysisUnit, String> {
    if !manuscript_analysis_unit_status_valid(&input.status) {
        return Err("Ungültiger Status der Manuskriptprüfeinheit.".into());
    }
    let db = lock_db(&state)?;
    let completed_at = if matches!(input.status.as_str(), "completed" | "skipped") {
        Some(input.completed_at.unwrap_or_else(now))
    } else {
        None
    };
    db.execute("UPDATE manuscript_analysis_units SET status=?2, retry_count=COALESCE(?3,retry_count), continuity_run_id=?4, requested_provider=COALESCE(?5,requested_provider), actual_provider=COALESCE(?6,actual_provider), prompt_version=COALESCE(?7,prompt_version), input_hash=COALESCE(?8,input_hash), output_hash=COALESCE(?9,output_hash), error_code=?10, content=COALESCE(?11,content), content_hash=COALESCE(?12,content_hash), error_message=?13, completed_at=?14, updated_at=?15 WHERE id=?1", params![input.id, input.status, input.retry_count, input.continuity_run_id, input.requested_provider, input.actual_provider, input.prompt_version, input.input_hash, input.output_hash, input.error_code, input.content, input.content_hash, input.error_message, completed_at, now()]).map_err(|error| sql_error("Manuskriptprüfeinheit konnte nicht aktualisiert werden", error))?;
    db.query_row("SELECT id, job_id, project_id, chapter_id, scene_id, order_index, page_number, start_offset, end_offset, content, content_hash, status, retry_count, continuity_run_id, requested_provider, actual_provider, prompt_version, input_hash, output_hash, error_code, error_message, created_at, updated_at, completed_at FROM manuscript_analysis_units WHERE id=?1", params![input.id], manuscript_analysis_unit_from_row).map_err(|error| sql_error("Manuskriptprüfeinheit konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn list_manuscript_analysis_draft_ledger(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<ManuscriptAnalysisDraftLedgerEntry>, String> {
    let db = lock_db(&state)?;
    let result = db.prepare("SELECT d.id, d.job_id, d.unit_id, d.project_id, d.entity_id, d.related_entity_id, d.state_kind, d.previous_state, d.new_state, d.chapter_id, d.scene_id, d.start_offset, d.end_offset, d.source_excerpt, d.source_reference_id, d.confidence, d.status, d.created_at, d.updated_at FROM manuscript_analysis_draft_ledger d JOIN manuscript_analysis_units u ON u.id=d.unit_id WHERE d.job_id=?1 ORDER BY u.order_index ASC, d.created_at ASC").map_err(|error| sql_error("Import-Draft-Ledger konnte nicht geladen werden", error))?.query_map(params![job_id], manuscript_analysis_draft_from_row).map_err(|error| sql_error("Import-Draft-Ledger konnte nicht gelesen werden", error))?.collect::<SqlResult<Vec<_>>>().map_err(|error| sql_error("Import-Draft-Ledger konnte nicht gelesen werden", error));
    result
}

#[tauri::command]
pub fn replace_manuscript_analysis_draft_ledger(
    state: State<'_, DbState>,
    unit_id: String,
    entries: Vec<SaveManuscriptAnalysisDraftLedgerInput>,
) -> Result<Vec<ManuscriptAnalysisDraftLedgerEntry>, String> {
    let db = lock_db(&state)?;
    let (job_id, project_id): (String, String) = db
        .query_row(
            "SELECT job_id, project_id FROM manuscript_analysis_units WHERE id=?1",
            params![unit_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| sql_error("Import-Prüfeinheit konnte nicht geladen werden", error))?;
    let tx = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Import-Draft-Ledger konnte nicht gespeichert werden", error))?;
    tx.execute(
        "DELETE FROM manuscript_analysis_draft_ledger WHERE unit_id=?1",
        params![unit_id],
    )
    .map_err(|error| sql_error("Alte Import-Zustände konnten nicht entfernt werden", error))?;
    let stamp = now();
    for entry in &entries {
        if entry.job_id != job_id || entry.unit_id != unit_id || entry.project_id != project_id {
            return Err("Import-Draft-Ledger und Prüfeinheit gehören nicht zusammen.".into());
        }
        if !manuscript_analysis_draft_status_valid(entry.status.as_deref().unwrap_or("proposed")) {
            return Err("Ungültiger Status des Import-Draft-Ledgers.".into());
        }
        validate_continuity_state_kind(&entry.state_kind)?;
        validate_probability(entry.confidence, "Die Draft-Ledger-Sicherheit")?;
        let valid_entity: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM story_entities WHERE id=?1 AND project_id=?2) OR EXISTS(SELECT 1 FROM provisional_entities WHERE id=?1 AND project_id=?2 AND job_id=?3)", params![entry.entity_id, entry.project_id, entry.job_id], |row| row.get(0)).map_err(|error| sql_error("Import-Draft-Entität konnte nicht geprüft werden", error))?;
        if !valid_entity {
            return Err("Import-Draft-Entität gehört nicht zum Projekt oder Analysejob.".into());
        }
        if let Some(related) = &entry.related_entity_id {
            let valid_related: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM story_entities WHERE id=?1 AND project_id=?2) OR EXISTS(SELECT 1 FROM provisional_entities WHERE id=?1 AND project_id=?2 AND job_id=?3)", params![related, entry.project_id, entry.job_id], |row| row.get(0)).map_err(|error| sql_error("Import-Draft-Relation konnte nicht geprüft werden", error))?;
            if !valid_related {
                return Err(
                    "Import-Draft-Relation gehört nicht zum Projekt oder Analysejob.".into(),
                );
            }
        }
        if let Some(source_id) = &entry.source_reference_id {
            let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2 UNION ALL SELECT 1 FROM project_source_references WHERE id=?1 AND project_id=?2)", params![source_id, entry.project_id], |row| row.get(0)).map_err(|error| sql_error("Import-Draft-Quelle konnte nicht geprüft werden", error))?;
            if !valid {
                return Err("Die Import-Draft-Quelle gehört nicht zum Projekt.".into());
            }
        }
        tx.execute("INSERT INTO manuscript_analysis_draft_ledger (id, job_id, unit_id, project_id, entity_id, related_entity_id, state_kind, previous_state, new_state, chapter_id, scene_id, start_offset, end_offset, source_excerpt, source_reference_id, confidence, status, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?18)", params![entry.id.clone().unwrap_or_else(new_id), entry.job_id, entry.unit_id, entry.project_id, entry.entity_id, entry.related_entity_id, entry.state_kind, entry.previous_state, entry.new_state, entry.chapter_id, entry.scene_id, entry.start_offset, entry.end_offset, entry.source_excerpt, entry.source_reference_id, entry.confidence, entry.status.clone().unwrap_or_else(|| "proposed".into()), stamp]).map_err(|error| sql_error("Import-Draft-Ledger konnte nicht gespeichert werden", error))?;
    }
    tx.commit().map_err(|error| {
        sql_error(
            "Import-Draft-Ledger konnte nicht abgeschlossen werden",
            error,
        )
    })?;
    let result = db.prepare("SELECT id, job_id, unit_id, project_id, entity_id, related_entity_id, state_kind, previous_state, new_state, chapter_id, scene_id, start_offset, end_offset, source_excerpt, source_reference_id, confidence, status, created_at, updated_at FROM manuscript_analysis_draft_ledger WHERE unit_id=?1 ORDER BY created_at ASC").map_err(|error| sql_error("Import-Draft-Ledger konnte nicht geladen werden", error))?.query_map(params![unit_id], manuscript_analysis_draft_from_row).map_err(|error| sql_error("Import-Draft-Ledger konnte nicht gelesen werden", error))?.collect::<SqlResult<Vec<_>>>().map_err(|error| sql_error("Import-Draft-Ledger konnte nicht gelesen werden", error));
    result
}

#[tauri::command]
pub fn review_manuscript_analysis_draft_ledger(
    state: State<'_, DbState>,
    id: String,
    status: String,
) -> Result<ManuscriptAnalysisDraftLedgerEntry, String> {
    if !manuscript_analysis_draft_status_valid(&status) {
        return Err("Ungültiger Status des Import-Draft-Ledgers.".into());
    }
    let db = lock_db(&state)?;
    let draft: ManuscriptAnalysisDraftLedgerEntry = db.query_row("SELECT id, job_id, unit_id, project_id, entity_id, related_entity_id, state_kind, previous_state, new_state, chapter_id, scene_id, start_offset, end_offset, source_excerpt, source_reference_id, confidence, status, created_at, updated_at FROM manuscript_analysis_draft_ledger WHERE id=?1", params![id], manuscript_analysis_draft_from_row).map_err(|error| sql_error("Import-Draft-Ledger konnte nicht geladen werden", error))?;
    if status == "confirmed" {
        let source_id = draft.source_reference_id.clone().ok_or_else(|| {
            "Ein bestätigter Importzustand benötigt eine Source Reference.".to_string()
        })?;
        let source_valid: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2 UNION ALL SELECT 1 FROM project_source_references WHERE id=?1 AND project_id=?2)", params![source_id, draft.project_id], |row| row.get(0)).map_err(|error| sql_error("Import-Draft-Quelle konnte nicht geprüft werden", error))?;
        if !source_valid {
            return Err("Die Import-Draft-Quelle gehört nicht zum Projekt.".into());
        }
        project_entity_exists(&db, &draft.project_id, &draft.entity_id, None)?;
        let stamp = now();
        db.execute("INSERT INTO continuity_state_ledger (id, project_id, entity_id, related_entity_id, state_kind, previous_state, new_state, reason, evidence_excerpt, chapter_id, scene_id, start_offset, end_offset, source_reference_id, status, confidence, author_confirmed, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'confirmed',?15,1,?16,?16) ON CONFLICT(id) DO UPDATE SET status='confirmed', author_confirmed=1, updated_at=excluded.updated_at", params![format!("import-state-{}", draft.id), draft.project_id, draft.entity_id, draft.related_entity_id, draft.state_kind, draft.previous_state, draft.new_state, format!("Import-Draft bestätigt; job:{}; unit:{}; draft:{}", draft.job_id, draft.unit_id, draft.id), draft.source_excerpt, draft.chapter_id, draft.scene_id, draft.start_offset, draft.end_offset, source_id, draft.confidence, stamp]).map_err(|error| sql_error("Import-Zustand konnte nicht bestätigt werden", error))?;
    }
    db.execute(
        "UPDATE manuscript_analysis_draft_ledger SET status=?2, updated_at=?3 WHERE id=?1",
        params![id, status, now()],
    )
    .map_err(|error| sql_error("Import-Draft-Ledger konnte nicht geprüft werden", error))?;
    db.query_row("SELECT id, job_id, unit_id, project_id, entity_id, related_entity_id, state_kind, previous_state, new_state, chapter_id, scene_id, start_offset, end_offset, source_excerpt, source_reference_id, confidence, status, created_at, updated_at FROM manuscript_analysis_draft_ledger WHERE id=?1", params![id], manuscript_analysis_draft_from_row).map_err(|error| sql_error("Import-Draft-Ledger konnte nicht geladen werden", error))
}

fn phase_result_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ManuscriptAnalysisPhaseResult> {
    Ok(ManuscriptAnalysisPhaseResult {
        id: row.get(0)?,
        job_id: row.get(1)?,
        project_id: row.get(2)?,
        phase: row.get(3)?,
        result_kind: row.get(4)?,
        payload: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or(serde_json::Value::Null),
        content_hash: row.get(6)?,
        provider_id: row.get(7)?,
        prompt_version: row.get(8)?,
        review_status: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

#[tauri::command]
pub fn save_manuscript_analysis_phase_result(
    state: State<'_, DbState>,
    input: SaveManuscriptAnalysisPhaseResultInput,
) -> Result<ManuscriptAnalysisPhaseResult, String> {
    if !matches!(
        input.review_status.as_deref().unwrap_or("pending"),
        "pending" | "confirmed" | "rejected" | "uncertain" | "skipped"
    ) || input.payload.to_string().chars().count() > 500_000
    {
        return Err("Ungültiges oder zu großes Phasenergebnis.".into());
    }
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job_for_project(&db, &input.job_id, &input.project_id)?;
    let id = input.id.clone().unwrap_or_else(new_id);
    let stamp = now();
    db.execute("INSERT INTO manuscript_analysis_phase_results(id,job_id,project_id,phase,result_kind,payload_json,content_hash,provider_id,prompt_version,review_status,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,COALESCE((SELECT created_at FROM manuscript_analysis_phase_results WHERE id=?1),?11),?11) ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json,content_hash=excluded.content_hash,provider_id=excluded.provider_id,prompt_version=excluded.prompt_version,review_status=excluded.review_status,updated_at=excluded.updated_at", params![id,input.job_id,job.project_id,input.phase,input.result_kind,input.payload.to_string(),input.content_hash,input.provider_id,input.prompt_version,input.review_status.unwrap_or_else(|| "pending".into()),stamp]).map_err(|e| sql_error("Strukturiertes Phasenergebnis konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,job_id,project_id,phase,result_kind,payload_json,content_hash,provider_id,prompt_version,review_status,created_at,updated_at FROM manuscript_analysis_phase_results WHERE id=?1", params![id], phase_result_from_row).map_err(|e| sql_error("Strukturiertes Phasenergebnis konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn list_manuscript_analysis_phase_results(
    state: State<'_, DbState>,
    job_id: String,
    phase: Option<String>,
) -> Result<Vec<ManuscriptAnalysisPhaseResult>, String> {
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &job_id)?;
    let mut statement = db.prepare("SELECT id,job_id,project_id,phase,result_kind,payload_json,content_hash,provider_id,prompt_version,review_status,created_at,updated_at FROM manuscript_analysis_phase_results WHERE job_id=?1 AND project_id=?2 AND (?3 IS NULL OR phase=?3) ORDER BY updated_at").map_err(|e| sql_error("Phasenergebnisse konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(
            params![job_id, job.project_id, phase],
            phase_result_from_row,
        )
        .map_err(|e| sql_error("Phasenergebnisse konnten nicht gelesen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Phasenergebnisse konnten nicht gelesen werden", e));
    result
}

fn artifact_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ManuscriptAnalysisArtifact> {
    Ok(ManuscriptAnalysisArtifact {
        id: row.get(0)?,
        job_id: row.get(1)?,
        project_id: row.get(2)?,
        phase: row.get(3)?,
        unit_id: row.get(4)?,
        artifact_type: row.get(5)?,
        artifact_id: row.get(6)?,
        review_status: row.get(7)?,
        explicitly_skipped: row.get::<_, i64>(8)? != 0,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

#[tauri::command]
pub fn save_manuscript_analysis_artifacts(
    state: State<'_, DbState>,
    job_id: String,
    artifacts: Vec<SaveManuscriptAnalysisArtifactInput>,
) -> Result<Vec<ManuscriptAnalysisArtifact>, String> {
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &job_id)?;
    let tx = db
        .unchecked_transaction()
        .map_err(|e| sql_error("Analyseartefakte konnten nicht gestartet werden", e))?;
    let stamp = now();
    for input in artifacts {
        if input.job_id != job_id
            || input.project_id != job.project_id
            || input.artifact_id.trim().is_empty()
            || !matches!(
                input.review_status.as_deref().unwrap_or("pending"),
                "pending" | "confirmed" | "rejected" | "uncertain" | "skipped"
            )
        {
            return Err("Ein Analyseartefakt ist ungültig oder projektfremd.".into());
        }
        let id = input.id.unwrap_or_else(new_id);
        tx.execute("INSERT INTO manuscript_analysis_artifacts(id,job_id,project_id,phase,unit_id,artifact_type,artifact_id,review_status,explicitly_skipped,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,COALESCE((SELECT created_at FROM manuscript_analysis_artifacts WHERE id=?1),?10),?10) ON CONFLICT(job_id,artifact_type,artifact_id) DO UPDATE SET phase=excluded.phase,unit_id=excluded.unit_id,review_status=excluded.review_status,explicitly_skipped=excluded.explicitly_skipped,updated_at=excluded.updated_at", params![id,input.job_id,input.project_id,input.phase,input.unit_id,input.artifact_type,input.artifact_id,input.review_status.unwrap_or_else(|| "pending".into()),input.explicitly_skipped.unwrap_or(false) as i64,stamp]).map_err(|e| sql_error("Analyseartefakt konnte nicht gespeichert werden", e))?;
    }
    tx.commit()
        .map_err(|e| sql_error("Analyseartefakte konnten nicht abgeschlossen werden", e))?;
    let mut statement = db.prepare("SELECT id,job_id,project_id,phase,unit_id,artifact_type,artifact_id,review_status,explicitly_skipped,created_at,updated_at FROM manuscript_analysis_artifacts WHERE job_id=?1 AND project_id=?2 ORDER BY created_at").map_err(|e| sql_error("Analyseartefakte konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(params![job_id, job.project_id], artifact_from_row)
        .map_err(|e| sql_error("Analyseartefakte konnten nicht gelesen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Analyseartefakte konnten nicht gelesen werden", e));
    result
}

#[tauri::command]
pub fn list_manuscript_analysis_artifacts(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<ManuscriptAnalysisArtifact>, String> {
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &job_id)?;
    let mut statement = db.prepare("SELECT id,job_id,project_id,phase,unit_id,artifact_type,artifact_id,review_status,explicitly_skipped,created_at,updated_at FROM manuscript_analysis_artifacts WHERE job_id=?1 AND project_id=?2 ORDER BY created_at").map_err(|e| sql_error("Analyseartefakte konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(params![job_id, job.project_id], artifact_from_row)
        .map_err(|e| sql_error("Analyseartefakte konnten nicht gelesen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Analyseartefakte konnten nicht gelesen werden", e));
    result
}

#[tauri::command]
pub fn review_manuscript_analysis_artifact(
    state: State<'_, DbState>,
    id: String,
    status: String,
    explicitly_skipped: Option<bool>,
) -> Result<ManuscriptAnalysisArtifact, String> {
    if !matches!(
        status.as_str(),
        "pending" | "confirmed" | "rejected" | "uncertain" | "skipped"
    ) {
        return Err("Ungültiger Artefaktstatus.".into());
    }
    let db = lock_db(&state)?;
    let changed = db.execute("UPDATE manuscript_analysis_artifacts SET review_status=?1,explicitly_skipped=?2,updated_at=?3 WHERE id=?4", params![status,explicitly_skipped.unwrap_or(status == "skipped") as i64,now(),id]).map_err(|e| sql_error("Artefaktreview konnte nicht gespeichert werden", e))?;
    if changed == 0 {
        return Err("Analyseartefakt nicht gefunden.".into());
    }
    db.query_row("SELECT id,job_id,project_id,phase,unit_id,artifact_type,artifact_id,review_status,explicitly_skipped,created_at,updated_at FROM manuscript_analysis_artifacts WHERE id=?1", params![id], artifact_from_row).map_err(|e| sql_error("Artefakt konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn review_manuscript_analysis_artifact_decision(
    state: State<'_, DbState>,
    id: String,
    status: String,
    explicitly_skipped: Option<bool>,
) -> Result<ManuscriptAnalysisArtifact, String> {
    if !matches!(
        status.as_str(),
        "confirmed" | "rejected" | "uncertain" | "skipped"
    ) {
        return Err("Ungültiger fachlicher Artefaktstatus.".into());
    }
    let db = lock_db(&state)?;
    let artifact = db.query_row("SELECT id,job_id,project_id,phase,unit_id,artifact_type,artifact_id,review_status,explicitly_skipped,created_at,updated_at FROM manuscript_analysis_artifacts WHERE id=?1", params![id], artifact_from_row).map_err(|e| sql_error("Analyseartefakt konnte nicht geladen werden", e))?;
    if artifact.review_status != "pending" && !explicitly_skipped.unwrap_or(false) {
        return Err("Dieses Analyseartefakt wurde bereits entschieden.".into());
    }
    let transaction = db
        .unchecked_transaction()
        .map_err(|e| sql_error("Fachlicher Review konnte nicht gestartet werden", e))?;
    let domain_status = if status == "confirmed" {
        "confirmed"
    } else if status == "uncertain" {
        "uncertain"
    } else {
        "rejected"
    };
    match artifact.artifact_type.as_str() {
        "bible_proposal"
        | "character_memory_proposal"
        | "project_rule_proposal"
        | "plot_thread_proposal" => {
            let table = match artifact.artifact_type.as_str() {
                "bible_proposal" => "bible_proposals",
                "character_memory_proposal" => "character_memory_proposals",
                "project_rule_proposal" => "project_rule_proposals",
                _ => "plot_thread_lifecycle_proposals",
            };
            let proposal_status = if status == "confirmed" {
                "accepted"
            } else if status == "rejected" || status == "skipped" {
                "rejected"
            } else {
                "pending"
            };
            let changed = transaction
                .execute(
                    &format!("UPDATE {table} SET review_status=?1 WHERE id=?2 AND project_id=?3"),
                    params![proposal_status, artifact.artifact_id, artifact.project_id],
                )
                .map_err(|e| sql_error("Vorschlag konnte nicht aktualisiert werden", e))?;
            if changed == 0 {
                return Err("Zugehöriger Vorschlag wurde nicht gefunden.".into());
            }
        }
        "continuity_finding" | "global_countercheck_finding" => {
            let changed = transaction.execute("UPDATE continuity_review_findings SET review_status=?1,user_decision=?2,updated_at=?3 WHERE id=?4 AND project_id=?5", params![if status == "confirmed" { "accepted" } else if status == "uncertain" { "deferred" } else { "dismissed" }, if status == "confirmed" { "Bestätigung gespeichert; konkrete Finding-Entscheidung bleibt erforderlich." } else { status.as_str() }, now(), artifact.artifact_id, artifact.project_id]).map_err(|e| sql_error("Finding konnte nicht aktualisiert werden", e))?;
            if changed == 0 {
                return Err("Zugehöriges Finding wurde nicht gefunden.".into());
            }
        }
        "import_draft_state" => {
            let changed = transaction.execute("UPDATE manuscript_analysis_draft_ledger SET status=?,updated_at=? WHERE id=? AND project_id=?", params![domain_status, now(), artifact.artifact_id, artifact.project_id]).map_err(|e| sql_error("Draft-Zustand konnte nicht aktualisiert werden", e))?;
            if changed == 0 {
                return Err("Zugehöriger Draft-Zustand wurde nicht gefunden.".into());
            }
        }
        "narrative_summary" => {
            let changed = transaction.execute("UPDATE narrative_summaries SET status=?,author_confirmed=?,updated_at=? WHERE id=? AND project_id=?", params![if status == "confirmed" { "confirmed" } else if status == "rejected" || status == "skipped" { "rejected" } else { "proposed" }, (status == "confirmed") as i64, now(), artifact.artifact_id, artifact.project_id]).map_err(|e| sql_error("Zusammenfassung konnte nicht aktualisiert werden", e))?;
            if changed == 0 {
                return Err("Zugehörige Zusammenfassung wurde nicht gefunden.".into());
            }
        }
        "timeline_event" => {
            if status == "confirmed" {
                let ids: String = transaction.query_row("SELECT participating_entity_ids_json FROM manuscript_timeline_events WHERE id=?1 AND project_id=?2", params![artifact.artifact_id, artifact.project_id], |row| row.get(0)).map_err(|e| sql_error("Timeline-Entität konnte nicht geprüft werden", e))?;
                let has_provisional: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM provisional_entities WHERE job_id=?1 AND project_id=?2 AND instr(?3, id) > 0)", params![artifact.job_id, artifact.project_id, ids], |row| row.get(0)).map_err(|e| sql_error("Timeline-Entität konnte nicht geprüft werden", e))?;
                if has_provisional {
                    return Err("Timeline-Ereignis muss vor der Bestätigung materialisierte Entitäten verwenden.".into());
                }
            }
            let changed = transaction.execute("UPDATE manuscript_timeline_events SET status=?,author_confirmed=?,updated_at=? WHERE id=? AND project_id=?", params![domain_status, (status == "confirmed") as i64, now(), artifact.artifact_id, artifact.project_id]).map_err(|e| sql_error("Timeline-Ereignis konnte nicht aktualisiert werden", e))?;
            if changed == 0 {
                return Err("Zugehöriges Timeline-Ereignis wurde nicht gefunden.".into());
            }
        }
        "story_graph_edge" => {
            if status == "confirmed" {
                let ids: (String, String) = transaction.query_row("SELECT source_entity_id,target_entity_id FROM story_graph_edges WHERE id=?1 AND project_id=?2", params![artifact.artifact_id, artifact.project_id], |row| Ok((row.get(0)?, row.get(1)?))).map_err(|e| sql_error("Graph-Entität konnte nicht geprüft werden", e))?;
                let has_provisional: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM provisional_entities WHERE job_id=?1 AND project_id=?2 AND (id=?3 OR id=?4))", params![artifact.job_id, artifact.project_id, ids.0, ids.1], |row| row.get(0)).map_err(|e| sql_error("Graph-Entität konnte nicht geprüft werden", e))?;
                if has_provisional {
                    return Err("Story-Graph-Kante muss vor der Bestätigung materialisierte Entitäten verwenden.".into());
                }
            }
            let changed = transaction.execute("UPDATE story_graph_edges SET status=?,author_confirmed=?,updated_at=? WHERE id=? AND project_id=?", params![domain_status, (status == "confirmed") as i64, now(), artifact.artifact_id, artifact.project_id]).map_err(|e| sql_error("Story-Graph-Kante konnte nicht aktualisiert werden", e))?;
            if changed == 0 {
                return Err("Zugehörige Story-Graph-Kante wurde nicht gefunden.".into());
            }
        }
        "provisional_entity" => {
            if status == "confirmed" {
                let existing_entity_id: Option<String> = transaction.query_row("SELECT existing_entity_id FROM provisional_entities WHERE id=?1 AND job_id=?2 AND project_id=?3", params![artifact.artifact_id, artifact.job_id, artifact.project_id], |row| row.get(0)).map_err(|e| sql_error("Vorläufige Entität konnte nicht geprüft werden", e))?;
                let input = MaterializeProvisionalEntityInput {
                    project_id: artifact.project_id.clone(),
                    job_id: artifact.job_id.clone(),
                    provisional_entity_id: artifact.artifact_id.clone(),
                    existing_entity_id: existing_entity_id.clone(),
                    decision: if existing_entity_id.is_some() {
                        "merge".into()
                    } else {
                        "accept".into()
                    },
                };
                materialize_provisional_entity_in_transaction(&transaction, &input)?;
            } else {
                let changed = transaction.execute("UPDATE provisional_entities SET review_status=?,updated_at=? WHERE id=? AND job_id=? AND project_id=?", params![if status == "uncertain" { "uncertain" } else { "rejected" }, now(), artifact.artifact_id, artifact.job_id, artifact.project_id]).map_err(|e| sql_error("Vorläufige Entität konnte nicht aktualisiert werden", e))?;
                if changed == 0 {
                    return Err("Zugehörige vorläufige Entität wurde nicht gefunden.".into());
                }
            }
        }
        "provisional_merge" => {
            if status == "confirmed" {
                let (left_id, existing_entity_id): (String, Option<String>) = transaction.query_row("SELECT left_provisional_entity_id,existing_entity_id FROM provisional_merge_proposals WHERE id=?1 AND job_id=?2 AND project_id=?3", params![artifact.artifact_id, artifact.job_id, artifact.project_id], |row| Ok((row.get(0)?, row.get(1)?))).map_err(|e| sql_error("Merge-Vorschlag konnte nicht geprüft werden", e))?;
                let existing_entity_id = existing_entity_id.ok_or_else(|| {
                    "Ein Merge muss vor der Bestätigung ein bestehendes Ziel auswählen.".to_string()
                })?;
                let input = MaterializeProvisionalEntityInput {
                    project_id: artifact.project_id.clone(),
                    job_id: artifact.job_id.clone(),
                    provisional_entity_id: left_id,
                    existing_entity_id: Some(existing_entity_id),
                    decision: "merge".into(),
                };
                materialize_provisional_entity_in_transaction(&transaction, &input)?;
                transaction.execute("UPDATE provisional_merge_proposals SET review_status='merged' WHERE id=?1 AND job_id=?2 AND project_id=?3", params![artifact.artifact_id, artifact.job_id, artifact.project_id]).map_err(|e| sql_error("Merge-Status konnte nicht aktualisiert werden", e))?;
            } else {
                let changed = transaction.execute("UPDATE provisional_merge_proposals SET review_status=? WHERE id=? AND job_id=? AND project_id=?", params![if status == "uncertain" { "uncertain" } else { "rejected" }, artifact.artifact_id, artifact.job_id, artifact.project_id]).map_err(|e| sql_error("Merge-Vorschlag konnte nicht aktualisiert werden", e))?;
                if changed == 0 {
                    return Err("Zugehöriger Merge-Vorschlag wurde nicht gefunden.".into());
                }
            }
        }
        "book_end_state_proposal" => {}
        other => return Err(format!("Unbekannter fachlicher Artefakttyp: {other}")),
    }
    transaction.execute("UPDATE manuscript_analysis_artifacts SET review_status=?1,explicitly_skipped=?2,updated_at=?3 WHERE id=?4 AND job_id=?5 AND project_id=?6", params![status, (explicitly_skipped.unwrap_or(false) || status == "skipped") as i64, now(), artifact.id, artifact.job_id, artifact.project_id]).map_err(|e| sql_error("Artefaktreview konnte nicht gespeichert werden", e))?;
    transaction
        .commit()
        .map_err(|e| sql_error("Fachlicher Review konnte nicht abgeschlossen werden", e))?;
    db.query_row("SELECT id,job_id,project_id,phase,unit_id,artifact_type,artifact_id,review_status,explicitly_skipped,created_at,updated_at FROM manuscript_analysis_artifacts WHERE id=?1", params![id], artifact_from_row).map_err(|e| sql_error("Artefakt konnte nicht geladen werden", e))
}

fn audit_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ManuscriptAnalysisReviewAudit> {
    Ok(ManuscriptAnalysisReviewAudit {
        id: row.get(0)?,
        job_id: row.get(1)?,
        project_id: row.get(2)?,
        action: row.get(3)?,
        artifact_ids: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
        artifact_types: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
        note: row.get(6)?,
        created_at: row.get(7)?,
    })
}

#[tauri::command]
pub fn save_manuscript_analysis_review_audit(
    state: State<'_, DbState>,
    input: SaveManuscriptAnalysisReviewAuditInput,
) -> Result<ManuscriptAnalysisReviewAudit, String> {
    if !matches!(
        input.action.as_str(),
        "skip_open_artifacts" | "complete_review"
    ) || input.note.trim().is_empty()
    {
        return Err("Ungültiger Review-Audit.".into());
    }
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &input.job_id)?;
    let ids = serde_json::to_string(&input.artifact_ids).unwrap_or_else(|_| "[]".into());
    let types = serde_json::to_string(&input.artifact_types).unwrap_or_else(|_| "[]".into());
    let id = input.id.unwrap_or_else(new_id);
    db.execute("INSERT INTO manuscript_analysis_review_audits(id,job_id,project_id,action,artifact_ids_json,artifact_types_json,note,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", params![id,input.job_id,job.project_id,input.action,ids,types,input.note,now()]).map_err(|e| sql_error("Review-Audit konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,job_id,project_id,action,artifact_ids_json,artifact_types_json,note,created_at FROM manuscript_analysis_review_audits WHERE id=?1", params![id], audit_from_row).map_err(|e| sql_error("Review-Audit konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn list_manuscript_analysis_review_audits(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<ManuscriptAnalysisReviewAudit>, String> {
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &job_id)?;
    let mut statement = db.prepare("SELECT id,job_id,project_id,action,artifact_ids_json,artifact_types_json,note,created_at FROM manuscript_analysis_review_audits WHERE job_id=?1 AND project_id=?2 ORDER BY created_at").map_err(|e| sql_error("Review-Audits konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(params![job_id, job.project_id], audit_from_row)
        .map_err(|e| sql_error("Review-Audits konnten nicht gelesen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Review-Audits konnten nicht gelesen werden", e));
    result
}

fn completion_report_from_row(
    row: &rusqlite::Row<'_>,
) -> SqlResult<ManuscriptAnalysisCompletionReport> {
    Ok(ManuscriptAnalysisCompletionReport {
        id: row.get(0)?,
        job_id: row.get(1)?,
        project_id: row.get(2)?,
        content_hash: row.get(3)?,
        payload: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or(serde_json::Value::Null),
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

#[tauri::command]
pub fn save_manuscript_analysis_completion_report(
    state: State<'_, DbState>,
    input: SaveManuscriptAnalysisCompletionReportInput,
) -> Result<ManuscriptAnalysisCompletionReport, String> {
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &input.job_id)?;
    if job.project_id != input.project_id {
        return Err("Abschlussbericht gehört nicht zum Projekt.".into());
    }
    let id = input.id.unwrap_or_else(new_id);
    let payload = serde_json::to_string(&input.payload)
        .map_err(|e| format!("Abschlussbericht konnte nicht serialisiert werden: {e}"))?;
    let stamp = now();
    db.execute("INSERT INTO manuscript_analysis_completion_reports(id,job_id,project_id,content_hash,payload_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?6) ON CONFLICT(job_id) DO UPDATE SET content_hash=excluded.content_hash,payload_json=excluded.payload_json,updated_at=excluded.updated_at", params![id,input.job_id,input.project_id,input.content_hash,payload,stamp]).map_err(|e| sql_error("Abschlussbericht konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,job_id,project_id,content_hash,payload_json,created_at,updated_at FROM manuscript_analysis_completion_reports WHERE job_id=?1", params![input.job_id], completion_report_from_row).map_err(|e| sql_error("Abschlussbericht konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn get_manuscript_analysis_completion_report(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Option<ManuscriptAnalysisCompletionReport>, String> {
    let db = lock_db(&state)?;
    let _job = load_manuscript_analysis_job(&db, &job_id)?;
    db.query_row("SELECT id,job_id,project_id,content_hash,payload_json,created_at,updated_at FROM manuscript_analysis_completion_reports WHERE job_id=?1", params![job_id], completion_report_from_row).optional().map_err(|e| sql_error("Abschlussbericht konnte nicht geladen werden", e))
}

fn structure_run_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ManuscriptStructureRun> {
    Ok(ManuscriptStructureRun {
        id: row.get(0)?,
        project_id: row.get(1)?,
        chapter_id: row.get(2)?,
        content_hash: row.get(3)?,
        provider_id: row.get(4)?,
        prompt_version: row.get(5)?,
        status: row.get(6)?,
        error_message: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn structure_proposal_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ManuscriptStructureProposal> {
    Ok(ManuscriptStructureProposal {
        id: row.get(0)?,
        run_id: row.get(1)?,
        project_id: row.get(2)?,
        chapter_id: row.get(3)?,
        temporary_id: row.get(4)?,
        start_offset: row.get(5)?,
        end_offset: row.get(6)?,
        title: row.get(7)?,
        pov_character_name: row.get(8)?,
        pov_entity_id: row.get(9)?,
        location: row.get(10)?,
        story_time: row.get(11)?,
        participating_character_names: serde_json::from_str(&row.get::<_, String>(12)?)
            .unwrap_or_default(),
        goal: row.get(13)?,
        conflict: row.get(14)?,
        important_events: serde_json::from_str(&row.get::<_, String>(15)?).unwrap_or_default(),
        transition_type: row.get(16)?,
        boundary_reason: row.get(17)?,
        confidence: row.get(18)?,
        evidence_excerpt: row.get(19)?,
        review_status: row.get(20)?,
        manual_changes: serde_json::from_str(&row.get::<_, String>(21)?)
            .unwrap_or_else(|_| serde_json::json!({})),
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
    })
}

fn chapter_plain_text(db: &Connection, chapter_id: &str) -> Result<String, String> {
    let mut statement = db
        .prepare("SELECT content FROM scenes WHERE chapter_id=?1 ORDER BY order_index")
        .map_err(|e| sql_error("Kapiteltext konnte nicht geladen werden", e))?;
    let contents = statement
        .query_map(params![chapter_id], |row| row.get::<_, String>(0))
        .map_err(|e| sql_error("Kapiteltext konnte nicht geladen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Kapiteltext konnte nicht gelesen werden", e))?;
    Ok(contents
        .iter()
        .map(|content| canonical_editor_text(content))
        .collect::<Vec<_>>()
        .join("\n\n"))
}

#[tauri::command]
pub fn create_manuscript_structure_run(
    state: State<'_, DbState>,
    input: CreateManuscriptStructureRunInput,
) -> Result<ManuscriptStructureRun, String> {
    let db = lock_db(&state)?;
    let chapter_project: Option<String> = db
        .query_row(
            "SELECT b.project_id FROM chapters c JOIN books b ON b.id=c.book_id WHERE c.id=?1",
            params![input.chapter_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| sql_error("Kapitel konnte nicht geprüft werden", e))?;
    if chapter_project.as_deref() != Some(input.project_id.as_str()) {
        return Err("Kapitel gehört nicht zum Projekt.".into());
    }
    let id = new_id();
    let stamp = now();
    db.execute("INSERT INTO manuscript_structure_runs(id,project_id,chapter_id,content_hash,provider_id,prompt_version,status,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,'pending',?7,?7)", params![id,input.project_id,input.chapter_id,input.content_hash,input.provider_id,input.prompt_version,stamp]).map_err(|e| sql_error("Strukturlauf konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,project_id,chapter_id,content_hash,provider_id,prompt_version,status,error_message,created_at,updated_at FROM manuscript_structure_runs WHERE id=?1", params![id], structure_run_from_row).map_err(|e| sql_error("Strukturlauf konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn update_manuscript_structure_run(
    state: State<'_, DbState>,
    id: String,
    status: String,
    error_message: Option<String>,
) -> Result<ManuscriptStructureRun, String> {
    if !matches!(
        status.as_str(),
        "pending" | "running" | "completed" | "failed" | "reviewed"
    ) {
        return Err("Ungültiger Strukturlaufstatus.".into());
    }
    let db = lock_db(&state)?;
    if db.execute("UPDATE manuscript_structure_runs SET status=?1,error_message=?2,updated_at=?3 WHERE id=?4", params![status,error_message,now(),id]).map_err(|e| sql_error("Strukturlauf konnte nicht aktualisiert werden", e))? == 0 { return Err("Strukturlauf nicht gefunden.".into()); }
    db.query_row("SELECT id,project_id,chapter_id,content_hash,provider_id,prompt_version,status,error_message,created_at,updated_at FROM manuscript_structure_runs WHERE id=?1", params![id], structure_run_from_row).map_err(|e| sql_error("Strukturlauf konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn list_manuscript_structure_runs(
    state: State<'_, DbState>,
    project_id: String,
    chapter_id: Option<String>,
) -> Result<Vec<ManuscriptStructureRun>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id,project_id,chapter_id,content_hash,provider_id,prompt_version,status,error_message,created_at,updated_at FROM manuscript_structure_runs WHERE project_id=?1 AND (?2 IS NULL OR chapter_id=?2) ORDER BY created_at DESC").map_err(|e| sql_error("Strukturläufe konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(params![project_id, chapter_id], structure_run_from_row)
        .map_err(|e| sql_error("Strukturläufe konnten nicht gelesen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Strukturläufe konnten nicht gelesen werden", e));
    result
}

#[tauri::command]
pub fn list_manuscript_structure_proposals(
    state: State<'_, DbState>,
    run_id: String,
) -> Result<Vec<ManuscriptStructureProposal>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT p.id,p.run_id,p.project_id,p.chapter_id,p.temporary_id,p.start_offset,p.end_offset,p.title,p.pov_character_name,p.pov_entity_id,p.location,p.story_time,p.participating_character_names_json,p.goal,p.conflict,p.important_events_json,p.transition_type,p.boundary_reason,p.confidence,p.evidence_excerpt,p.review_status,p.manual_changes_json,p.created_at,p.updated_at FROM manuscript_structure_proposals p JOIN manuscript_structure_runs r ON r.id=p.run_id WHERE p.run_id=?1 AND p.project_id=r.project_id ORDER BY p.start_offset").map_err(|e| sql_error("Szenenvorschläge konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(params![run_id], structure_proposal_from_row)
        .map_err(|e| sql_error("Szenenvorschläge konnten nicht gelesen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Szenenvorschläge konnten nicht gelesen werden", e));
    result
}

#[tauri::command]
pub fn save_manuscript_structure_proposals(
    state: State<'_, DbState>,
    run_id: String,
    proposals: Vec<SaveManuscriptStructureProposalInput>,
) -> Result<Vec<ManuscriptStructureProposal>, String> {
    let mut db = lock_db(&state)?;
    let run: ManuscriptStructureRun = db.query_row("SELECT id,project_id,chapter_id,content_hash,provider_id,prompt_version,status,error_message,created_at,updated_at FROM manuscript_structure_runs WHERE id=?1", params![run_id], structure_run_from_row).map_err(|e| sql_error("Strukturlauf konnte nicht geladen werden", e))?;
    if proposals.is_empty() {
        return Err("Mindestens ein Szenenvorschlag ist erforderlich.".into());
    }
    if proposals.iter().any(|proposal| {
        proposal.project_id != run.project_id
            || proposal.chapter_id != run.chapter_id
            || proposal.run_id != run.id
            || !(0.0..=1.0).contains(&proposal.confidence)
    }) {
        return Err(
            "Szenenvorschlag gehört nicht zum Strukturlauf oder enthält ungültige Confidence."
                .into(),
        );
    }
    let text = chapter_plain_text(&db, &run.chapter_id)?;
    let chars: Vec<char> = text.chars().collect();
    let mut ordered = proposals.clone();
    ordered.sort_by_key(|proposal| proposal.start_offset);
    if ordered[0].start_offset != 0
        || ordered.last().map(|p| p.end_offset) != Some(chars.len() as i64)
    {
        return Err("Szenenvorschläge müssen den vollständigen Kapiteltext abdecken.".into());
    }
    let mut expected = 0_i64;
    for proposal in &ordered {
        if proposal.start_offset != expected
            || proposal.start_offset < 0
            || proposal.end_offset < proposal.start_offset
            || proposal.end_offset > chars.len() as i64
        {
            return Err("Szenenvorschläge enthalten eine Lücke oder Überlappung.".into());
        }
        let excerpt: String = chars[proposal.start_offset as usize..proposal.end_offset as usize]
            .iter()
            .collect();
        if excerpt != proposal.evidence_excerpt {
            return Err("Szenenbeleg stimmt nicht mit dem Kapiteltext überein.".into());
        }
        expected = proposal.end_offset;
    }
    let transaction = db
        .transaction()
        .map_err(|e| sql_error("Strukturspeicherung konnte nicht gestartet werden", e))?;
    transaction
        .execute(
            "DELETE FROM manuscript_structure_proposals WHERE run_id=?1",
            params![run_id],
        )
        .map_err(|e| sql_error("Alte Szenenvorschläge konnten nicht entfernt werden", e))?;
    for proposal in ordered {
        let id = proposal.id.unwrap_or_else(new_id);
        let participants = serde_json::to_string(&proposal.participating_character_names)
            .unwrap_or_else(|_| "[]".into());
        let events =
            serde_json::to_string(&proposal.important_events).unwrap_or_else(|_| "[]".into());
        let manual =
            serde_json::to_string(&proposal.manual_changes).unwrap_or_else(|_| "{}".into());
        transaction.execute("INSERT INTO manuscript_structure_proposals(id,run_id,project_id,chapter_id,temporary_id,start_offset,end_offset,title,pov_character_name,pov_entity_id,location,story_time,participating_character_names_json,goal,conflict,important_events_json,transition_type,boundary_reason,confidence,evidence_excerpt,review_status,manual_changes_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,COALESCE(?21,'proposed'),?22,?23,?23)", params![id,proposal.run_id,proposal.project_id,proposal.chapter_id,proposal.temporary_id,proposal.start_offset,proposal.end_offset,proposal.title,proposal.pov_character_name,proposal.pov_entity_id,proposal.location,proposal.story_time,participants,proposal.goal,proposal.conflict,events,proposal.transition_type,proposal.boundary_reason,proposal.confidence,proposal.evidence_excerpt,proposal.review_status,manual,now()]).map_err(|e| sql_error("Szenenvorschlag konnte nicht gespeichert werden", e))?;
    }
    transaction
        .commit()
        .map_err(|e| sql_error("Szenenvorschläge konnten nicht gespeichert werden", e))?;
    drop(db);
    list_manuscript_structure_proposals(state, run_id)
}

#[tauri::command]
pub fn review_manuscript_structure_proposal(
    state: State<'_, DbState>,
    id: String,
    review_status: String,
    manual_changes: Option<serde_json::Value>,
) -> Result<ManuscriptStructureProposal, String> {
    if !matches!(
        review_status.as_str(),
        "proposed" | "accepted" | "edited" | "rejected" | "uncertain"
    ) {
        return Err("Ungültiger Szenenvorschlagsstatus.".into());
    }
    let db = lock_db(&state)?;
    let changes = serde_json::to_string(&manual_changes.unwrap_or_else(|| serde_json::json!({})))
        .unwrap_or_else(|_| "{}".into());
    if db.execute("UPDATE manuscript_structure_proposals SET review_status=?1,manual_changes_json=?2,updated_at=?3 WHERE id=?4", params![review_status,changes,now(),id]).map_err(|e| sql_error("Szenenvorschlag konnte nicht geprüft werden", e))? == 0 { return Err("Szenenvorschlag nicht gefunden.".into()); }
    db.query_row("SELECT id,run_id,project_id,chapter_id,temporary_id,start_offset,end_offset,title,pov_character_name,pov_entity_id,location,story_time,participating_character_names_json,goal,conflict,important_events_json,transition_type,boundary_reason,confidence,evidence_excerpt,review_status,manual_changes_json,created_at,updated_at FROM manuscript_structure_proposals WHERE id=?1", params![id], structure_proposal_from_row).map_err(|e| sql_error("Szenenvorschlag konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn apply_manuscript_structure(
    state: State<'_, DbState>,
    project_id: String,
    run_id: String,
) -> Result<Vec<Scene>, String> {
    let mut db = lock_db(&state)?;
    let run: ManuscriptStructureRun = db.query_row("SELECT id,project_id,chapter_id,content_hash,provider_id,prompt_version,status,error_message,created_at,updated_at FROM manuscript_structure_runs WHERE id=?1", params![run_id], structure_run_from_row).map_err(|e| sql_error("Strukturlauf konnte nicht geladen werden", e))?;
    if run.project_id != project_id {
        return Err("Strukturlauf gehört nicht zum Projekt.".into());
    }
    let proposals = {
        let mut statement = db.prepare("SELECT id,run_id,project_id,chapter_id,temporary_id,start_offset,end_offset,title,pov_character_name,pov_entity_id,location,story_time,participating_character_names_json,goal,conflict,important_events_json,transition_type,boundary_reason,confidence,evidence_excerpt,review_status,manual_changes_json,created_at,updated_at FROM manuscript_structure_proposals WHERE run_id=?1 ORDER BY start_offset").map_err(|e| sql_error("Szenenvorschläge konnten nicht geladen werden", e))?;
        let rows = statement
            .query_map(params![run_id], structure_proposal_from_row)
            .map_err(|e| sql_error("Szenenvorschläge konnten nicht gelesen werden", e))?
            .collect::<SqlResult<Vec<_>>>()
            .map_err(|e| sql_error("Szenenvorschläge konnten nicht gelesen werden", e));
        rows?
    };
    if proposals.is_empty()
        || proposals
            .iter()
            .any(|proposal| !matches!(proposal.review_status.as_str(), "accepted" | "edited"))
    {
        return Err(
            "Alle Szenenvorschläge müssen vor der Übernahme bestätigt oder bearbeitet werden."
                .into(),
        );
    }
    let text = chapter_plain_text(&db, &run.chapter_id)?;
    let chars: Vec<char> = text.chars().collect();
    let mut expected = 0_i64;
    for proposal in &proposals {
        if proposal.start_offset != expected
            || proposal.end_offset < proposal.start_offset
            || proposal.end_offset as usize > chars.len()
        {
            return Err("Szenenvorschläge sind nicht lückenlos.".into());
        }
        expected = proposal.end_offset;
    }
    if expected != chars.len() as i64 {
        return Err("Szenenvorschläge decken das Kapitel nicht vollständig ab.".into());
    }
    let old_scene: Scene = db.query_row("SELECT id,chapter_id,title,order_index,content,pov,location,story_time,status,goal,notes,is_implicit,created_at,updated_at FROM scenes WHERE chapter_id=?1 ORDER BY order_index LIMIT 1", params![run.chapter_id], scene_from_row).map_err(|e| sql_error("Implizite Importszene konnte nicht geladen werden", e))?;
    let timestamp = now();
    let transaction = db
        .transaction()
        .map_err(|e| sql_error("Strukturübernahme konnte nicht gestartet werden", e))?;
    insert_scene_version_in_transaction(&transaction, &old_scene, &timestamp, "structure_review")?;
    let mut result = Vec::new();
    for (index, proposal) in proposals.iter().enumerate() {
        let content: String = chars[proposal.start_offset as usize..proposal.end_offset as usize]
            .iter()
            .collect();
        let id = if index == 0 {
            old_scene.id.clone()
        } else {
            new_id()
        };
        transaction.execute("INSERT INTO scenes(id,chapter_id,title,order_index,content,pov,location,story_time,status,goal,notes,is_implicit,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'draft',?9,?10,0,?11,?11) ON CONFLICT(id) DO UPDATE SET title=excluded.title,order_index=excluded.order_index,content=excluded.content,pov=excluded.pov,location=excluded.location,story_time=excluded.story_time,status=excluded.status,goal=excluded.goal,notes=excluded.notes,is_implicit=0,updated_at=excluded.updated_at", params![id,run.chapter_id,proposal.title,index as i64+1,content,proposal.pov_character_name.clone().unwrap_or_default(),proposal.location,proposal.story_time,proposal.goal,proposal.boundary_reason,timestamp]).map_err(|e| sql_error("Szene konnte nicht atomar übernommen werden", e))?;
        let scene = transaction.query_row("SELECT id,chapter_id,title,order_index,content,pov,location,story_time,status,goal,notes,is_implicit,created_at,updated_at FROM scenes WHERE id=?1", params![id], scene_from_row).map_err(|e| sql_error("Übernommene Szene konnte nicht geladen werden", e))?;
        transaction.execute("INSERT INTO scene_versions(id,scene_id,content,reason,created_at,version_number,snapshot_json) VALUES(?1,?2,?3,'structure_review',?4,1,'')", params![new_id(),scene.id,scene.content,timestamp]).map_err(|e| sql_error("Anfangsversion der Szene konnte nicht gespeichert werden", e))?;
        result.push(scene);
    }
    transaction
        .execute(
            "DELETE FROM scenes WHERE chapter_id=?1 AND id<>?2 AND is_implicit=1",
            params![run.chapter_id, old_scene.id],
        )
        .map_err(|e| sql_error("Alte implizite Szenen konnten nicht bereinigt werden", e))?;
    let structure_jobs: Vec<(String, String)> = {
        let mut statement = transaction
            .prepare("SELECT id,provider_id FROM manuscript_analysis_jobs WHERE project_id=?1 AND current_phase='structure' AND status='awaiting_structure_review'")
            .map_err(|e| sql_error("Strukturreview-Jobs konnten nicht geladen werden", e))?;
        let rows = statement
            .query_map(params![project_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| sql_error("Strukturreview-Jobs konnten nicht gelesen werden", e))?
            .collect::<SqlResult<Vec<_>>>()
            .map_err(|e| sql_error("Strukturreview-Jobs konnten nicht gelesen werden", e))?;
        rows
    };
    for (job_id, provider_id) in structure_jobs {
        let old_units: Vec<(String, i64, Option<i64>, i64, i64)> = {
            let mut statement = transaction
                .prepare("SELECT id,order_index,page_number,start_offset,end_offset FROM manuscript_analysis_units WHERE job_id=?1 ORDER BY order_index")
                .map_err(|e| sql_error("Alte Analyse-Units konnten nicht geladen werden", e))?;
            let rows = statement
                .query_map(params![job_id], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                })
                .map_err(|e| sql_error("Alte Analyse-Units konnten nicht gelesen werden", e))?
                .collect::<SqlResult<Vec<_>>>()
                .map_err(|e| sql_error("Alte Analyse-Units konnten nicht gelesen werden", e))?;
            rows
        };
        transaction
            .execute(
                "DELETE FROM manuscript_analysis_draft_ledger WHERE job_id=?1",
                params![job_id],
            )
            .map_err(|e| sql_error("Alte Draft-Zustände konnten nicht entfernt werden", e))?;
        transaction
            .execute(
                "DELETE FROM manuscript_analysis_artifacts WHERE job_id=?1",
                params![job_id],
            )
            .map_err(|e| sql_error("Alte Analyseartefakte konnten nicht entfernt werden", e))?;
        transaction
            .execute(
                "DELETE FROM manuscript_analysis_phase_results WHERE job_id=?1",
                params![job_id],
            )
            .map_err(|e| sql_error("Alte Phasenergebnisse konnten nicht entfernt werden", e))?;
        transaction
            .execute(
                "DELETE FROM manuscript_analysis_units WHERE job_id=?1",
                params![job_id],
            )
            .map_err(|e| sql_error("Alte Analyse-Units konnten nicht entfernt werden", e))?;
        let mut order_index = 0_i64;
        for (old_id, old_order, page_number, unit_start, unit_end) in old_units {
            for (scene_index, proposal) in proposals.iter().enumerate() {
                let start = unit_start.max(proposal.start_offset);
                let end = unit_end.min(proposal.end_offset);
                if end <= start {
                    continue;
                }
                let content: String = chars[start as usize..end as usize].iter().collect();
                transaction.execute("INSERT INTO manuscript_analysis_units (id,job_id,project_id,chapter_id,scene_id,order_index,page_number,start_offset,end_offset,content,content_hash,status,retry_count,requested_provider,prompt_version,input_hash,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'pending',0,?12,'manuscript-analysis-v1',?11,?13,?13)", params![format!("{old_id}-{scene_index}"), job_id, project_id, run.chapter_id, result[scene_index].id, order_index, page_number, start, end, content, canonical_content_hash(&content), provider_id, timestamp]) .map_err(|e| sql_error("Neue Analyse-Unit konnte nicht gespeichert werden", e))?;
                order_index += 1;
            }
            let _ = old_order;
        }
        transaction.execute("UPDATE manuscript_analysis_jobs SET status='pending',current_phase='passage_continuity',phase_progress_json='{}',phase_errors_json='{}',total_units=?1,completed_units=0,failed_units=0,current_unit_id=NULL,last_successful_unit_id=NULL,error_message=NULL,updated_at=?2 WHERE id=?3", params![order_index, timestamp, job_id]).map_err(|e| sql_error("Analysejob konnte nach Strukturübernahme nicht zurückgesetzt werden", e))?;
    }
    transaction
        .commit()
        .map_err(|e| sql_error("Strukturübernahme konnte nicht abgeschlossen werden", e))?;
    drop(db);
    update_manuscript_structure_run(state, run_id, "reviewed".into(), None)?;
    Ok(result)
}

#[tauri::command]
pub fn apply_reviewed_manuscript_structure(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<Scene>, String> {
    let db = lock_db(&state)?;
    let (project_id, chapter_id): (String, String) = db
        .query_row(
            "SELECT project_id,chapter_id FROM manuscript_analysis_units WHERE job_id=?1 ORDER BY order_index LIMIT 1",
            params![job_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| sql_error("Analysejob konnte nicht geladen werden", e))?;
    let run_id: String = db
        .query_row(
            "SELECT id FROM manuscript_structure_runs WHERE project_id=?1 AND chapter_id=?2 AND status IN ('completed','reviewed') ORDER BY updated_at DESC LIMIT 1",
            params![project_id, chapter_id],
            |row| row.get(0),
        )
        .map_err(|e| sql_error("Strukturlauf konnte nicht geladen werden", e))?;
    drop(db);
    apply_manuscript_structure(state, project_id, run_id)
}

fn provisional_entity_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProvisionalEntity> {
    Ok(ProvisionalEntity {
        id: row.get(0)?,
        job_id: row.get(1)?,
        project_id: row.get(2)?,
        entity_type: row.get(3)?,
        canonical_name: row.get(4)?,
        aliases: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
        description: row.get(6)?,
        first_source_reference_id: row.get(7)?,
        last_source_reference_id: row.get(8)?,
        confidence: row.get(9)?,
        review_status: row.get(10)?,
        existing_entity_id: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}
fn provisional_mention_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProvisionalEntityMention> {
    Ok(ProvisionalEntityMention {
        id: row.get(0)?,
        job_id: row.get(1)?,
        project_id: row.get(2)?,
        passage_unit_id: row.get(3)?,
        chapter_id: row.get(4)?,
        scene_id: row.get(5)?,
        start_offset: row.get(6)?,
        end_offset: row.get(7)?,
        excerpt: row.get(8)?,
        mention_text: row.get(9)?,
        resolved_provisional_entity_id: row.get(10)?,
        alternative_entity_ids: serde_json::from_str(&row.get::<_, String>(11)?)
            .unwrap_or_default(),
        confidence: row.get(12)?,
        resolution_reason: row.get(13)?,
        created_at: row.get(14)?,
    })
}
fn provisional_merge_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProvisionalMergeProposal> {
    Ok(ProvisionalMergeProposal {
        id: row.get(0)?,
        job_id: row.get(1)?,
        project_id: row.get(2)?,
        left_provisional_entity_id: row.get(3)?,
        right_provisional_entity_id: row.get(4)?,
        existing_entity_id: row.get(5)?,
        reason: row.get(6)?,
        confidence: row.get(7)?,
        review_status: row.get(8)?,
        created_at: row.get(9)?,
    })
}

#[tauri::command]
pub fn list_provisional_entities(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<ProvisionalEntity>, String> {
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &job_id)?;
    let mut statement = db.prepare("SELECT id,job_id,project_id,entity_type,canonical_name,aliases_json,description,first_source_reference_id,last_source_reference_id,confidence,review_status,existing_entity_id,created_at,updated_at FROM provisional_entities WHERE job_id=?1 AND project_id=?2 ORDER BY created_at").map_err(|e| sql_error("Provisorische Entitäten konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(params![job_id, job.project_id], provisional_entity_from_row)
        .map_err(|e| sql_error("Provisorische Entitäten konnten nicht gelesen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Provisorische Entitäten konnten nicht gelesen werden", e));
    result
}

#[tauri::command]
pub fn save_provisional_entity(
    state: State<'_, DbState>,
    input: SaveProvisionalEntityInput,
) -> Result<ProvisionalEntity, String> {
    if !(0.0..=1.0).contains(&input.confidence) || input.canonical_name.trim().is_empty() {
        return Err("Ungültige provisorische Entität.".into());
    }
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &input.job_id)?;
    if job.project_id != input.project_id {
        return Err("Provisorische Entität gehört nicht zum Projekt.".into());
    }
    let aliases = serde_json::to_string(&input.aliases).unwrap_or_else(|_| "[]".into());
    let id = input.id.unwrap_or_else(new_id);
    let stamp = now();
    db.execute("INSERT INTO provisional_entities(id,job_id,project_id,entity_type,canonical_name,aliases_json,description,first_source_reference_id,last_source_reference_id,confidence,review_status,existing_entity_id,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,COALESCE(?11,'proposed'),?12,?13,?13) ON CONFLICT(id) DO UPDATE SET canonical_name=excluded.canonical_name,aliases_json=excluded.aliases_json,description=excluded.description,confidence=excluded.confidence,review_status=excluded.review_status,existing_entity_id=excluded.existing_entity_id,last_source_reference_id=excluded.last_source_reference_id,updated_at=excluded.updated_at", params![id,input.job_id,input.project_id,input.entity_type,input.canonical_name.trim(),aliases,input.description,input.first_source_reference_id,input.last_source_reference_id,input.confidence,input.review_status,input.existing_entity_id,stamp]).map_err(|e| sql_error("Provisorische Entität konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,job_id,project_id,entity_type,canonical_name,aliases_json,description,first_source_reference_id,last_source_reference_id,confidence,review_status,existing_entity_id,created_at,updated_at FROM provisional_entities WHERE id=?1", params![id], provisional_entity_from_row).map_err(|e| sql_error("Provisorische Entität konnte nicht geladen werden", e))
}

fn materialize_provisional_entity_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    input: &MaterializeProvisionalEntityInput,
) -> Result<String, String> {
    if input.decision != "accept" && input.decision != "merge" {
        return Err("Ungültige Materialisierungsentscheidung.".into());
    }
    let job_project: String = transaction
        .query_row(
            "SELECT project_id FROM manuscript_analysis_jobs WHERE id=?1",
            params![input.job_id],
            |row| row.get(0),
        )
        .map_err(|e| sql_error("Analysejob konnte nicht geprüft werden", e))?;
    if job_project != input.project_id {
        return Err("Analysejob gehört nicht zum Projekt.".into());
    }
    let provisional: (String, String, String, String, f64, Option<String>) = transaction.query_row("SELECT entity_type,canonical_name,aliases_json,description,confidence,existing_entity_id FROM provisional_entities WHERE id=?1 AND job_id=?2 AND project_id=?3", params![input.provisional_entity_id, input.job_id, input.project_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))).map_err(|e| sql_error("Provisorische Entität konnte nicht geladen werden", e))?;
    let target_id = input.existing_entity_id.clone().or(provisional.5.clone());
    let canonical_id = if let Some(existing_id) = target_id.clone() {
        let valid: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM story_entities WHERE id=?1 AND project_id=?2)",
                params![existing_id, input.project_id],
                |row| row.get(0),
            )
            .map_err(|e| sql_error("Zielentität konnte nicht geprüft werden", e))?;
        if !valid || input.decision != "merge" {
            return Err("Die ausgewählte bestehende Entität ist ungültig.".into());
        }
        existing_id
    } else {
        if input.decision != "accept" {
            return Err("Für einen Merge muss eine bestehende Entität ausgewählt werden.".into());
        }
        let id = new_id();
        let entity_type = if provisional.0 == "world_rule_candidate" {
            "world_rule"
        } else if provisional.0 == "author_note" {
            "author_note"
        } else {
            provisional.0.as_str()
        };
        transaction.execute("INSERT INTO story_entities (id,project_id,name,entity_type,description,status,confidence,source,chapter,scene,author_confirmed,updated_at,origin,tags_json) VALUES (?1,?2,?3,?4,?5,'confirmed',?6,'manuscript_analysis','','',1,?7,'edited',?8)", params![id, input.project_id, provisional.1, entity_type, provisional.3, provisional.4.clamp(0.0, 1.0), now(), provisional.2]).map_err(|e| sql_error("Kanonische Entität konnte nicht erstellt werden", e))?;
        id
    };
    let old_id = &input.provisional_entity_id;
    transaction
        .execute(
            "UPDATE story_source_references SET entity_id=?2 WHERE entity_id=?1 AND project_id=?3",
            params![old_id, canonical_id, input.project_id],
        )
        .map_err(|e| sql_error("Quellen konnten nicht umgehängt werden", e))?;
    transaction.execute("UPDATE provisional_entity_mentions SET resolved_provisional_entity_id=?2, alternative_entity_ids_json=REPLACE(alternative_entity_ids_json,?1,?2) WHERE job_id=?3 AND project_id=?4", params![old_id, canonical_id, input.job_id, input.project_id]).map_err(|e| sql_error("Entitätserwähnungen konnten nicht umgehängt werden", e))?;
    transaction.execute("UPDATE provisional_relations SET source_provisional_entity_id=CASE WHEN source_provisional_entity_id=?1 THEN ?2 ELSE source_provisional_entity_id END, target_provisional_entity_id=CASE WHEN target_provisional_entity_id=?1 THEN ?2 ELSE target_provisional_entity_id END WHERE job_id=?3 AND project_id=?4", params![old_id, canonical_id, input.job_id, input.project_id]).map_err(|e| sql_error("Beziehungen konnten nicht umgehängt werden", e))?;
    for (table, column) in [
        ("provisional_events", "participant_entity_ids_json"),
        (
            "manuscript_timeline_events",
            "participating_entity_ids_json",
        ),
    ] {
        transaction
            .execute(
                &format!("UPDATE {table} SET {column}=REPLACE({column},?1,?2) WHERE project_id=?3"),
                params![old_id, canonical_id, input.project_id],
            )
            .map_err(|e| sql_error("Ereignisbeteiligte konnten nicht umgehängt werden", e))?;
    }
    transaction.execute("UPDATE story_graph_edges SET source_entity_id=CASE WHEN source_entity_id=?1 THEN ?2 ELSE source_entity_id END, target_entity_id=CASE WHEN target_entity_id=?1 THEN ?2 ELSE target_entity_id END, source_reference_ids_json=REPLACE(source_reference_ids_json,?1,?2) WHERE project_id=?3", params![old_id, canonical_id, input.project_id]).map_err(|e| sql_error("Graph-Kanten konnten nicht umgehängt werden", e))?;
    transaction.execute("UPDATE character_memory_proposals SET subject_character_id=CASE WHEN subject_character_id=?1 THEN ?2 ELSE subject_character_id END, related_character_id=CASE WHEN related_character_id=?1 THEN ?2 ELSE related_character_id END, target_entity_id=CASE WHEN target_entity_id=?1 THEN ?2 ELSE target_entity_id END, payload_json=REPLACE(payload_json,?1,?2) WHERE project_id=?3", params![old_id, canonical_id, input.project_id]).map_err(|e| sql_error("Character-Memory-Verweise konnten nicht umgehängt werden", e))?;
    transaction.execute("UPDATE manuscript_analysis_draft_ledger SET entity_id=CASE WHEN entity_id=?1 THEN ?2 ELSE entity_id END, related_entity_id=CASE WHEN related_entity_id=?1 THEN ?2 ELSE related_entity_id END WHERE job_id=?3 AND project_id=?4", params![old_id, canonical_id, input.job_id, input.project_id]).map_err(|e| sql_error("Draft-Zustände konnten nicht umgehängt werden", e))?;
    transaction.execute("UPDATE provisional_entities SET existing_entity_id=?2, review_status=?3, updated_at=?4 WHERE id=?1 AND job_id=?5 AND project_id=?6", params![old_id, canonical_id, if target_id.is_some() { "merged" } else { "accepted" }, now(), input.job_id, input.project_id]).map_err(|e| sql_error("Materialisierungsstatus konnte nicht gespeichert werden", e))?;
    Ok(canonical_id)
}

#[tauri::command]
pub fn materialize_provisional_entity(
    state: State<'_, DbState>,
    input: MaterializeProvisionalEntityInput,
) -> Result<StoryEntity, String> {
    let db = lock_db(&state)?;
    let transaction = db
        .unchecked_transaction()
        .map_err(|e| sql_error("Materialisierung konnte nicht gestartet werden", e))?;
    let canonical_id = materialize_provisional_entity_in_transaction(&transaction, &input)?;
    transaction
        .commit()
        .map_err(|e| sql_error("Materialisierung konnte nicht abgeschlossen werden", e))?;
    db.query_row(entity_query(), params![canonical_id], entity_from_row)
        .map_err(|e| sql_error("Materialisierte Entität konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn invalidate_manuscript_analysis_from(
    state: State<'_, DbState>,
    job_id: String,
    order_index: i64,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    let transaction = db
        .unchecked_transaction()
        .map_err(|e| sql_error("Invalidierung konnte nicht gestartet werden", e))?;
    let project_id: String = transaction
        .query_row(
            "SELECT project_id FROM manuscript_analysis_jobs WHERE id=?1",
            params![job_id],
            |row| row.get(0),
        )
        .map_err(|e| sql_error("Analysejob konnte nicht geprüft werden", e))?;
    let later_units: Vec<String> = transaction
        .prepare("SELECT id FROM manuscript_analysis_units WHERE job_id=?1 AND order_index>=?2")
        .map_err(|e| sql_error("Spätere Analyse-Units konnten nicht geladen werden", e))?
        .query_map(params![job_id, order_index], |row| row.get(0))
        .map_err(|e| sql_error("Spätere Analyse-Units konnten nicht gelesen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Spätere Analyse-Units konnten nicht gelesen werden", e))?;
    transaction.execute("DELETE FROM bible_proposals WHERE id IN (SELECT artifact_id FROM manuscript_analysis_artifacts WHERE job_id=?1 AND unit_id IN (SELECT id FROM manuscript_analysis_units WHERE job_id=?1 AND order_index>=?2) AND artifact_type='bible_proposal')", params![job_id, order_index]).map_err(|e| sql_error("Bible-Vorschläge konnten nicht invalidiert werden", e))?;
    transaction.execute("DELETE FROM character_memory_proposals WHERE id IN (SELECT artifact_id FROM manuscript_analysis_artifacts WHERE job_id=?1 AND unit_id IN (SELECT id FROM manuscript_analysis_units WHERE job_id=?1 AND order_index>=?2) AND artifact_type='character_memory_proposal')", params![job_id, order_index]).map_err(|e| sql_error("Character-Memory-Vorschläge konnten nicht invalidiert werden", e))?;
    transaction.execute("DELETE FROM continuity_review_findings WHERE id IN (SELECT artifact_id FROM manuscript_analysis_artifacts WHERE job_id=?1 AND unit_id IN (SELECT id FROM manuscript_analysis_units WHERE job_id=?1 AND order_index>=?2) AND artifact_type='continuity_finding')", params![job_id, order_index]).map_err(|e| sql_error("Findings konnten nicht invalidiert werden", e))?;
    transaction.execute("DELETE FROM manuscript_analysis_draft_ledger WHERE job_id=?1 AND unit_id IN (SELECT id FROM manuscript_analysis_units WHERE job_id=?1 AND order_index>=?2)", params![job_id, order_index]).map_err(|e| sql_error("Draft-Zustände konnten nicht invalidiert werden", e))?;
    transaction.execute("DELETE FROM provisional_entity_mentions WHERE job_id=?1 AND passage_unit_id IN (SELECT id FROM manuscript_analysis_units WHERE job_id=?1 AND order_index>=?2)", params![job_id, order_index]).map_err(|e| sql_error("Entitätserwähnungen konnten nicht invalidiert werden", e))?;
    transaction.execute("DELETE FROM provisional_events WHERE job_id=?1 AND passage_unit_id IN (SELECT id FROM manuscript_analysis_units WHERE job_id=?1 AND order_index>=?2)", params![job_id, order_index]).map_err(|e| sql_error("Provisorische Ereignisse konnten nicht invalidiert werden", e))?;
    transaction.execute("DELETE FROM manuscript_timeline_events WHERE project_id=?1 AND passage_unit_id IN (SELECT id FROM manuscript_analysis_units WHERE job_id=?2 AND order_index>=?3)", params![project_id, job_id, order_index]).map_err(|e| sql_error("Timeline-Ereignisse konnten nicht invalidiert werden", e))?;
    for unit_id in later_units {
        let pattern = format!("%{job_id}-{unit_id}-%");
        transaction
            .execute(
                "DELETE FROM provisional_relations WHERE job_id=?1 AND id LIKE ?2",
                params![job_id, pattern],
            )
            .map_err(|e| {
                sql_error(
                    "Provisorische Beziehungen konnten nicht invalidiert werden",
                    e,
                )
            })?;
        transaction
            .execute(
                "DELETE FROM story_graph_edges WHERE project_id=?1 AND id LIKE ?2",
                params![project_id, pattern],
            )
            .map_err(|e| sql_error("Graph-Kanten konnten nicht invalidiert werden", e))?;
    }
    transaction.execute("DELETE FROM manuscript_analysis_artifacts WHERE job_id=?1 AND unit_id IN (SELECT id FROM manuscript_analysis_units WHERE job_id=?1 AND order_index>=?2)", params![job_id, order_index]).map_err(|e| sql_error("Analyseartefakte konnten nicht invalidiert werden", e))?;
    transaction.execute("UPDATE manuscript_analysis_units SET status='stale',continuity_run_id=NULL,error_code='STALE_CONTEXT',error_message='Durch eine frühere Textänderung veraltet.',completed_at=NULL,updated_at=?3 WHERE job_id=?1 AND order_index>=?2", params![job_id, order_index, now()]).map_err(|e| sql_error("Analyse-Units konnten nicht invalidiert werden", e))?;
    transaction
        .commit()
        .map_err(|e| sql_error("Invalidierung konnte nicht abgeschlossen werden", e))
}

#[tauri::command]
pub fn list_provisional_entity_mentions(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<ProvisionalEntityMention>, String> {
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &job_id)?;
    let mut statement = db.prepare("SELECT id,job_id,project_id,passage_unit_id,chapter_id,scene_id,start_offset,end_offset,excerpt,mention_text,resolved_provisional_entity_id,alternative_entity_ids_json,confidence,resolution_reason,created_at FROM provisional_entity_mentions WHERE job_id=?1 AND project_id=?2 ORDER BY chapter_id,start_offset").map_err(|e| sql_error("Erwähnungen konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(
            params![job_id, job.project_id],
            provisional_mention_from_row,
        )
        .map_err(|e| sql_error("Erwähnungen konnten nicht gelesen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Erwähnungen konnten nicht gelesen werden", e));
    result
}

#[tauri::command]
pub fn save_provisional_mentions(
    state: State<'_, DbState>,
    mentions: Vec<SaveProvisionalMentionInput>,
) -> Result<Vec<ProvisionalEntityMention>, String> {
    let mut db = lock_db(&state)?;
    if mentions.is_empty() {
        return Ok(Vec::new());
    }
    let job = load_manuscript_analysis_job(&db, &mentions[0].job_id)?;
    if mentions.iter().any(|item| {
        item.job_id != job.id
            || item.project_id != job.project_id
            || item.start_offset < 0
            || item.end_offset < item.start_offset
            || !(0.0..=1.0).contains(&item.confidence)
    }) {
        return Err("Ungültige provisorische Erwähnung.".into());
    }
    let transaction = db
        .transaction()
        .map_err(|e| sql_error("Erwähnungen konnten nicht gespeichert werden", e))?;
    for input in mentions {
        let id = input.id.unwrap_or_else(new_id);
        let alternatives =
            serde_json::to_string(&input.alternative_entity_ids).unwrap_or_else(|_| "[]".into());
        transaction.execute("INSERT INTO provisional_entity_mentions(id,job_id,project_id,passage_unit_id,chapter_id,scene_id,start_offset,end_offset,excerpt,mention_text,resolved_provisional_entity_id,alternative_entity_ids_json,confidence,resolution_reason,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)", params![id,input.job_id,input.project_id,input.passage_unit_id,input.chapter_id,input.scene_id,input.start_offset,input.end_offset,input.excerpt,input.mention_text,input.resolved_provisional_entity_id,alternatives,input.confidence,input.resolution_reason,now()]).map_err(|e| sql_error("Provisorische Erwähnung konnte nicht gespeichert werden", e))?;
    }
    transaction
        .commit()
        .map_err(|e| sql_error("Erwähnungen konnten nicht gespeichert werden", e))?;
    drop(db);
    list_provisional_entity_mentions(state, job.id)
}

#[tauri::command]
pub fn list_provisional_merge_proposals(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<ProvisionalMergeProposal>, String> {
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &job_id)?;
    let mut statement = db.prepare("SELECT id,job_id,project_id,left_provisional_entity_id,right_provisional_entity_id,existing_entity_id,reason,confidence,review_status,created_at FROM provisional_merge_proposals WHERE job_id=?1 AND project_id=?2 ORDER BY created_at").map_err(|e| sql_error("Merge-Vorschläge konnten nicht geladen werden", e))?;
    let result = statement
        .query_map(params![job_id, job.project_id], provisional_merge_from_row)
        .map_err(|e| sql_error("Merge-Vorschläge konnten nicht gelesen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Merge-Vorschläge konnten nicht gelesen werden", e));
    result
}

#[tauri::command]
pub fn save_provisional_merge_proposal(
    state: State<'_, DbState>,
    input: SaveProvisionalMergeProposalInput,
) -> Result<ProvisionalMergeProposal, String> {
    if !(0.0..=1.0).contains(&input.confidence) {
        return Err("Ungültige Merge-Confidence.".into());
    }
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &input.job_id)?;
    if job.project_id != input.project_id {
        return Err("Merge-Vorschlag gehört nicht zum Projekt.".into());
    }
    let id = input.id.unwrap_or_else(new_id);
    db.execute("INSERT INTO provisional_merge_proposals(id,job_id,project_id,left_provisional_entity_id,right_provisional_entity_id,existing_entity_id,reason,confidence,review_status) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,COALESCE(?9,'proposed')) ON CONFLICT(id) DO UPDATE SET reason=excluded.reason,confidence=excluded.confidence,review_status=excluded.review_status", params![id,input.job_id,input.project_id,input.left_provisional_entity_id,input.right_provisional_entity_id,input.existing_entity_id,input.reason,input.confidence,input.review_status]).map_err(|e| sql_error("Merge-Vorschlag konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,job_id,project_id,left_provisional_entity_id,right_provisional_entity_id,existing_entity_id,reason,confidence,review_status,created_at FROM provisional_merge_proposals WHERE id=?1", params![id], provisional_merge_from_row).map_err(|e| sql_error("Merge-Vorschlag konnte nicht geladen werden", e))
}

fn provisional_relation_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProvisionalRelation> {
    Ok(ProvisionalRelation {
        id: row.get(0)?,
        job_id: row.get(1)?,
        project_id: row.get(2)?,
        source_provisional_entity_id: row.get(3)?,
        target_provisional_entity_id: row.get(4)?,
        relation_type: row.get(5)?,
        label: row.get(6)?,
        confidence: row.get(7)?,
        review_status: row.get(8)?,
        source_reference_id: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}
fn provisional_event_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ProvisionalEvent> {
    Ok(ProvisionalEvent {
        id: row.get(0)?,
        job_id: row.get(1)?,
        project_id: row.get(2)?,
        passage_unit_id: row.get(3)?,
        chapter_id: row.get(4)?,
        scene_id: row.get(5)?,
        title: row.get(6)?,
        summary: row.get(7)?,
        participant_entity_ids: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
        start_offset: row.get(9)?,
        end_offset: row.get(10)?,
        confidence: row.get(11)?,
        review_status: row.get(12)?,
        source_reference_id: row.get(13)?,
        created_at: row.get(14)?,
    })
}

#[tauri::command]
pub fn save_provisional_relation(
    state: State<'_, DbState>,
    input: SaveProvisionalRelationInput,
) -> Result<ProvisionalRelation, String> {
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &input.job_id)?;
    if job.project_id != input.project_id || !(0.0..=1.0).contains(&input.confidence) {
        return Err("Ungültige provisorische Beziehung.".into());
    }
    let id = input.id.unwrap_or_else(new_id);
    let stamp = now();
    db.execute("INSERT INTO provisional_relations(id,job_id,project_id,source_provisional_entity_id,target_provisional_entity_id,relation_type,label,confidence,review_status,source_reference_id,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,COALESCE(?9,'proposed'),?10,?11,?11) ON CONFLICT(id) DO UPDATE SET label=excluded.label,confidence=excluded.confidence,review_status=excluded.review_status,updated_at=excluded.updated_at", params![id,input.job_id,input.project_id,input.source_provisional_entity_id,input.target_provisional_entity_id,input.relation_type,input.label,input.confidence,input.review_status,input.source_reference_id,stamp]).map_err(|e| sql_error("Provisorische Beziehung konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,job_id,project_id,source_provisional_entity_id,target_provisional_entity_id,relation_type,label,confidence,review_status,source_reference_id,created_at,updated_at FROM provisional_relations WHERE id=?1", params![id], provisional_relation_from_row).map_err(|e| sql_error("Provisorische Beziehung konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn save_provisional_event(
    state: State<'_, DbState>,
    input: SaveProvisionalEventInput,
) -> Result<ProvisionalEvent, String> {
    let db = lock_db(&state)?;
    let job = load_manuscript_analysis_job(&db, &input.job_id)?;
    if job.project_id != input.project_id
        || input.start_offset < 0
        || input.end_offset < input.start_offset
        || !(0.0..=1.0).contains(&input.confidence)
    {
        return Err("Ungültiges provisorisches Ereignis.".into());
    }
    let id = input.id.unwrap_or_else(new_id);
    let participants =
        serde_json::to_string(&input.participant_entity_ids).unwrap_or_else(|_| "[]".into());
    let stamp = now();
    db.execute("INSERT INTO provisional_events(id,job_id,project_id,passage_unit_id,chapter_id,scene_id,title,summary,participant_entity_ids_json,start_offset,end_offset,confidence,review_status,source_reference_id,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,COALESCE(?13,'proposed'),?14,?15)", params![id,input.job_id,input.project_id,input.passage_unit_id,input.chapter_id,input.scene_id,input.title,input.summary,participants,input.start_offset,input.end_offset,input.confidence,input.review_status,input.source_reference_id,stamp]).map_err(|e| sql_error("Provisorisches Ereignis konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,job_id,project_id,passage_unit_id,chapter_id,scene_id,title,summary,participant_entity_ids_json,start_offset,end_offset,confidence,review_status,source_reference_id,created_at FROM provisional_events WHERE id=?1", params![id], provisional_event_from_row).map_err(|e| sql_error("Provisorisches Ereignis konnte nicht geladen werden", e))
}

fn timeline_event_from_row(row: &rusqlite::Row<'_>) -> SqlResult<PersistentTimelineEvent> {
    Ok(PersistentTimelineEvent {
        id: row.get(0)?,
        project_id: row.get(1)?,
        book_id: row.get(2)?,
        chapter_id: row.get(3)?,
        scene_id: row.get(4)?,
        passage_unit_id: row.get(5)?,
        title: row.get(6)?,
        summary: row.get(7)?,
        story_time_text: row.get(8)?,
        normalized_time: row.get(9)?,
        temporal_order: row.get(10)?,
        time_certainty: row.get(11)?,
        location_entity_id: row.get(12)?,
        pov_character_id: row.get(13)?,
        participating_entity_ids: json_strings(&row.get::<_, String>(14)?),
        cause_event_ids: json_strings(&row.get::<_, String>(15)?),
        consequence_event_ids: json_strings(&row.get::<_, String>(16)?),
        knowledge_changes: json_strings(&row.get::<_, String>(17)?),
        state_changes: json_strings(&row.get::<_, String>(18)?),
        related_plot_thread_ids: json_strings(&row.get::<_, String>(19)?),
        source_reference_ids: json_strings(&row.get::<_, String>(20)?),
        confidence: row.get(21)?,
        status: row.get(22)?,
        author_confirmed: row.get(23)?,
        origin: row.get(24)?,
        created_at: row.get(25)?,
        updated_at: row.get(26)?,
    })
}

fn validate_timeline_input(
    db: &Connection,
    input: &SavePersistentTimelineEventInput,
) -> Result<(), String> {
    if !(0.0..=1.0).contains(&input.confidence)
        || input.status.as_deref().unwrap_or("proposed") == "confirmed" && !input.author_confirmed
    {
        return Err("Ungültiger Timeline-Status oder Confidence.".into());
    }
    let chapter_exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM chapters c JOIN books b ON b.id=c.book_id WHERE c.id=?1 AND c.book_id=?2 AND b.project_id=?3)", params![input.chapter_id, input.book_id, input.project_id], |row| row.get(0)).map_err(|e| e.to_string())?;
    let scene_exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM scenes WHERE id=?1 AND chapter_id=?2)",
            params![input.scene_id, input.chapter_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !chapter_exists || !scene_exists {
        return Err("Timeline-Ereignis gehört nicht zum Projekt.".into());
    }
    for source_id in &input.source_reference_ids {
        let valid: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2) OR EXISTS(SELECT 1 FROM project_source_references WHERE id=?1 AND project_id=?2)", params![source_id, input.project_id], |row| row.get(0)).map_err(|e| e.to_string())?;
        if !valid {
            return Err("Eine Timeline-Quelle gehört nicht zum Projekt.".into());
        }
    }
    Ok(())
}

#[tauri::command]
pub fn list_timeline_events(
    state: State<'_, DbState>,
    project_id: String,
    status: Option<String>,
) -> Result<Vec<PersistentTimelineEvent>, String> {
    let db = lock_db(&state)?;
    let mut stmt = db.prepare("SELECT id,project_id,book_id,chapter_id,scene_id,passage_unit_id,title,summary,story_time_text,normalized_time,temporal_order,time_certainty,location_entity_id,pov_character_id,participating_entity_ids_json,cause_event_ids_json,consequence_event_ids_json,knowledge_changes_json,state_changes_json,related_plot_thread_ids_json,source_reference_ids_json,confidence,status,author_confirmed,origin,created_at,updated_at FROM manuscript_timeline_events WHERE project_id=?1 AND (?2 IS NULL OR status=?2) ORDER BY temporal_order, created_at").map_err(|e| sql_error("Timeline konnte nicht geladen werden", e))?;
    let result = stmt
        .query_map(params![project_id, status], timeline_event_from_row)
        .map_err(|e| sql_error("Timeline konnte nicht geladen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Timeline konnte nicht geladen werden", e));
    result
}

#[tauri::command]
pub fn save_timeline_event(
    state: State<'_, DbState>,
    input: SavePersistentTimelineEventInput,
) -> Result<PersistentTimelineEvent, String> {
    let db = lock_db(&state)?;
    validate_timeline_input(&db, &input)?;
    let id = input.id.clone().unwrap_or_else(new_id);
    let stamp = now();
    let arrays: Vec<String> = [
        &input.participating_entity_ids,
        &input.cause_event_ids,
        &input.consequence_event_ids,
        &input.knowledge_changes,
        &input.state_changes,
        &input.related_plot_thread_ids,
        &input.source_reference_ids,
    ]
    .iter()
    .map(|value| serde_json::to_string(value).unwrap_or_else(|_| "[]".into()))
    .collect();
    db.execute("INSERT INTO manuscript_timeline_events(id,project_id,book_id,chapter_id,scene_id,passage_unit_id,title,summary,story_time_text,normalized_time,temporal_order,time_certainty,location_entity_id,pov_character_id,participating_entity_ids_json,cause_event_ids_json,consequence_event_ids_json,knowledge_changes_json,state_changes_json,related_plot_thread_ids_json,source_reference_ids_json,confidence,status,author_confirmed,origin,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,COALESCE((SELECT created_at FROM manuscript_timeline_events WHERE id=?1),?26),?26) ON CONFLICT(id) DO UPDATE SET title=excluded.title,summary=excluded.summary,story_time_text=excluded.story_time_text,normalized_time=excluded.normalized_time,temporal_order=excluded.temporal_order,time_certainty=excluded.time_certainty,location_entity_id=excluded.location_entity_id,pov_character_id=excluded.pov_character_id,participating_entity_ids_json=excluded.participating_entity_ids_json,cause_event_ids_json=excluded.cause_event_ids_json,consequence_event_ids_json=excluded.consequence_event_ids_json,knowledge_changes_json=excluded.knowledge_changes_json,state_changes_json=excluded.state_changes_json,related_plot_thread_ids_json=excluded.related_plot_thread_ids_json,source_reference_ids_json=excluded.source_reference_ids_json,confidence=excluded.confidence,status=excluded.status,author_confirmed=excluded.author_confirmed,updated_at=excluded.updated_at", params![id,input.project_id,input.book_id,input.chapter_id,input.scene_id,input.passage_unit_id,input.title,input.summary,input.story_time_text,input.normalized_time,input.temporal_order,input.time_certainty,input.location_entity_id,input.pov_character_id,arrays[0],arrays[1],arrays[2],arrays[3],arrays[4],arrays[5],arrays[6],input.confidence,input.status.unwrap_or_else(|| "proposed".into()),input.author_confirmed,input.origin,stamp]).map_err(|e| sql_error("Timeline-Ereignis konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,project_id,book_id,chapter_id,scene_id,passage_unit_id,title,summary,story_time_text,normalized_time,temporal_order,time_certainty,location_entity_id,pov_character_id,participating_entity_ids_json,cause_event_ids_json,consequence_event_ids_json,knowledge_changes_json,state_changes_json,related_plot_thread_ids_json,source_reference_ids_json,confidence,status,author_confirmed,origin,created_at,updated_at FROM manuscript_timeline_events WHERE id=?1", params![id], timeline_event_from_row).map_err(|e| sql_error("Timeline-Ereignis konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn review_timeline_event(
    state: State<'_, DbState>,
    id: String,
    status: String,
    input: Option<SavePersistentTimelineEventInput>,
) -> Result<PersistentTimelineEvent, String> {
    let db = lock_db(&state)?;
    let current = db.query_row("SELECT id,project_id,book_id,chapter_id,scene_id,passage_unit_id,title,summary,story_time_text,normalized_time,temporal_order,time_certainty,location_entity_id,pov_character_id,participating_entity_ids_json,cause_event_ids_json,consequence_event_ids_json,knowledge_changes_json,state_changes_json,related_plot_thread_ids_json,source_reference_ids_json,confidence,status,author_confirmed,origin,created_at,updated_at FROM manuscript_timeline_events WHERE id=?1", params![id], timeline_event_from_row).map_err(|e| sql_error("Timeline-Ereignis konnte nicht geladen werden", e))?;
    drop(db);
    let mut next = input.unwrap_or_else(|| SavePersistentTimelineEventInput {
        id: Some(current.id),
        project_id: current.project_id,
        book_id: current.book_id,
        chapter_id: current.chapter_id,
        scene_id: current.scene_id,
        passage_unit_id: current.passage_unit_id,
        title: current.title,
        summary: current.summary,
        story_time_text: current.story_time_text,
        normalized_time: current.normalized_time,
        temporal_order: current.temporal_order,
        time_certainty: current.time_certainty,
        location_entity_id: current.location_entity_id,
        pov_character_id: current.pov_character_id,
        participating_entity_ids: current.participating_entity_ids,
        cause_event_ids: current.cause_event_ids,
        consequence_event_ids: current.consequence_event_ids,
        knowledge_changes: current.knowledge_changes,
        state_changes: current.state_changes,
        related_plot_thread_ids: current.related_plot_thread_ids,
        source_reference_ids: current.source_reference_ids,
        confidence: current.confidence,
        status: Some(status.clone()),
        author_confirmed: status == "confirmed",
        origin: current.origin,
    });
    next.id = Some(id);
    next.status = Some(status.clone());
    next.author_confirmed = status == "confirmed";
    save_timeline_event(state, next)
}

fn graph_edge_from_row(row: &rusqlite::Row<'_>) -> SqlResult<StoryGraphEdge> {
    Ok(StoryGraphEdge {
        id: row.get(0)?,
        project_id: row.get(1)?,
        source_entity_id: row.get(2)?,
        target_entity_id: row.get(3)?,
        relation_type: row.get(4)?,
        label: row.get(5)?,
        valid_from_chapter_id: row.get(6)?,
        valid_from_scene_id: row.get(7)?,
        valid_from_offset: row.get(8)?,
        valid_until_chapter_id: row.get(9)?,
        valid_until_scene_id: row.get(10)?,
        valid_until_offset: row.get(11)?,
        source_reference_ids: json_strings(&row.get::<_, String>(12)?),
        confidence: row.get(13)?,
        status: row.get(14)?,
        author_confirmed: row.get(15)?,
        origin: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

#[tauri::command]
pub fn list_story_graph_edges(
    state: State<'_, DbState>,
    project_id: String,
    status: Option<String>,
) -> Result<Vec<StoryGraphEdge>, String> {
    let db = lock_db(&state)?;
    let mut stmt = db.prepare("SELECT id,project_id,source_entity_id,target_entity_id,relation_type,label,valid_from_chapter_id,valid_from_scene_id,valid_from_offset,valid_until_chapter_id,valid_until_scene_id,valid_until_offset,source_reference_ids_json,confidence,status,author_confirmed,origin,created_at,updated_at FROM story_graph_edges WHERE project_id=?1 AND (?2 IS NULL OR status=?2) ORDER BY created_at").map_err(|e| sql_error("Story-Graph konnte nicht geladen werden", e))?;
    let result = stmt
        .query_map(params![project_id, status], graph_edge_from_row)
        .map_err(|e| sql_error("Story-Graph konnte nicht geladen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Story-Graph konnte nicht geladen werden", e));
    result
}

#[tauri::command]
pub fn save_story_graph_edge(
    state: State<'_, DbState>,
    input: SaveStoryGraphEdgeInput,
) -> Result<StoryGraphEdge, String> {
    let db = lock_db(&state)?;
    if input.source_entity_id == input.target_entity_id
        || !(0.0..=1.0).contains(&input.confidence)
        || input.status.as_deref().unwrap_or("proposed") == "confirmed" && !input.author_confirmed
    {
        return Err("Ungültige Story-Graph-Kante.".into());
    }
    for entity_id in [&input.source_entity_id, &input.target_entity_id] {
        let valid: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM story_entities WHERE id=?1 AND project_id=?2) OR EXISTS(SELECT 1 FROM provisional_entities WHERE id=?1 AND project_id=?2)", params![entity_id,input.project_id], |row| row.get(0)).map_err(|e| e.to_string())?;
        if !valid {
            return Err("Story-Graph-Knoten gehört nicht zum Projekt.".into());
        }
    }
    let source_json =
        serde_json::to_string(&input.source_reference_ids).unwrap_or_else(|_| "[]".into());
    let id = input.id.clone().unwrap_or_else(new_id);
    let stamp = now();
    db.execute("INSERT INTO story_graph_edges(id,project_id,source_entity_id,target_entity_id,relation_type,label,valid_from_chapter_id,valid_from_scene_id,valid_from_offset,valid_until_chapter_id,valid_until_scene_id,valid_until_offset,source_reference_ids_json,confidence,status,author_confirmed,origin,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,COALESCE(?15,'proposed'),?16,?17,COALESCE((SELECT created_at FROM story_graph_edges WHERE id=?1),?18),?18) ON CONFLICT(id) DO UPDATE SET label=excluded.label,relation_type=excluded.relation_type,source_reference_ids_json=excluded.source_reference_ids_json,confidence=excluded.confidence,status=excluded.status,author_confirmed=excluded.author_confirmed,updated_at=excluded.updated_at", params![id,input.project_id,input.source_entity_id,input.target_entity_id,input.relation_type,input.label,input.valid_from_chapter_id,input.valid_from_scene_id,input.valid_from_offset,input.valid_until_chapter_id,input.valid_until_scene_id,input.valid_until_offset,source_json,input.confidence,input.status,input.author_confirmed,input.origin,stamp]).map_err(|e| sql_error("Story-Graph-Kante konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,project_id,source_entity_id,target_entity_id,relation_type,label,valid_from_chapter_id,valid_from_scene_id,valid_from_offset,valid_until_chapter_id,valid_until_scene_id,valid_until_offset,source_reference_ids_json,confidence,status,author_confirmed,origin,created_at,updated_at FROM story_graph_edges WHERE id=?1", params![id], graph_edge_from_row).map_err(|e| sql_error("Story-Graph-Kante konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn review_story_graph_edge(
    state: State<'_, DbState>,
    id: String,
    status: String,
    input: Option<SaveStoryGraphEdgeInput>,
) -> Result<StoryGraphEdge, String> {
    let db = lock_db(&state)?;
    let current = db.query_row("SELECT id,project_id,source_entity_id,target_entity_id,relation_type,label,valid_from_chapter_id,valid_from_scene_id,valid_from_offset,valid_until_chapter_id,valid_until_scene_id,valid_until_offset,source_reference_ids_json,confidence,status,author_confirmed,origin,created_at,updated_at FROM story_graph_edges WHERE id=?1", params![id], graph_edge_from_row).map_err(|e| sql_error("Story-Graph-Kante konnte nicht geladen werden", e))?;
    drop(db);
    let mut next = input.unwrap_or_else(|| SaveStoryGraphEdgeInput {
        id: Some(current.id),
        project_id: current.project_id,
        source_entity_id: current.source_entity_id,
        target_entity_id: current.target_entity_id,
        relation_type: current.relation_type,
        label: current.label,
        valid_from_chapter_id: current.valid_from_chapter_id,
        valid_from_scene_id: current.valid_from_scene_id,
        valid_from_offset: current.valid_from_offset,
        valid_until_chapter_id: current.valid_until_chapter_id,
        valid_until_scene_id: current.valid_until_scene_id,
        valid_until_offset: current.valid_until_offset,
        source_reference_ids: current.source_reference_ids,
        confidence: current.confidence,
        status: Some(status.clone()),
        author_confirmed: status == "confirmed",
        origin: current.origin,
    });
    next.id = Some(id);
    next.status = Some(status.clone());
    next.author_confirmed = status == "confirmed";
    save_story_graph_edge(state, next)
}

fn mindmap_layout_from_row(row: &rusqlite::Row<'_>) -> SqlResult<MindmapLayout> {
    Ok(MindmapLayout {
        id: row.get(0)?,
        project_id: row.get(1)?,
        user_id: row.get(2)?,
        node_id: row.get(3)?,
        position_x: row.get(4)?,
        position_y: row.get(5)?,
        width: row.get(6)?,
        height: row.get(7)?,
        group_id: row.get(8)?,
        hidden: row.get(9)?,
        fixed: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

#[tauri::command]
pub fn list_mindmap_layouts(
    state: State<'_, DbState>,
    project_id: String,
    user_id: String,
) -> Result<Vec<MindmapLayout>, String> {
    let db = lock_db(&state)?;
    let mut stmt = db.prepare("SELECT id,project_id,user_id,node_id,position_x,position_y,width,height,group_id,hidden,fixed,updated_at FROM mindmap_layouts WHERE project_id=?1 AND user_id=?2 ORDER BY node_id").map_err(|e| sql_error("Mindmap-Layout konnte nicht geladen werden", e))?;
    let result = stmt
        .query_map(params![project_id, user_id], mindmap_layout_from_row)
        .map_err(|e| sql_error("Mindmap-Layout konnte nicht geladen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Mindmap-Layout konnte nicht geladen werden", e));
    result
}

#[tauri::command]
pub fn save_mindmap_layout(
    state: State<'_, DbState>,
    input: SaveMindmapLayoutInput,
) -> Result<MindmapLayout, String> {
    let db = lock_db(&state)?;
    if !input.position_x.is_finite()
        || !input.position_y.is_finite()
        || !input.width.is_finite()
        || !input.height.is_finite()
    {
        return Err("Ungültige Mindmap-Position.".into());
    }
    let id = input.id.unwrap_or_else(new_id);
    let stamp = now();
    db.execute("INSERT INTO mindmap_layouts(id,project_id,user_id,node_id,position_x,position_y,width,height,group_id,hidden,fixed,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(project_id,user_id,node_id) DO UPDATE SET position_x=excluded.position_x,position_y=excluded.position_y,width=excluded.width,height=excluded.height,group_id=excluded.group_id,hidden=excluded.hidden,fixed=excluded.fixed,updated_at=excluded.updated_at", params![id,input.project_id,input.user_id,input.node_id,input.position_x,input.position_y,input.width,input.height,input.group_id,input.hidden,input.fixed,stamp]).map_err(|e| sql_error("Mindmap-Layout konnte nicht gespeichert werden", e))?;
    db.query_row("SELECT id,project_id,user_id,node_id,position_x,position_y,width,height,group_id,hidden,fixed,updated_at FROM mindmap_layouts WHERE project_id=?1 AND user_id=?2 AND node_id=?3", params![input.project_id,input.user_id,input.node_id], mindmap_layout_from_row).map_err(|e| sql_error("Mindmap-Layout konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn list_continuity_review_runs(
    state: State<'_, DbState>,
    project_id: String,
    chapter_id: Option<String>,
    scene_id: Option<String>,
) -> Result<Vec<ContinuityReviewRun>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT r.id, r.project_id, r.chapter_id, r.scene_id, r.source_kind, r.content_hash, r.start_offset, r.end_offset, r.provider_id, COALESCE(s.status, r.status), r.created_at, COALESCE(s.completed_at, r.completed_at), COALESCE(s.error_message, r.error_message) FROM continuity_review_runs r LEFT JOIN continuity_review_run_statuses s ON s.run_id=r.id WHERE r.project_id=?1 AND (?2 IS NULL OR r.chapter_id=?2) AND (?3 IS NULL OR r.scene_id=?3) ORDER BY r.created_at DESC").map_err(|error| sql_error("Kontinuitätsprüfungen konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(
            params![project_id, chapter_id, scene_id],
            continuity_run_from_row,
        )
        .map_err(|error| sql_error("Kontinuitätsprüfungen konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Kontinuitätsprüfungen konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn list_continuity_review_findings(
    state: State<'_, DbState>,
    project_id: String,
    run_id: Option<String>,
) -> Result<Vec<ContinuityReviewFinding>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id, run_id, project_id, chapter_id, scene_id, finding_type, severity, subject_entity_id, related_entity_ids_json, related_state_ids_json, related_rule_ids_json, objective_conflict, lore_explanations_json, evidence_excerpt, source_reference_id, counter_evidence_json, counter_evidence_structured_json, confidence, start_offset, end_offset, reason, review_status, user_decision, created_at, updated_at FROM continuity_review_findings WHERE project_id=?1 AND (?2 IS NULL OR run_id=?2) ORDER BY created_at DESC").map_err(|error| sql_error("Kontinuitätswarnungen konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![project_id, run_id], continuity_finding_from_row)
        .map_err(|error| sql_error("Kontinuitätswarnungen konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Kontinuitätswarnungen konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn save_continuity_review_findings(
    state: State<'_, DbState>,
    run_id: String,
    findings: Vec<SaveContinuityFindingInput>,
) -> Result<Vec<ContinuityReviewFinding>, String> {
    let db = lock_db(&state)?;
    let run_project: String = db
        .query_row(
            "SELECT project_id FROM continuity_review_runs WHERE id=?1",
            params![run_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Kontinuitätsprüfung konnte nicht geladen werden", error))?;
    let tx = db.unchecked_transaction().map_err(|error| {
        sql_error(
            "Kontinuitätswarnungen konnten nicht gespeichert werden",
            error,
        )
    })?;
    for input in findings {
        if input.run_id != run_id || input.project_id != run_project {
            return Err("Warnung und Prüflauf gehören nicht zusammen.".into());
        }
        if !continuity_finding_type_valid(&input.finding_type)
            || !continuity_severity_valid(&input.severity)
        {
            return Err("Ungültiger Kontinuitätstyp oder Schweregrad.".into());
        }
        if !(0.0..=1.0).contains(&input.confidence) {
            return Err("Die Continuity-Sicherheit muss zwischen 0 und 1 liegen.".into());
        }
        let status = input.review_status.unwrap_or_else(|| "open".into());
        if !continuity_review_status_valid(&status) {
            return Err("Ungültiger Status der Kontinuitätswarnung.".into());
        }
        if let Some(source_id) = &input.source_reference_id {
            let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2 UNION ALL SELECT 1 FROM project_source_references WHERE id=?1 AND project_id=?2)", params![source_id, input.project_id], |row| row.get(0)).map_err(|error| sql_error("Finding-Quelle konnte nicht geprüft werden", error))?;
            if !valid {
                return Err("Die Finding-Quelle gehört nicht zum Projekt.".into());
            }
        }
        for counter in &input.counter_evidence {
            if let Some(source_id) = &counter.source_reference_id {
                let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2 UNION ALL SELECT 1 FROM project_source_references WHERE id=?1 AND project_id=?2)", params![source_id, input.project_id], |row| row.get(0)).map_err(|error| sql_error("Gegenquelle konnte nicht geprüft werden", error))?;
                if !valid {
                    return Err("Die Gegenquelle gehört nicht zum Projekt.".into());
                }
            }
        }
        for entity_id in input
            .related_entity_ids
            .iter()
            .chain(input.subject_entity_id.iter())
        {
            project_entity_exists(&tx, &input.project_id, entity_id, None)?;
        }
        for state_id in &input.related_state_ids {
            let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM continuity_state_ledger WHERE id=?1 AND project_id=?2)", params![state_id, input.project_id], |row| row.get(0)).map_err(|error| sql_error("Finding-Zustand konnte nicht geprüft werden", error))?;
            if !valid {
                return Err("Das Finding verweist auf einen unbekannten Zustand.".into());
            }
        }
        for rule_id in &input.related_rule_ids {
            let valid: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM project_rules WHERE id=?1 AND project_id=?2)",
                    params![rule_id, input.project_id],
                    |row| row.get(0),
                )
                .map_err(|error| sql_error("Finding-Regel konnte nicht geprüft werden", error))?;
            if !valid {
                return Err("Das Finding verweist auf eine unbekannte Projektregel.".into());
            }
        }
        let id = input.id.unwrap_or_else(new_id);
        let stamp = now();
        tx.execute("INSERT INTO continuity_review_findings (id, run_id, project_id, chapter_id, scene_id, finding_type, severity, subject_entity_id, related_entity_ids_json, related_state_ids_json, related_rule_ids_json, objective_conflict, lore_explanations_json, evidence_excerpt, source_reference_id, counter_evidence_json, counter_evidence_structured_json, confidence, start_offset, end_offset, reason, review_status, user_decision, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,COALESCE((SELECT created_at FROM continuity_review_findings WHERE id=?1),?24),?24) ON CONFLICT(id) DO UPDATE SET objective_conflict=excluded.objective_conflict, lore_explanations_json=excluded.lore_explanations_json, evidence_excerpt=excluded.evidence_excerpt, source_reference_id=excluded.source_reference_id, counter_evidence_json=excluded.counter_evidence_json, counter_evidence_structured_json=excluded.counter_evidence_structured_json, confidence=excluded.confidence, review_status=excluded.review_status, user_decision=excluded.user_decision, updated_at=excluded.updated_at", params![id, run_id, input.project_id, input.chapter_id, input.scene_id, input.finding_type, input.severity, input.subject_entity_id, serde_json::to_string(&input.related_entity_ids).unwrap_or_else(|_| "[]".into()), serde_json::to_string(&input.related_state_ids).unwrap_or_else(|_| "[]".into()), serde_json::to_string(&input.related_rule_ids).unwrap_or_else(|_| "[]".into()), input.objective_conflict, serde_json::to_string(&input.lore_explanations).unwrap_or_else(|_| "[]".into()), input.evidence_excerpt, input.source_reference_id, serde_json::to_string(&input.counter_evidence_excerpts).unwrap_or_else(|_| "[]".into()), serde_json::to_string(&input.counter_evidence).unwrap_or_else(|_| "[]".into()), input.confidence, input.start_offset, input.end_offset, input.reason, status, input.user_decision, stamp]).map_err(|error| sql_error("Kontinuitätswarnung konnte nicht gespeichert werden", error))?;
    }
    tx.commit().map_err(|error| {
        sql_error(
            "Kontinuitätswarnungen konnten nicht gespeichert werden",
            error,
        )
    })?;
    let mut statement = db.prepare("SELECT id, run_id, project_id, chapter_id, scene_id, finding_type, severity, subject_entity_id, related_entity_ids_json, related_state_ids_json, related_rule_ids_json, objective_conflict, lore_explanations_json, evidence_excerpt, source_reference_id, counter_evidence_json, counter_evidence_structured_json, confidence, start_offset, end_offset, reason, review_status, user_decision, created_at, updated_at FROM continuity_review_findings WHERE run_id=?1 ORDER BY created_at").map_err(|error| sql_error("Kontinuitätswarnungen konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![run_id], continuity_finding_from_row)
        .map_err(|error| sql_error("Kontinuitätswarnungen konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Kontinuitätswarnungen konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn review_continuity_finding(
    state: State<'_, DbState>,
    id: String,
    review_status: String,
    user_decision: Option<String>,
) -> Result<ContinuityReviewFinding, String> {
    if !continuity_review_status_valid(&review_status) || review_status == "open" {
        return Err("Ungültige Entscheidung für die Kontinuitätswarnung.".into());
    }
    let db = lock_db(&state)?;
    let current: String = db
        .query_row(
            "SELECT review_status FROM continuity_review_findings WHERE id=?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Kontinuitätswarnung konnte nicht geladen werden", error))?;
    if current != "open" {
        return Err("Diese Kontinuitätswarnung wurde bereits entschieden.".into());
    }
    db.execute("UPDATE continuity_review_findings SET review_status=?2, user_decision=?3, updated_at=?4 WHERE id=?1", params![id, review_status, user_decision, now()]).map_err(|error| sql_error("Entscheidung konnte nicht gespeichert werden", error))?;
    sync_manuscript_artifact(
        &db,
        "continuity_finding",
        &id,
        if review_status == "dismissed" {
            "rejected"
        } else {
            "confirmed"
        },
    )?;
    db.query_row("SELECT id, run_id, project_id, chapter_id, scene_id, finding_type, severity, subject_entity_id, related_entity_ids_json, related_state_ids_json, related_rule_ids_json, objective_conflict, lore_explanations_json, evidence_excerpt, source_reference_id, counter_evidence_json, counter_evidence_structured_json, confidence, start_offset, end_offset, reason, review_status, user_decision, created_at, updated_at FROM continuity_review_findings WHERE id=?1", params![id], continuity_finding_from_row).map_err(|error| sql_error("Entscheidung konnte nicht geladen werden", error))
}

fn continuity_decision_status_valid(value: &str) -> bool {
    matches!(
        value,
        "open"
            | "resolved_after_text_change"
            | "resolved_with_confirmed_rule"
            | "accepted_exception"
            | "deferred_rule_review"
            | "deferred_canon_review"
            | "deferred_open_question"
            | "dismissed"
    )
}

fn continuity_decision_kind_valid(value: &str) -> bool {
    matches!(
        value,
        "text_correction"
            | "confirmed_rule"
            | "new_rule"
            | "canon_review"
            | "intentional_exception"
            | "open_question"
            | "dismiss"
    )
}

fn continuity_decision_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ContinuityFindingDecision> {
    Ok(ContinuityFindingDecision {
        id: row.get(0)?,
        finding_id: row.get(1)?,
        project_id: row.get(2)?,
        status: row.get(3)?,
        decision_kind: row.get(4)?,
        rule_id: row.get(5)?,
        rule_proposal_id: row.get(6)?,
        open_question_entity_id: row.get(7)?,
        source_reference_id: row.get(8)?,
        exception_reason: row.get(9)?,
        content_hash: row.get(10)?,
        payload: serde_json::from_str(&row.get::<_, String>(11)?).unwrap_or(serde_json::json!({})),
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn continuity_canon_audit_from_row(
    row: &rusqlite::Row<'_>,
) -> SqlResult<ContinuityCanonChangeAudit> {
    Ok(ContinuityCanonChangeAudit {
        id: row.get(0)?,
        finding_id: row.get(1)?,
        project_id: row.get(2)?,
        target_entity_id: row.get(3)?,
        target_state_id: row.get(4)?,
        action: row.get(5)?,
        reason: row.get(6)?,
        previous_source_reference_id: row.get(7)?,
        new_source_reference_id: row.get(8)?,
        source_reference_ids: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
        payload: serde_json::from_str(&row.get::<_, String>(10)?).unwrap_or(serde_json::json!({})),
        created_at: row.get(11)?,
    })
}

#[tauri::command]
pub fn list_continuity_finding_decisions(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<ContinuityFindingDecision>, String> {
    let db = lock_db(&state)?;
    let result = db.prepare("SELECT id, finding_id, project_id, status, decision_kind, rule_id, rule_proposal_id, open_question_entity_id, source_reference_id, exception_reason, content_hash, payload_json, created_at, updated_at FROM continuity_review_decisions WHERE project_id=?1 ORDER BY updated_at DESC").map_err(|error| sql_error("Continuity-Entscheidungen konnten nicht geladen werden", error))?.query_map(params![project_id], continuity_decision_from_row).map_err(|error| sql_error("Continuity-Entscheidungen konnten nicht geladen werden", error))?.collect::<SqlResult<Vec<_>>>().map_err(|error| sql_error("Continuity-Entscheidungen konnten nicht gelesen werden", error));
    result
}

#[tauri::command]
pub fn apply_continuity_finding_decision(
    state: State<'_, DbState>,
    input: ApplyContinuityFindingDecisionInput,
) -> Result<ContinuityFindingDecision, String> {
    if !continuity_decision_status_valid(&input.status)
        || !continuity_decision_kind_valid(&input.decision_kind)
    {
        return Err("Ungültige Continuity-Entscheidung.".into());
    }
    if input.status == "accepted_exception"
        && input
            .exception_reason
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return Err("Eine bewusste Ausnahme benötigt eine Begründung.".into());
    }
    let db = lock_db(&state)?;
    let (finding_project, _finding_scene, finding_status, finding_source): (String, Option<String>, String, Option<String>) = db.query_row("SELECT project_id, scene_id, review_status, source_reference_id FROM continuity_review_findings WHERE id=?1", params![input.finding_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).map_err(|error| sql_error("Kontinuitätswarnung konnte nicht geladen werden", error))?;
    if finding_project != input.project_id {
        return Err("Finding und Entscheidung gehören nicht zum Projekt.".into());
    }
    if input.status != "open" && finding_status != "open" {
        return Err("Diese Kontinuitätswarnung wurde bereits entschieden.".into());
    }
    if let Some(rule_id) = &input.rule_id {
        let valid: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM project_rules WHERE id=?1 AND project_id=?2 AND status='confirmed' AND author_confirmed=1)", params![rule_id, input.project_id], |row| row.get(0)).map_err(|error| sql_error("Projektregel konnte nicht geprüft werden", error))?;
        if !valid {
            return Err("Nur bestätigte Projektregeln können ausgewählt werden.".into());
        }
    }
    if let Some(proposal_id) = &input.rule_proposal_id {
        let valid: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM project_rule_proposals WHERE id=?1 AND project_id=?2)",
                params![proposal_id, input.project_id],
                |row| row.get(0),
            )
            .map_err(|error| sql_error("Regelvorschlag konnte nicht geprüft werden", error))?;
        if !valid {
            return Err("Der Regelvorschlag gehört nicht zum Projekt.".into());
        }
    }
    if let Some(entity_id) = &input.open_question_entity_id {
        project_entity_exists(&db, &input.project_id, entity_id, Some("open_question"))?;
    }
    if let Some(source_id) = &input.source_reference_id {
        let valid: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2)", params![source_id, input.project_id], |row| row.get(0)).map_err(|error| sql_error("Entscheidungsquelle konnte nicht geprüft werden", error))?;
        if !valid {
            return Err("Die Entscheidungsquelle gehört nicht zum Projekt.".into());
        }
    }
    if let Some(action) = &input.canon_action {
        if !matches!(
            action.as_str(),
            "previous_incomplete"
                | "retcon"
                | "new_information"
                | "unreliable_perspective"
                | "cancelled"
        ) {
            return Err("Ungültige Kanonaktion.".into());
        }
        if input
            .canon_reason
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            return Err("Eine Kanonentscheidung benötigt eine Begründung.".into());
        }
        if let Some(entity_id) = &input.canon_target_entity_id {
            project_entity_exists(&db, &input.project_id, entity_id, None)?;
        }
        if let Some(state_id) = &input.canon_target_state_id {
            let valid: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM continuity_state_ledger WHERE id=?1 AND project_id=?2)", params![state_id, input.project_id], |row| row.get(0)).map_err(|error| sql_error("Kanon-Zustand konnte nicht geprüft werden", error))?;
            if !valid {
                return Err("Der Kanon-Zustand gehört nicht zum Projekt.".into());
            }
        }
        for source_id in &input.canon_source_reference_ids {
            let valid: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2)", params![source_id, input.project_id], |row| row.get(0)).map_err(|error| sql_error("Kanonquelle konnte nicht geprüft werden", error))?;
            if !valid {
                return Err("Eine Kanonquelle gehört nicht zum Projekt.".into());
            }
        }
    }
    let stamp = now();
    let tx = db.unchecked_transaction().map_err(|error| {
        sql_error(
            "Continuity-Entscheidung konnte nicht gestartet werden",
            error,
        )
    })?;
    let decision_id: String = tx
        .query_row(
            "SELECT id FROM continuity_review_decisions WHERE finding_id=?1",
            params![input.finding_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| sql_error("Continuity-Entscheidung konnte nicht gelesen werden", error))?
        .unwrap_or_else(new_id);
    tx.execute("INSERT INTO continuity_review_decisions (id, finding_id, project_id, status, decision_kind, rule_id, rule_proposal_id, open_question_entity_id, source_reference_id, exception_reason, content_hash, payload_json, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,COALESCE((SELECT created_at FROM continuity_review_decisions WHERE id=?1),?13),?13) ON CONFLICT(finding_id) DO UPDATE SET status=excluded.status, decision_kind=excluded.decision_kind, rule_id=excluded.rule_id, rule_proposal_id=excluded.rule_proposal_id, open_question_entity_id=excluded.open_question_entity_id, source_reference_id=excluded.source_reference_id, exception_reason=excluded.exception_reason, content_hash=excluded.content_hash, payload_json=excluded.payload_json, updated_at=excluded.updated_at", params![decision_id, input.finding_id, input.project_id, input.status, input.decision_kind, input.rule_id, input.rule_proposal_id, input.open_question_entity_id, input.source_reference_id, input.exception_reason, input.content_hash, serde_json::to_string(&input.payload).unwrap_or_else(|_| "{}".into()), stamp]).map_err(|error| sql_error("Continuity-Entscheidung konnte nicht gespeichert werden", error))?;
    let legacy_status = match input.status.as_str() {
        "dismissed" => "dismissed",
        "resolved_after_text_change" | "resolved_with_confirmed_rule" => "resolved",
        "accepted_exception" => "accepted",
        "deferred_rule_review" | "deferred_canon_review" | "deferred_open_question" => "deferred",
        _ => "open",
    };
    tx.execute("UPDATE continuity_review_findings SET review_status=?2, user_decision=?3, updated_at=?4 WHERE id=?1", params![input.finding_id, legacy_status, input.decision_kind, stamp]).map_err(|error| sql_error("Findingstatus konnte nicht gespeichert werden", error))?;
    if let Some(action) = &input.canon_action {
        tx.execute("INSERT INTO continuity_canon_change_audits (id, finding_id, project_id, target_entity_id, target_state_id, action, reason, previous_source_reference_id, new_source_reference_id, source_reference_ids_json, payload_json, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)", params![new_id(), input.finding_id, input.project_id, input.canon_target_entity_id, input.canon_target_state_id, action, input.canon_reason.as_deref().unwrap_or_default(), finding_source, input.source_reference_id, serde_json::to_string(&input.canon_source_reference_ids).unwrap_or_else(|_| "[]".into()), serde_json::to_string(&input.payload).unwrap_or_else(|_| "{}".into()), stamp]).map_err(|error| sql_error("Kanon-Audit konnte nicht gespeichert werden", error))?;
    }
    tx.commit().map_err(|error| {
        sql_error(
            "Continuity-Entscheidung konnte nicht abgeschlossen werden",
            error,
        )
    })?;
    sync_manuscript_artifact(
        &db,
        "continuity_finding",
        &input.finding_id,
        if input.status == "dismissed" {
            "rejected"
        } else {
            "confirmed"
        },
    )?;
    db.query_row("SELECT id, finding_id, project_id, status, decision_kind, rule_id, rule_proposal_id, open_question_entity_id, source_reference_id, exception_reason, content_hash, payload_json, created_at, updated_at FROM continuity_review_decisions WHERE finding_id=?1", params![input.finding_id], continuity_decision_from_row).map_err(|error| sql_error("Gespeicherte Continuity-Entscheidung konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn reconcile_continuity_text_correction(
    state: State<'_, DbState>,
    input: ReconcileContinuityTextCorrectionInput,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    let run_project: String = db
        .query_row(
            "SELECT project_id FROM continuity_review_runs WHERE id=?1",
            params![input.run_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Continuity-Run konnte nicht geladen werden", error))?;
    if run_project != input.project_id {
        return Err("Continuity-Run und Textkorrektur gehören nicht zusammen.".into());
    }
    let tx = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Textkorrektur konnte nicht abgeglichen werden", error))?;
    let mut statement = tx.prepare("SELECT d.id, f.finding_type, f.subject_entity_id, f.scene_id FROM continuity_review_decisions d JOIN continuity_review_findings f ON f.id=d.finding_id WHERE d.project_id=?1 AND d.decision_kind='text_correction' AND d.status='open' AND f.scene_id=?2") .map_err(|error| sql_error("Textkorrekturen konnten nicht geladen werden", error))?;
    let rows = statement
        .query_map(params![input.project_id, input.scene_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|error| sql_error("Textkorrekturen konnten nicht gelesen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Textkorrekturen konnten nicht gelesen werden", error))?;
    for (decision_id, finding_type, subject_entity_id) in rows {
        let remains: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM continuity_review_findings WHERE run_id=?1 AND finding_type=?2 AND (subject_entity_id=?3 OR (subject_entity_id IS NULL AND ?3 IS NULL)) AND review_status='open')", params![input.run_id, finding_type, subject_entity_id], |row| row.get(0)).map_err(|error| sql_error("Textkorrektur konnte nicht abgeglichen werden", error))?;
        if !remains {
            tx.execute("UPDATE continuity_review_decisions SET status='resolved_after_text_change', content_hash=?2, updated_at=?3 WHERE id=?1", params![decision_id, input.content_hash, now()]).map_err(|error| sql_error("Textkorrekturentscheidung konnte nicht abgeschlossen werden", error))?;
            let finding_id: String = tx
                .query_row(
                    "SELECT finding_id FROM continuity_review_decisions WHERE id=?1",
                    params![decision_id],
                    |row| row.get(0),
                )
                .map_err(|error| {
                    sql_error("Textkorrektur-Finding konnte nicht geladen werden", error)
                })?;
            tx.execute("UPDATE continuity_review_findings SET review_status='resolved', user_decision='resolved_after_text_change', updated_at=?2 WHERE id=?1", params![finding_id, now()]).map_err(|error| sql_error("Finding konnte nach Textkorrektur nicht gelöst werden", error))?;
        }
    }
    drop(statement);
    tx.commit()
        .map_err(|error| sql_error("Textkorrektur konnte nicht abgeschlossen werden", error))
}

#[tauri::command]
pub fn list_continuity_canon_change_audits(
    state: State<'_, DbState>,
    project_id: String,
    finding_id: Option<String>,
) -> Result<Vec<ContinuityCanonChangeAudit>, String> {
    let db = lock_db(&state)?;
    let result = db.prepare("SELECT id, finding_id, project_id, target_entity_id, target_state_id, action, reason, previous_source_reference_id, new_source_reference_id, source_reference_ids_json, payload_json, created_at FROM continuity_canon_change_audits WHERE project_id=?1 AND (?2 IS NULL OR finding_id=?2) ORDER BY created_at DESC").map_err(|error| sql_error("Kanon-Audits konnten nicht geladen werden", error))?.query_map(params![project_id, finding_id], continuity_canon_audit_from_row).map_err(|error| sql_error("Kanon-Audits konnten nicht gelesen werden", error))?.collect::<SqlResult<Vec<_>>>().map_err(|error| sql_error("Kanon-Audits konnten nicht gelesen werden", error));
    result
}

fn validate_plot_thread(db: &Connection, project_id: &str, entity_id: &str) -> Result<(), String> {
    project_entity_exists(db, project_id, entity_id, Some("plot_thread"))
}

#[tauri::command]
pub fn list_plot_thread_lifecycles(
    state: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<PlotThreadLifecycle>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id, project_id, entity_id, lifecycle_status, last_source_reference_id, updated_at FROM plot_thread_lifecycle WHERE project_id=?1 ORDER BY updated_at DESC").map_err(|error| sql_error("Handlungsstränge konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![project_id], lifecycle_from_row)
        .map_err(|error| sql_error("Handlungsstränge konnten nicht geladen werden", error))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| sql_error("Handlungsstränge konnten nicht geladen werden", error));
    result
}

#[tauri::command]
pub fn save_plot_thread_lifecycle(
    state: State<'_, DbState>,
    input: SavePlotThreadLifecycleInput,
) -> Result<PlotThreadLifecycle, String> {
    if !lifecycle_status_valid(&input.lifecycle_status) {
        return Err("Unbekannter Handlungsstrangstatus.".into());
    }
    let db = lock_db(&state)?;
    validate_plot_thread(&db, &input.project_id, &input.entity_id)?;
    let id = input.id.unwrap_or_else(new_id);
    let stamp = now();
    db.execute("INSERT INTO plot_thread_lifecycle (id, project_id, entity_id, lifecycle_status, last_source_reference_id, updated_at) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(entity_id) DO UPDATE SET lifecycle_status=excluded.lifecycle_status, last_source_reference_id=excluded.last_source_reference_id, updated_at=excluded.updated_at", params![id, input.project_id, input.entity_id, input.lifecycle_status, input.last_source_reference_id, stamp]).map_err(|error| sql_error("Handlungsstrangstatus konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT id, project_id, entity_id, lifecycle_status, last_source_reference_id, updated_at FROM plot_thread_lifecycle WHERE entity_id=?1", params![input.entity_id], lifecycle_from_row).map_err(|error| sql_error("Handlungsstrangstatus konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn list_plot_thread_lifecycle_proposals(
    state: State<'_, DbState>,
    project_id: String,
    run_id: Option<String>,
) -> Result<Vec<PlotThreadLifecycleProposal>, String> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare("SELECT id, run_id, project_id, entity_id, proposed_status, evidence_excerpt, source_reference_id, start_offset, end_offset, reason, confidence, review_status, reviewed_at, created_at FROM plot_thread_lifecycle_proposals WHERE project_id=?1 AND (?2 IS NULL OR run_id=?2) ORDER BY created_at DESC").map_err(|error| sql_error("Handlungsstrang-Vorschläge konnten nicht geladen werden", error))?;
    let result = statement
        .query_map(params![project_id, run_id], lifecycle_proposal_from_row)
        .map_err(|error| {
            sql_error(
                "Handlungsstrang-Vorschläge konnten nicht geladen werden",
                error,
            )
        })?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|error| {
            sql_error(
                "Handlungsstrang-Vorschläge konnten nicht geladen werden",
                error,
            )
        });
    result
}

#[tauri::command]
pub fn save_plot_thread_lifecycle_proposal(
    state: State<'_, DbState>,
    input: SavePlotThreadLifecycleProposalInput,
) -> Result<PlotThreadLifecycleProposal, String> {
    if !lifecycle_status_valid(&input.proposed_status) {
        return Err("Unbekannter Handlungsstrangstatus.".into());
    }
    let db = lock_db(&state)?;
    validate_plot_thread(&db, &input.project_id, &input.entity_id)?;
    let run_project: String = db
        .query_row(
            "SELECT project_id FROM continuity_review_runs WHERE id=?1",
            params![input.run_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Kontinuitätsrun konnte nicht geladen werden", error))?;
    if run_project != input.project_id {
        return Err("Handlungsstrang-Vorschlag und Prüflauf gehören nicht zusammen.".into());
    }
    if let Some(source_id) = &input.source_reference_id {
        let valid: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2)", params![source_id, input.project_id], |row| row.get(0)).map_err(|error| sql_error("Handlungsstrangquelle konnte nicht geprüft werden", error))?;
        if !valid {
            return Err("Die Handlungsstrangquelle gehört nicht zum Projekt.".into());
        }
    }
    let id = input.id.unwrap_or_else(new_id);
    let status = input.review_status.unwrap_or_else(|| "pending".into());
    if status != "pending" && !matches!(status.as_str(), "accepted" | "edited" | "rejected") {
        return Err("Ungültiger Handlungsstrang-Reviewstatus.".into());
    }
    if input.proposed_status == "resolved" {
        return Err("Der AI-Vorschlag darf keinen resolved-Status setzen.".into());
    }
    validate_probability(input.confidence, "Die Handlungsstrang-Confidence")?;
    db.execute("INSERT INTO plot_thread_lifecycle_proposals (id, run_id, project_id, entity_id, proposed_status, evidence_excerpt, source_reference_id, start_offset, end_offset, reason, confidence, review_status, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(id) DO UPDATE SET proposed_status=excluded.proposed_status, reason=excluded.reason, source_reference_id=excluded.source_reference_id, confidence=excluded.confidence, review_status=excluded.review_status", params![id, input.run_id, input.project_id, input.entity_id, input.proposed_status, input.evidence_excerpt, input.source_reference_id, input.start_offset, input.end_offset, input.reason, input.confidence, status, now()]).map_err(|error| sql_error("Handlungsstrang-Vorschlag konnte nicht gespeichert werden", error))?;
    db.query_row("SELECT id, run_id, project_id, entity_id, proposed_status, evidence_excerpt, source_reference_id, start_offset, end_offset, reason, confidence, review_status, reviewed_at, created_at FROM plot_thread_lifecycle_proposals WHERE id=?1", params![id], lifecycle_proposal_from_row).map_err(|error| sql_error("Handlungsstrang-Vorschlag konnte nicht geladen werden", error))
}

#[tauri::command]
pub fn review_plot_thread_lifecycle_proposal(
    state: State<'_, DbState>,
    id: String,
    review_status: String,
    lifecycle_status: Option<String>,
) -> Result<PlotThreadLifecycleProposal, String> {
    if !matches!(review_status.as_str(), "accepted" | "edited" | "rejected") {
        return Err("Ungültiger Handlungsstrang-Reviewstatus.".into());
    }
    let db = lock_db(&state)?;
    let proposal = db.query_row("SELECT id, run_id, project_id, entity_id, proposed_status, evidence_excerpt, source_reference_id, start_offset, end_offset, reason, confidence, review_status, reviewed_at, created_at FROM plot_thread_lifecycle_proposals WHERE id=?1", params![id], lifecycle_proposal_from_row).map_err(|error| sql_error("Handlungsstrang-Vorschlag konnte nicht geladen werden", error))?;
    if proposal.review_status != "pending" {
        return Err("Dieser Handlungsstrang-Vorschlag wurde bereits geprüft.".into());
    }
    let tx = db
        .unchecked_transaction()
        .map_err(|error| sql_error("Handlungsstrangreview konnte nicht gestartet werden", error))?;
    if review_status != "rejected" {
        let status = lifecycle_status.unwrap_or_else(|| proposal.proposed_status.clone());
        if !lifecycle_status_valid(&status) {
            return Err("Unbekannter Handlungsstrangstatus.".into());
        }
        tx.execute("INSERT INTO plot_thread_lifecycle (id, project_id, entity_id, lifecycle_status, last_source_reference_id, updated_at) VALUES (COALESCE((SELECT id FROM plot_thread_lifecycle WHERE entity_id=?1),?2),?3,?1,?4,?5,?6) ON CONFLICT(entity_id) DO UPDATE SET lifecycle_status=excluded.lifecycle_status, last_source_reference_id=excluded.last_source_reference_id, updated_at=excluded.updated_at", params![proposal.entity_id, new_id(), proposal.project_id, status, proposal.source_reference_id, now()]).map_err(|error| sql_error("Handlungsstrangstatus konnte nicht übernommen werden", error))?;
    }
    tx.execute(
        "UPDATE plot_thread_lifecycle_proposals SET review_status=?2, reviewed_at=?3 WHERE id=?1",
        params![id, review_status, now()],
    )
    .map_err(|error| {
        sql_error(
            "Handlungsstrangreview konnte nicht gespeichert werden",
            error,
        )
    })?;
    tx.commit().map_err(|error| {
        sql_error(
            "Handlungsstrangreview konnte nicht abgeschlossen werden",
            error,
        )
    })?;
    sync_manuscript_artifact(
        &db,
        "plot_thread_proposal",
        &id,
        if review_status == "rejected" {
            "rejected"
        } else {
            "confirmed"
        },
    )?;
    db.query_row("SELECT id, run_id, project_id, entity_id, proposed_status, evidence_excerpt, source_reference_id, start_offset, end_offset, reason, confidence, review_status, reviewed_at, created_at FROM plot_thread_lifecycle_proposals WHERE id=?1", params![id], lifecycle_proposal_from_row).map_err(|error| sql_error("Handlungsstrangreview konnte nicht geladen werden", error))
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

fn validate_book_project(db: &Connection, project_id: &str, book_id: &str) -> Result<(), String> {
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM books WHERE id=?1 AND project_id=?2)",
            params![book_id, project_id],
            |row| row.get(0),
        )
        .map_err(|error| sql_error("Buch konnte nicht geprüft werden", error))?;
    if exists {
        Ok(())
    } else {
        Err("Das Buch gehört nicht zum Projekt.".into())
    }
}
fn validate_chapter_project(
    db: &Connection,
    project_id: &str,
    chapter_id: &str,
) -> Result<(), String> {
    let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM chapters JOIN books ON books.id=chapters.book_id WHERE chapters.id=?1 AND books.project_id=?2)", params![chapter_id, project_id], |row| row.get(0)).map_err(|error| sql_error("Kapitel konnte nicht geprüft werden", error))?;
    if exists {
        Ok(())
    } else {
        Err("Das Kapitel gehört nicht zu diesem Projekt.".into())
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
    sync_manuscript_artifact(
        &db,
        "character_memory_proposal",
        &input.proposal_id,
        if status == "rejected" {
            "rejected"
        } else if status == "accepted" || status == "edited" {
            "confirmed"
        } else {
            "uncertain"
        },
    )?;
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
        content_hash: row.get(11)?,
        draft_context_hash: row.get(12)?,
        draft_state: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

#[tauri::command]
pub fn list_chapter_generation_sections(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<ChapterGenerationSection>, String> {
    let db = lock_db(&state)?;
    let mut s=db.prepare("SELECT id,job_id,plan_beat_id,order_index,target_words,actual_words,content,continuation_summary,continuity_state_json,status,provider_id,content_hash,draft_context_hash,draft_state,created_at,updated_at FROM chapter_generation_sections WHERE job_id=?1 ORDER BY order_index").map_err(|e|sql_error("Kapitelabschnitte konnten nicht geladen werden",e))?;
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
    let content_hash = input
        .content_hash
        .unwrap_or_else(|| canonical_content_hash(&input.content));
    let draft_context_hash = input.draft_context_hash.unwrap_or_default();
    let draft_state = input.draft_state.unwrap_or_else(|| "valid".into());
    if !matches!(
        draft_state.as_str(),
        "valid" | "stale" | "regenerate_requested"
    ) {
        return Err("Ungültiger Draft-Abschnittsstatus.".into());
    }
    db.execute("INSERT INTO chapter_generation_sections(id,job_id,plan_beat_id,order_index,target_words,actual_words,content,continuation_summary,continuity_state_json,status,provider_id,content_hash,draft_context_hash,draft_state,created_at,updated_at) VALUES(COALESCE((SELECT id FROM chapter_generation_sections WHERE job_id=?1 AND order_index=?2),?3),?1,?4,?2,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,COALESCE((SELECT created_at FROM chapter_generation_sections WHERE job_id=?1 AND order_index=?2),?15),?15) ON CONFLICT(job_id,order_index) DO UPDATE SET plan_beat_id=excluded.plan_beat_id,target_words=excluded.target_words,actual_words=excluded.actual_words,content=excluded.content,continuation_summary=excluded.continuation_summary,continuity_state_json=excluded.continuity_state_json,status=excluded.status,provider_id=excluded.provider_id,content_hash=excluded.content_hash,draft_context_hash=excluded.draft_context_hash,draft_state=excluded.draft_state,updated_at=excluded.updated_at",params![input.job_id,input.order_index,id,input.plan_beat_id,input.target_words,actual,input.content,input.continuation_summary,state_json,input.status,input.provider_id,content_hash,draft_context_hash,draft_state,stamp]).map_err(|e|sql_error("Kapitelabschnitt konnte nicht gespeichert werden",e))?;
    db.query_row("SELECT id,job_id,plan_beat_id,order_index,target_words,actual_words,content,continuation_summary,continuity_state_json,status,provider_id,content_hash,draft_context_hash,draft_state,created_at,updated_at FROM chapter_generation_sections WHERE job_id=?1 AND order_index=?2",params![input.job_id,input.order_index],section_from_row).map_err(|e|sql_error("Gespeicherter Kapitelabschnitt konnte nicht geladen werden",e))
}

fn review_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ChapterGenerationReview> {
    Ok(ChapterGenerationReview {
        id: row.get(0)?,
        job_id: row.get(1)?,
        section_id: row.get(2)?,
        continuity_run_id: row.get(3)?,
        review_scope: row.get(4)?,
        issue_type: row.get(5)?,
        severity: row.get(6)?,
        title: row.get(7)?,
        description: row.get(8)?,
        related_entity_ids: json_array(row.get(9)?).unwrap_or_default(),
        related_source_ids: json_array(row.get(10)?).unwrap_or_default(),
        suggested_action: row.get(11)?,
        status: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

#[tauri::command]
pub fn list_chapter_generation_reviews(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<ChapterGenerationReview>, String> {
    let db = lock_db(&state)?;
    let mut s=db.prepare("SELECT id,job_id,section_id,continuity_run_id,review_scope,issue_type,severity,title,description,related_entity_ids_json,related_source_ids_json,suggested_action,status,created_at,updated_at FROM chapter_generation_reviews WHERE job_id=?1 ORDER BY created_at").map_err(|e|sql_error("Kapitelprüfungen konnten nicht geladen werden",e))?;
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
        if let Some(section_id) = &review.section_id {
            let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM chapter_generation_sections WHERE id=?1 AND job_id=?2)", params![section_id, job_id], |row| row.get(0)).map_err(|e| sql_error("Prüfungsabschnitt konnte nicht geprüft werden", e))?;
            if !valid {
                return Err("Die Prüfung gehört nicht zum Schreibauftrag.".into());
            }
        }
        if let Some(run_id) = &review.continuity_run_id {
            let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM continuity_review_runs WHERE id=?1 AND project_id=(SELECT project_id FROM chapter_generation_jobs WHERE id=?2))", params![run_id, job_id], |row| row.get(0)).map_err(|e| sql_error("Continuity-Run konnte nicht geprüft werden", e))?;
            if !valid {
                return Err("Der Continuity-Run gehört nicht zum Projekt.".into());
            }
        }
        for entity_id in &review.related_entity_ids {
            let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM story_entities WHERE id=?1 AND project_id=(SELECT project_id FROM chapter_generation_jobs WHERE id=?2))", params![entity_id, job_id], |row| row.get(0)).map_err(|e| sql_error("Prüfungsentität konnte nicht geprüft werden", e))?;
            if !valid {
                return Err("Eine Prüfungsentität gehört nicht zum Projekt.".into());
            }
        }
        for source_id in &review.related_source_ids {
            let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=(SELECT project_id FROM chapter_generation_jobs WHERE id=?2))", params![source_id, job_id], |row| row.get(0)).map_err(|e| sql_error("Prüfungsquelle konnte nicht geprüft werden", e))?;
            if !valid {
                return Err("Eine Prüfungsquelle gehört nicht zum Projekt.".into());
            }
        }
        let id = new_id();
        let related_entities = serde_json::to_string(&review.related_entity_ids)
            .map_err(|e| sql_error("Prüfungsbezug konnte nicht serialisiert werden", e))?;
        let related_sources = serde_json::to_string(&review.related_source_ids)
            .map_err(|e| sql_error("Prüfungsquellen konnten nicht serialisiert werden", e))?;
        tx.execute("INSERT INTO chapter_generation_reviews(id,job_id,section_id,continuity_run_id,review_scope,issue_type,severity,title,description,related_entity_ids_json,related_source_ids_json,suggested_action,status,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14)", params![id, job_id, review.section_id, review.continuity_run_id, review.review_scope, review.issue_type, review.severity, review.title, review.description, related_entities, related_sources, review.suggested_action, review.status, now()]).map_err(|e| sql_error("Kapitelprüfung konnte nicht gespeichert werden", e))?;
    }
    tx.commit()
        .map_err(|e| sql_error("Kapitelprüfung konnte nicht abgeschlossen werden", e))?;
    let mut stmt = db.prepare("SELECT id,job_id,section_id,continuity_run_id,review_scope,issue_type,severity,title,description,related_entity_ids_json,related_source_ids_json,suggested_action,status,created_at,updated_at FROM chapter_generation_reviews WHERE job_id=?1 ORDER BY created_at").map_err(|e| sql_error("Kapitelprüfungen konnten nicht geladen werden", e))?;
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
    db.query_row("SELECT id,job_id,section_id,continuity_run_id,review_scope,issue_type,severity,title,description,related_entity_ids_json,related_source_ids_json,suggested_action,status,created_at,updated_at FROM chapter_generation_reviews WHERE id=?1", params![id], review_from_row).map_err(|e| sql_error("Kapitelprüfung konnte nicht geladen werden", e))
}

#[tauri::command]
pub fn accept_chapter_generation_job(
    state: State<'_, DbState>,
    input: AcceptChapterGenerationJobInput,
) -> Result<ChapterGenerationJob, String> {
    let db = lock_db(&state)?;
    accept_chapter_generation_job_in_db(&db, input)
}

fn accept_chapter_generation_job_in_db(
    db: &Connection,
    input: AcceptChapterGenerationJobInput,
) -> Result<ChapterGenerationJob, String> {
    let job = load_job(db, &input.job_id)?;
    if job.status != "draft_ready" {
        return Err("Der Entwurf ist noch nicht zur Übernahme bereit.".into());
    }
    if input.current_context_hash != job.content_context_hash && !job.context_override_accepted {
        return Err("Der Projektkontext hat sich seit Beginn des Entwurfs verändert.".into());
    }
    let plan = db
        .query_row(plan_select(), params![input.job_id], plan_from_row)
        .map_err(|e| sql_error("Kapitelplan fehlt", e))?;
    let sections = list_chapter_generation_sections_from_db(db, &job.id)?;
    if sections.is_empty() || sections.len() != plan.beats.len() {
        return Err("Der Entwurf enthält noch keine Abschnitte.".into());
    }
    if plan.review_status != "accepted"
        || sections.iter().any(|section| {
            section.content.trim().is_empty()
                || matches!(
                    section.status.as_str(),
                    "pending" | "regenerate_requested" | "failed"
                )
                || matches!(
                    section.draft_state.as_str(),
                    "stale" | "regenerate_requested"
                )
                || (!section.content_hash.is_empty()
                    && canonical_content_hash(&section.content) != section.content_hash)
        })
    {
        return Err("Plan und alle Abschnitte müssen vor der Übernahme bestätigt sein.".into());
    }
    let draft_mismatch: i64 = db.query_row("SELECT COUNT(*) FROM chapter_generation_draft_ledger d JOIN chapter_generation_sections s ON s.id=d.section_id WHERE d.job_id=?1 AND d.status='proposed' AND d.content_hash<>s.content_hash", params![input.job_id], |row| row.get(0)).map_err(|e| sql_error("Draft-Ledger konnte nicht geprüft werden", e))?;
    if draft_mismatch > 0 {
        return Err("Der Draft-Ledger passt nicht mehr zum aktuellen Abschnittstext.".into());
    }
    let blocking: i64 = db.query_row("SELECT COUNT(*) FROM chapter_generation_reviews WHERE job_id=?1 AND severity='blocking' AND status='open'", params![input.job_id], |row| row.get(0)).map_err(|e| sql_error("Kapitelprüfungen konnten nicht geprüft werden", e))?;
    if blocking > 0 {
        return Err("Offene blockierende Kapitelprüfung verhindert die Übernahme.".into());
    }
    let critical: i64 = db.query_row("SELECT COUNT(*) FROM continuity_review_findings WHERE severity='critical' AND review_status='open' AND run_id IN (SELECT continuity_run_id FROM chapter_generation_reviews WHERE job_id=?1 AND continuity_run_id IS NOT NULL)", params![input.job_id], |row| row.get(0)).map_err(|e| sql_error("Continuity-Findings konnten nicht geprüft werden", e))?;
    if critical > 0 {
        return Err("Offene kritische Continuity-Findings verhindern die Übernahme.".into());
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
            is_implicit: false,
            created_at: now(),
            updated_at: now(),
        };
        insert_scene_version_in_transaction(&tx, &scene, &now(), "manual")?;
        let mut draft_statement = tx.prepare("SELECT id,entity_id,related_entity_id,state_kind,previous_state,new_state,source_excerpt,source_start_offset,source_end_offset,content_hash,confidence FROM chapter_generation_draft_ledger WHERE job_id=?1 AND section_id=?2 AND status='proposed' ORDER BY created_at").map_err(|e| sql_error("Draft-Zustände konnten nicht geladen werden", e))?;
        let drafts = draft_statement
            .query_map(params![job.id, section.id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, f64>(10)?,
                ))
            })
            .map_err(|e| sql_error("Draft-Zustände konnten nicht geladen werden", e))?
            .collect::<SqlResult<Vec<_>>>()
            .map_err(|e| sql_error("Draft-Zustände konnten nicht geladen werden", e))?;
        drop(draft_statement);
        for (
            draft_id,
            entity_id,
            related_entity_id,
            state_kind,
            previous_state,
            new_state,
            excerpt,
            start_offset,
            end_offset,
            draft_hash,
            confidence,
        ) in drafts
        {
            if draft_hash != section.content_hash {
                return Err("Ein Draft-Zustand gehört nicht zum aktuellen Abschnittstext.".into());
            }
            let source_id = new_id();
            tx.execute("INSERT INTO story_source_references(id,project_id,entity_id,chapter_id,scene_id,excerpt,start_offset,end_offset) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", params![source_id, job.project_id, entity_id, chapter_id, scene_id, excerpt, start_offset, end_offset]).map_err(|e| sql_error("Draft-Quelle konnte nicht gespeichert werden", e))?;
            tx.execute("INSERT INTO continuity_state_ledger(id,project_id,entity_id,related_entity_id,state_kind,previous_state,new_state,reason,evidence_excerpt,chapter_id,scene_id,start_offset,end_offset,source_reference_id,status,confidence,author_confirmed) VALUES(?1,?2,?3,?4,?5,?6,?7,'Longform-Draft: Nutzerreview erforderlich',?8,?9,?10,?11,?12,?13,'proposed',?14,0)", params![new_id(), job.project_id, entity_id, related_entity_id, state_kind, previous_state, new_state, excerpt, chapter_id, scene_id, start_offset, end_offset, source_id, confidence]).map_err(|e| sql_error("Draft-Zustand konnte nicht als Vorschlag gespeichert werden", e))?;
            tx.execute("UPDATE chapter_generation_draft_ledger SET status='accepted_for_manuscript_review',source_reference_id=?1,updated_at=?2 WHERE id=?3 AND job_id=?4", params![source_id, now(), draft_id, job.id]).map_err(|e| sql_error("Draft-Zustand konnte nicht markiert werden", e))?;
        }
    }
    tx.execute("UPDATE chapter_generation_jobs SET status='accepted',completed_at=?1,updated_at=?1 WHERE id=?2",params![now(),job.id]).map_err(|e|sql_error("Schreibauftrag konnte nicht abgeschlossen werden",e))?;
    tx.commit()
        .map_err(|e| sql_error("Kapitelübernahme konnte nicht abgeschlossen werden", e))?;
    load_job(db, &job.id)
}

fn list_chapter_generation_sections_from_db(
    db: &Connection,
    job_id: &str,
) -> Result<Vec<ChapterGenerationSection>, String> {
    let mut s=db.prepare("SELECT id,job_id,plan_beat_id,order_index,target_words,actual_words,content,continuation_summary,continuity_state_json,status,provider_id,content_hash,draft_context_hash,draft_state,created_at,updated_at FROM chapter_generation_sections WHERE job_id=?1 ORDER BY order_index").map_err(|e|sql_error("Kapitelabschnitte konnten nicht geladen werden",e))?;
    let rows = s
        .query_map(params![job_id], section_from_row)
        .map_err(|e| sql_error("Kapitelabschnitte konnten nicht geladen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Kapitelabschnitte konnten nicht geladen werden", e))?;
    Ok(rows)
}

fn draft_ledger_from_row(row: &rusqlite::Row<'_>) -> SqlResult<ChapterGenerationDraftLedgerEntry> {
    Ok(ChapterGenerationDraftLedgerEntry {
        id: row.get(0)?,
        job_id: row.get(1)?,
        section_id: row.get(2)?,
        project_id: row.get(3)?,
        entity_id: row.get(4)?,
        related_entity_id: row.get(5)?,
        state_kind: row.get(6)?,
        previous_state: row.get(7)?,
        new_state: row.get(8)?,
        source_excerpt: row.get(9)?,
        source_start_offset: row.get(10)?,
        source_end_offset: row.get(11)?,
        content_hash: row.get(12)?,
        confidence: row.get(13)?,
        status: row.get(14)?,
        source_reference_id: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

fn draft_ledger_select() -> &'static str {
    "SELECT id,job_id,section_id,project_id,entity_id,related_entity_id,state_kind,previous_state,new_state,source_excerpt,source_start_offset,source_end_offset,content_hash,confidence,status,source_reference_id,created_at,updated_at FROM chapter_generation_draft_ledger"
}

#[tauri::command]
pub fn list_chapter_generation_draft_ledger(
    state: State<'_, DbState>,
    job_id: String,
) -> Result<Vec<ChapterGenerationDraftLedgerEntry>, String> {
    let db = lock_db(&state)?;
    let job = load_job(&db, &job_id)?;
    let mut statement = db
        .prepare(&format!(
            "{} WHERE job_id=?1 ORDER BY created_at",
            draft_ledger_select()
        ))
        .map_err(|e| sql_error("Draft-Ledger konnte nicht geladen werden", e))?;
    let result = statement
        .query_map(params![job.id], draft_ledger_from_row)
        .map_err(|e| sql_error("Draft-Ledger konnte nicht geladen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Draft-Ledger konnte nicht geladen werden", e));
    result
}

#[tauri::command]
pub fn replace_chapter_generation_draft_ledger(
    state: State<'_, DbState>,
    section_id: String,
    entries: Vec<SaveChapterGenerationDraftLedgerInput>,
) -> Result<Vec<ChapterGenerationDraftLedgerEntry>, String> {
    let db = lock_db(&state)?;
    let (job_id, project_id): (String, String) = db
        .query_row("SELECT s.job_id,j.project_id FROM chapter_generation_sections s JOIN chapter_generation_jobs j ON j.id=s.job_id WHERE s.id=?1", params![section_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| sql_error("Draft-Abschnitt konnte nicht geprüft werden", e))?;
    let tx = db
        .unchecked_transaction()
        .map_err(|e| sql_error("Draft-Ledger konnte nicht gestartet werden", e))?;
    tx.execute(
        "DELETE FROM chapter_generation_draft_ledger WHERE section_id=?1",
        params![section_id],
    )
    .map_err(|e| sql_error("Draft-Ledger konnte nicht ersetzt werden", e))?;
    for input in entries {
        if input.job_id != job_id
            || input.project_id != project_id
            || input.section_id != section_id
            || input.new_state.trim().is_empty()
            || !(0.0..=1.0).contains(&input.confidence)
            || !matches!(
                input.status.as_deref().unwrap_or("proposed"),
                "proposed" | "superseded" | "rejected" | "accepted_for_manuscript_review"
            )
        {
            return Err("Ungültiger oder fremder Draft-Ledger-Eintrag.".into());
        }
        let entity_valid: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM story_entities WHERE id=?1 AND project_id=?2)",
                params![input.entity_id, project_id],
                |row| row.get(0),
            )
            .map_err(|e| sql_error("Draft-Entität konnte nicht geprüft werden", e))?;
        if !entity_valid {
            return Err("Die Draft-Entität gehört nicht zum Projekt.".into());
        }
        if let Some(related) = &input.related_entity_id {
            let valid: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM story_entities WHERE id=?1 AND project_id=?2)",
                    params![related, project_id],
                    |row| row.get(0),
                )
                .map_err(|e| sql_error("Draft-Bezug konnte nicht geprüft werden", e))?;
            if !valid {
                return Err("Die Draft-Bezugsentität gehört nicht zum Projekt.".into());
            }
        }
        if let Some(source) = &input.source_reference_id {
            let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM story_source_references WHERE id=?1 AND project_id=?2)", params![source, project_id], |row| row.get(0)).map_err(|e| sql_error("Draft-Quelle konnte nicht geprüft werden", e))?;
            if !valid {
                return Err("Die Draft-Quelle gehört nicht zum Projekt.".into());
            }
        }
        let id = input.id.unwrap_or_else(new_id);
        let stamp = now();
        let status = input.status.unwrap_or_else(|| "proposed".into());
        tx.execute("INSERT INTO chapter_generation_draft_ledger(id,job_id,section_id,project_id,entity_id,related_entity_id,state_kind,previous_state,new_state,source_excerpt,source_start_offset,source_end_offset,content_hash,confidence,status,source_reference_id,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?17)", params![id, job_id, section_id, project_id, input.entity_id, input.related_entity_id, input.state_kind, input.previous_state, input.new_state, input.source_excerpt, input.source_start_offset, input.source_end_offset, input.content_hash, input.confidence, status, input.source_reference_id, stamp]).map_err(|e| sql_error("Draft-Ledger konnte nicht gespeichert werden", e))?;
    }
    tx.commit()
        .map_err(|e| sql_error("Draft-Ledger konnte nicht abgeschlossen werden", e))?;
    let mut statement = db
        .prepare(&format!(
            "{} WHERE section_id=?1 ORDER BY created_at",
            draft_ledger_select()
        ))
        .map_err(|e| sql_error("Draft-Ledger konnte nicht geladen werden", e))?;
    let result = statement
        .query_map(params![section_id], draft_ledger_from_row)
        .map_err(|e| sql_error("Draft-Ledger konnte nicht geladen werden", e))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| sql_error("Draft-Ledger konnte nicht geladen werden", e));
    result
}

#[tauri::command]
pub fn supersede_chapter_generation_draft_ledger_from(
    state: State<'_, DbState>,
    job_id: String,
    order_index: i64,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    let _ = load_job(&db, &job_id)?;
    db.execute("UPDATE chapter_generation_draft_ledger SET status='superseded',updated_at=?1 WHERE job_id=?2 AND status='proposed' AND section_id IN (SELECT id FROM chapter_generation_sections WHERE job_id=?2 AND order_index>=?3)", params![now(), job_id, order_index]).map_err(|e| sql_error("Draft-Ledger konnte nicht invalidiert werden", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{
        database_path_for_test, has_column, initialize_connection, seed_if_empty,
    };
    use crate::models::{DraftContinuityState, ManuscriptImportChapterInput};
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
        assert!(result.scenes.iter().all(|scene| scene.is_implicit));
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
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM scenes WHERE chapter_id IN (?1, ?2) AND is_implicit=1",
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
            is_implicit: false,
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
                origin: None,
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
            31
        );
        assert!(has_column(&db, "bible_update_runs", "analyzed_content").unwrap());
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn backend_rejects_stale_longform_section_before_acceptance() {
        let (path, db) = connection("longform-stale");
        db.execute("INSERT INTO chapter_generation_jobs(id,project_id,target_book_id,target_words,user_instruction,status,active_provider,content_context_hash) VALUES('job-stale','project-zugestellt','book-1',100,'Schreib','draft_ready','fake','context')", []).unwrap();
        let beat = ChapterPlanBeat {
            id: "beat-stale".into(),
            order_index: 0,
            title: "Abschnitt".into(),
            purpose: String::new(),
            location: None,
            pov_character_id: None,
            participating_character_ids: vec![],
            starting_state: String::new(),
            event: String::new(),
            conflict: String::new(),
            new_information: vec![],
            knowledge_changes: vec![],
            relationship_changes: vec![],
            clues_used: vec![],
            lore_entity_ids: vec![],
            ending_hook: String::new(),
            target_words: 100,
        };
        db.execute("INSERT INTO chapter_generation_plans(id,job_id,chapter_title,chapter_goal,chapter_summary,plan_json,review_status) VALUES('plan-stale','job-stale','Kapitel','Ziel','',?1,'accepted')", params![serde_json::to_string(&vec![beat]).unwrap()]).unwrap();
        let state = DraftContinuityState {
            current_location: String::new(),
            current_story_time: String::new(),
            present_character_ids: vec![],
            character_states: vec![],
            established_facts: vec![],
            knowledge_changes: vec![],
            relationship_changes: vec![],
            moved_objects: vec![],
            injuries: vec![],
            clues_introduced: vec![],
            promises_created: vec![],
            unresolved_actions: vec![],
            last_paragraph_summary: String::new(),
        };
        db.execute("INSERT INTO chapter_generation_sections(id,job_id,plan_beat_id,order_index,target_words,actual_words,content,continuation_summary,continuity_state_json,status,content_hash,draft_context_hash,draft_state) VALUES('section-stale','job-stale','beat-stale',0,100,1,'Text','',?1,'generated',?2,'','stale')", params![serde_json::to_string(&state).unwrap(), canonical_content_hash("Text")]).unwrap();
        let result = accept_chapter_generation_job_in_db(
            &db,
            AcceptChapterGenerationJobInput {
                job_id: "job-stale".into(),
                current_context_hash: "context".into(),
            },
        );
        assert!(result.unwrap_err().contains("Plan und alle Abschnitte"));
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

    #[test]
    fn continuity_graph_migrates_and_unconfirmed_rules_are_inactive() {
        let (path, db) = connection("continuity-rule-graph");
        for table in [
            "project_rules",
            "project_rule_proposals",
            "continuity_state_ledger",
        ] {
            assert!(db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                    params![table],
                    |row| row.get::<_, bool>(0)
                )
                .unwrap());
        }
        db.execute("INSERT INTO project_rules (id,project_id,title,statement,status,confidence,author_confirmed) VALUES ('rule-test','project-zugestellt','Regel','Eine bestätigte Projektregel.','proposed',0.8,0)", []).unwrap();
        assert_eq!(db.query_row("SELECT COUNT(*) FROM project_rules WHERE project_id='project-zugestellt' AND status='confirmed' AND author_confirmed=1", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
        db.execute(
            "UPDATE project_rules SET status='confirmed', author_confirmed=1 WHERE id='rule-test'",
            [],
        )
        .unwrap();
        assert_eq!(db.query_row("SELECT COUNT(*) FROM project_rules WHERE project_id='project-zugestellt' AND status='confirmed' AND author_confirmed=1", [], |row| row.get::<_, i64>(0)).unwrap(), 1);
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn continuity_ledger_excludes_future_and_preserves_past_state() {
        let (path, db) = connection("continuity-ledger");
        db.execute("INSERT INTO continuity_state_ledger (id,project_id,entity_id,state_kind,new_state,status,confidence,author_confirmed,chapter_id,scene_id) VALUES ('ledger-before','project-zugestellt','entity-package','item_availability','verfügbar','confirmed',1,1,'chapter-1','scene-1')", []).unwrap();
        db.execute("INSERT INTO continuity_state_ledger (id,project_id,entity_id,state_kind,previous_state,new_state,status,confidence,author_confirmed,chapter_id,scene_id) VALUES ('ledger-after','project-zugestellt','entity-package','item_availability','verfügbar','nicht verfügbar','confirmed',1,1,'chapter-3','scene-3')", []).unwrap();
        let early: String = db.query_row("SELECT new_state FROM continuity_state_ledger l JOIN chapters c ON c.id=l.chapter_id JOIN scenes s ON s.id=l.scene_id WHERE l.project_id='project-zugestellt' AND l.entity_id='entity-package' AND l.state_kind='item_availability' AND l.status='confirmed' AND l.author_confirmed=1 AND (c.order_index < 2 OR (c.order_index=2 AND s.order_index <= 1)) ORDER BY c.order_index DESC, s.order_index DESC LIMIT 1", [], |row| row.get(0)).unwrap();
        assert_eq!(early, "verfügbar");
        let future_count: i64 = db.query_row("SELECT COUNT(*) FROM continuity_state_ledger l JOIN chapters c ON c.id=l.chapter_id WHERE l.id='ledger-after' AND c.order_index < 3", [], |row| row.get(0)).unwrap();
        assert_eq!(future_count, 0);
        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn incremental_continuity_review_keeps_objective_conflict_and_plot_closure_pending() {
        let (path, db) = connection("incremental-continuity-review");
        db.execute("INSERT INTO continuity_review_runs (id,project_id,chapter_id,scene_id,source_kind,content_hash,provider_id,status) VALUES ('continuity-run','project-zugestellt','chapter-2','scene-2','bible_update','hash','local-continuity-review','completed')", []).unwrap();
        db.execute("INSERT INTO continuity_review_findings (id,run_id,project_id,chapter_id,scene_id,finding_type,severity,subject_entity_id,objective_conflict,evidence_excerpt,reason) VALUES ('finding-object','continuity-run','project-zugestellt','chapter-2','scene-2','critical_contradiction','critical','entity-package','Der Zettel wurde weggeworfen, wird aber erneut gezeigt.','zeigt den Zettel','Bestätigter Zustand widerspricht dem neuen Text.')", []).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT finding_type FROM continuity_review_findings WHERE id='finding-object'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "critical_contradiction"
        );
        db.execute("INSERT INTO plot_thread_lifecycle_proposals (id,run_id,project_id,entity_id,proposed_status,evidence_excerpt,reason) VALUES ('thread-proposal','continuity-run','project-zugestellt','entity-package','closure_candidate','Die Spur ist geklärt.','Möglicher Abschluss; Autorenentscheidung erforderlich.')", []).unwrap();
        assert_eq!(db.query_row("SELECT review_status FROM plot_thread_lifecycle_proposals WHERE id='thread-proposal'", [], |row| row.get::<_, String>(0)).unwrap(), "pending");
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM plot_thread_lifecycle WHERE entity_id='entity-package'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        drop(db);
        let _ = fs::remove_file(path);
    }
}
