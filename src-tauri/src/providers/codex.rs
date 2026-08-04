use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

pub const BIBLE_PROMPT_VERSION: &str = "storymemory-bible-v1";
pub const CHAT_PROMPT_VERSION: &str = "storymemory-chat-v1";
const MAX_BIBLE_SNAPSHOT: usize = 2 * 1024 * 1024;
const MAX_CHAT_SNAPSHOT: usize = 3 * 1024 * 1024;
const MAX_SCENE_TEXT: usize = 1024 * 1024;
const MAX_STDOUT: usize = 8 * 1024 * 1024;
const MAX_STDERR: usize = 64 * 1024;
const MAX_JSONL_EVENTS: usize = 10_000;
const MAX_JSONL_LINE: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodexAuthenticationState {
    Authenticated,
    NotAuthenticated,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCliCapabilities {
    pub installed: bool,
    pub binary_path: Option<String>,
    pub version: Option<String>,
    pub supports_exec: bool,
    pub supports_json: bool,
    pub supports_ephemeral: bool,
    pub supports_output_schema: bool,
    pub supports_read_only_sandbox: bool,
    pub supports_skip_git_check: bool,
    pub supports_model: bool,
    pub supports_disable_features: bool,
    pub authentication: CodexAuthenticationState,
    pub compatible: bool,
    pub detail: String,
}

impl CodexCliCapabilities {
    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            installed: false,
            binary_path: None,
            version: None,
            supports_exec: false,
            supports_json: false,
            supports_ephemeral: false,
            supports_output_schema: false,
            supports_read_only_sandbox: false,
            supports_skip_git_check: false,
            supports_model: false,
            supports_disable_features: false,
            authentication: CodexAuthenticationState::Unknown,
            compatible: false,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodexTaskKind {
    ExtractBiblePatch,
    ExtractCharacterMemoryPatch,
    AnswerWithProjectContext,
    AnalyzeProjectStyle,
    SummarizeScene,
    SummarizeChapter,
    SummarizeBook,
    PlanChapterDraft,
    DraftChapterSection,
    ReviewChapterSection,
    ReviewCompleteChapter,
    AnalyzeContinuityPassage,
    AnalyzeManuscriptStructure,
    ResolveManuscriptEntityMentions,
    AnalyzeNarrativeSummaries,
    SynthesizePlotThreads,
    AnalyzeBookEndState,
    GlobalCountercheck,
    AnalyzeLoreDraft,
    BuildLoreSheet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCodexTaskInput {
    pub task_id: String,
    pub task_kind: CodexTaskKind,
    pub request_json: Value,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexTaskResult {
    pub task_id: String,
    pub task_kind: CodexTaskKind,
    pub status: String,
    pub result: Value,
    pub warnings: Vec<String>,
    pub prompt_template_version: String,
    pub turn_completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderSettings {
    pub active_provider: String,
    pub codex_binary_path: Option<String>,
    pub codex_model_override: Option<String>,
    pub bible_update_timeout_seconds: u64,
    pub chat_timeout_seconds: u64,
    pub allow_local_fallback: bool,
    #[serde(default)]
    pub codex_privacy_acknowledged_at: Option<String>,
}

impl Default for AiProviderSettings {
    fn default() -> Self {
        Self {
            active_provider: "local-prototype".into(),
            codex_binary_path: None,
            codex_model_override: None,
            bible_update_timeout_seconds: 120,
            chat_timeout_seconds: 90,
            allow_local_fallback: true,
            codex_privacy_acknowledged_at: None,
        }
    }
}

pub fn validate_codex_privacy(settings: &AiProviderSettings) -> Result<(), CodexError> {
    if settings.active_provider != "codex-cli" {
        return Ok(());
    }
    let Some(timestamp) = settings.codex_privacy_acknowledged_at.as_deref() else {
        return Err(CodexError::new(
            "CODEX_PRIVACY_NOT_ACKNOWLEDGED",
            "Bitte bestätige zuerst die lokale Codex-Zugriffsgrenze.",
        ));
    };
    if timestamp.trim().is_empty() || chrono::DateTime::parse_from_rfc3339(timestamp).is_err() {
        return Err(CodexError::new(
            "CODEX_PRIVACY_NOT_ACKNOWLEDGED",
            "Die Datenschutzbestätigung ist ungültig. Bitte bestätige sie erneut.",
        ));
    }
    Ok(())
}

pub struct CodexRuntimeState {
    pub tasks: Mutex<HashMap<String, Arc<AtomicBool>>>,
}
impl Default for CodexRuntimeState {
    fn default() -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodexError {
    pub code: &'static str,
    pub message: String,
}
impl CodexError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for CodexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for CodexError {}

fn valid_task_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn command_output(binary: &Path, args: &[&str]) -> io::Result<String> {
    let output = std::process::Command::new(binary).args(args).output()?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&output.stderr).into_owned();
    }
    Ok(text)
}

fn resolve_binary_with_path(
    explicit: Option<&str>,
    path_value: Option<OsString>,
) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(value) = explicit.filter(|value| !value.trim().is_empty()) {
        candidates.push(PathBuf::from(value));
    }
    if let Some(path) = path_value {
        candidates.extend(env::split_paths(&path).map(|dir| dir.join("codex")));
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn resolve_binary(explicit: Option<&str>) -> Option<PathBuf> {
    resolve_binary_with_path(explicit, env::var_os("PATH")).or_else(|| {
        [
            PathBuf::from("/opt/homebrew/bin/codex"),
            PathBuf::from("/usr/local/bin/codex"),
        ]
        .into_iter()
        .find(|path| path.is_file())
    })
}

fn has_flag(help: &str, flag: &str) -> bool {
    help.lines().any(|line| {
        line.split_whitespace()
            .any(|part| part == flag || part.starts_with(&format!("{flag}=")))
    })
}

pub fn inspect_codex(explicit: Option<&str>) -> CodexCliCapabilities {
    let Some(binary) = resolve_binary(explicit) else {
        return CodexCliCapabilities::unavailable(
            "Codex CLI wurde nicht gefunden. Installiere Codex oder wähle den Binary-Pfad.",
        );
    };
    let version_output = command_output(&binary, &["--version"]).unwrap_or_default();
    let version = version_output
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned);
    let root_help = command_output(&binary, &["--help"]).unwrap_or_default();
    let exec_help = command_output(&binary, &["exec", "--help"]).unwrap_or_default();
    let supports_exec = root_help.contains("exec") && !exec_help.trim().is_empty();
    let supports_json = has_flag(&exec_help, "--json");
    let supports_ephemeral = has_flag(&exec_help, "--ephemeral");
    let supports_output_schema = has_flag(&exec_help, "--output-schema");
    let supports_sandbox = has_flag(&exec_help, "--sandbox")
        && (exec_help.contains("read-only") || exec_help.contains("read_only"));
    let supports_skip_git = has_flag(&exec_help, "--skip-git-repo-check");
    let supports_model = has_flag(&exec_help, "--model");
    let supports_disable_features = has_flag(&exec_help, "--disable");
    let login_supported = root_help.contains("login") || exec_help.contains("login");
    let authentication = if login_supported {
        match command_output(&binary, &["login", "status"]) {
            Ok(output)
                if output.to_lowercase().contains("logged in")
                    || output.to_lowercase().contains("authenticated") =>
            {
                CodexAuthenticationState::Authenticated
            }
            Ok(output)
                if output.to_lowercase().contains("not logged")
                    || output.to_lowercase().contains("not authenticated") =>
            {
                CodexAuthenticationState::NotAuthenticated
            }
            _ => CodexAuthenticationState::Unknown,
        }
    } else {
        CodexAuthenticationState::Unknown
    };
    let compatible = supports_exec
        && supports_json
        && supports_ephemeral
        && supports_output_schema
        && supports_sandbox
        && supports_skip_git;
    let detail = if compatible {
        "Codex unterstützt die sichere read-only Bridge."
    } else {
        "Diese Codex-Version unterstützt nicht alle erforderlichen sicheren Bridge-Funktionen."
    };
    CodexCliCapabilities {
        installed: true,
        binary_path: Some(binary.display().to_string()),
        version,
        supports_exec,
        supports_json,
        supports_ephemeral,
        supports_output_schema,
        supports_read_only_sandbox: supports_sandbox,
        supports_skip_git_check: supports_skip_git,
        supports_model,
        supports_disable_features,
        authentication,
        compatible,
        detail: detail.into(),
    }
}

pub fn codex_status(explicit: Option<&str>) -> CodexCliCapabilities {
    inspect_codex(explicit)
}

const BIBLE_SCHEMA: &str = r#"{
  "$schema":"https://json-schema.org/draft/2020-12/schema",
  "type":"object",
  "additionalProperties":false,
  "required":["proposals","warnings"],
  "properties":{
    "warnings":{"type":"array","items":{"type":"string","maxLength":500}},
    "proposals":{
      "type":"array",
      "maxItems":100,
      "items":{
        "type":"object",
        "additionalProperties":false,
        "required":["targetEntityId","proposalAction","entityType","candidateName","candidateDescription","candidateStatus","confidence","classification","evidenceExcerpt","startOffset","endOffset","reason"],
        "properties":{
          "targetEntityId":{"type":["string","null"]},
          "proposalAction":{"enum":["create_entity","update_entity","add_source","mark_contradiction","create_open_question","create_author_note"]},
          "entityType":{"type":"string"},
          "candidateName":{"type":"string","minLength":1,"maxLength":200},
          "candidateDescription":{"type":"string","maxLength":4000},
          "candidateStatus":{"enum":["confirmed","proposed","uncertain","contradicted","retconned"]},
          "confidence":{"type":"number","minimum":0,"maximum":1},
          "classification":{"enum":["observable_fact","interpretation","open_question","possible_contradiction","author_note"]},
          "evidenceExcerpt":{"type":"string","maxLength":1000},
          "startOffset":{"type":["integer","null"],"minimum":0},
          "endOffset":{"type":["integer","null"],"minimum":0},
          "reason":{"type":"string","maxLength":1000}
        }
      }
    }
  }
}"#;
const CHAT_SCHEMA: &str = r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["answer","usedEntityIds","usedSourceIds","uncertainty","warnings"],"properties":{"answer":{"type":"string","minLength":1,"maxLength":6000},"usedEntityIds":{"type":"array","maxItems":100,"items":{"type":"string"}},"usedSourceIds":{"type":"array","maxItems":8,"items":{"type":"string"}},"uncertainty":{"enum":["low","medium","high"]},"warnings":{"type":"array","items":{"type":"string","maxLength":500}}}}"#;
const CONTINUITY_SCHEMA: &str = r##"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["observedActions","proposedStateChanges","objectiveContradictions","missingExplanations","matchedLoreRules","newRuleProposals","plotThreadChanges","confidence","evidence","warnings"],"properties":{"observedActions":{"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,"required":["summary","evidenceExcerpt","entityIds","startOffset","endOffset"],"properties":{"summary":{"type":"string","maxLength":2000},"evidenceExcerpt":{"type":"string","maxLength":1500},"entityIds":{"type":"array","items":{"type":"string"}},"startOffset":{"type":["integer","null"],"minimum":0},"endOffset":{"type":["integer","null"],"minimum":0}}}},"proposedStateChanges":{"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,"required":["entityId","relatedEntityId","stateKind","previousState","newState","confidence","evidenceExcerpt","sourceReferenceId","startOffset","endOffset","reason"],"properties":{"entityId":{"type":"string"},"relatedEntityId":{"type":["string","null"]},"stateKind":{"enum":["item_existence","item_availability","ownership","location","physical_condition","injury","property","knowledge","relationship","promise","goal","open_action"]},"previousState":{"type":"string","maxLength":1000},"newState":{"type":"string","minLength":1,"maxLength":1000},"confidence":{"type":"number","minimum":0,"maximum":1},"evidenceExcerpt":{"type":"string","maxLength":1500},"sourceReferenceId":{"type":["string","null"]},"startOffset":{"type":["integer","null"],"minimum":0},"endOffset":{"type":["integer","null"],"minimum":0},"reason":{"type":"string","maxLength":1500}}}},"objectiveContradictions":{"type":"array","maxItems":100,"items":{"$ref":"#/$defs/finding"}},"missingExplanations":{"type":"array","maxItems":100,"items":{"$ref":"#/$defs/finding"}},"matchedLoreRules":{"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,"required":["ruleId","rationale","confidence"],"properties":{"ruleId":{"type":"string"},"rationale":{"type":"string","maxLength":1500},"confidence":{"type":"number","minimum":0,"maximum":1}}}},"newRuleProposals":{"type":"array","maxItems":30,"items":{"type":"object","additionalProperties":false,"required":["projectId","targetRuleId","title","statement","scope","prerequisites","effects","exceptions","connectedLoreIds","sourceReferenceIds","evidenceExcerpt","chapterId","sceneId","startOffset","endOffset","confidence","reason"],"properties":{"projectId":{"type":"string"},"targetRuleId":{"type":["string","null"]},"title":{"type":"string","maxLength":300},"statement":{"type":"string","maxLength":4000},"scope":{"enum":["project","book","arc"]},"prerequisites":{"type":"array","items":{"type":"string"}},"effects":{"type":"array","items":{"type":"string"}},"exceptions":{"type":"array","items":{"type":"string"}},"connectedLoreIds":{"type":"array","items":{"type":"string"}},"sourceReferenceIds":{"type":"array","items":{"type":"string"}},"evidenceExcerpt":{"type":"string","maxLength":1500},"chapterId":{"type":["string","null"]},"sceneId":{"type":["string","null"]},"startOffset":{"type":["integer","null"],"minimum":0},"endOffset":{"type":["integer","null"],"minimum":0},"confidence":{"type":"number","minimum":0,"maximum":1},"reason":{"type":"string","maxLength":1500}}}},"plotThreadChanges":{"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,"required":["entityId","proposedStatus","evidenceExcerpt","sourceReferenceId","startOffset","endOffset","reason","confidence"],"properties":{"entityId":{"type":"string"},"proposedStatus":{"enum":["open","closure_candidate","partially_resolved","reopened","abandoned"]},"evidenceExcerpt":{"type":"string","maxLength":1500},"sourceReferenceId":{"type":["string","null"]},"startOffset":{"type":["integer","null"],"minimum":0},"endOffset":{"type":["integer","null"],"minimum":0},"reason":{"type":"string","maxLength":1500},"confidence":{"type":"number","minimum":0,"maximum":1}}}},"confidence":{"type":"number","minimum":0,"maximum":1},"evidence":{"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,"required":["id","label","chapterId","sceneId","entityId","excerpt","startOffset","endOffset"],"properties":{"id":{"type":"string"},"label":{"type":"string"},"chapterId":{"type":["string","null"]},"sceneId":{"type":["string","null"]},"entityId":{"type":["string","null"]},"excerpt":{"type":["string","null"]},"startOffset":{"type":["integer","null"],"minimum":0},"endOffset":{"type":["integer","null"],"minimum":0}}}},"warnings":{"type":"array","items":{"type":"string","maxLength":500}}},"$defs":{"finding":{"type":"object","additionalProperties":false,"required":["findingType","subjectEntityId","relatedEntityIds","relatedStateIds","objectiveConflict","evidenceExcerpt","sourceReferenceId","counterEvidenceExcerpts","confidence","startOffset","endOffset","reason"],"properties":{"findingType":{"enum":["critical_contradiction","probable_contradiction","missing_explanation","lore_compatible_anomaly","possible_intentional_exception"]},"subjectEntityId":{"type":["string","null"]},"relatedEntityIds":{"type":"array","items":{"type":"string"}},"relatedStateIds":{"type":"array","items":{"type":"string"}},"objectiveConflict":{"type":"string","maxLength":3000},"evidenceExcerpt":{"type":"string","maxLength":1500},"sourceReferenceId":{"type":["string","null"]},"counterEvidenceExcerpts":{"type":"array","items":{"type":"string","maxLength":1500}},"confidence":{"type":"number","minimum":0,"maximum":1},"startOffset":{"type":["integer","null"],"minimum":0},"endOffset":{"type":["integer","null"],"minimum":0},"reason":{"type":"string","maxLength":1500}}}}}"##;

fn continuity_schema() -> &'static str {
    static SCHEMA: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SCHEMA.get_or_init(|| CONTINUITY_SCHEMA
        .replace("\"counterEvidenceExcerpts\":{\"type\":\"array\",\"items\":{\"type\":\"string\",\"maxLength\":1500}},\"confidence\"", "\"counterEvidenceExcerpts\":{\"type\":\"array\",\"items\":{\"type\":\"string\",\"maxLength\":1500}},\"counterEvidence\":{\"type\":[\"array\",\"null\"],\"items\":{\"type\":\"object\",\"additionalProperties\":false,\"required\":[\"sourceReferenceId\",\"excerpt\",\"chapterId\",\"sceneId\",\"startOffset\",\"endOffset\"],\"properties\":{\"sourceReferenceId\":{\"type\":[\"string\",\"null\"]},\"excerpt\":{\"type\":\"string\",\"maxLength\":1500},\"chapterId\":{\"type\":[\"string\",\"null\"]},\"sceneId\":{\"type\":[\"string\",\"null\"]},\"startOffset\":{\"type\":[\"integer\",\"null\"]},\"endOffset\":{\"type\":[\"integer\",\"null\"]}}}},\"confidence\"" )
        .replace("\"excerpt\":{\"type\":[\"string\",\"null\"]},\"startOffset\"", "\"excerpt\":{\"type\":[\"string\",\"null\"]},\"sourceReferenceId\":{\"type\":[\"string\",\"null\"]},\"startOffset\"") )
}
const CHARACTER_MEMORY_SCHEMA: &str = r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["proposals","warnings"],"properties":{"proposals":{"type":"array","maxItems":100,"items":{"type":"object","required":["proposalKind","subjectCharacterId","relatedCharacterId","targetEntityId","payload","classification","confidence","evidenceExcerpt","startOffset","endOffset","reason"],"properties":{"proposalKind":{"enum":["voice_pattern","experience","dialogue_memory","relationship_memory","knowledge_change","profile_observation","character_relation"]},"subjectCharacterId":{"type":["string","null"]},"relatedCharacterId":{"type":["string","null"]},"targetEntityId":{"type":["string","null"]},"payload":{"type":"object"},"classification":{"enum":["observable","interpretation","author_decision_required","possible_contradiction"]},"confidence":{"type":"number","minimum":0,"maximum":1},"evidenceExcerpt":{"type":"string","maxLength":1000},"startOffset":{"type":["integer","null"],"minimum":0},"endOffset":{"type":["integer","null"],"minimum":0},"reason":{"type":"string","maxLength":1000}}}},"warnings":{"type":"array","items":{"type":"string","maxLength":500}}}}"#;
const STYLE_ANALYSIS_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["observations","overallSummary","warnings"],"properties":{"observations":{"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,"required":["observationType","observationText","recommendation","confidence","evidence"],"properties":{"observationType":{"type":"string"},"observationText":{"type":"string","maxLength":4000},"recommendation":{"type":"string","maxLength":2000},"confidence":{"type":"number","minimum":0,"maximum":1},"evidence":{"type":"array","maxItems":10,"items":{"type":"string"}}}}},"overallSummary":{"type":"string","maxLength":6000},"warnings":{"type":"array","items":{"type":"string","maxLength":500}}}}"#;
const SUMMARY_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["summary","importantEvents","openThreads","characterChanges","knowledgeChanges","relationshipEffects","warnings"],"properties":{"summary":{"type":"string","maxLength":8000},"importantEvents":{"type":"array","items":{"type":"string","maxLength":1000}},"openThreads":{"type":"array","items":{"type":"string","maxLength":1000}},"characterChanges":{"type":"array","items":{"type":"string","maxLength":1000}},"knowledgeChanges":{"type":"array","items":{"type":"string","maxLength":1000}},"relationshipEffects":{"type":"array","items":{"type":"string","maxLength":1000}},"warnings":{"type":"array","items":{"type":"string","maxLength":500}}}}"#;
const STRUCTURE_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["scenes","warnings"],"properties":{"scenes":{"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,"required":["temporaryId","chapterId","startOffset","endOffset","title","povCharacterName","povEntityId","location","storyTime","participatingCharacterNames","goal","conflict","importantEvents","transitionType","boundaryReason","confidence","evidenceExcerpt"],"properties":{"temporaryId":{"type":"string"},"chapterId":{"type":"string"},"startOffset":{"type":"integer","minimum":0},"endOffset":{"type":"integer","minimum":0},"title":{"type":"string"},"povCharacterName":{"type":["string","null"]},"povEntityId":{"type":["string","null"]},"location":{"type":"string"},"storyTime":{"type":"string"},"participatingCharacterNames":{"type":"array","items":{"type":"string"}},"goal":{"type":"string"},"conflict":{"type":"string"},"importantEvents":{"type":"array","items":{"type":"string"}},"transitionType":{"enum":["location_change","time_jump","pov_change","character_group_change","flashback_start","flashback_end","dream_start","dream_end","action_break","narrative_transition","chapter_continuation"]},"boundaryReason":{"type":"string"},"confidence":{"type":"number","minimum":0,"maximum":1},"evidenceExcerpt":{"type":"string"}}}},"warnings":{"type":"array","items":{"type":"string"}}}}"#;
const ENTITY_RESOLUTION_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["entities","mentions","relations","events","mergeProposals","warnings"],"properties":{"entities":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["temporaryId","entityType","canonicalName","aliases","description","confidence","existingEntityId"],"properties":{"temporaryId":{"type":"string"},"entityType":{"enum":["character","place","organization","object","event","fact","clue","secret","plot_thread","open_question","world_rule_candidate","author_note"]},"canonicalName":{"type":"string"},"aliases":{"type":"array","items":{"type":"string"}},"description":{"type":"string"},"confidence":{"type":"number","minimum":0,"maximum":1},"existingEntityId":{"type":["string","null"]}}}},"mentions":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["mentionText","startOffset","endOffset","temporaryEntityId","alternativeTemporaryIds","confidence","resolutionReason","excerpt"],"properties":{"mentionText":{"type":"string"},"startOffset":{"type":"integer","minimum":0},"endOffset":{"type":"integer","minimum":0},"temporaryEntityId":{"type":["string","null"]},"alternativeTemporaryIds":{"type":"array","items":{"type":"string"}},"confidence":{"type":"number","minimum":0,"maximum":1},"resolutionReason":{"type":"string"},"excerpt":{"type":"string"}}}},"relations":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["sourceTemporaryId","targetTemporaryId","relationType","label","confidence"],"properties":{"sourceTemporaryId":{"type":"string"},"targetTemporaryId":{"type":"string"},"relationType":{"type":"string"},"label":{"type":"string"},"confidence":{"type":"number","minimum":0,"maximum":1}}}},"events":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["title","summary","participantTemporaryIds","startOffset","endOffset","confidence","excerpt"],"properties":{"title":{"type":"string"},"summary":{"type":"string"},"participantTemporaryIds":{"type":"array","items":{"type":"string"}},"startOffset":{"type":"integer","minimum":0},"endOffset":{"type":"integer","minimum":0},"confidence":{"type":"number","minimum":0,"maximum":1},"excerpt":{"type":"string"}}}},"mergeProposals":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["leftTemporaryId","rightTemporaryId","existingEntityId","reason","confidence"],"properties":{"leftTemporaryId":{"type":"string"},"rightTemporaryId":{"type":["string","null"]},"existingEntityId":{"type":["string","null"]},"reason":{"type":"string"},"confidence":{"type":"number","minimum":0,"maximum":1}}}},"warnings":{"type":"array","items":{"type":"string"}}}}"#;
const PLOT_THREAD_SYNTHESIS_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["summary","openQuestions","threadGoals","developments","closureCandidates","partiallyResolved","reopened","threadProposals","warnings"],"properties":{"summary":{"type":"string"},"openQuestions":{"type":"array","items":{"type":"string"}},"threadGoals":{"type":"array","items":{"type":"string"}},"developments":{"type":"array","items":{"type":"string"}},"closureCandidates":{"type":"array","items":{"type":"string"}},"partiallyResolved":{"type":"array","items":{"type":"string"}},"reopened":{"type":"array","items":{"type":"string"}},"threadProposals":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["entityId","proposedStatus","evidenceExcerpt","reason","confidence","sourceReferenceId"],"properties":{"entityId":{"type":"string"},"proposedStatus":{"enum":["open","closure_candidate","partially_resolved","reopened","abandoned"]},"evidenceExcerpt":{"type":"string"},"reason":{"type":"string"},"confidence":{"type":"number","minimum":0,"maximum":1},"sourceReferenceId":{"type":["string","null"]}}}},"warnings":{"type":"array","items":{"type":"string"}}}}"#;
const BOOK_END_STATE_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["summary","characterEndStates","knowledgeStates","falseBeliefs","relationships","objectOwners","injuries","locations","openActions","unresolvedThreads","endStateProposals","warnings"],"properties":{"summary":{"type":"string"},"characterEndStates":{"type":"array","items":{"type":"string"}},"knowledgeStates":{"type":"array","items":{"type":"string"}},"falseBeliefs":{"type":"array","items":{"type":"string"}},"relationships":{"type":"array","items":{"type":"string"}},"objectOwners":{"type":"array","items":{"type":"string"}},"injuries":{"type":"array","items":{"type":"string"}},"locations":{"type":"array","items":{"type":"string"}},"openActions":{"type":"array","items":{"type":"string"}},"unresolvedThreads":{"type":"array","items":{"type":"string"}},"endStateProposals":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["category","entityId","statement","confidence","evidenceExcerpt","sourceReferenceId"],"properties":{"category":{"type":"string"},"entityId":{"type":["string","null"]},"statement":{"type":"string"},"confidence":{"type":"number","minimum":0,"maximum":1},"evidenceExcerpt":{"type":["string","null"]},"sourceReferenceId":{"type":["string","null"]}}}},"warnings":{"type":"array","items":{"type":"string"}}}}"#;
const GLOBAL_COUNTERCHECK_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["summary","contradictoryFacts","prematureKnowledge","lostOrDestroyedObjects","timeAndLocationConflicts","contradictoryRules","unclearExceptions","uncertainSources","countercheckFindings","warnings"],"properties":{"summary":{"type":"string"},"contradictoryFacts":{"type":"array","items":{"type":"string"}},"prematureKnowledge":{"type":"array","items":{"type":"string"}},"lostOrDestroyedObjects":{"type":"array","items":{"type":"string"}},"timeAndLocationConflicts":{"type":"array","items":{"type":"string"}},"contradictoryRules":{"type":"array","items":{"type":"string"}},"unclearExceptions":{"type":"array","items":{"type":"string"}},"uncertainSources":{"type":"array","items":{"type":"string"}},"countercheckFindings":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["severity","category","objectiveConflict","reason","confidence","evidenceExcerpt","sourceReferenceId"],"properties":{"severity":{"enum":["info","warning","critical"]},"category":{"type":"string"},"objectiveConflict":{"type":"string"},"reason":{"type":"string"},"confidence":{"type":"number","minimum":0,"maximum":1},"evidenceExcerpt":{"type":["string","null"]},"sourceReferenceId":{"type":["string","null"]}}}},"warnings":{"type":"array","items":{"type":"string"}}}}"#;
const LORE_DRAFT_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["understandingSummary","confirmedStatements","proposedWorldRules","prerequisites","effects","limitations","costs","exceptions","terminology","relevantOrganizations","relevantLocations","historicalBackground","unresolvedQuestions","contradictions","excludedContent","clarificationQuestions","confidence","warnings"],"properties":{"understandingSummary":{"type":"string"},"confirmedStatements":{"type":"array","items":{"type":"string"}},"proposedWorldRules":{"type":"array","items":{"type":"string"}},"prerequisites":{"type":"array","items":{"type":"string"}},"effects":{"type":"array","items":{"type":"string"}},"limitations":{"type":"array","items":{"type":"string"}},"costs":{"type":"array","items":{"type":"string"}},"exceptions":{"type":"array","items":{"type":"string"}},"terminology":{"type":"array","items":{"type":"string"}},"relevantOrganizations":{"type":"array","items":{"type":"string"}},"relevantLocations":{"type":"array","items":{"type":"string"}},"historicalBackground":{"type":"array","items":{"type":"string"}},"unresolvedQuestions":{"type":"array","items":{"type":"string"}},"contradictions":{"type":"array","items":{"type":"string"}},"excludedContent":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["content","suggestedTarget","reason"],"properties":{"content":{"type":"string"},"suggestedTarget":{"enum":["character_memory","plot_thread","continuity_state","manuscript","style"]},"reason":{"type":"string"}}}},"clarificationQuestions":{"type":"array","items":{"type":"string"}},"confidence":{"type":"number","minimum":0,"maximum":1},"warnings":{"type":"array","items":{"type":"string"}}}}"#;
const LORE_SHEET_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["title","premise","categories","worldRules","worldRuleObjects","prerequisites","effects","limitations","costs","exceptions","terminology","organizations","locations","historicalEvents","knownAspects","unknownAspects","ruleConnections","openQuestions","warnings"],"properties":{"title":{"type":"string"},"premise":{"type":"string"},"categories":{"type":"array","items":{"type":"string"}},"worldRules":{"type":"array","items":{"type":"string"}},"worldRuleObjects":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["temporaryId","title","statement","prerequisites","effects","limitations","costs","exceptions","relatedTerminology","connectedItemIds","sourceSpans","confidence"],"properties":{"temporaryId":{"type":"string"},"title":{"type":"string"},"statement":{"type":"string"},"prerequisites":{"type":"array","items":{"type":"string"}},"effects":{"type":"array","items":{"type":"string"}},"limitations":{"type":"array","items":{"type":"string"}},"costs":{"type":"array","items":{"type":"string"}},"exceptions":{"type":"array","items":{"type":"string"}},"relatedTerminology":{"type":"array","items":{"type":"string"}},"connectedItemIds":{"type":"array","items":{"type":"string"}},"sourceSpans":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["excerpt","startOffset","endOffset"],"properties":{"excerpt":{"type":"string"},"startOffset":{"type":"integer","minimum":0},"endOffset":{"type":"integer","minimum":0}}}},"confidence":{"type":"number","minimum":0,"maximum":1}}}},"prerequisites":{"type":"array","items":{"type":"string"}},"effects":{"type":"array","items":{"type":"string"}},"limitations":{"type":"array","items":{"type":"string"}},"costs":{"type":"array","items":{"type":"string"}},"exceptions":{"type":"array","items":{"type":"string"}},"terminology":{"type":"array","items":{"type":"string"}},"organizations":{"type":"array","items":{"type":"string"}},"locations":{"type":"array","items":{"type":"string"}},"historicalEvents":{"type":"array","items":{"type":"string"}},"knownAspects":{"type":"array","items":{"type":"string"}},"unknownAspects":{"type":"array","items":{"type":"string"}},"ruleConnections":{"type":"array","items":{"type":"string"}},"openQuestions":{"type":"array","items":{"type":"string"}},"warnings":{"type":"array","items":{"type":"string"}}}}"#;
const CHAPTER_PLAN_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["chapterTitle","chapterGoal","povCharacterId","startingState","endingState","chapterSummary","endingConnection","newInformation","withheldInformation","assumptions","beats","warnings"],"properties":{"chapterTitle":{"type":"string","minLength":1,"maxLength":300},"chapterGoal":{"type":"string","maxLength":2000},"povCharacterId":{"type":["string","null"]},"startingState":{"type":"string","maxLength":3000},"endingState":{"type":"string","maxLength":3000},"chapterSummary":{"type":"string","maxLength":6000},"endingConnection":{"type":"string","maxLength":3000},"newInformation":{"type":"array","items":{"type":"string","maxLength":1000}},"withheldInformation":{"type":"array","items":{"type":"string","maxLength":1000}},"assumptions":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["type","text"],"properties":{"type":{"type":"string"},"text":{"type":"string","maxLength":2000}}}},"beats":{"type":"array","minItems":1,"maxItems":12,"items":{"type":"object","additionalProperties":false,"required":["id","orderIndex","title","purpose","participatingCharacterIds","startingState","event","conflict","newInformation","knowledgeChanges","relationshipChanges","cluesUsed","loreEntityIds","endingHook","targetWords"],"properties":{"id":{"type":"string"},"orderIndex":{"type":"integer","minimum":0},"title":{"type":"string","maxLength":300},"purpose":{"type":"string","maxLength":2000},"participatingCharacterIds":{"type":"array","items":{"type":"string"}},"startingState":{"type":"string"},"event":{"type":"string"},"conflict":{"type":"string"},"newInformation":{"type":"array","items":{"type":"string"}},"knowledgeChanges":{"type":"array","items":{"type":"object"}},"relationshipChanges":{"type":"array","items":{"type":"object"}},"cluesUsed":{"type":"array","items":{"type":"string"}},"loreEntityIds":{"type":"array","items":{"type":"string"}},"endingHook":{"type":"string"},"targetWords":{"type":"integer","minimum":1}}}},"warnings":{"type":"array","items":{"type":"string","maxLength":500}}}}"#;
const CHAPTER_SECTION_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["content","continuationSummary","continuityState","usedEntityIds","usedMemoryIds","usedSourceIds","warnings"],"properties":{"content":{"type":"string","minLength":1,"maxLength":50000},"continuationSummary":{"type":"string","maxLength":3000},"continuityState":{"type":"object","additionalProperties":false,"required":["currentLocation","currentStoryTime","presentCharacterIds","characterStates","establishedFacts","knowledgeChanges","relationshipChanges","movedObjects","injuries","cluesIntroduced","promisesCreated","unresolvedActions","lastParagraphSummary"],"properties":{"currentLocation":{"type":"string"},"currentStoryTime":{"type":"string"},"presentCharacterIds":{"type":"array","items":{"type":"string"}},"characterStates":{"type":"array","items":{"type":"object"}},"establishedFacts":{"type":"array","items":{"type":"string"}},"knowledgeChanges":{"type":"array","items":{"type":"object"}},"relationshipChanges":{"type":"array","items":{"type":"object"}},"movedObjects":{"type":"array","items":{"type":"object"}},"injuries":{"type":"array","items":{"type":"object"}},"cluesIntroduced":{"type":"array","items":{"type":"string"}},"promisesCreated":{"type":"array","items":{"type":"string"}},"unresolvedActions":{"type":"array","items":{"type":"string"}},"lastParagraphSummary":{"type":"string"}}},"usedEntityIds":{"type":"array","items":{"type":"string"}},"usedMemoryIds":{"type":"array","items":{"type":"string"}},"usedSourceIds":{"type":"array","items":{"type":"string"}},"warnings":{"type":"array","items":{"type":"string","maxLength":500}}}}"#;
const REVIEW_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["issues","warnings"],"properties":{"issues":{"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,"required":["reviewScope","issueType","severity","title","description","relatedEntityIds","relatedSourceIds","suggestedAction","status"],"properties":{"reviewScope":{"enum":["section","chapter"]},"issueType":{"type":"string"},"severity":{"enum":["info","warning","blocking"]},"title":{"type":"string","maxLength":300},"description":{"type":"string","maxLength":4000},"relatedEntityIds":{"type":"array","items":{"type":"string"}},"relatedSourceIds":{"type":"array","items":{"type":"string"}},"suggestedAction":{"type":"string","maxLength":1000},"status":{"type":"string"}}}},"warnings":{"type":"array","items":{"type":"string","maxLength":500}}}}"#;

const CHARACTER_MEMORY_SCHEMA_STRICT: &str = r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["proposals","warnings"],"properties":{"proposals":{"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,"required":["proposalKind","subjectCharacterId","relatedCharacterId","targetEntityId","payload","classification","confidence","evidenceExcerpt","startOffset","endOffset","reason"],"properties":{"proposalKind":{"enum":["voice_pattern","experience","dialogue_memory","relationship_memory","knowledge_change","profile_observation","character_relation"]},"subjectCharacterId":{"type":["string","null"]},"relatedCharacterId":{"type":["string","null"]},"targetEntityId":{"type":["string","null"]},"payload":{"oneOf":[{"type":"object","additionalProperties":false,"required":["patternType","patternText","description","contextCondition"],"properties":{"patternType":{"type":"string"},"patternText":{"type":"string","minLength":1,"maxLength":4000},"description":{"type":"string","maxLength":4000},"contextCondition":{"type":"string","maxLength":1000},"relatedCharacterId":{"type":"string"}}},{"type":"object","additionalProperties":false,"required":["title","objectiveSummary","subjectiveInterpretation","emotionalImpact","lastingEffect","significance","memoryReliability"],"properties":{"title":{"type":"string","minLength":1,"maxLength":4000},"objectiveSummary":{"type":"string","maxLength":4000},"subjectiveInterpretation":{"type":"string","maxLength":4000},"emotionalImpact":{"type":"string","maxLength":2000},"lastingEffect":{"type":"string","maxLength":2000},"significance":{"type":"string"},"memoryReliability":{"type":"string"},"eventEntityId":{"type":"string"}}},{"type":"object","additionalProperties":false,"required":["dialogueKind","topic","summary","exactExcerpt","emotionalTone","hiddenIntent","significance","truthfulness","participants"],"properties":{"dialogueKind":{"type":"string"},"topic":{"type":"string","maxLength":1000},"summary":{"type":"string","minLength":1,"maxLength":4000},"exactExcerpt":{"type":"string","maxLength":4000},"emotionalTone":{"type":"string","maxLength":1000},"hiddenIntent":{"type":"string","maxLength":2000},"significance":{"type":"string"},"truthfulness":{"type":"string"},"participants":{"type":"array","minItems":1,"maxItems":30,"items":{"type":"object","additionalProperties":false,"required":["characterId","role"],"properties":{"characterId":{"type":"string"},"role":{"enum":["speaker","listener","present","mentioned"]}}}}}},{"type":"object","additionalProperties":false,"required":["relatedCharacterId","memoryType","title","summary","privateMeaning","relationshipEffect","significance"],"properties":{"relatedCharacterId":{"type":"string"},"memoryType":{"type":"string"},"title":{"type":"string","minLength":1,"maxLength":4000},"summary":{"type":"string","minLength":1,"maxLength":4000},"privateMeaning":{"type":"string","maxLength":3000},"relationshipEffect":{"type":"string","maxLength":3000},"significance":{"type":"string"}}},{"type":"object","additionalProperties":false,"required":["factEntityId","knowledgeState","certainty","notes"],"properties":{"factEntityId":{"type":"string"},"knowledgeState":{"enum":["knows","suspects","believes_false","denies","forgot","unknown"]},"certainty":{"type":"number","minimum":0,"maximum":1},"sourceCharacterId":{"type":"string"},"notes":{"type":"string","maxLength":3000}}},{"type":"object","additionalProperties":false,"required":["field","observedBehavior","possibleInterpretation"],"properties":{"field":{"type":"string","minLength":1,"maxLength":4000},"observedBehavior":{"type":"string","minLength":1,"maxLength":4000},"possibleInterpretation":{"type":"string","maxLength":3000}}},{"type":"object","additionalProperties":false,"required":["relationType","label"],"properties":{"relationType":{"type":"string"},"label":{"type":"string","maxLength":160}}}]},"classification":{"enum":["observable","interpretation","author_decision_required","possible_contradiction"]},"confidence":{"type":"number","minimum":0,"maximum":1},"evidenceExcerpt":{"type":"string","maxLength":1000},"startOffset":{"type":["integer","null"],"minimum":0},"endOffset":{"type":["integer","null"],"minimum":0},"reason":{"type":"string","maxLength":1000}}}},"warnings":{"type":"array","items":{"type":"string","maxLength":500}}}}"#;

const CHAPTER_PLAN_SCHEMA_STRICT: &str = r#"{"type":"object","additionalProperties":false,"required":["chapterTitle","chapterGoal","povCharacterId","startingState","endingState","chapterSummary","endingConnection","newInformation","withheldInformation","assumptions","beats","warnings"],"properties":{"chapterTitle":{"type":"string","minLength":1,"maxLength":300},"chapterGoal":{"type":"string","maxLength":2000},"povCharacterId":{"type":["string","null"]},"startingState":{"type":"string","maxLength":3000},"endingState":{"type":"string","maxLength":3000},"chapterSummary":{"type":"string","maxLength":6000},"endingConnection":{"type":"string","maxLength":3000},"newInformation":{"type":"array","items":{"type":"string","maxLength":1000}},"withheldInformation":{"type":"array","items":{"type":"string","maxLength":1000}},"assumptions":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["type","text"],"properties":{"type":{"type":"string"},"text":{"type":"string","maxLength":2000}}}},"beats":{"type":"array","minItems":1,"maxItems":12,"items":{"type":"object","additionalProperties":false,"required":["id","orderIndex","title","purpose","participatingCharacterIds","startingState","event","conflict","newInformation","knowledgeChanges","relationshipChanges","cluesUsed","loreEntityIds","endingHook","targetWords"],"properties":{"id":{"type":"string"},"orderIndex":{"type":"integer","minimum":0},"title":{"type":"string","maxLength":300},"purpose":{"type":"string","maxLength":2000},"participatingCharacterIds":{"type":"array","items":{"type":"string"}},"startingState":{"type":"string"},"event":{"type":"string"},"conflict":{"type":"string"},"newInformation":{"type":"array","items":{"type":"string"}},"knowledgeChanges":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["characterId","factEntityId","nextState","reason"],"properties":{"characterId":{"type":"string"},"factEntityId":{"type":"string"},"nextState":{"enum":["knows","suspects","believes_false","denies","forgot","unknown"]},"reason":{"type":"string","maxLength":1000}}}},"relationshipChanges":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["characterAId","characterBId","change","reason"],"properties":{"characterAId":{"type":"string"},"characterBId":{"type":"string"},"change":{"type":"string","maxLength":1000},"reason":{"type":"string","maxLength":1000}}}},"cluesUsed":{"type":"array","items":{"type":"string"}},"loreEntityIds":{"type":"array","items":{"type":"string"}},"endingHook":{"type":"string"},"targetWords":{"type":"integer","minimum":1}}}},"warnings":{"type":"array","items":{"type":"string","maxLength":500}}}}"#;

const CHAPTER_SECTION_SCHEMA_STRICT: &str = r#"{"type":"object","additionalProperties":false,"required":["content","continuationSummary","continuityState","usedEntityIds","usedMemoryIds","usedSourceIds","warnings"],"properties":{"content":{"type":"string","minLength":1,"maxLength":50000},"continuationSummary":{"type":"string","maxLength":3000},"continuityState":{"type":"object","additionalProperties":false,"required":["currentLocation","currentStoryTime","presentCharacterIds","characterStates","establishedFacts","knowledgeChanges","relationshipChanges","movedObjects","injuries","cluesIntroduced","promisesCreated","unresolvedActions","lastParagraphSummary"],"properties":{"currentLocation":{"type":"string"},"currentStoryTime":{"type":"string"},"presentCharacterIds":{"type":"array","items":{"type":"string"}},"characterStates":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["characterId","state","change"],"properties":{"characterId":{"type":"string"},"state":{"type":"string"},"change":{"type":"string"}}}},"establishedFacts":{"type":"array","items":{"type":"string"}},"knowledgeChanges":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["characterId","factEntityId","nextState","reason"],"properties":{"characterId":{"type":"string"},"factEntityId":{"type":"string"},"nextState":{"enum":["knows","suspects","believes_false","denies","forgot","unknown"]},"reason":{"type":"string","maxLength":1000}}}},"relationshipChanges":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["characterAId","characterBId","change","reason"],"properties":{"characterAId":{"type":"string"},"characterBId":{"type":"string"},"change":{"type":"string","maxLength":1000},"reason":{"type":"string","maxLength":1000}}}},"movedObjects":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["objectId","location","state"],"properties":{"objectId":{"type":"string"},"location":{"type":"string"},"state":{"type":"string"}}}},"injuries":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["characterId","description","severity"],"properties":{"characterId":{"type":"string"},"description":{"type":"string"},"severity":{"type":"string"}}}},"cluesIntroduced":{"type":"array","items":{"type":"string"}},"promisesCreated":{"type":"array","items":{"type":"string"}},"unresolvedActions":{"type":"array","items":{"type":"string"}},"lastParagraphSummary":{"type":"string"}}},"usedEntityIds":{"type":"array","items":{"type":"string"}},"usedMemoryIds":{"type":"array","items":{"type":"string"}},"usedSourceIds":{"type":"array","items":{"type":"string"}},"warnings":{"type":"array","items":{"type":"string","maxLength":500}}}}"#;

fn schema_for_task(kind: &CodexTaskKind) -> &'static str {
    let _legacy_schemas = (
        CHARACTER_MEMORY_SCHEMA,
        CHAPTER_PLAN_SCHEMA,
        CHAPTER_SECTION_SCHEMA,
    );
    match kind {
        CodexTaskKind::AnalyzeProjectStyle => STYLE_ANALYSIS_SCHEMA,
        CodexTaskKind::SummarizeScene
        | CodexTaskKind::SummarizeChapter
        | CodexTaskKind::SummarizeBook => SUMMARY_SCHEMA,
        CodexTaskKind::AnalyzeNarrativeSummaries => SUMMARY_SCHEMA,
        CodexTaskKind::SynthesizePlotThreads => PLOT_THREAD_SYNTHESIS_SCHEMA,
        CodexTaskKind::AnalyzeBookEndState => BOOK_END_STATE_SCHEMA,
        CodexTaskKind::GlobalCountercheck => GLOBAL_COUNTERCHECK_SCHEMA,
        CodexTaskKind::AnalyzeLoreDraft => LORE_DRAFT_SCHEMA,
        CodexTaskKind::BuildLoreSheet => LORE_SHEET_SCHEMA,
        CodexTaskKind::PlanChapterDraft => CHAPTER_PLAN_SCHEMA_STRICT,
        CodexTaskKind::DraftChapterSection => CHAPTER_SECTION_SCHEMA_STRICT,
        CodexTaskKind::ReviewChapterSection | CodexTaskKind::ReviewCompleteChapter => REVIEW_SCHEMA,
        CodexTaskKind::AnalyzeContinuityPassage => continuity_schema(),
        CodexTaskKind::AnalyzeManuscriptStructure => STRUCTURE_SCHEMA,
        CodexTaskKind::ResolveManuscriptEntityMentions => ENTITY_RESOLUTION_SCHEMA,
        _ => CHAT_SCHEMA,
    }
}

fn prompt_for(kind: &CodexTaskKind) -> (&'static str, &'static str) {
    match kind {
        CodexTaskKind::ExtractBiblePatch => (BIBLE_PROMPT_VERSION, "Du analysierst ein Romanmanuskript für eine kontrollierte Story Bible. Verändere keine Dateien, führe keine Shell-Befehle aus und erfinde keine Informationen. Lies ausschließlich request.json. Liefere ausschließlich JSON nach output-schema.json. Trenne beobachtbare Fakten, Interpretationen, offene Fragen, mögliche Widersprüche und Autorennotizen. Verwende bei entityType ausschließlich character, relationship, place, organization, world_rule, object, event, fact, clue, secret, plot_thread, retcon oder author_note. Wenn keine sicher belegte Information vorliegt, liefere proposals als leeres Array. Ein bestätigter Kanon darf nie still überschrieben werden; nutze bei Konflikten mark_contradiction und targetEntityId. Optionale Werte targetEntityId, startOffset und endOffset müssen als null ausgegeben werden, wenn sie nicht gelten. AI-Offsets sind Unicode-Zeichenpositionen im normalisierten Klartext aus request.json.scene.content."),
        CodexTaskKind::ExtractCharacterMemoryPatch => ("storymemory-character-memory-v1", "Analysiere ausschließlich request.json für quellengebundene Charakterbeobachtungen. Verändere keine Dateien und erfinde keine Figuren, Teilnehmer, Psychologie oder Wissensstände. Trenne beobachtbares Verhalten von Interpretation. Liefere ausschließlich JSON nach output-schema.json. Verwende nur vorhandene Character-IDs. Keine dauerhafte Speicherung, keine vollständigen Dialogkopien. Eine einmalige Formulierung ist nur ein proposed Muster. AI-Offsets sind Unicode-Zeichenpositionen aus request.json.scene.content."),
        CodexTaskKind::AnswerWithProjectContext => (CHAT_PROMPT_VERSION, "Du bist ein projektbezogener Roman-Assistent. Antworte ausschließlich aus request.json. Erfinde keine Quellen oder IDs. Verwende nur vorhandene Entity- und Source-IDs und liefere ausschließlich JSON nach output-schema.json. Trenne bestätigten Kanon, Vermutungen, Widersprüche und fehlende Informationen."),
        CodexTaskKind::AnalyzeProjectStyle => ("storymemory-style-v1", "Analysiere ausschließlich request.json und liefere strukturierte, quellengebundene Stilbeobachtungen. Überschreibe keine Autorregeln."),
        CodexTaskKind::SummarizeScene | CodexTaskKind::SummarizeChapter | CodexTaskKind::SummarizeBook => ("storymemory-summary-v1", "Fasse ausschließlich den übergebenen Projektkontext zusammen. Trenne Ereignisse, Wissensänderungen und offene Handlungsstränge."),
        CodexTaskKind::PlanChapterDraft => ("storymemory-plan-v1", "Erstelle ausschließlich einen überprüfbaren Kapitelplan. Erzeuge noch keinen Manuskripttext und mache Annahmen sichtbar."),
        CodexTaskKind::DraftChapterSection => ("storymemory-section-v1", "Erzeuge nur den angeforderten Abschnitt aus dem bestätigten Plan und Kontext. Verändere keine Dateien."),
        CodexTaskKind::ReviewChapterSection | CodexTaskKind::ReviewCompleteChapter => ("storymemory-review-v1", "Prüfe den Entwurf ausschließlich auf strukturierte Issues. Verändere den Text nicht."),
        CodexTaskKind::AnalyzeContinuityPassage => ("storymemory-continuity-v1", "Analysiere ausschließlich request.json als projektspezifischen Continuity-Pass. Semantische Paraphrasen müssen gleich behandelt werden; verwende keine Schlüsselwortregeln als Entscheidung. Nutze nur bestätigte Story-Bible-Einträge und bestätigte Projektregeln als Kanon. Berücksichtige continuityDecisions als bereits getroffene, lokale Autorentscheidungen; eine accepted_exception gilt nur für ihren verknüpften Finding-/Quellenkontext und niemals global. Liefere Beobachtungen, Zustandsänderungsvorschläge, objektive Konflikte, Erklärungslücken, passende bestätigte Regeln, unbestätigte neue Regelvorschläge und Plot-Thread-Kandidaten. Nimm niemals Kanonänderungen oder Ledger-Zustände automatisch vor. Schlage für Handlungsstränge höchstens closure_candidate, partially_resolved, reopened, open oder abandoned vor; resolved ist ausschließlich eine Nutzerentscheidung. Laktoseintoleranz, medizinische Ausnahmen und produktspezifische Ausnahmen sind mögliche Erklärungen, keine automatischen harten Fehler. Liefere ausschließlich JSON nach output-schema.json. Offsets sind Unicode-Zeichenpositionen im passage.text."),
        CodexTaskKind::AnalyzeManuscriptStructure => ("storymemory-structure-v1", "Analysiere ausschließlich den vollständigen Kapiteltext aus request.json und schlage semantische Szenengrenzen vor. Lokale Absatz- oder Seitenhinweise sind nur Hinweise, niemals alleinige Begründungen. Liefere mindestens eine lückenlose, nicht überlappende Szene, falls keine belastbare Grenze erkennbar ist. Verändere keinen Manuskripttext. Alle Offsets sind Unicode-Codepoints relativ zum vollständigen chapter.content. Liefere ausschließlich JSON nach output-schema.json."),
        CodexTaskKind::ResolveManuscriptEntityMentions => ("storymemory-entity-resolution-v1", "Löse Entitätserwähnungen ausschließlich chronologisch auf. Verwende nur bestätigte Entitäten und provisorische Entitäten aus früheren Einheiten. Spätere Informationen dürfen nicht verwendet werden. Unsichere Pronomen, Umschreibungen und Duplikate bleiben unresolved oder werden als Merge-Vorschlag geliefert. Alle Offsets sind Unicode-Codepoints relativ zum passage.text. Verändere keinen Kanon."),
        CodexTaskKind::AnalyzeNarrativeSummaries => ("storymemory-narrative-summary-v1", "Erzeuge ausschließlich die narrative Buchzusammenfassung. Liefere keine Plot-Lifecycle-Entscheidungen oder Endzustände."),
        CodexTaskKind::SynthesizePlotThreads => ("storymemory-plot-thread-v1", "Synthetisiere offene Handlungsstränge. Liefere niemals resolved; closure_candidate ist nur ein Vorschlag für Nutzerreview."),
        CodexTaskKind::AnalyzeBookEndState => ("storymemory-book-end-state-v1", "Ermittle ausschließlich vorgeschlagene Figuren-, Wissens-, Beziehungs-, Orts- und Gegenstands-Endzustände."),
        CodexTaskKind::GlobalCountercheck => ("storymemory-global-countercheck-v1", "Führe ausschließlich eine globale Gegenprüfung aus. Ändere keinen bestätigten Kanon und liefere Quellenunsicherheiten als Vorschläge."),
        CodexTaskKind::AnalyzeLoreDraft => ("storymemory-lore-draft-v1", "Analysiere ausschließlich freie Lore-Notizen. Erkläre das Verständnis, trenne Weltregeln von Inhalten für andere Storybereiche und übernehme nichts automatisch in Story Bible oder Projektregeln."),
        CodexTaskKind::BuildLoreSheet => ("storymemory-lore-sheet-v1", "Erstelle ausschließlich aus dem geprüften Lore-Verständnis ein vorgeschlagenes Lore Sheet. Erfinde keine Fakten und bestätige keinen Kanon."),
    }
}

fn write_read_only(path: &Path, bytes: &[u8]) -> Result<(), CodexError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| CodexError::new("CODEX_SNAPSHOT_FAILED", error.to_string()))?;
    file.write_all(bytes)
        .map_err(|error| CodexError::new("CODEX_SNAPSHOT_FAILED", error.to_string()))?;
    file.sync_all().ok();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o444))
            .map_err(|error| CodexError::new("CODEX_SNAPSHOT_FAILED", error.to_string()))?;
    }
    Ok(())
}

