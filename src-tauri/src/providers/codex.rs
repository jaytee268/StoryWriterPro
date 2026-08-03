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
    AnswerWithProjectContext,
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

fn prompt_for(kind: &CodexTaskKind) -> (&'static str, &'static str) {
    match kind {
        CodexTaskKind::ExtractBiblePatch => (BIBLE_PROMPT_VERSION, "Du analysierst ein Romanmanuskript für eine kontrollierte Story Bible. Verändere keine Dateien, führe keine Shell-Befehle aus und erfinde keine Informationen. Lies ausschließlich request.json. Liefere ausschließlich JSON nach output-schema.json. Trenne beobachtbare Fakten, Interpretationen, offene Fragen, mögliche Widersprüche und Autorennotizen. Verwende bei entityType ausschließlich character, relationship, place, organization, world_rule, object, event, fact, clue, secret, plot_thread, retcon oder author_note. Wenn keine sicher belegte Information vorliegt, liefere proposals als leeres Array. Ein bestätigter Kanon darf nie still überschrieben werden; nutze bei Konflikten mark_contradiction und targetEntityId. Optionale Werte targetEntityId, startOffset und endOffset müssen als null ausgegeben werden, wenn sie nicht gelten. AI-Offsets sind Unicode-Zeichenpositionen im normalisierten Klartext aus request.json.scene.content."),
        CodexTaskKind::AnswerWithProjectContext => (CHAT_PROMPT_VERSION, "Du bist ein projektbezogener Roman-Assistent. Antworte ausschließlich aus request.json. Erfinde keine Quellen oder IDs. Verwende nur vorhandene Entity- und Source-IDs und liefere ausschließlich JSON nach output-schema.json. Trenne bestätigten Kanon, Vermutungen, Widersprüche und fehlende Informationen."),
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
    let limit = if input.task_kind == CodexTaskKind::ExtractBiblePatch {
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
            if input.task_kind == CodexTaskKind::ExtractBiblePatch {
                BIBLE_SCHEMA.as_bytes()
            } else {
                CHAT_SCHEMA.as_bytes()
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

fn extract_final_json(stdout: &[u8]) -> Result<(Value, Vec<String>, bool), CodexError> {
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
            if let Some(object) = candidate.as_object() {
                if object.contains_key("proposals") || object.contains_key("answer") {
                    final_value = Some(Value::Object(object.clone()));
                }
            } else if let Some(string) = candidate.as_str() {
                if let Ok(value) = serde_json::from_str::<Value>(string) {
                    if value.get("proposals").is_some() || value.get("answer").is_some() {
                        final_value = Some(value);
                    }
                }
            }
        }
        if event.get("proposals").is_some() || event.get("answer").is_some() {
            final_value = Some(event);
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
        let (result, mut warnings, turn_completed) = extract_final_json(&stdout)?;
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
        _invocation: CodexInvocation,
        _cancellation: Arc<AtomicBool>,
    ) -> Result<CodexProcessResult, CodexError> {
        let (result, mut warnings, turn_completed) = extract_final_json(&self.stdout)?;
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
        CodexTaskKind::AnswerWithProjectContext => validate_chat_result(&raw, &input.request_json)?,
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
                "scene": {"id":"synthetic-scene","title":"Testszene","content":"Synthetischer Test ohne auswertbare Story-Bible-Fakten.","pov":"","location":"","storyTime":"","goal":"","notes":"Synthetischer Test"},
                "existingEntities": [],
                "relevantSources": []
            }),
            timeout_seconds: 90,
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
        let (_, warnings, completed) = extract_final_json(warning).expect("non-fatal item events");
        assert_eq!(warnings.len(), 2);
        assert!(completed);
        for fatal in ["turn.failed", "error", "fatal"] {
            let input = format!(r#"{{"type":"{fatal}"}}"#);
            assert!(extract_final_json(input.as_bytes()).is_err());
        }
        let incomplete = br#"{"type":"message","result":{"answer":"ok"}}"#;
        assert!(extract_final_json(incomplete).is_err());
    }

    #[test]
    fn jsonl_limits_are_enforced() {
        let long_line = format!(
            r#"{{"type":"unknown","payload":"{}"}}"#,
            "x".repeat(MAX_JSONL_LINE)
        );
        assert_eq!(
            extract_final_json(long_line.as_bytes()).unwrap_err().code,
            "CODEX_INVALID_JSONL"
        );
        let many = (0..=MAX_JSONL_EVENTS)
            .map(|_| r#"{"type":"unknown"}"#)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            extract_final_json(many.as_bytes()).unwrap_err().code,
            "CODEX_INVALID_JSONL"
        );
    }

    #[test]
    fn live_codex_e2e_is_opt_in_and_uses_only_synthetic_scene() {
        if env::var("STORYMEMORY_RUN_CODEX_E2E").ok().as_deref() != Some("1") {
            return;
        }
        let input = synthetic_input(&format!("live-e2e-{}", std::process::id()));
        let codex_binary_path = [
            "/Users/juliantows/.local/bin/codex",
            "/Applications/ChatGPT.app/Contents/Resources/codex",
        ]
        .into_iter()
        .find(|path| Path::new(path).is_file())
        .map(str::to_owned);
        let result = run_task(
            Arc::new(CodexRuntimeState::default()),
            input,
            AiProviderSettings {
                active_provider: "codex-cli".into(),
                codex_binary_path,
                codex_privacy_acknowledged_at: Some("2026-08-03T00:00:00Z".into()),
                ..AiProviderSettings::default()
            },
        )
        .expect("synthetic Codex task should complete");
        assert_eq!(result.status, "completed");
        assert!(result.turn_completed);
        assert!(result.result.get("proposals").is_some());
        assert!(!env::temp_dir()
            .join(format!("storymemory-codex-live-e2e-{}", std::process::id()))
            .exists());
    }
}
