use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    pub title: String,
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub volume_title: String,
    #[serde(default = "default_volume")]
    pub volume: i64,
}

fn default_volume() -> i64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChapterInput {
    pub book_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChapterInput {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSceneInput {
    pub chapter_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneInput {
    pub id: String,
    pub chapter_id: String,
    pub title: String,
    pub order_index: i64,
    pub content: String,
    pub pov: String,
    pub location: String,
    pub story_time: String,
    pub status: String,
    pub goal: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPreferences {
    pub font_family: String,
    pub font_size: i64,
    pub line_height: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneVersion {
    pub id: String,
    pub scene_id: String,
    pub version_number: i64,
    pub content: String,
    pub reason: String,
    pub created_at: String,
    pub scene: Scene,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSceneVersionInput {
    pub scene_id: String,
    pub version_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSceneVersionInput {
    pub scene_id: String,
    #[serde(default = "default_scene_version_reason")]
    pub reason: String,
}

fn default_scene_version_reason() -> String {
    "manual".into()
}

pub fn validate_scene_version_reason(value: &str) -> Result<(), String> {
    match value {
        "manual"
        | "before_correction"
        | "before_ai_change"
        | "before_import"
        | "automatic_checkpoint" => Ok(()),
        _ => Err(format!("Ungültiger Versionsgrund: {value}")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryEntityInput {
    pub id: String,
    pub project_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub description: String,
    pub status: String,
    pub confidence: f64,
    pub source: String,
    pub chapter: String,
    pub scene: String,
    pub author_confirmed: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStoryEntityInput {
    pub project_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub description: String,
    pub status: String,
    pub confidence: f64,
    pub chapter_id: Option<String>,
    pub scene_id: Option<String>,
    pub excerpt: String,
    pub author_confirmed: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStoryEntityInput {
    pub id: String,
    pub project_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub description: String,
    pub status: String,
    pub confidence: f64,
    pub chapter_id: Option<String>,
    pub scene_id: Option<String>,
    pub excerpt: String,
    pub author_confirmed: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSourceReferenceInput {
    pub project_id: String,
    pub entity_id: Option<String>,
    pub proposal_id: Option<String>,
    pub chapter_id: String,
    pub scene_id: String,
    pub excerpt: String,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorySourceReference {
    pub id: String,
    pub project_id: String,
    pub entity_id: Option<String>,
    pub proposal_id: Option<String>,
    pub chapter_id: String,
    pub scene_id: String,
    pub excerpt: String,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BibleUpdateRun {
    pub id: String,
    pub project_id: String,
    pub scene_id: String,
    pub scene_updated_at: String,
    pub content_hash: String,
    pub extractor_id: String,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBibleUpdateRunInput {
    pub project_id: String,
    pub scene_id: String,
    pub scene_updated_at: String,
    pub content_hash: String,
    pub extractor_id: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BibleProposalInput {
    pub id: Option<String>,
    pub run_id: String,
    pub project_id: String,
    pub scene_id: String,
    pub target_entity_id: Option<String>,
    pub proposal_action: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    pub candidate_name: String,
    pub candidate_description: String,
    pub candidate_status: String,
    pub confidence: f64,
    pub classification: String,
    pub evidence_excerpt: String,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BibleProposal {
    pub id: String,
    pub run_id: String,
    pub project_id: String,
    pub scene_id: String,
    pub target_entity_id: Option<String>,
    pub proposal_action: String,
    pub entity_type: String,
    pub candidate_name: String,
    pub candidate_description: String,
    pub candidate_status: String,
    pub confidence: f64,
    pub classification: String,
    pub evidence_excerpt: String,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub reason: String,
    pub review_status: String,
    pub reviewed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBibleProposalInput {
    pub proposal_id: String,
    pub review_status: String,
    pub candidate_name: Option<String>,
    pub candidate_description: Option<String>,
    pub candidate_status: Option<String>,
    pub classification: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub word_count: i64,
    pub open_warnings: i64,
    pub bible_progress: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub volume: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub id: String,
    pub chapter_id: String,
    pub title: String,
    pub order_index: i64,
    pub content: String,
    pub pov: String,
    pub location: String,
    pub story_time: String,
    pub status: String,
    pub goal: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub id: String,
    pub book_id: String,
    pub title: String,
    pub order_index: i64,
    pub scenes: Vec<Scene>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryEntity {
    pub id: String,
    pub project_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub description: String,
    pub status: String,
    pub confidence: f64,
    pub source: String,
    pub chapter: String,
    pub scene: String,
    pub author_confirmed: bool,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Vec<String>,
    pub origin: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceSnapshot {
    pub project: Project,
    pub books: Vec<Book>,
    pub chapters: Vec<Chapter>,
    pub entities: Vec<StoryEntity>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub path: String,
    pub connected: bool,
    pub engine: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderStatus {
    pub id: String,
    pub available: bool,
    pub label: String,
    pub detail: String,
}

pub fn validate_scene_status(value: &str) -> Result<(), String> {
    match value {
        "draft" | "revised" | "final" => Ok(()),
        _ => Err(format!("Ungültiger Szenenstatus: {value}")),
    }
}

pub fn validate_entity_status(value: &str) -> Result<(), String> {
    match value {
        "confirmed" | "proposed" | "uncertain" | "contradicted" | "retconned" | "archived" => {
            Ok(())
        }
        _ => Err(format!("Ungültiger Story-Bible-Status: {value}")),
    }
}

pub fn validate_proposal_action(value: &str) -> Result<(), String> {
    match value {
        "create_entity"
        | "update_entity"
        | "add_source"
        | "mark_contradiction"
        | "create_open_question"
        | "create_author_note" => Ok(()),
        _ => Err(format!("Ungültige Proposal-Aktion: {value}")),
    }
}

pub fn validate_proposal_classification(value: &str) -> Result<(), String> {
    match value {
        "observable_fact"
        | "interpretation"
        | "open_question"
        | "possible_contradiction"
        | "author_note" => Ok(()),
        _ => Err(format!("Ungültige Proposal-Klassifikation: {value}")),
    }
}

pub fn validate_review_status(value: &str) -> Result<(), String> {
    match value {
        "pending" | "accepted" | "edited" | "rejected" => Ok(()),
        _ => Err(format!("Ungültiger Review-Status: {value}")),
    }
}