fn make_tree_writable(path: &Path) -> Result<(), CodexError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| CodexError::new("CODEX_SNAPSHOT_CLEANUP_FAILED", error.to_string()))?;
    if metadata.file_type().is_symlink() {
        return Err(CodexError::new(
            "CODEX_SNAPSHOT_CLEANUP_FAILED",
            "Symlinks sind in Codex-Snapshots nicht erlaubt.",
        ));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)
            .map_err(|error| CodexError::new("CODEX_SNAPSHOT_CLEANUP_FAILED", error.to_string()))?
        {
            make_tree_writable(
                &entry
                    .map_err(|error| {
                        CodexError::new("CODEX_SNAPSHOT_CLEANUP_FAILED", error.to_string())
                    })?
                    .path(),
            )?;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if metadata.is_dir() { 0o700 } else { 0o600 };
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|error| CodexError::new("CODEX_SNAPSHOT_CLEANUP_FAILED", error.to_string()))?;
    }
    Ok(())
}

fn remove_snapshot_path(path: &Path) -> Result<(), CodexError> {
    if !path.exists() {
        return Ok(());
    }
    make_tree_writable(path)?;
    fs::remove_dir_all(path).map_err(|error| {
        CodexError::new(
            "CODEX_SNAPSHOT_CLEANUP_FAILED",
            format!("Snapshot konnte nicht gelöscht werden: {error}"),
        )
    })
}

