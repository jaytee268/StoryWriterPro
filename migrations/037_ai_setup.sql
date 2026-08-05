CREATE TABLE IF NOT EXISTS ai_setup_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    status TEXT NOT NULL CHECK (status IN ('pending', 'completed')),
    selected_mode TEXT CHECK (selected_mode IN ('api', 'codex-cli', 'offline')),
    selected_provider TEXT CHECK (selected_provider IS NULL OR selected_provider = 'openai-api'),
    completed_at TEXT,
    updated_at TEXT NOT NULL
);
