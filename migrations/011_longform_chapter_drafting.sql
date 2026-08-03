CREATE TABLE IF NOT EXISTS project_story_direction (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    premise TEXT NOT NULL DEFAULT '',
    current_story_phase TEXT NOT NULL DEFAULT '',
    book_goal TEXT NOT NULL DEFAULT '',
    planned_ending TEXT NOT NULL DEFAULT '',
    ending_status TEXT NOT NULL DEFAULT 'open' CHECK(ending_status IN ('fixed','preferred','open')),
    central_twist TEXT NOT NULL DEFAULT '',
    thematic_goal TEXT NOT NULL DEFAULT '',
    must_happen_json TEXT NOT NULL DEFAULT '[]',
    must_not_happen_json TEXT NOT NULL DEFAULT '[]',
    next_turning_point TEXT NOT NULL DEFAULT '',
    reveal_constraints_json TEXT NOT NULL DEFAULT '[]',
    author_notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS project_writing_preferences (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    words_per_page INTEGER NOT NULL DEFAULT 250 CHECK(words_per_page BETWEEN 150 AND 500),
    preferred_section_words INTEGER NOT NULL DEFAULT 850 CHECK(preferred_section_words BETWEEN 400 AND 1500),
    maximum_section_words INTEGER NOT NULL DEFAULT 1200 CHECK(maximum_section_words BETWEEN 600 AND 2000),
    default_scene_count INTEGER NOT NULL DEFAULT 4 CHECK(default_scene_count BETWEEN 1 AND 12),
    require_plan_confirmation INTEGER NOT NULL DEFAULT 1,
    require_final_confirmation INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS narrative_summaries (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    scope_type TEXT NOT NULL CHECK(scope_type IN ('scene','chapter','book','project')),
    scope_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    summary TEXT NOT NULL,
    important_events_json TEXT NOT NULL DEFAULT '[]',
    open_threads_json TEXT NOT NULL DEFAULT '[]',
    character_changes_json TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'proposed' CHECK(status IN ('proposed','confirmed','outdated','rejected')),
    author_confirmed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_id, scope_type, scope_id, content_hash)
);
CREATE INDEX IF NOT EXISTS idx_narrative_summaries_scope ON narrative_summaries(project_id, scope_type, scope_id);

CREATE TABLE IF NOT EXISTS project_style_analysis_runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_hash TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    error_message TEXT
);

CREATE TABLE IF NOT EXISTS project_style_observations (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES project_style_analysis_runs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    observation_type TEXT NOT NULL,
    observation_text TEXT NOT NULL,
    recommendation TEXT NOT NULL DEFAULT '',
    confidence REAL NOT NULL CHECK(confidence BETWEEN 0 AND 1),
    evidence_json TEXT NOT NULL DEFAULT '[]',
    review_status TEXT NOT NULL DEFAULT 'pending' CHECK(review_status IN ('pending','accepted','edited','rejected')),
    reviewed_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS chapter_generation_jobs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    target_book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    target_after_chapter_id TEXT REFERENCES chapters(id) ON DELETE SET NULL,
    requested_pages REAL,
    target_words INTEGER NOT NULL CHECK(target_words > 0),
    requested_scene_count INTEGER,
    user_instruction TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('preparing','needs_input','planning','plan_ready','generating','reviewing','draft_ready','accepted','cancelled','failed')),
    active_provider TEXT NOT NULL,
    content_context_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    error_message TEXT
);
CREATE INDEX IF NOT EXISTS idx_generation_jobs_project_status ON chapter_generation_jobs(project_id, status, updated_at DESC);

CREATE TABLE IF NOT EXISTS chapter_generation_assumptions (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES chapter_generation_jobs(id) ON DELETE CASCADE,
    assumption_type TEXT NOT NULL,
    assumption_text TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','accepted','edited','rejected')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS chapter_generation_plans (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL UNIQUE REFERENCES chapter_generation_jobs(id) ON DELETE CASCADE,
    chapter_title TEXT NOT NULL,
    chapter_goal TEXT NOT NULL,
    pov_character_id TEXT REFERENCES story_entities(id) ON DELETE SET NULL,
    starting_state TEXT NOT NULL DEFAULT '',
    ending_state TEXT NOT NULL DEFAULT '',
    chapter_summary TEXT NOT NULL,
    ending_connection TEXT NOT NULL DEFAULT '',
    new_information TEXT NOT NULL DEFAULT '',
    withheld_information TEXT NOT NULL DEFAULT '',
    plan_json TEXT NOT NULL,
    review_status TEXT NOT NULL DEFAULT 'pending' CHECK(review_status IN ('pending','accepted','edited','rejected')),
    reviewed_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS chapter_generation_sections (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES chapter_generation_jobs(id) ON DELETE CASCADE,
    plan_beat_id TEXT NOT NULL,
    order_index INTEGER NOT NULL,
    target_words INTEGER NOT NULL,
    actual_words INTEGER NOT NULL DEFAULT 0,
    content TEXT NOT NULL DEFAULT '',
    continuation_summary TEXT NOT NULL DEFAULT '',
    continuity_state_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','generating','generated','reviewed','regenerate_requested','failed')),
    provider_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(job_id, order_index)
);

CREATE TABLE IF NOT EXISTS chapter_generation_reviews (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES chapter_generation_jobs(id) ON DELETE CASCADE,
    section_id TEXT REFERENCES chapter_generation_sections(id) ON DELETE CASCADE,
    review_scope TEXT NOT NULL CHECK(review_scope IN ('section','chapter')),
    issue_type TEXT NOT NULL,
    severity TEXT NOT NULL CHECK(severity IN ('info','warning','blocking')),
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    related_entity_ids_json TEXT NOT NULL DEFAULT '[]',
    related_source_ids_json TEXT NOT NULL DEFAULT '[]',
    suggested_action TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'open',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_generation_sections_job ON chapter_generation_sections(job_id, order_index);
CREATE INDEX IF NOT EXISTS idx_generation_reviews_job ON chapter_generation_reviews(job_id, status);