pub struct CodexSnapshotGuard {
    path: PathBuf,
    cleaned: bool,
}

impl CodexSnapshotGuard {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            cleaned: false,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn cleanup(&mut self) -> Result<(), CodexError> {
        if self.cleaned {
            return Ok(());
        }
        let result = remove_snapshot_path(&self.path);
        if result.is_ok() || !self.path.exists() {
            self.cleaned = true;
        }
        result
    }
}

impl Drop for CodexSnapshotGuard {
    fn drop(&mut self) {
        if !self.cleaned {
            let _ = remove_snapshot_path(&self.path);
        }
    }
}

fn cleanup_stale_snapshots(current_snapshot_name: &str) -> Result<(), CodexError> {
    if let Ok(entries) = fs::read_dir(env::temp_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_snapshot = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("storymemory-codex-") && name != current_snapshot_name
                });
            let is_stale = fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age > Duration::from_secs(60 * 60));
            if is_snapshot && is_stale && path.is_dir() {
                remove_snapshot_path(&path)?;
            }
        }
    }
    Ok(())
}

fn create_snapshot(input: &RunCodexTaskInput) -> Result<CodexSnapshotGuard, CodexError> {
    if !valid_task_id(&input.task_id) {
        return Err(CodexError::new(
            "CODEX_SNAPSHOT_FAILED",
            "Ungültige Task-ID.",
        ));
    }
    cleanup_stale_snapshots(&format!("storymemory-codex-{}", input.task_id))?;
    let serialized = serde_json::to_vec_pretty(&input.request_json)
        .map_err(|error| CodexError::new("CODEX_SNAPSHOT_FAILED", error.to_string()))?;
    let limit = if matches!(
        input.task_kind,
        CodexTaskKind::ExtractBiblePatch | CodexTaskKind::ExtractCharacterMemoryPatch
    ) {
        MAX_BIBLE_SNAPSHOT
    } else {
        MAX_CHAT_SNAPSHOT
    };
    if serialized.len() > limit {
        return Err(CodexError::new(
            "CODEX_OUTPUT_TOO_LARGE",
            format!(
                "Der sichere Projektsnapshot ist größer als {} MB.",
                limit / 1024 / 1024
            ),
        ));
    }
    if input
        .request_json
        .pointer("/scene/content")
        .and_then(Value::as_str)
        .is_some_and(|content| content.len() > MAX_SCENE_TEXT)
    {
        return Err(CodexError::new(
            "CODEX_OUTPUT_TOO_LARGE",
            "Der Szenentext überschreitet das sichere 1-MB-Limit.",
        ));
    }
    let directory = env::temp_dir().join(format!("storymemory-codex-{}", input.task_id));
    if directory.exists() {
        return Err(CodexError::new(
            "CODEX_PROCESS_FAILED",
            "Für diese Task-ID läuft bereits ein Snapshot.",
        ));
    }
    fs::create_dir(&directory)
        .map_err(|error| CodexError::new("CODEX_SNAPSHOT_FAILED", error.to_string()))?;
    let (version, instructions) = prompt_for(&input.task_kind);
    let result = (|| {
        write_read_only(&directory.join("request.json"), &serialized)?;
        write_read_only(
            &directory.join("output-schema.json"),
            match input.task_kind {
                CodexTaskKind::ExtractBiblePatch => BIBLE_SCHEMA.as_bytes(),
                CodexTaskKind::ExtractCharacterMemoryPatch => {
                    CHARACTER_MEMORY_SCHEMA_STRICT.as_bytes()
                }
                CodexTaskKind::AnalyzeContinuityPassage => continuity_schema().as_bytes(),
                _ => schema_for_task(&input.task_kind).as_bytes(),
            },
        )?;
        write_read_only(
            &directory.join("TASK.md"),
            format!("Prompt-Version: {version}\n\n{instructions}\n\nLies ausschließlich request.json und output-schema.json.\n").as_bytes(),
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o555))
                .map_err(|error| CodexError::new("CODEX_SNAPSHOT_FAILED", error.to_string()))?;
        }
        Ok(())
    })();
    let mut guard = CodexSnapshotGuard::new(directory);
    if let Err(error) = result {
        return match guard.cleanup() {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(CodexError::new(
                "CODEX_SNAPSHOT_CLEANUP_FAILED",
                format!("{}; {}", error.message, cleanup_error.message),
            )),
        };
    }
    Ok(guard)
}

