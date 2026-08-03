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
    pub analyzed_content: String,
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
    pub analyzed_content: String,
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
    #[serde(default)]
    pub decision: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLoreMetadataInput {
    pub entity_id: String,
    pub project_id: String,
    pub category: String,
    pub scope: String,
    pub reveal_state: String,
    pub importance: String,
    pub truth_statement: String,
    pub rules_text: String,
    pub exceptions_text: String,
    pub author_knowledge: String,
    pub reader_knowledge: String,
    pub reveal_plan: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoreMetadata {
    pub entity_id: String,
    pub project_id: String,
    pub category: String,
    pub scope: String,
    pub reveal_state: String,
    pub importance: String,
    pub truth_statement: String,
    pub rules_text: String,
    pub exceptions_text: String,
    pub author_knowledge: String,
    pub reader_knowledge: String,
    pub reveal_plan: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCharacterProfileInput {
    pub entity_id: String,
    pub project_id: String,
    pub core_want: String,
    pub core_need: String,
    pub fears: String,
    pub false_belief: String,
    pub values: String,
    pub strengths: String,
    pub flaws: String,
    pub pressure_behavior: String,
    pub voice: String,
    pub backstory: String,
    pub arc_summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterProfile {
    pub entity_id: String,
    pub project_id: String,
    pub core_want: String,
    pub core_need: String,
    pub fears: String,
    pub false_belief: String,
    pub values: String,
    pub strengths: String,
    pub flaws: String,
    pub pressure_behavior: String,
    pub voice: String,
    pub backstory: String,
    pub arc_summary: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCharacterSceneStateInput {
    pub id: Option<String>,
    pub project_id: String,
    pub character_entity_id: String,
    pub scene_id: String,
    pub emotional_state: String,
    pub physical_state: String,
    pub goal: String,
    pub conflict: String,
    #[serde(alias = "knowledge")]
    pub knowledge_notes: String,
    pub relationship_state: String,
    pub change_note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterSceneState {
    pub id: String,
    pub project_id: String,
    pub character_entity_id: String,
    pub scene_id: String,
    pub emotional_state: String,
    pub physical_state: String,
    pub goal: String,
    pub conflict: String,
    pub knowledge_notes: String,
    pub relationship_state: String,
    pub change_note: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectStyleInput {
    pub project_id: String,
    pub narrative_pov: String,
    pub tense: String,
    pub sentence_style: String,
    pub dialogue_style: String,
    pub description_density: String,
    pub inner_monologue: String,
    pub preferred_patterns: Vec<String>,
    pub avoided_patterns: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStyle {
    pub project_id: String,
    pub narrative_pov: String,
    pub tense: String,
    pub sentence_style: String,
    pub dialogue_style: String,
    pub description_density: String,
    pub inner_monologue: String,
    pub preferred_patterns: Vec<String>,
    pub avoided_patterns: Vec<String>,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStyleReferenceInput {
    pub project_id: String,
    pub chapter_id: Option<String>,
    pub scene_id: String,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub category: String,
    pub label: String,
    pub excerpt: String,
    pub notes: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleReference {
    pub id: String,
    pub project_id: String,
    pub chapter_id: Option<String>,
    pub scene_id: String,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub category: String,
    pub label: String,
    pub excerpt: String,
    pub notes: String,
    pub weight: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStyleReferenceInput {
    pub id: String,
    pub project_id: String,
    pub label: String,
    pub category: String,
    pub notes: String,
    pub weight: f64,
}

pub fn validate_lore_truth_scope(value: &str) -> Result<(), String> {
    match value {
        "world_truth" | "author_only" | "reader_revealed" | "planned_reveal" => Ok(()),
        _ => Err(format!("Ungültiger Lore-Wissensbereich: {value}")),
    }
}

pub fn validate_lore_category(value: &str) -> Result<(), String> {
    match value {
        "world_rule" | "history" | "objective_truth" | "belief" | "myth" | "mystery"
        | "terminology" => Ok(()),
        _ => Err(format!("Ungültige Lore-Kategorie: {value}")),
    }
}
pub fn validate_lore_scope(value: &str) -> Result<(), String> {
    match value {
        "series" | "book" | "arc" => Ok(()),
        _ => Err(format!("Ungültiger Lore-Gültigkeitsbereich: {value}")),
    }
}
pub fn validate_lore_reveal_state(value: &str) -> Result<(), String> {
    match value {
        "author_only" | "foreshadowed" | "reader_revealed" => Ok(()),
        _ => Err(format!("Ungültiger Enthüllungsstatus: {value}")),
    }
}
pub fn validate_lore_importance(value: &str) -> Result<(), String> {
    match value {
        "core" | "supporting" | "background" => Ok(()),
        _ => Err(format!("Ungültige Lore-Bedeutung: {value}")),
    }
}
pub fn validate_relation_type(value: &str) -> Result<(), String> {
    match value {
        "affects" | "explains" | "contradicts" | "reveals" | "hides" | "depends_on"
        | "applies_to" | "caused_by" | "connected_to" => Ok(()),
        _ => Err(format!("Ungültiger Relationstyp: {value}")),
    }
}
pub fn validate_style_reference_category(value: &str) -> Result<(), String> {
    match value {
        "general" | "dialogue" | "tension" | "description" | "inner_monologue" | "humor" => Ok(()),
        _ => Err(format!("Ungültige Stilreferenz-Kategorie: {value}")),
    }
}
pub fn validate_lore_entity_type(value: &str) -> Result<(), String> {
    match value {
        "world_rule" | "fact" | "event" | "secret" | "clue" | "organization" | "place"
        | "object" | "plot_thread" | "author_note" => Ok(()),
        _ => Err(format!(
            "Dieser Entity-Typ ist für Lore nicht zulässig: {value}"
        )),
    }
}

pub fn validate_character_memory_status(value: &str) -> Result<(), String> {
    match value {
        "proposed" | "confirmed" | "uncertain" | "rejected" | "retired" | "retconned" => Ok(()),
        _ => Err(format!("Ungültiger Charaktergedächtnis-Status: {value}")),
    }
}
pub fn validate_character_voice_pattern_type(value: &str) -> Result<(), String> {
    match value {
        "signature_word"
        | "signature_phrase"
        | "filler_word"
        | "nickname"
        | "address_pattern"
        | "sentence_pattern"
        | "humor_pattern"
        | "metaphor_pattern"
        | "avoidance_pattern"
        | "lie_pattern"
        | "stress_pattern"
        | "relationship_specific_voice"
        | "dialogue_rule" => Ok(()),
        _ => Err(format!("Ungültiger Sprachmuster-Typ: {value}")),
    }
}
pub fn validate_character_significance(value: &str) -> Result<(), String> {
    match value {
        "minor" | "supporting" | "major" | "defining" | "important" | "core" => Ok(()),
        _ => Err(format!("Ungültige Bedeutung: {value}")),
    }
}
pub fn validate_memory_reliability(value: &str) -> Result<(), String> {
    match value {
        "reliable" | "uncertain" | "distorted" | "implanted" | "forgotten" => Ok(()),
        _ => Err(format!("Ungültige Erinnerungssicherheit: {value}")),
    }
}
pub fn validate_dialogue_kind(value: &str) -> Result<(), String> {
    match value {
        "statement" | "promise" | "threat" | "lie" | "confession" | "reveal" | "argument"
        | "inside_joke" | "nickname" | "secret_shared" | "secret_hidden" | "boundary"
        | "callback" | "question" | "accusation" | "apology" => Ok(()),
        _ => Err(format!("Ungültiger Dialogtyp: {value}")),
    }
}
pub fn validate_truthfulness(value: &str) -> Result<(), String> {
    match value {
        "true" | "false" | "partially_true" | "speaker_believes_true" | "unknown" => Ok(()),
        _ => Err(format!("Ungültiger Wahrheitsgehalt: {value}")),
    }
}
pub fn validate_relationship_memory_type(value: &str) -> Result<(), String> {
    match value {
        "inside_joke" | "nickname" | "shared_memory" | "shared_secret" | "promise" | "betrayal"
        | "argument" | "trust_gain" | "trust_loss" | "relationship_shift" | "debt" | "favor"
        | "fear" | "attraction" | "resentment" | "callback" | "boundary" => Ok(()),
        _ => Err(format!("Ungültiger Beziehungserinnerungstyp: {value}")),
    }
}
pub fn validate_knowledge_state(value: &str) -> Result<(), String> {
    match value {
        "knows" | "suspects" | "believes_false" | "denies" | "forgot" | "unknown" => Ok(()),
        _ => Err(format!("Ungültiger Wissensstatus: {value}")),
    }
}
pub fn validate_memory_kind(value: &str) -> Result<(), String> {
    match value {
        "voice_pattern"
        | "experience"
        | "dialogue_memory"
        | "relationship_memory"
        | "knowledge_state"
        | "profile_observation" => Ok(()),
        _ => Err(format!("Ungültige Gedächtnisart: {value}")),
    }
}
pub fn validate_evidence_role(value: &str) -> Result<(), String> {
    match value {
        "primary" | "supporting" | "contradicting" => Ok(()),
        _ => Err(format!("Ungültige Belegrolle: {value}")),
    }
}
pub fn validate_participant_role(value: &str) -> Result<(), String> {
    match value {
        "speaker" | "listener" | "present" | "mentioned" => Ok(()),
        _ => Err(format!("Ungültige Teilnehmerrolle: {value}")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStoryEntityRelationInput {
    pub project_id: String,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relation_type: String,
    #[serde(default)]
    pub label: String,
    #[serde(default = "default_author_confirmed")]
    pub author_confirmed: bool,
}

fn default_author_confirmed() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryEntityRelation {
    pub id: String,
    pub project_id: String,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relation_type: String,
    pub label: String,
    pub author_confirmed: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLoreEntryInput {
    pub project_id: String,
    pub name: String,
    pub entity_type: String,
    pub description: String,
    pub status: String,
    pub category: String,
    pub scope: String,
    pub reveal_state: String,
    pub importance: String,
    pub truth_statement: String,
    pub rules_text: String,
    pub exceptions_text: String,
    pub author_knowledge: String,
    pub reader_knowledge: String,
    pub reveal_plan: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoreEntry {
    pub entity: StoryEntity,
    pub metadata: LoreMetadata,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterVoicePattern {
    pub id: String,
    pub project_id: String,
    pub character_id: String,
    pub related_character_id: Option<String>,
    pub pattern_type: String,
    pub pattern_text: String,
    pub description: String,
    pub context_condition: String,
    pub confidence: f64,
    pub status: String,
    pub author_confirmed: bool,
    pub occurrence_count: i64,
    pub first_observed_scene_id: Option<String>,
    pub last_observed_scene_id: Option<String>,
    pub retired_scene_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCharacterVoicePatternInput {
    pub id: Option<String>,
    pub project_id: String,
    pub character_id: String,
    pub related_character_id: Option<String>,
    pub pattern_type: String,
    pub pattern_text: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub context_condition: String,
    pub confidence: f64,
    pub status: String,
    pub author_confirmed: bool,
    #[serde(default = "default_occurrence_count")]
    pub occurrence_count: i64,
    #[serde(default)]
    pub first_observed_scene_id: Option<String>,
    #[serde(default)]
    pub last_observed_scene_id: Option<String>,
    #[serde(default)]
    pub retired_scene_id: Option<String>,
}
fn default_occurrence_count() -> i64 {
    1
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterExperience {
    pub id: String,
    pub project_id: String,
    pub character_id: String,
    pub event_entity_id: Option<String>,
    pub scene_id: Option<String>,
    pub title: String,
    pub objective_summary: String,
    pub subjective_interpretation: String,
    pub emotional_impact: String,
    pub lasting_effect: String,
    pub significance: String,
    pub memory_reliability: String,
    pub status: String,
    pub author_confirmed: bool,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCharacterExperienceInput {
    pub id: Option<String>,
    pub project_id: String,
    pub character_id: String,
    pub event_entity_id: Option<String>,
    pub scene_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub objective_summary: String,
    #[serde(default)]
    pub subjective_interpretation: String,
    #[serde(default)]
    pub emotional_impact: String,
    #[serde(default)]
    pub lasting_effect: String,
    pub significance: String,
    pub memory_reliability: String,
    pub status: String,
    pub author_confirmed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogueMemoryParticipant {
    pub dialogue_memory_id: String,
    pub character_id: String,
    pub role: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterDialogueMemory {
    pub id: String,
    pub project_id: String,
    pub speaker_id: String,
    pub scene_id: String,
    pub dialogue_kind: String,
    pub topic: String,
    pub summary: String,
    pub exact_excerpt: String,
    pub emotional_tone: String,
    pub hidden_intent: String,
    pub significance: String,
    pub truthfulness: String,
    pub status: String,
    pub author_confirmed: bool,
    pub participants: Vec<DialogueMemoryParticipant>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCharacterDialogueMemoryInput {
    pub id: Option<String>,
    pub project_id: String,
    pub speaker_id: String,
    pub scene_id: String,
    pub dialogue_kind: String,
    #[serde(default)]
    pub topic: String,
    pub summary: String,
    #[serde(default)]
    pub exact_excerpt: String,
    #[serde(default)]
    pub emotional_tone: String,
    #[serde(default)]
    pub hidden_intent: String,
    pub significance: String,
    pub truthfulness: String,
    pub status: String,
    pub author_confirmed: bool,
    #[serde(default)]
    pub participants: Vec<DialogueMemoryParticipantInput>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogueMemoryParticipantInput {
    pub character_id: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipMemory {
    pub id: String,
    pub project_id: String,
    pub character_a_id: String,
    pub character_b_id: String,
    pub scene_id: Option<String>,
    pub memory_type: String,
    pub title: String,
    pub summary: String,
    pub private_meaning: String,
    pub relationship_effect: String,
    pub significance: String,
    pub status: String,
    pub author_confirmed: bool,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRelationshipMemoryInput {
    pub id: Option<String>,
    pub project_id: String,
    pub character_a_id: String,
    pub character_b_id: String,
    pub scene_id: Option<String>,
    pub memory_type: String,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub private_meaning: String,
    #[serde(default)]
    pub relationship_effect: String,
    pub significance: String,
    pub status: String,
    pub author_confirmed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterKnowledgeState {
    pub id: String,
    pub project_id: String,
    pub character_id: String,
    pub fact_entity_id: String,
    pub knowledge_state: String,
    pub acquired_scene_id: Option<String>,
    pub changed_scene_id: Option<String>,
    pub source_character_id: Option<String>,
    pub certainty: f64,
    pub notes: String,
    pub status: String,
    pub author_confirmed: bool,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCharacterKnowledgeStateInput {
    pub id: Option<String>,
    pub project_id: String,
    pub character_id: String,
    pub fact_entity_id: String,
    pub knowledge_state: String,
    pub acquired_scene_id: Option<String>,
    pub changed_scene_id: Option<String>,
    pub source_character_id: Option<String>,
    pub certainty: f64,
    #[serde(default)]
    pub notes: String,
    pub status: String,
    pub author_confirmed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMemoryEvidence {
    pub id: String,
    pub project_id: String,
    pub memory_kind: String,
    pub memory_id: String,
    pub source_reference_id: String,
    pub evidence_role: String,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCharacterMemoryEvidenceInput {
    pub project_id: String,
    pub memory_kind: String,
    pub memory_id: String,
    pub source_reference_id: String,
    pub evidence_role: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMemoryUpdateRun {
    pub id: String,
    pub project_id: String,
    pub scene_id: String,
    pub content_hash: String,
    pub extractor_id: String,
    pub analyzed_content: String,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCharacterMemoryUpdateRunInput {
    pub project_id: String,
    pub scene_id: String,
    pub content_hash: String,
    pub extractor_id: String,
    #[serde(default)]
    pub analyzed_content: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMemoryProposal {
    pub id: String,
    pub run_id: String,
    pub project_id: String,
    pub scene_id: String,
    pub proposal_kind: String,
    pub subject_character_id: Option<String>,
    pub related_character_id: Option<String>,
    pub target_entity_id: Option<String>,
    pub payload: serde_json::Value,
    pub classification: String,
    pub confidence: f64,
    pub evidence_excerpt: String,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub reason: String,
    pub review_status: String,
    pub reviewed_at: Option<String>,
    pub analyzed_content_hash: String,
    pub accepted_memory_id: Option<String>,
    pub accepted_memory_kind: Option<String>,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMemoryProposalDraft {
    pub proposal_kind: String,
    pub subject_character_id: Option<String>,
    pub related_character_id: Option<String>,
    pub target_entity_id: Option<String>,
    pub payload: serde_json::Value,
    pub classification: String,
    pub confidence: f64,
    pub evidence_excerpt: String,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub reason: String,
    #[serde(default)]
    pub analyzed_content_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCharacterMemoryProposalInput {
    pub proposal_id: String,
    pub review_status: String,
    pub decision: Option<String>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryDirection {
    pub project_id: String,
    pub premise: String,
    pub current_story_phase: String,
    pub book_goal: String,
    pub planned_ending: String,
    pub ending_status: String,
    pub central_twist: String,
    pub thematic_goal: String,
    pub must_happen: Vec<String>,
    pub must_not_happen: Vec<String>,
    pub next_turning_point: String,
    pub reveal_constraints: Vec<serde_json::Value>,
    pub author_notes: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveStoryDirectionInput {
    pub project_id: String,
    pub premise: String,
    pub current_story_phase: String,
    pub book_goal: String,
    pub planned_ending: String,
    pub ending_status: String,
    pub central_twist: String,
    pub thematic_goal: String,
    pub must_happen: Vec<String>,
    pub must_not_happen: Vec<String>,
    pub next_turning_point: String,
    pub reveal_constraints: Vec<serde_json::Value>,
    pub author_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WritingPreferences {
    pub project_id: String,
    pub words_per_page: i64,
    pub preferred_section_words: i64,
    pub maximum_section_words: i64,
    pub default_scene_count: i64,
    pub require_plan_confirmation: bool,
    pub require_final_confirmation: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveWritingPreferencesInput {
    pub project_id: String,
    pub words_per_page: i64,
    pub preferred_section_words: i64,
    pub maximum_section_words: i64,
    pub default_scene_count: i64,
    pub require_plan_confirmation: bool,
    pub require_final_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterGenerationJob {
    pub id: String,
    pub project_id: String,
    pub target_book_id: String,
    pub target_after_chapter_id: Option<String>,
    pub requested_pages: Option<f64>,
    pub target_words: i64,
    pub requested_scene_count: Option<i64>,
    pub user_instruction: String,
    pub status: String,
    pub active_provider: String,
    pub content_context_hash: String,
    pub context_override_accepted: bool,
    pub last_resumed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChapterGenerationJobInput {
    pub project_id: String,
    pub target_book_id: String,
    pub target_after_chapter_id: Option<String>,
    pub requested_pages: Option<f64>,
    pub target_words: i64,
    pub requested_scene_count: Option<i64>,
    pub user_instruction: String,
    pub active_provider: String,
    pub content_context_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterPlanBeat {
    pub id: String,
    pub order_index: i64,
    pub title: String,
    pub purpose: String,
    pub location: Option<String>,
    pub pov_character_id: Option<String>,
    pub participating_character_ids: Vec<String>,
    pub starting_state: String,
    pub event: String,
    pub conflict: String,
    pub new_information: Vec<String>,
    pub knowledge_changes: Vec<serde_json::Value>,
    pub relationship_changes: Vec<serde_json::Value>,
    pub clues_used: Vec<String>,
    pub lore_entity_ids: Vec<String>,
    pub ending_hook: String,
    pub target_words: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterGenerationPlan {
    pub id: String,
    pub job_id: String,
    pub chapter_title: String,
    pub chapter_goal: String,
    pub pov_character_id: Option<String>,
    pub starting_state: String,
    pub ending_state: String,
    pub chapter_summary: String,
    pub ending_connection: String,
    pub new_information: Vec<String>,
    pub withheld_information: Vec<String>,
    pub beats: Vec<ChapterPlanBeat>,
    pub review_status: String,
    pub reviewed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveChapterGenerationPlanInput {
    pub job_id: String,
    pub chapter_title: String,
    pub chapter_goal: String,
    pub pov_character_id: Option<String>,
    pub starting_state: String,
    pub ending_state: String,
    pub chapter_summary: String,
    pub ending_connection: String,
    pub new_information: Vec<String>,
    pub withheld_information: Vec<String>,
    pub beats: Vec<ChapterPlanBeat>,
    pub review_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterGenerationSection {
    pub id: String,
    pub job_id: String,
    pub plan_beat_id: String,
    pub order_index: i64,
    pub target_words: i64,
    pub actual_words: i64,
    pub content: String,
    pub continuation_summary: String,
    pub continuity_state: serde_json::Value,
    pub status: String,
    pub provider_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveChapterGenerationSectionInput {
    pub job_id: String,
    pub plan_beat_id: String,
    pub order_index: i64,
    pub target_words: i64,
    pub content: String,
    pub continuation_summary: String,
    pub continuity_state: serde_json::Value,
    pub status: String,
    pub provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterGenerationReview {
    pub id: String,
    pub job_id: String,
    pub section_id: Option<String>,
    pub review_scope: String,
    pub issue_type: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub related_entity_ids: Vec<String>,
    pub related_source_ids: Vec<String>,
    pub suggested_action: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveChapterGenerationReviewInput {
    pub job_id: String,
    pub section_id: Option<String>,
    pub review_scope: String,
    pub issue_type: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub related_entity_ids: Vec<String>,
    #[serde(default)]
    pub related_source_ids: Vec<String>,
    #[serde(default)]
    pub suggested_action: String,
    #[serde(default = "default_review_status")]
    pub status: String,
}
fn default_review_status() -> String {
    "open".into()
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