fn invocation(
    binary: &str,
    settings: &AiProviderSettings,
    snapshot: &Path,
) -> Result<(PathBuf, Vec<OsString>), CodexError> {
    let capabilities = inspect_codex(settings.codex_binary_path.as_deref());
    if !capabilities.installed {
        return Err(CodexError::new("CODEX_NOT_INSTALLED", capabilities.detail));
    }
    if matches!(
        capabilities.authentication,
        CodexAuthenticationState::NotAuthenticated
    ) {
        return Err(CodexError::new(
            "CODEX_NOT_AUTHENTICATED",
            "Codex ist nicht angemeldet. Öffne Codex im Terminal und folge dem Anmeldeablauf.",
        ));
    }
    if !capabilities.compatible {
        return Err(CodexError::new("CODEX_INCOMPATIBLE", capabilities.detail));
    }
    let path = PathBuf::from(capabilities.binary_path.unwrap_or_else(|| binary.into()));
    let schema = snapshot.join("output-schema.json");
    let mut args = vec![
        OsString::from("exec"),
        OsString::from("--json"),
        OsString::from("--ephemeral"),
        OsString::from("--output-schema"),
        schema.into_os_string(),
        OsString::from("--sandbox"),
        OsString::from("read-only"),
        OsString::from("--skip-git-repo-check"),
    ];
    if capabilities.supports_disable_features {
        args.extend([OsString::from("--disable"), OsString::from("skill_search")]);
    }
    if let Some(model) = settings
        .codex_model_override
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if !capabilities.supports_model {
            return Err(CodexError::new(
                "CODEX_INCOMPATIBLE",
                "Diese Codex-Version unterstützt keinen sicheren Modell-Override.",
            ));
        }
        if model.len() > 100 || model.starts_with('-') {
            return Err(CodexError::new(
                "CODEX_INCOMPATIBLE",
                "Das optionale Modell ist ungültig.",
            ));
        }
        args.extend([OsString::from("--model"), OsString::from(model)]);
    }
    Ok((path, args))
}

fn read_limited<R: Read>(mut reader: R, maximum: usize) -> Result<Vec<u8>, CodexError> {
    let mut result = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut exceeded = false;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Err(error) => {
                return Err(CodexError::new(
                    "CODEX_PROCESS_FAILED",
                    format!("Codex-Ausgabe konnte nicht gelesen werden: {error}"),
                ));
            }
            Ok(count) => {
                let remaining = maximum.saturating_sub(result.len());
                if count > remaining {
                    exceeded = true;
                }
                if remaining > 0 {
                    result.extend_from_slice(&buffer[..count.min(remaining)]);
                }
            }
        }
    }
    if exceeded {
        Err(CodexError::new(
            "CODEX_OUTPUT_TOO_LARGE",
            format!("Codex-Ausgabe überschreitet das Limit von {maximum} Bytes."),
        ))
    } else {
        Ok(result)
    }
}

fn bounded_diagnostic(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(1000)
        .collect()
}

fn result_matches_task(value: &Value, task: &CodexTaskKind) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    match task {
        CodexTaskKind::ExtractBiblePatch | CodexTaskKind::ExtractCharacterMemoryPatch => {
            object.contains_key("proposals")
        }
        CodexTaskKind::AnswerWithProjectContext => object.contains_key("answer"),
        CodexTaskKind::AnalyzeProjectStyle => {
            object.contains_key("observations") && object.contains_key("overallSummary")
        }
        CodexTaskKind::SummarizeScene
        | CodexTaskKind::SummarizeChapter
        | CodexTaskKind::SummarizeBook => {
            object.contains_key("summary") && object.contains_key("importantEvents")
        }
        CodexTaskKind::AnalyzeNarrativeSummaries => {
            object.contains_key("summary") && object.contains_key("importantEvents")
        }
        CodexTaskKind::SynthesizePlotThreads => {
            object.contains_key("openQuestions") && object.contains_key("closureCandidates")
        }
        CodexTaskKind::AnalyzeBookEndState => {
            object.contains_key("characterEndStates") && object.contains_key("unresolvedThreads")
        }
        CodexTaskKind::GlobalCountercheck => {
            object.contains_key("contradictoryFacts") && object.contains_key("uncertainSources")
        }
        CodexTaskKind::AnalyzeLoreDraft => {
            object.contains_key("understandingSummary") && object.contains_key("excludedContent")
        }
        CodexTaskKind::BuildLoreSheet => {
            object.contains_key("title") && object.contains_key("worldRules")
        }
        CodexTaskKind::PlanChapterDraft => {
            object.contains_key("chapterTitle") && object.contains_key("beats")
        }
        CodexTaskKind::DraftChapterSection => {
            object.contains_key("content") && object.contains_key("continuityState")
        }
        CodexTaskKind::ReviewChapterSection | CodexTaskKind::ReviewCompleteChapter => {
            object.contains_key("issues")
        }
        CodexTaskKind::AnalyzeContinuityPassage => {
            object.contains_key("objectiveContradictions")
                && object.contains_key("proposedStateChanges")
        }
        CodexTaskKind::AnalyzeManuscriptStructure => {
            object.contains_key("scenes") && object.contains_key("warnings")
        }
        CodexTaskKind::ResolveManuscriptEntityMentions => {
            object.contains_key("entities")
                && object.contains_key("mentions")
                && object.contains_key("warnings")
        }
    }
}

fn extract_candidate(value: &Value, task: &CodexTaskKind) -> Option<Value> {
    if result_matches_task(value, task) {
        return Some(value.clone());
    }
    value
        .as_str()
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .and_then(|parsed| extract_candidate(&parsed, task))
}

fn extract_final_json(
    stdout: &[u8],
    task: &CodexTaskKind,
) -> Result<(Value, Vec<String>, bool), CodexError> {
    let text = String::from_utf8(stdout.to_vec())
        .map_err(|_| CodexError::new("CODEX_INVALID_JSONL", "Codex lieferte ungültiges UTF-8."))?;
    let mut final_value = None;
    let mut warnings = Vec::new();
    let mut completed = false;
    let mut event_count = 0;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        event_count += 1;
        if event_count > MAX_JSONL_EVENTS {
            return Err(CodexError::new(
                "CODEX_INVALID_JSONL",
                "Codex lieferte mehr als 10.000 JSONL-Events.",
            ));
        }
        if line.len() > MAX_JSONL_LINE {
            return Err(CodexError::new(
                "CODEX_INVALID_JSONL",
                "Eine Codex-JSONL-Zeile überschreitet das 1-MB-Limit.",
            ));
        }
        let event: Value = serde_json::from_str(line).map_err(|error| {
            CodexError::new(
                "CODEX_INVALID_JSONL",
                format!("Ungültige JSONL-Zeile: {error}"),
            )
        })?;
        let kind = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match kind {
            "turn.failed" | "error" | "fatal" => {
                return Err(CodexError::new(
                    "CODEX_PROCESS_FAILED",
                    "Codex meldete einen fehlgeschlagenen Turn.",
                ));
            }
            "item.failed" | "tool.failed" | "warning" | "diagnostic" => {
                warnings.push(format!("Codex meldete das nicht-fatal Event {kind}."));
            }
            "turn.completed" => completed = true,
            _ => {}
        }
        let candidates = [
            event.get("result"),
            event.get("structuredOutput"),
            event.get("structured_output"),
            event.get("output"),
            event.get("finalOutput"),
            event.get("final_output"),
            event.get("item").and_then(|item| item.get("text")),
        ];
        for candidate in candidates.into_iter().flatten() {
            if let Some(value) = extract_candidate(candidate, task) {
                final_value = Some(value);
            }
        }
        if let Some(value) = extract_candidate(&event, task) {
            final_value = Some(value);
        }
    }
    if !completed {
        return Err(CodexError::new(
            "CODEX_PROCESS_FAILED",
            "Codex lieferte keinen abgeschlossenen Turn.",
        ));
    }
    final_value
        .map(|value| (value, warnings, completed))
        .ok_or_else(|| {
            CodexError::new(
                "CODEX_PROCESS_FAILED",
                "Codex lieferte kein strukturiertes Ergebnis.",
            )
        })
}

fn string_set(request: &Value, pointer: &str) -> std::collections::HashSet<String> {
    request
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .or_else(|| value.get("id").and_then(Value::as_str).map(str::to_owned))
                })
                .collect()
        })
        .unwrap_or_default()
}
fn valid_string<'a>(value: &'a Value, field: &str, maximum: usize) -> Result<&'a str, CodexError> {
    let string = value.get(field).and_then(Value::as_str).ok_or_else(|| {
        CodexError::new("CODEX_SCHEMA_VALIDATION_FAILED", format!("{field} fehlt."))
    })?;
    if string.trim().is_empty() && matches!(field, "candidateName" | "evidenceExcerpt" | "reason") {
        return Err(CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            format!("{field} darf nicht leer sein."),
        ));
    }
    if string.chars().count() > maximum {
        return Err(CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            format!("{field} ist zu lang."),
        ));
    }
    Ok(string)
}

pub fn validate_bible_result(result: &Value, request: &Value) -> Result<Value, CodexError> {
    let object = result.as_object().ok_or_else(|| {
        CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            "Bible-Ergebnis ist kein Objekt.",
        )
    })?;
    let proposals = object
        .get("proposals")
        .and_then(Value::as_array)
        .ok_or_else(|| CodexError::new("CODEX_SCHEMA_VALIDATION_FAILED", "proposals fehlt."))?;
    if proposals.len() > 100 {
        return Err(CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            "Maximal 100 Vorschläge sind erlaubt.",
        ));
    }
    let ids = string_set(request, "/existingEntities");
    let content = request
        .pointer("/scene/content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let actions = [
        "create_entity",
        "update_entity",
        "add_source",
        "mark_contradiction",
        "create_open_question",
        "create_author_note",
    ];
    let types = [
        "character",
        "relationship",
        "place",
        "organization",
        "world_rule",
        "object",
        "event",
        "fact",
        "clue",
        "secret",
        "plot_thread",
        "retcon",
        "author_note",
    ];
    let statuses = [
        "confirmed",
        "proposed",
        "uncertain",
        "contradicted",
        "retconned",
    ];
    let classes = [
        "observable_fact",
        "interpretation",
        "open_question",
        "possible_contradiction",
        "author_note",
    ];
    for proposal in proposals {
        let action = valid_string(proposal, "proposalAction", 40)?;
        if !actions.contains(&action) {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                "Unbekannte Proposal-Aktion.",
            ));
        }
        let entity_type = valid_string(proposal, "entityType", 40)?;
        if !types.contains(&entity_type) {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                "Unbekannter Entity-Typ.",
            ));
        }
        let status = valid_string(proposal, "candidateStatus", 40)?;
        if !statuses.contains(&status) {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                "Unbekannter Entity-Status.",
            ));
        }
        let classification = valid_string(proposal, "classification", 40)?;
        if !classes.contains(&classification) {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                "Unbekannte Klassifikation.",
            ));
        }
        valid_string(proposal, "candidateName", 200)?;
        valid_string(proposal, "candidateDescription", 4000)?;
        valid_string(proposal, "evidenceExcerpt", 1000)?;
        valid_string(proposal, "reason", 1000)?;
        let confidence = proposal
            .get("confidence")
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                CodexError::new("CODEX_SCHEMA_VALIDATION_FAILED", "confidence fehlt.")
            })?;
        if !(0.0..=1.0).contains(&confidence) {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                "confidence muss zwischen 0 und 1 liegen.",
            ));
        }
        if let Some(target) = proposal.get("targetEntityId").and_then(Value::as_str) {
            if !ids.contains(target) {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "targetEntityId ist im Kontext nicht vorhanden.",
                ));
            }
        }
        let start = proposal.get("startOffset").and_then(Value::as_u64);
        let end = proposal.get("endOffset").and_then(Value::as_u64);
        if start.is_some() != end.is_some() {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                "startOffset und endOffset müssen gemeinsam gesetzt werden.",
            ));
        }
        if let (Some(start), Some(end)) = (start, end) {
            let chars: Vec<char> = content.chars().collect();
            if start > end || end > chars.len() as u64 {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "Offset liegt außerhalb der aktuellen Szene.",
                ));
            }
            let excerpt: String = chars[start as usize..end as usize].iter().collect();
            let evidence = proposal
                .get("evidenceExcerpt")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !excerpt.contains(evidence) && !evidence.contains(&excerpt) {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "Evidence passt nicht zum angegebenen Textbereich.",
                ));
            }
        }
    }
    Ok(result.clone())
}

pub fn validate_character_memory_result(
    result: &Value,
    request: &Value,
) -> Result<Value, CodexError> {
    let proposals = result
        .get("proposals")
        .and_then(Value::as_array)
        .ok_or_else(|| CodexError::new("CODEX_SCHEMA_VALIDATION_FAILED", "proposals fehlt."))?;
    if proposals.len() > 100 {
        return Err(CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            "Maximal 100 Charaktergedächtnis-Vorschläge sind erlaubt.",
        ));
    }
    let characters = string_set(request, "/characters");
    let entities = string_set(request, "/existingEntities");
    let content: Vec<char> = request
        .pointer("/scene/content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .collect();
    let kinds = [
        "voice_pattern",
        "experience",
        "dialogue_memory",
        "relationship_memory",
        "knowledge_change",
        "profile_observation",
        "character_relation",
    ];
    let classes = [
        "observable",
        "interpretation",
        "author_decision_required",
        "possible_contradiction",
    ];
    for proposal in proposals {
        let kind = valid_string(proposal, "proposalKind", 40)?;
        if !kinds.contains(&kind) {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                "Unbekannter Character-Memory-Typ.",
            ));
        }
        let classification = valid_string(proposal, "classification", 40)?;
        if !classes.contains(&classification) {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                "Unbekannte Character-Memory-Klassifikation.",
            ));
        }
        let confidence = proposal
            .get("confidence")
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                CodexError::new("CODEX_SCHEMA_VALIDATION_FAILED", "confidence fehlt.")
            })?;
        if !(0.0..=1.0).contains(&confidence) {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                "confidence muss zwischen 0 und 1 liegen.",
            ));
        }
        valid_string(proposal, "evidenceExcerpt", 1000)?;
        valid_string(proposal, "reason", 1000)?;
        for field in ["subjectCharacterId", "relatedCharacterId"] {
            if let Some(id) = proposal.get(field).and_then(Value::as_str) {
                if !characters.contains(id) {
                    return Err(CodexError::new(
                        "CODEX_INVALID_REFERENCE",
                        format!("{field} ist keine vorhandene Figur."),
                    ));
                }
            }
        }
        if let Some(id) = proposal.get("targetEntityId").and_then(Value::as_str) {
            if !entities.contains(id) {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "targetEntityId ist im Kontext nicht vorhanden.",
                ));
            }
        }
        let start = proposal.get("startOffset").and_then(Value::as_u64);
        let end = proposal.get("endOffset").and_then(Value::as_u64);
        if start.is_some() != end.is_some() {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                "startOffset und endOffset müssen gemeinsam gesetzt werden.",
            ));
        }
        if let (Some(start), Some(end)) = (start, end) {
            if start > end || end > content.len() as u64 {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "Offset liegt außerhalb der Szene.",
                ));
            }
            let excerpt: String = content[start as usize..end as usize].iter().collect();
            let evidence = proposal
                .get("evidenceExcerpt")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !excerpt.contains(evidence) && !evidence.contains(&excerpt) {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "Evidence passt nicht zum Textbereich.",
                ));
            }
        }
    }
    Ok(result.clone())
}

pub fn validate_longform_result(result: &Value) -> Result<Value, CodexError> {
    let object = result.as_object().ok_or_else(|| {
        CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            "Langformergebnis ist kein Objekt.",
        )
    })?;
    if object.get("warnings").and_then(Value::as_array).is_none() {
        return Err(CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            "Langformergebnis benötigt warnings.",
        ));
    }
    if object
        .get("content")
        .and_then(Value::as_str)
        .is_some_and(|value| value.chars().count() > 50_000)
    {
        return Err(CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            "Langformabschnitt überschreitet das sichere Größenlimit.",
        ));
    }
    Ok(result.clone())
}

fn validate_continuity_result(result: &Value, request: &Value) -> Result<Value, CodexError> {
    let object = result.as_object().ok_or_else(|| {
        CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            "Continuity-Ergebnis ist kein Objekt.",
        )
    })?;
    for field in [
        "observedActions",
        "proposedStateChanges",
        "objectiveContradictions",
        "missingExplanations",
        "matchedLoreRules",
        "newRuleProposals",
        "plotThreadChanges",
        "evidence",
        "warnings",
    ] {
        if !object.get(field).is_some_and(Value::is_array) {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                format!("Continuity-Ergebnis benötigt {field}."),
            ));
        }
    }
    let entities: std::collections::HashSet<&str> = request
        .get("confirmedStoryBible")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .collect();
    let passage = request.get("passage").ok_or_else(|| {
        CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            "Continuity-Request benötigt passage.",
        )
    })?;
    if passage.get("coordinateSystem").and_then(Value::as_str) != Some("unicode_codepoints") {
        return Err(CodexError::new(
            "CODEX_INVALID_OFFSET",
            "Continuity verwendet ausschließlich Unicode-Codepoints.",
        ));
    }
    let passage_start = passage
        .get("passageStartOffset")
        .and_then(Value::as_i64)
        .ok_or_else(|| CodexError::new("CODEX_INVALID_OFFSET", "passageStartOffset fehlt."))?;
    let passage_end = passage
        .get("passageEndOffset")
        .and_then(Value::as_i64)
        .ok_or_else(|| CodexError::new("CODEX_INVALID_OFFSET", "passageEndOffset fehlt."))?;
    let passage_len = passage
        .get("text")
        .and_then(Value::as_str)
        .map(|text| text.chars().count() as i64)
        .unwrap_or(0);
    if passage_start < 0
        || passage_end < passage_start
        || passage_end - passage_start != passage_len
    {
        return Err(CodexError::new(
            "CODEX_INVALID_OFFSET",
            "Passage-Grenzen sind keine absoluten Unicode-Codepoint-Grenzen.",
        ));
    }
    let allowed_chapter = passage.get("chapterId").and_then(Value::as_str);
    let allowed_scene = passage.get("sceneId").and_then(Value::as_str);
    let validate_location = |value: &Value| -> Result<(), CodexError> {
        for (field, allowed) in [("chapterId", allowed_chapter), ("sceneId", allowed_scene)] {
            if let Some(id) = value.get(field).and_then(Value::as_str) {
                if Some(id) != allowed {
                    return Err(CodexError::new(
                        "CODEX_INVALID_REFERENCE",
                        "Continuity-Ergebnis verweist auf eine nicht angeforderte Textposition.",
                    ));
                }
            }
        }
        Ok(())
    };
    let rules: std::collections::HashSet<&str> = request
        .get("confirmedRules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .collect();
    let mut states: std::collections::HashSet<&str> = request
        .get("continuityStatesBeforePosition")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .collect();
    if let Some(draft) = request.get("draftLedger").and_then(Value::as_array) {
        for item in draft {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                states.insert(id);
            }
        }
    }
    let source_ids: std::collections::HashSet<&str> = request
        .get("relevantSources")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .collect();
    let lore_entities: std::collections::HashSet<&str> = request
        .get("confirmedLore")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("entityId").and_then(Value::as_str))
        .collect();
    let passage_chars = request
        .get("passage")
        .and_then(|passage| passage.get("text"))
        .and_then(Value::as_str)
        .map(|text| text.chars().count())
        .unwrap_or(0);
    let validate_offsets = |value: &Value| -> Result<(), CodexError> {
        let start = value.get("startOffset").and_then(Value::as_i64);
        let end = value.get("endOffset").and_then(Value::as_i64);
        if let (Some(start), Some(end)) = (start, end) {
            if start < 0 || end < start || end as usize > passage_chars {
                return Err(CodexError::new(
                    "CODEX_INVALID_OFFSET",
                    "Continuity-Offsets liegen außerhalb von passage.text.",
                ));
            }
            if let Some(excerpt) = value.get("evidenceExcerpt").and_then(Value::as_str) {
                let expected: String = request
                    .get("passage")
                    .and_then(|p| p.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .chars()
                    .skip(start as usize)
                    .take((end - start) as usize)
                    .collect();
                if !excerpt.is_empty() && expected != excerpt {
                    return Err(CodexError::new(
                        "CODEX_EVIDENCE_OFFSET_MISMATCH",
                        "Belegstelle stimmt nicht mit den Unicode-Offsets überein.",
                    ));
                }
            }
        }
        Ok(())
    };
    for change in object
        .get("proposedStateChanges")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        validate_offsets(change)?;
        let entity_id = valid_string(change, "entityId", 200)?;
        if !entities.contains(entity_id) {
            return Err(CodexError::new(
                "CODEX_INVALID_REFERENCE",
                "Zustandsvorschlag verweist auf keine bestätigte Entität.",
            ));
        }
        if let Some(related) = change.get("relatedEntityId").and_then(Value::as_str) {
            if !entities.contains(related) {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "Zustandsvorschlag verweist auf eine unbekannte verbundene Entität.",
                ));
            }
        }
        if let Some(source) = change.get("sourceReferenceId").and_then(Value::as_str) {
            if !source_ids.contains(source) {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "Zustandsvorschlag verweist auf eine unbekannte Quelle.",
                ));
            }
        }
        let confidence = change
            .get("confidence")
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                CodexError::new(
                    "CODEX_SCHEMA_VALIDATION_FAILED",
                    "Zustandsvorschlag benötigt confidence.",
                )
            })?;
        if !(0.0..=1.0).contains(&confidence) {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                "confidence muss zwischen 0 und 1 liegen.",
            ));
        }
    }
    for collection in ["objectiveContradictions", "missingExplanations"] {
        for finding in object
            .get(collection)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            validate_offsets(finding)?;
            if let Some(subject) = finding.get("subjectEntityId").and_then(Value::as_str) {
                if !entities.contains(subject) {
                    return Err(CodexError::new(
                        "CODEX_INVALID_REFERENCE",
                        "Continuity-Finding verweist auf eine unbekannte Subjekt-Entität.",
                    ));
                }
            }
            for id in finding
                .get("relatedEntityIds")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if !entities.contains(id) {
                    return Err(CodexError::new(
                        "CODEX_INVALID_REFERENCE",
                        "Continuity-Finding verweist auf eine unbekannte Entität.",
                    ));
                }
            }
            if let Some(source) = finding.get("sourceReferenceId").and_then(Value::as_str) {
                if !source_ids.contains(source) {
                    return Err(CodexError::new(
                        "CODEX_INVALID_REFERENCE",
                        "Continuity-Finding verweist auf eine unbekannte Quelle.",
                    ));
                }
            }
            for counter in finding
                .get("counterEvidence")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                validate_location(counter)?;
                if let Some(source) = counter.get("sourceReferenceId").and_then(Value::as_str) {
                    if !source_ids.contains(source) {
                        return Err(CodexError::new(
                            "CODEX_INVALID_REFERENCE",
                            "Gegenbeleg verweist auf eine unbekannte Quelle.",
                        ));
                    }
                }
            }
            for id in finding
                .get("relatedStateIds")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if !states.contains(id) {
                    return Err(CodexError::new(
                        "CODEX_INVALID_REFERENCE",
                        "Continuity-Finding verweist auf einen unbekannten Zustand.",
                    ));
                }
            }
            let confidence = finding
                .get("confidence")
                .and_then(Value::as_f64)
                .ok_or_else(|| {
                    CodexError::new(
                        "CODEX_SCHEMA_VALIDATION_FAILED",
                        "Continuity-Finding benötigt confidence.",
                    )
                })?;
            if !(0.0..=1.0).contains(&confidence) {
                return Err(CodexError::new(
                    "CODEX_SCHEMA_VALIDATION_FAILED",
                    "confidence muss zwischen 0 und 1 liegen.",
                ));
            }
        }
    }
    for rule in object
        .get("matchedLoreRules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("ruleId").and_then(Value::as_str))
    {
        if !rules.contains(rule) {
            return Err(CodexError::new(
                "CODEX_INVALID_REFERENCE",
                "Nur bestätigte Projektregeln dürfen als Erklärung verwendet werden.",
            ));
        }
    }
    for proposal in object
        .get("newRuleProposals")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        validate_offsets(proposal)?;
        validate_location(proposal)?;
        if proposal.get("projectId").and_then(Value::as_str)
            != request.get("projectId").and_then(Value::as_str)
        {
            return Err(CodexError::new(
                "CODEX_INVALID_REFERENCE",
                "Regelvorschlag gehört nicht zum Projekt.",
            ));
        }
        for id in proposal
            .get("connectedLoreIds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !lore_entities.contains(id) && !entities.contains(id) {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "Regelvorschlag verweist auf unbekannte Lore.",
                ));
            }
        }
        for id in proposal
            .get("sourceReferenceIds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !source_ids.contains(id) {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "Regelvorschlag verweist auf eine unbekannte Quelle.",
                ));
            }
        }
        if let Some(target) = proposal.get("targetRuleId").and_then(Value::as_str) {
            if !rules.contains(target) {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "Regelvorschlag verweist auf eine unbekannte Zielregel.",
                ));
            }
        }
    }
    for change in object
        .get("plotThreadChanges")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        validate_offsets(change)?;
        if change.get("proposedStatus").and_then(Value::as_str) == Some("resolved") {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                "Die AI darf keinen Handlungsstrang als resolved setzen.",
            ));
        }
        let entity_id = valid_string(change, "entityId", 200)?;
        if !entities.contains(entity_id) {
            return Err(CodexError::new(
                "CODEX_INVALID_REFERENCE",
                "Plot-Thread-Vorschlag verweist auf keine bestätigte Entität.",
            ));
        }
    }
    for observed in object
        .get("observedActions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        validate_offsets(observed)?;
        for id in observed
            .get("entityIds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !entities.contains(id) {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "Beobachtung verweist auf eine unbekannte Entität.",
                ));
            }
        }
    }
    for evidence in object
        .get("evidence")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        validate_location(evidence)?;
        if let Some(id) = evidence.get("entityId").and_then(Value::as_str) {
            if !entities.contains(id) {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "Evidence verweist auf eine unbekannte Entität.",
                ));
            }
        }
        if let Some(id) = evidence.get("sourceReferenceId").and_then(Value::as_str) {
            if !source_ids.contains(id) {
                return Err(CodexError::new(
                    "CODEX_INVALID_REFERENCE",
                    "Evidence verweist auf eine unbekannte Quelle.",
                ));
            }
        }
        validate_offsets(evidence)?;
    }
    Ok(result.clone())
}

fn validate_longform_result_for_task(
    result: &Value,
    kind: &CodexTaskKind,
) -> Result<Value, CodexError> {
    let value = validate_longform_result(result)?;
    let required: &[&str] = match kind {
        CodexTaskKind::AnalyzeProjectStyle => &["observations", "overallSummary"],
        CodexTaskKind::SummarizeScene
        | CodexTaskKind::SummarizeChapter
        | CodexTaskKind::SummarizeBook => &[
            "summary",
            "importantEvents",
            "openThreads",
            "characterChanges",
            "knowledgeChanges",
            "relationshipEffects",
        ],
        CodexTaskKind::AnalyzeNarrativeSummaries => &[
            "summary",
            "importantEvents",
            "openThreads",
            "characterChanges",
            "knowledgeChanges",
            "relationshipEffects",
        ],
        CodexTaskKind::SynthesizePlotThreads => &[
            "summary",
            "openQuestions",
            "threadGoals",
            "developments",
            "closureCandidates",
            "partiallyResolved",
            "reopened",
        ],
        CodexTaskKind::AnalyzeBookEndState => &[
            "summary",
            "characterEndStates",
            "knowledgeStates",
            "falseBeliefs",
            "relationships",
            "objectOwners",
            "injuries",
            "locations",
            "openActions",
            "unresolvedThreads",
        ],
        CodexTaskKind::GlobalCountercheck => &[
            "summary",
            "contradictoryFacts",
            "prematureKnowledge",
            "lostOrDestroyedObjects",
            "timeAndLocationConflicts",
            "contradictoryRules",
            "unclearExceptions",
            "uncertainSources",
        ],
        CodexTaskKind::AnalyzeLoreDraft => &[
            "understandingSummary",
            "confirmedStatements",
            "proposedWorldRules",
            "prerequisites",
            "effects",
            "limitations",
            "costs",
            "exceptions",
            "terminology",
            "relevantOrganizations",
            "relevantLocations",
            "historicalBackground",
            "unresolvedQuestions",
            "contradictions",
            "excludedContent",
            "clarificationQuestions",
            "confidence",
        ],
        CodexTaskKind::BuildLoreSheet => &[
            "title",
            "premise",
            "categories",
            "worldRules",
            "prerequisites",
            "effects",
            "limitations",
            "costs",
            "exceptions",
            "terminology",
            "organizations",
            "locations",
            "historicalEvents",
            "knownAspects",
            "unknownAspects",
            "ruleConnections",
            "openQuestions",
        ],
        CodexTaskKind::PlanChapterDraft => &[
            "chapterTitle",
            "chapterGoal",
            "povCharacterId",
            "startingState",
            "endingState",
            "chapterSummary",
            "endingConnection",
            "newInformation",
            "withheldInformation",
            "assumptions",
            "beats",
        ],
        CodexTaskKind::DraftChapterSection => &[
            "content",
            "continuationSummary",
            "continuityState",
            "usedEntityIds",
            "usedMemoryIds",
            "usedSourceIds",
        ],
        CodexTaskKind::ReviewChapterSection | CodexTaskKind::ReviewCompleteChapter => &["issues"],
        CodexTaskKind::AnalyzeContinuityPassage => &[
            "observedActions",
            "proposedStateChanges",
            "objectiveContradictions",
            "missingExplanations",
            "matchedLoreRules",
            "newRuleProposals",
            "plotThreadChanges",
            "confidence",
            "evidence",
        ],
        CodexTaskKind::AnalyzeManuscriptStructure => &["scenes", "warnings"],
        CodexTaskKind::ResolveManuscriptEntityMentions => &[
            "entities",
            "mentions",
            "relations",
            "events",
            "mergeProposals",
            "warnings",
        ],
        _ => &[],
    };
    for field in required {
        if value.get(*field).is_none() {
            return Err(CodexError::new(
                "CODEX_SCHEMA_VALIDATION_FAILED",
                format!("Langformergebnis benötigt {field}."),
            ));
        }
    }
    Ok(value)
}

pub fn validate_chat_result(result: &Value, request: &Value) -> Result<Value, CodexError> {
    let object = result.as_object().ok_or_else(|| {
        CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            "Chat-Ergebnis ist kein Objekt.",
        )
    })?;
    let answer = valid_string(result, "answer", 6000)?;
    if answer.trim().is_empty() {
        return Err(CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            "Die Chat-Antwort ist leer.",
        ));
    }
    let entity_ids = string_set(request, "/projectContext/relevantEntities");
    let source_ids = string_set(request, "/projectContext/relevantSources");
    for id in object
        .get("usedEntityIds")
        .and_then(Value::as_array)
        .ok_or_else(|| CodexError::new("CODEX_SCHEMA_VALIDATION_FAILED", "usedEntityIds fehlt."))?
    {
        let id = id.as_str().ok_or_else(|| {
            CodexError::new("CODEX_SCHEMA_VALIDATION_FAILED", "Ungültige Entity-ID.")
        })?;
        if !entity_ids.contains(id) {
            return Err(CodexError::new(
                "CODEX_INVALID_REFERENCE",
                "Codex verwendet eine unbekannte Entity-ID.",
            ));
        }
    }
    let used_sources = object
        .get("usedSourceIds")
        .and_then(Value::as_array)
        .ok_or_else(|| CodexError::new("CODEX_SCHEMA_VALIDATION_FAILED", "usedSourceIds fehlt."))?;
    if used_sources.len() > 8 {
        return Err(CodexError::new(
            "CODEX_INVALID_REFERENCE",
            "Maximal acht Quellen sind erlaubt.",
        ));
    }
    for id in used_sources {
        let id = id.as_str().ok_or_else(|| {
            CodexError::new("CODEX_SCHEMA_VALIDATION_FAILED", "Ungültige Source-ID.")
        })?;
        if !source_ids.contains(id) {
            return Err(CodexError::new(
                "CODEX_INVALID_REFERENCE",
                "Codex verwendet eine unbekannte Source-ID.",
            ));
        }
    }
    if !matches!(
        object.get("uncertainty").and_then(Value::as_str),
        Some("low" | "medium" | "high")
    ) {
        return Err(CodexError::new(
            "CODEX_SCHEMA_VALIDATION_FAILED",
            "Ungültige Unsicherheit.",
        ));
    }
    Ok(result.clone())
}

pub struct CodexInvocation {
    pub task_kind: CodexTaskKind,
    pub binary: PathBuf,
    pub args: Vec<OsString>,
    pub snapshot: PathBuf,
    pub prompt: String,
    pub timeout_seconds: u64,
}

pub struct CodexProcessResult {
    pub result: Value,
    pub warnings: Vec<String>,
    pub turn_completed: bool,
}

pub trait CodexProcessRunner: Send + Sync {
    fn inspect(&self, binary: &Path) -> Result<CodexCliCapabilities, CodexError>;
    fn run(
        &self,
        invocation: CodexInvocation,
        cancellation: Arc<AtomicBool>,
    ) -> Result<CodexProcessResult, CodexError>;
}

pub fn sanitized_codex_environment(
    source: impl Iterator<Item = (OsString, OsString)>,
) -> Vec<(OsString, OsString)> {
    const ALLOWLIST: &[&str] = &[
        "HOME",
        "USER",
        "LOGNAME",
        "PATH",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "TERM",
        "TMPDIR",
        "CODEX_HOME",
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
        "ALL_PROXY",
    ];
    source
        .filter(|(key, _)| {
            let Some(name) = key.to_str() else {
                return false;
            };
            let upper = name.to_ascii_uppercase();
            ALLOWLIST.contains(&name)
                && ![
                    "SECRET",
                    "TOKEN",
                    "PASSWORD",
                    "CREDENTIAL",
                    "PRIVATE_KEY",
                    "DATABASE",
                ]
                .iter()
                .any(|part| upper.contains(part))
        })
        .collect()
}

pub struct SystemCodexProcessRunner;

impl CodexProcessRunner for SystemCodexProcessRunner {
    fn inspect(&self, binary: &Path) -> Result<CodexCliCapabilities, CodexError> {
        Ok(inspect_codex(binary.to_str()))
    }

    fn run(
        &self,
        invocation: CodexInvocation,
        cancel: Arc<AtomicBool>,
    ) -> Result<CodexProcessResult, CodexError> {
        let mut command = Command::new(invocation.binary);
        command
            .args(invocation.args)
            .current_dir(&invocation.snapshot)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in sanitized_codex_environment(env::vars_os()) {
            command.env(key, value);
        }
        let mut child = command.spawn().map_err(|error| {
            CodexError::new(
                "CODEX_PROCESS_FAILED",
                format!("Codex konnte nicht gestartet werden: {error}"),
            )
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(invocation.prompt.as_bytes())
                .map_err(|error| CodexError::new("CODEX_PROCESS_FAILED", error.to_string()))?;
            drop(stdin);
        }
        let stdout = child.stdout.take().ok_or_else(|| {
            CodexError::new(
                "CODEX_PROCESS_FAILED",
                "Codex-stdout konnte nicht geöffnet werden.",
            )
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            CodexError::new(
                "CODEX_PROCESS_FAILED",
                "Codex-stderr konnte nicht geöffnet werden.",
            )
        })?;
        let stdout_task = std::thread::spawn(move || read_limited(stdout, MAX_STDOUT));
        let stderr_task = std::thread::spawn(move || read_limited(stderr, MAX_STDERR));
        let timeout = Duration::from_secs(invocation.timeout_seconds.clamp(1, 900));
        let started = std::time::Instant::now();
        let status = loop {
            if cancel.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CodexError::new(
                    "CODEX_CANCELLED",
                    "Die Codex-Analyse wurde abgebrochen.",
                ));
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CodexError::new(
                    "CODEX_TIMEOUT",
                    "Die Codex-Analyse hat das Zeitlimit überschritten.",
                ));
            }
            if let Some(status) = child
                .try_wait()
                .map_err(|error| CodexError::new("CODEX_PROCESS_FAILED", error.to_string()))?
            {
                break status;
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        let stdout = stdout_task.join().map_err(|_| {
            CodexError::new("CODEX_PROCESS_FAILED", "stdout-Leser ist fehlgeschlagen.")
        })??;
        let stderr = stderr_task.join().map_err(|_| {
            CodexError::new("CODEX_PROCESS_FAILED", "stderr-Leser ist fehlgeschlagen.")
        })??;
        if !status.success() {
            let diagnostic = bounded_diagnostic(&stderr);
            return Err(CodexError::new(
                "CODEX_PROCESS_FAILED",
                format!(
                    "Codex wurde mit Exit-Code {} beendet{}.",
                    status.code().unwrap_or(-1),
                    if diagnostic.is_empty() {
                        String::new()
                    } else {
                        format!(": {diagnostic}")
                    }
                ),
            ));
        }
        let (result, mut warnings, turn_completed) =
            extract_final_json(&stdout, &invocation.task_kind)?;
        if !stderr.is_empty() {
            warnings.push("Codex hat zusätzliche Diagnoseausgaben geschrieben.".into());
        }
        Ok(CodexProcessResult {
            result,
            warnings,
            turn_completed,
        })
    }
}

#[cfg(test)]
pub struct FakeCodexProcessRunner {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[cfg(test)]
impl CodexProcessRunner for FakeCodexProcessRunner {
    fn inspect(&self, _binary: &Path) -> Result<CodexCliCapabilities, CodexError> {
        Ok(CodexCliCapabilities::unavailable("Fake Codex Runner"))
    }

    fn run(
        &self,
        invocation: CodexInvocation,
        _cancellation: Arc<AtomicBool>,
    ) -> Result<CodexProcessResult, CodexError> {
        let (result, mut warnings, turn_completed) =
            extract_final_json(&self.stdout, &invocation.task_kind)?;
        if !self.stderr.is_empty() {
            warnings.push("Codex hat zusätzliche Diagnoseausgaben geschrieben.".into());
        }
        Ok(CodexProcessResult {
            result,
            warnings,
            turn_completed,
        })
    }
}

fn run_process(
    input: &RunCodexTaskInput,
    settings: &AiProviderSettings,
    cancel: Arc<AtomicBool>,
) -> Result<(Value, Vec<String>, bool), CodexError> {
    let mut snapshot = create_snapshot(input)?;
    let result = {
        let (binary, args) = invocation("codex", settings, snapshot.path())?;
        SystemCodexProcessRunner
            .run(
                CodexInvocation {
                    task_kind: input.task_kind.clone(),
                    binary,
                    args,
                    snapshot: snapshot.path().to_path_buf(),
                    prompt: prompt_for(&input.task_kind).1.into(),
                    timeout_seconds: input.timeout_seconds,
                },
                cancel,
            )
            .map(|result| (result.result, result.warnings, result.turn_completed))
    };
    match (result, snapshot.cleanup()) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(_), Err(cleanup_error)) => Err(cleanup_error),
        (Err(primary), Err(cleanup_error)) => Err(CodexError::new(
            "CODEX_SNAPSHOT_CLEANUP_FAILED",
            format!("{}; {}", primary.message, cleanup_error.message),
        )),
    }
}

pub fn run_task(
    state: Arc<CodexRuntimeState>,
    input: RunCodexTaskInput,
    settings: AiProviderSettings,
) -> Result<CodexTaskResult, CodexError> {
    validate_codex_privacy(&settings)?;
    if input.task_id.is_empty() {
        return Err(CodexError::new("CODEX_UNKNOWN_ERROR", "Task-ID fehlt."));
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut tasks = state.tasks.lock().map_err(|_| {
            CodexError::new("CODEX_UNKNOWN_ERROR", "Task-Registry ist nicht verfügbar.")
        })?;
        if tasks.contains_key(&input.task_id) {
            return Err(CodexError::new(
                "CODEX_PROCESS_FAILED",
                "Diese Task-ID läuft bereits.",
            ));
        }
        tasks.insert(input.task_id.clone(), cancel.clone());
    }
    let result = run_process(&input, &settings, cancel);
    if let Ok(mut tasks) = state.tasks.lock() {
        tasks.remove(&input.task_id);
    }
    let (raw, mut warnings, turn_completed) = result?;
    let validated = match input.task_kind {
        CodexTaskKind::ExtractBiblePatch => validate_bible_result(&raw, &input.request_json)?,
        CodexTaskKind::ExtractCharacterMemoryPatch => {
            validate_character_memory_result(&raw, &input.request_json)?
        }
        CodexTaskKind::AnswerWithProjectContext => validate_chat_result(&raw, &input.request_json)?,
        CodexTaskKind::AnalyzeContinuityPassage => {
            validate_continuity_result(&raw, &input.request_json)?
        }
        CodexTaskKind::AnalyzeManuscriptStructure => {
            validate_longform_result_for_task(&raw, &input.task_kind)?
        }
        CodexTaskKind::ResolveManuscriptEntityMentions => {
            validate_longform_result_for_task(&raw, &input.task_kind)?
        }
        CodexTaskKind::AnalyzeProjectStyle
        | CodexTaskKind::SummarizeScene
        | CodexTaskKind::SummarizeChapter
        | CodexTaskKind::SummarizeBook
        | CodexTaskKind::AnalyzeNarrativeSummaries
        | CodexTaskKind::SynthesizePlotThreads
        | CodexTaskKind::AnalyzeBookEndState
        | CodexTaskKind::GlobalCountercheck
        | CodexTaskKind::AnalyzeLoreDraft
        | CodexTaskKind::BuildLoreSheet
        | CodexTaskKind::PlanChapterDraft
        | CodexTaskKind::DraftChapterSection
        | CodexTaskKind::ReviewChapterSection
        | CodexTaskKind::ReviewCompleteChapter => {
            validate_longform_result_for_task(&raw, &input.task_kind)?
        }
    };
    if let Some(extra) = validated.get("warnings").and_then(Value::as_array) {
        warnings.extend(extra.iter().filter_map(Value::as_str).map(str::to_owned));
    }
    Ok(CodexTaskResult {
        task_id: input.task_id,
        task_kind: input.task_kind.clone(),
        status: "completed".into(),
        result: validated,
        warnings,
        prompt_template_version: prompt_for(&input.task_kind).0.into(),
        turn_completed,
    })
}

pub fn cancel_task(state: &CodexRuntimeState, task_id: &str) -> Result<(), CodexError> {
    let tasks = state.tasks.lock().map_err(|_| {
        CodexError::new("CODEX_UNKNOWN_ERROR", "Task-Registry ist nicht verfügbar.")
    })?;
    if let Some(flag) = tasks.get(task_id) {
        flag.store(true, Ordering::SeqCst);
        Ok(())
    } else {
        Err(CodexError::new(
            "CODEX_UNKNOWN_ERROR",
            "Die Codex-Task wurde nicht gefunden.",
        ))
    }
}

pub fn task_audit_payload(
    input: &RunCodexTaskInput,
    status: &str,
    error_code: Option<&str>,
) -> Value {
    json!({"taskId":input.task_id,"projectId":input.request_json.get("projectId"),"sceneId":input.request_json.get("sceneId"),"taskKind":input.task_kind,"providerId":"codex-cli","requestedProvider":"codex-cli","actualProvider":"codex-cli","usedFallback":false,"fallbackReasonCode":null,"promptTemplateVersion":prompt_for(&input.task_kind).0,"inputHash":format!("{:x}", md_hash(&serde_json::to_vec(&input.request_json).unwrap_or_default())),"status":status,"errorCode":error_code})
}
fn md_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(1469598103934665603_u64, |hash, byte| {
        (hash ^ (*byte as u64)).wrapping_mul(1099511628211)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn synthetic_input(task_id: &str) -> RunCodexTaskInput {
        RunCodexTaskInput {
            task_id: task_id.into(),
            task_kind: CodexTaskKind::ExtractBiblePatch,
            request_json: json!({
                "projectId": "synthetic-project",
                "sceneId": "synthetic-scene",
                "project": {"id":"synthetic-project","title":"Synthetischer Test","author":"Test"},
                "chapter": {"id":"synthetic-chapter","title":"Kapitel 1"},
                "scene": {"id":"synthetic-scene","title":"Testszene","content":"😀 Zettel wartet.","pov":"char-1","location":"Zimmer","storyTime":"Abend","goal":"Beobachten","notes":"Synthetischer Test"},
                "existingEntities": [{"id":"entity-1","type":"object","name":"Zettel"},{"id":"char-1","type":"character","name":"Malik"},{"id":"thread-1","type":"plot_thread","name":"Wer nahm den Zettel?"}],
                "characters": [{"id":"char-1","name":"Malik"}],
                "relevantSources": [{"id":"source-1","chapterId":"synthetic-chapter","sceneId":"synthetic-scene","excerpt":"😀 Zettel","startOffset":0,"endOffset":8}],
                "projectContext": {"relevantEntities":[{"id":"entity-1"}],"relevantSources":[{"id":"source-1"}]},
                "passage": {"text":"😀 Zettel","chapterId":"synthetic-chapter","sceneId":"synthetic-scene","passageStartOffset":0,"passageEndOffset":8,"coordinateSystem":"unicode_codepoints"},
                "confirmedStoryBible": [{"id":"entity-1"},{"id":"char-1"},{"id":"thread-1"}],
                "confirmedRules": [{"id":"rule-1"}],
                "confirmedLore": [{"entityId":"entity-1"}],
                "continuityStatesBeforePosition": [{"id":"state-1"}],
                "draftLedger": [{"id":"draft-state-1"}]
            }),
            timeout_seconds: 90,
        }
    }

    #[test]
    fn strict_task_schemas_are_valid_json() {
        for schema in [
            CHARACTER_MEMORY_SCHEMA_STRICT,
            CHAPTER_PLAN_SCHEMA_STRICT,
            CHAPTER_SECTION_SCHEMA_STRICT,
            continuity_schema(),
        ] {
            serde_json::from_str::<Value>(schema).expect("structured task schema must be JSON");
        }
    }

    #[test]
    fn missing_binary_is_not_installed() {
        assert!(resolve_binary_with_path(
            Some("/definitely/not/a/codex"),
            Some(OsString::from("/definitely/not/a/path"))
        )
        .is_none());
    }
    #[test]
    fn invalid_target_reference_is_rejected() {
        let request = json!({"scene":{"content":"Text"},"existingEntities":[{"id":"known"}]});
        let result = json!({"proposals":[{"proposalAction":"create_entity","entityType":"fact","candidateName":"X","candidateDescription":"Y","candidateStatus":"proposed","confidence":0.9,"classification":"observable_fact","evidenceExcerpt":"Text","reason":"R","targetEntityId":"nope"}],"warnings":[]});
        assert_eq!(
            validate_bible_result(&result, &request).unwrap_err().code,
            "CODEX_INVALID_REFERENCE"
        );
    }

    #[test]
    fn continuity_nullable_fixture_matches_rust_schema_and_request_contract() {
        serde_json::from_str::<Value>(continuity_schema()).expect("continuity schema must be JSON");
        let request = json!({
            "projectId": "project",
            "passage": {"text":"Text", "passageStartOffset":0, "passageEndOffset":4, "coordinateSystem":"unicode_codepoints"},
            "confirmedStoryBible":[{"id":"entity"}], "confirmedRules":[], "continuityStatesBeforePosition":[], "draftLedger":[], "confirmedLore":[], "relevantSources":[]
        });
        let fixture = json!({
            "observedActions":[{"summary":"","evidenceExcerpt":"","entityIds":[],"startOffset":null,"endOffset":null}],
            "proposedStateChanges":[{"entityId":"entity","relatedEntityId":null,"stateKind":"location","previousState":"","newState":"unbekannt","confidence":0.5,"evidenceExcerpt":"","sourceReferenceId":null,"startOffset":null,"endOffset":null,"reason":""}],
            "objectiveContradictions":[{"findingType":"missing_explanation","subjectEntityId":null,"relatedEntityIds":[],"relatedStateIds":[],"objectiveConflict":"","evidenceExcerpt":"","sourceReferenceId":null,"counterEvidenceExcerpts":[],"confidence":0.5,"startOffset":null,"endOffset":null,"reason":""}],
            "missingExplanations":[],"matchedLoreRules":[],"newRuleProposals":[{"projectId":"project","targetRuleId":null,"title":"","statement":"","scope":"project","prerequisites":[],"effects":[],"exceptions":[],"connectedLoreIds":[],"sourceReferenceIds":[],"evidenceExcerpt":"","chapterId":null,"sceneId":null,"startOffset":null,"endOffset":null,"confidence":0.5,"reason":""}],
            "plotThreadChanges":[{"entityId":"entity","proposedStatus":"open","evidenceExcerpt":"","sourceReferenceId":null,"startOffset":null,"endOffset":null,"reason":"","confidence":0.5}],"confidence":0.5,
            "evidence":[{"id":"evidence","label":"","chapterId":null,"sceneId":null,"entityId":null,"excerpt":null,"startOffset":null,"endOffset":null}],"warnings":[]
        });
        validate_continuity_result(&fixture, &request).expect("nullable fixture must validate");
    }
    #[test]
    fn chat_rejects_unknown_source() {
        let request =
            json!({"projectContext":{"relevantEntities":[],"relevantSources":[{"id":"source-1"}]}});
        let result = json!({"answer":"Antwort","usedEntityIds":[],"usedSourceIds":["source-2"],"uncertainty":"low","warnings":[]});
        assert_eq!(
            validate_chat_result(&result, &request).unwrap_err().code,
            "CODEX_INVALID_REFERENCE"
        );
    }
    #[test]
    fn fake_runner_parses_completed_jsonl_without_starting_a_process() {
        let runner = FakeCodexProcessRunner {
            stdout: br#"{"type":"warning","message":"diagnostic"}
{"type":"turn.completed","result":{"answer":"Belegt","usedEntityIds":[],"usedSourceIds":[],"uncertainty":"low","warnings":[]}}"#.to_vec(),
            stderr: Vec::new(),
        };
        let result = runner
            .run(
                CodexInvocation {
                    task_kind: CodexTaskKind::AnswerWithProjectContext,
                    binary: PathBuf::from("/not-used"),
                    args: Vec::new(),
                    snapshot: PathBuf::from("/not-used"),
                    prompt: "test".into(),
                    timeout_seconds: 1,
                },
                Arc::new(AtomicBool::new(false)),
            )
            .expect("fake runner should parse JSONL");
        assert_eq!(
            result.result.get("answer").and_then(Value::as_str),
            Some("Belegt")
        );
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn snapshot_guard_removes_read_only_snapshot_and_is_idempotent() {
        let mut guard = create_snapshot(&synthetic_input("guard-test")).expect("snapshot");
        let path = guard.path().to_path_buf();
        assert!(path.join("request.json").is_file());
        assert!(!path.join("context.md").exists());
        guard.cleanup().expect("cleanup");
        assert!(!path.exists());
        guard.cleanup().expect("second cleanup");
    }

    #[test]
    fn snapshot_is_cleaned_when_invocation_fails_before_process_start() {
        let input = synthetic_input("invocation-error-test");
        let path = env::temp_dir().join(format!("storymemory-codex-{}", input.task_id));
        let binary = env::temp_dir().join("storymemory-not-executable");
        fs::write(&binary, b"not a binary").expect("test binary");
        let result = run_process(
            &input,
            &AiProviderSettings {
                codex_binary_path: Some(binary.display().to_string()),
                ..AiProviderSettings::default()
            },
            Arc::new(AtomicBool::new(false)),
        );
        assert_eq!(result.expect_err("must fail").code, "CODEX_INCOMPATIBLE");
        assert!(!path.exists());
        let _ = fs::remove_file(binary);
    }

    #[test]
    fn output_limit_is_a_visible_error() {
        let output = read_limited(Cursor::new(vec![b'x'; MAX_STDOUT + 1]), MAX_STDOUT);
        assert_eq!(
            output.expect_err("must reject oversized output").code,
            "CODEX_OUTPUT_TOO_LARGE"
        );
    }

    #[test]
    fn environment_uses_only_the_safe_allowlist() {
        let values = vec![
            (OsString::from("HOME"), OsString::from("/tmp/home")),
            (OsString::from("PATH"), OsString::from("/bin")),
            (OsString::from("CODEX_HOME"), OsString::from("/tmp/codex")),
            (OsString::from("OPENAI_API_KEY"), OsString::from("secret")),
            (OsString::from("GITHUB_TOKEN"), OsString::from("secret")),
            (OsString::from("DATABASE_URL"), OsString::from("secret")),
            (
                OsString::from("STORYMEMORY_PRIVATE_TOKEN"),
                OsString::from("secret"),
            ),
            (
                OsString::from("SOMETHING_PASSWORD"),
                OsString::from("secret"),
            ),
            (OsString::from("HARMLESS_UNKNOWN"), OsString::from("no")),
        ];
        let environment = sanitized_codex_environment(values.into_iter());
        let names: Vec<_> = environment
            .iter()
            .filter_map(|(key, _)| key.to_str())
            .collect();
        assert!(names.contains(&"HOME"));
        assert!(names.contains(&"PATH"));
        assert!(names.contains(&"CODEX_HOME"));
        assert!(!names.iter().any(|name| name.contains("TOKEN")
            || name.contains("DATABASE")
            || name.contains("PASSWORD")));
        assert!(!names.contains(&"HARMLESS_UNKNOWN"));
    }

    #[test]
    fn privacy_acknowledgement_is_required_only_for_codex() {
        assert!(validate_codex_privacy(&AiProviderSettings::default()).is_ok());
        assert_eq!(
            validate_codex_privacy(&AiProviderSettings {
                active_provider: "codex-cli".into(),
                ..AiProviderSettings::default()
            })
            .unwrap_err()
            .code,
            "CODEX_PRIVACY_NOT_ACKNOWLEDGED"
        );
        assert!(validate_codex_privacy(&AiProviderSettings {
            active_provider: "codex-cli".into(),
            codex_privacy_acknowledged_at: Some("2026-08-03T00:00:00Z".into()),
            ..AiProviderSettings::default()
        })
        .is_ok());
    }

    #[test]
    fn jsonl_event_failures_are_classified_precisely() {
        let warning = br#"{"type":"item.failed"}
{"type":"tool.failed"}
{"type":"turn.completed","result":{"answer":"ok","usedEntityIds":[],"usedSourceIds":[],"uncertainty":"low","warnings":[]}}"#;
        let (_, warnings, completed) =
            extract_final_json(warning, &CodexTaskKind::AnswerWithProjectContext)
                .expect("non-fatal item events");
        assert_eq!(warnings.len(), 2);
        assert!(completed);
        for fatal in ["turn.failed", "error", "fatal"] {
            let input = format!(r#"{{"type":"{fatal}"}}"#);
            assert!(
                extract_final_json(input.as_bytes(), &CodexTaskKind::AnswerWithProjectContext)
                    .is_err()
            );
        }
        let incomplete = br#"{"type":"message","result":{"answer":"ok"}}"#;
        assert!(extract_final_json(incomplete, &CodexTaskKind::AnswerWithProjectContext).is_err());
    }

    #[test]
    fn jsonl_limits_are_enforced() {
        let long_line = format!(
            r#"{{"type":"unknown","payload":"{}"}}"#,
            "x".repeat(MAX_JSONL_LINE)
        );
        assert_eq!(
            extract_final_json(
                long_line.as_bytes(),
                &CodexTaskKind::AnswerWithProjectContext
            )
            .unwrap_err()
            .code,
            "CODEX_INVALID_JSONL"
        );
        let many = (0..=MAX_JSONL_EVENTS)
            .map(|_| r#"{"type":"unknown"}"#)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            extract_final_json(many.as_bytes(), &CodexTaskKind::AnswerWithProjectContext)
                .unwrap_err()
                .code,
            "CODEX_INVALID_JSONL"
        );
    }

    fn synthetic_task_result(kind: CodexTaskKind) -> Value {
        match kind {
            CodexTaskKind::ExtractBiblePatch => {
                json!({"proposals":[{"targetEntityId":"entity-1","proposalAction":"update_entity","entityType":"object","candidateName":"Zettel","candidateDescription":"Ein beschrifteter Zettel.","candidateStatus":"proposed","confidence":0.91,"classification":"observable_fact","evidenceExcerpt":"😀 Zettel","startOffset":0,"endOffset":8,"reason":"Die Passage benennt den Gegenstand."}],"warnings":[]})
            }
            CodexTaskKind::ExtractCharacterMemoryPatch => {
                json!({"proposals":[{"proposalKind":"knowledge_change","subjectCharacterId":"char-1","relatedCharacterId":null,"targetEntityId":"entity-1","payload":{"factEntityId":"entity-1","knowledgeState":"suspects","certainty":0.62,"notes":"Malik vermutet etwas über den Zettel."},"classification":"interpretation","confidence":0.72,"evidenceExcerpt":"😀 Zettel","startOffset":0,"endOffset":8,"reason":"Der Kontext legt eine Vermutung nahe."}],"warnings":[]})
            }
            CodexTaskKind::AnswerWithProjectContext => {
                json!({"answer":"Der Zettel ist die relevante Entität.","usedEntityIds":["entity-1"],"usedSourceIds":["source-1"],"uncertainty":"medium","warnings":[]})
            }
            CodexTaskKind::AnalyzeProjectStyle => {
                json!({"observations":[{"observationType":"sentence_length","observationText":"Kurze, klare Sätze.","recommendation":"Beibehalten.","confidence":0.8,"evidence":["😀 Zettel wartet."]}],"overallSummary":"Knapp und beobachtend.","warnings":[]})
            }
            CodexTaskKind::SummarizeScene
            | CodexTaskKind::SummarizeChapter
            | CodexTaskKind::SummarizeBook => {
                json!({"summary":"Malik beobachtet den Zettel.","importantEvents":["Der Zettel wird eingeführt."],"openThreads":["Wer nahm den Zettel?"],"characterChanges":["Malik bleibt aufmerksam."],"knowledgeChanges":["Malik vermutet einen Zusammenhang."],"relationshipEffects":[],"warnings":[]})
            }
            CodexTaskKind::AnalyzeNarrativeSummaries => {
                json!({"summary":"Die narrative Entwicklung führt von der Beobachtung zur offenen Spur.","importantEvents":["Der Zettel wird eingeführt."],"openThreads":["Seine Herkunft bleibt offen."],"characterChanges":["Malik wird aufmerksamer."],"knowledgeChanges":["Malik vermutet einen Zusammenhang."],"relationshipEffects":[],"warnings":[]})
            }
            CodexTaskKind::SynthesizePlotThreads => {
                json!({"summary":"Die Spur wird weiterverfolgt, aber nicht abgeschlossen.","openQuestions":["Wer hinterließ den Zettel?"],"threadGoals":["Herkunft klären"],"developments":["Eine neue Spur wird sichtbar."],"closureCandidates":[],"partiallyResolved":["Die Verbindung ist teilweise erkennbar."],"reopened":[],"threadProposals":[{"entityId":"thread-1","proposedStatus":"closure_candidate","evidenceExcerpt":"😀 Zettel","reason":"Die Spur bewegt sich weiter.","confidence":0.61,"sourceReferenceId":"source-1"}],"warnings":[]})
            }
            CodexTaskKind::AnalyzeBookEndState => {
                json!({"summary":"Vorgeschlagener Buchendzustand.","characterEndStates":["Malik bleibt vorsichtig."],"knowledgeStates":["Malik kennt die Nummer."],"falseBeliefs":[],"relationships":[],"objectOwners":["Der Zettel ist bei Malik."],"injuries":[],"locations":["Wohnung"],"openActions":["Herkunft klären"],"unresolvedThreads":["Wer hinterließ den Zettel?"],"endStateProposals":[{"category":"object_owner","entityId":"entity-1","statement":"Der Zettel ist bei Malik.","confidence":0.8,"evidenceExcerpt":"😀 Zettel","sourceReferenceId":"source-1"}],"warnings":[]})
            }
            CodexTaskKind::GlobalCountercheck => {
                json!({"summary":"Eine Gegenprüfung bleibt als Vorschlag offen.","contradictoryFacts":["Der Status des Zettels ist uneindeutig."],"prematureKnowledge":[],"lostOrDestroyedObjects":[],"timeAndLocationConflicts":[],"contradictoryRules":[],"unclearExceptions":[],"uncertainSources":["source-1"],"countercheckFindings":[{"severity":"warning","category":"object_state","objectiveConflict":"Der Status des Zettels ist uneindeutig.","reason":"Zwei Zustände treffen aufeinander.","confidence":0.55,"evidenceExcerpt":"😀 Zettel","sourceReferenceId":"source-1"}],"warnings":[]})
            }
            CodexTaskKind::AnalyzeLoreDraft => {
                json!({"understandingSummary":"Eine vorläufige Weltbeschreibung mit einer regelhaften Struktur.","confirmedStatements":["Die Notiz beschreibt ein System."],"proposedWorldRules":["Das System hat eine erkennbare Grenze."],"prerequisites":["Eine bestimmte Voraussetzung."],"effects":["Eine beobachtbare Auswirkung."],"limitations":["Die Wirkung ist nicht unbegrenzt."],"costs":["Die Anwendung hat einen Preis."],"exceptions":["Eine mögliche Ausnahme."],"terminology":["System"],"relevantOrganizations":[],"relevantLocations":[],"historicalBackground":[],"unresolvedQuestions":["Welche Bedingung gilt genau?"],"contradictions":[],"excludedContent":[{"content":"Eine einzelne Szene.","suggestedTarget":"manuscript","reason":"Das ist eine konkrete Handlung statt einer Weltregel."}],"clarificationQuestions":["Soll die Grenze immer gelten?"],"confidence":0.74,"warnings":[]})
            }
            CodexTaskKind::BuildLoreSheet => {
                json!({"title":"Vorgeschlagenes Lore Sheet","premise":"Eine vorläufig strukturierte Weltbeschreibung.","categories":["world_rule"],"worldRules":["Das System hat eine erkennbare Grenze."],"worldRuleObjects":[{"temporaryId":"rule-1","title":"Grenze des Systems","statement":"Das System hat eine erkennbare Grenze.","prerequisites":["Eine bestimmte Voraussetzung."],"effects":["Eine beobachtbare Auswirkung."],"limitations":["Die Wirkung ist nicht unbegrenzt."],"costs":["Die Anwendung hat einen Preis."],"exceptions":["Eine mögliche Ausnahme."],"relatedTerminology":["System"],"connectedItemIds":[],"sourceSpans":[{"excerpt":"😀 Zettel","startOffset":0,"endOffset":8}],"confidence":0.74}],"prerequisites":["Eine bestimmte Voraussetzung."],"effects":["Eine beobachtbare Auswirkung."],"limitations":["Die Wirkung ist nicht unbegrenzt."],"costs":["Die Anwendung hat einen Preis."],"exceptions":["Eine mögliche Ausnahme."],"terminology":["System"],"organizations":[],"locations":[],"historicalEvents":[],"knownAspects":["Die Notiz beschreibt ein System."],"unknownAspects":["Welche Bedingung gilt genau?"],"ruleConnections":[],"openQuestions":["Soll die Grenze immer gelten?"],"warnings":[]})
            }
            CodexTaskKind::PlanChapterDraft => {
                json!({"chapterTitle":"Kapitel","chapterGoal":"Den Zettel einordnen.","povCharacterId":"char-1","startingState":"Der Zettel liegt bereit.","endingState":"Malik fasst eine Spur.","chapterSummary":"Malik untersucht einen Zettel.","endingConnection":"Eine neue Spur öffnet sich.","newInformation":["Der Zettel ist wichtig."],"withheldInformation":["Wer ihn hinterließ."],"assumptions":[{"type":"continuity","text":"Der Zettel bleibt erhalten."}],"beats":[{"id":"beat-1","orderIndex":0,"title":"Beobachtung","purpose":"Spur einführen.","participatingCharacterIds":["char-1"],"startingState":"Ruhe","event":"Malik findet den Zettel.","conflict":"Er versteht ihn nicht.","newInformation":["Eine Spur erscheint."],"knowledgeChanges":[{"characterId":"char-1","factEntityId":"entity-1","nextState":"suspects","reason":"Der Text ist auffällig."}],"relationshipChanges":[],"cluesUsed":["entity-1"],"loreEntityIds":["entity-1"],"endingHook":"Eine Frage bleibt.","targetWords":120}],"warnings":[]})
            }
            CodexTaskKind::DraftChapterSection => {
                json!({"content":"Malik hob den Zettel auf.","continuationSummary":"Der Zettel bleibt bei Malik.","continuityState":{"currentLocation":"Zimmer","currentStoryTime":"Abend","presentCharacterIds":["char-1"],"characterStates":[{"characterId":"char-1","state":"aufmerksam","change":"beobachtet"}],"establishedFacts":["Der Zettel existiert."],"knowledgeChanges":[{"characterId":"char-1","factEntityId":"entity-1","nextState":"suspects","reason":"Er liest die Notiz."}],"relationshipChanges":[],"movedObjects":[{"objectId":"entity-1","location":"Maliks Hand","state":"gehalten"}],"injuries":[],"cluesIntroduced":["entity-1"],"promisesCreated":[],"unresolvedActions":["Herkunft des Zettels klären"],"lastParagraphSummary":"Malik hält den Zettel."},"usedEntityIds":["entity-1","char-1"],"usedMemoryIds":[],"usedSourceIds":["source-1"],"warnings":[]})
            }
            CodexTaskKind::ReviewChapterSection | CodexTaskKind::ReviewCompleteChapter => {
                json!({"issues":[{"reviewScope":"section","issueType":"continuity","severity":"warning","title":"Herkunft offen","description":"Die Herkunft des Zettels ist noch offen.","relatedEntityIds":["entity-1"],"relatedSourceIds":["source-1"],"suggestedAction":"Als offene Frage prüfen.","status":"open"}],"warnings":[]})
            }
            CodexTaskKind::AnalyzeContinuityPassage => {
                json!({"observedActions":[{"summary":"Malik sieht den Zettel.","evidenceExcerpt":"😀 Zettel","entityIds":["entity-1","char-1"],"startOffset":0,"endOffset":8}],"proposedStateChanges":[{"entityId":"entity-1","relatedEntityId":null,"stateKind":"item_availability","previousState":"unbekannt","newState":"bei Malik","confidence":0.86,"evidenceExcerpt":"😀 Zettel","sourceReferenceId":"source-1","startOffset":0,"endOffset":8,"reason":"Die Passage etabliert den Gegenstand."}],"objectiveContradictions":[{"findingType":"probable_contradiction","subjectEntityId":"entity-1","relatedEntityIds":[],"relatedStateIds":["state-1"],"objectiveConflict":"Der Status muss geprüft werden.","evidenceExcerpt":"😀 Zettel","sourceReferenceId":"source-1","counterEvidenceExcerpts":["Früherer Zustand"],"counterEvidence":[{"sourceReferenceId":null,"excerpt":"Früherer Zustand","chapterId":null,"sceneId":null,"startOffset":null,"endOffset":null}],"confidence":0.55,"startOffset":0,"endOffset":8,"reason":"Zwei Zustände treffen aufeinander."}],"missingExplanations":[],"matchedLoreRules":[{"ruleId":"rule-1","rationale":"Die Regel könnte die Abweichung erklären.","confidence":0.63}],"newRuleProposals":[{"projectId":"synthetic-project","targetRuleId":null,"title":"Beweise können sich ändern","statement":"Ein physischer Beweis kann unter Bedingungen verändert werden.","scope":"project","prerequisites":["Spezielle Bedingung"],"effects":["Der Beweisstatus ändert sich."],"exceptions":["Keine automatische Kanonänderung"],"connectedLoreIds":["entity-1"],"sourceReferenceIds":["source-1"],"evidenceExcerpt":"😀 Zettel","chapterId":"synthetic-chapter","sceneId":"synthetic-scene","startOffset":0,"endOffset":8,"confidence":0.44,"reason":"Eine neue Regel könnte die Beobachtung erklären."}],"plotThreadChanges":[{"entityId":"thread-1","proposedStatus":"closure_candidate","evidenceExcerpt":"😀 Zettel","sourceReferenceId":"source-1","startOffset":0,"endOffset":8,"reason":"Die Spur bewegt sich weiter.","confidence":0.61}],"confidence":0.71,"evidence":[{"id":"evidence-1","label":"Passage","chapterId":"synthetic-chapter","sceneId":"synthetic-scene","entityId":"entity-1","excerpt":"😀 Zettel","sourceReferenceId":"source-1","startOffset":0,"endOffset":8}],"warnings":[]})
            }
            CodexTaskKind::AnalyzeManuscriptStructure => {
                json!({"scenes":[{"temporaryId":"scene-1","chapterId":"synthetic-chapter","startOffset":0,"endOffset":8,"title":"Zettel","povCharacterName":"Malik","povEntityId":"char-1","location":"Zimmer","storyTime":"Abend","participatingCharacterNames":["Malik"],"goal":"Zettel verstehen","conflict":"Herkunft offen","importantEvents":["Malik findet den Zettel."],"transitionType":"chapter_continuation","boundaryReason":"Kapitelbeginn","confidence":0.8,"evidenceExcerpt":"😀 Zettel"}],"warnings":[]})
            }
            CodexTaskKind::ResolveManuscriptEntityMentions => {
                json!({"entities":[{"temporaryId":"entity-1","entityType":"object","canonicalName":"Zettel","aliases":["Notiz"],"description":"Ein beschrifteter Zettel.","confidence":0.82,"existingEntityId":null}],"mentions":[{"mentionText":"Zettel","startOffset":0,"endOffset":8,"temporaryEntityId":"entity-1","alternativeTemporaryIds":[],"confidence":0.82,"resolutionReason":"Direkte Benennung.","excerpt":"😀 Zettel"}],"relations":[],"events":[],"mergeProposals":[],"warnings":[]})
            }
        }
    }

    #[test]
    fn fake_runner_parses_every_task_result_shape() {
        let kinds = [
            CodexTaskKind::ExtractBiblePatch,
            CodexTaskKind::ExtractCharacterMemoryPatch,
            CodexTaskKind::AnswerWithProjectContext,
            CodexTaskKind::AnalyzeProjectStyle,
            CodexTaskKind::SummarizeScene,
            CodexTaskKind::SummarizeChapter,
            CodexTaskKind::SummarizeBook,
            CodexTaskKind::PlanChapterDraft,
            CodexTaskKind::DraftChapterSection,
            CodexTaskKind::ReviewChapterSection,
            CodexTaskKind::ReviewCompleteChapter,
            CodexTaskKind::AnalyzeContinuityPassage,
            CodexTaskKind::AnalyzeManuscriptStructure,
            CodexTaskKind::ResolveManuscriptEntityMentions,
            CodexTaskKind::AnalyzeNarrativeSummaries,
            CodexTaskKind::SynthesizePlotThreads,
            CodexTaskKind::AnalyzeBookEndState,
            CodexTaskKind::GlobalCountercheck,
            CodexTaskKind::AnalyzeLoreDraft,
            CodexTaskKind::BuildLoreSheet,
        ];
        for kind in kinds {
            let result = synthetic_task_result(kind.clone());
            let stdout = format!("{{\"type\":\"turn.completed\",\"result\":{result}}}");
            let (parsed, _, completed) =
                extract_final_json(stdout.as_bytes(), &kind).expect("task result");
            assert!(completed);
            assert!(result_matches_task(&parsed, &kind));
            let input = synthetic_input(&format!("fixture-{:?}", kind));
            match kind {
                CodexTaskKind::ExtractBiblePatch => {
                    validate_bible_result(&parsed, &input.request_json).expect("Bible fixture")
                }
                CodexTaskKind::ExtractCharacterMemoryPatch => {
                    validate_character_memory_result(&parsed, &input.request_json)
                        .expect("memory fixture")
                }
                CodexTaskKind::AnswerWithProjectContext => {
                    validate_chat_result(&parsed, &input.request_json).expect("chat fixture")
                }
                CodexTaskKind::AnalyzeContinuityPassage => {
                    validate_continuity_result(&parsed, &input.request_json)
                        .expect("continuity fixture")
                }
                _ => validate_longform_result_for_task(&parsed, &kind).expect("longform fixture"),
            };
        }
    }

    #[test]
    fn parser_rejects_result_for_the_wrong_task() {
        let stdout = br#"{"type":"turn.completed","result":{"answer":"ok","usedEntityIds":[],"usedSourceIds":[],"uncertainty":"low","warnings":[]}}"#;
        assert_eq!(
            extract_final_json(stdout, &CodexTaskKind::PlanChapterDraft)
                .unwrap_err()
                .code,
            "CODEX_PROCESS_FAILED"
        );
    }

    #[test]
    fn live_codex_e2e_is_opt_in_and_uses_only_synthetic_scene() {
        if env::var("STORYMEMORY_RUN_CODEX_CONTINUITY_E2E")
            .ok()
            .as_deref()
            != Some("1")
        {
            eprintln!("SKIP live continuity Codex E2E: STORYMEMORY_RUN_CODEX_CONTINUITY_E2E != 1");
            return;
        }
        let capabilities = codex_status(None);
        if !capabilities.installed
            || !capabilities.compatible
            || capabilities.authentication != CodexAuthenticationState::Authenticated
        {
            eprintln!("SKIP live continuity Codex E2E: {}", capabilities.detail);
            return;
        }
        let mut input = synthetic_input(&format!("live-continuity-{}", std::process::id()));
        input.task_kind = CodexTaskKind::AnalyzeContinuityPassage;
        let result = run_task(
            Arc::new(CodexRuntimeState::default()),
            input,
            AiProviderSettings {
                active_provider: "codex-cli".into(),
                codex_binary_path: capabilities.binary_path.clone(),
                codex_privacy_acknowledged_at: Some("2026-08-03T00:00:00Z".into()),
                ..AiProviderSettings::default()
            },
        )
        .expect("synthetic Codex continuity task should complete");
        assert_eq!(result.status, "completed");
        assert!(result.turn_completed);
        assert!(result.result.get("objectiveContradictions").is_some());
        assert!(!env::temp_dir()
            .join(format!(
                "storymemory-codex-live-continuity-{}",
                std::process::id()
            ))
            .exists());
    }

    #[test]
    fn live_longform_e2e_is_opt_in_and_uses_only_synthetic_plan() {
        if env::var("STORYMEMORY_RUN_CODEX_LONGFORM_E2E")
            .ok()
            .as_deref()
            != Some("1")
        {
            eprintln!("SKIP live longform Codex E2E: STORYMEMORY_RUN_CODEX_LONGFORM_E2E != 1");
            return;
        }
        let capabilities = codex_status(None);
        if !capabilities.installed
            || !capabilities.compatible
            || capabilities.authentication != CodexAuthenticationState::Authenticated
        {
            eprintln!("SKIP live longform Codex E2E: {}", capabilities.detail);
            return;
        }
        let settings = AiProviderSettings {
            active_provider: "codex-cli".into(),
            codex_binary_path: capabilities.binary_path.clone(),
            codex_privacy_acknowledged_at: Some("2026-08-03T00:00:00Z".into()),
            ..AiProviderSettings::default()
        };
        let state = Arc::new(CodexRuntimeState::default());
        let mut plan_input = synthetic_input(&format!("live-longform-plan-{}", std::process::id()));
        plan_input.task_kind = CodexTaskKind::PlanChapterDraft;
        let plan = run_task(state.clone(), plan_input, settings.clone()).expect("synthetic plan");
        assert_eq!(plan.status, "completed");
        let mut draft_input =
            synthetic_input(&format!("live-longform-draft-{}", std::process::id()));
        draft_input.task_kind = CodexTaskKind::DraftChapterSection;
        let draft = run_task(state, draft_input, settings).expect("synthetic draft");
        assert_eq!(draft.status, "completed");
    }
}
