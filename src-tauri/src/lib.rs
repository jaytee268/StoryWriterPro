#![allow(dead_code)]

mod commands;
mod database;
mod models;
mod providers;
mod services;

use database::DbState;
use providers::codex::CodexRuntimeState;
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = DbState::open(app.handle())?;
            app.manage(state);
            app.manage(Arc::new(CodexRuntimeState::default()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_workspace,
            commands::create_project,
            commands::create_chapter,
            commands::update_chapter,
            commands::create_scene,
            commands::update_scene,
            commands::create_scene_version,
            commands::list_scene_versions,
            commands::restore_scene_version,
            commands::get_editor_preferences,
            commands::save_editor_preferences,
            commands::list_story_entities,
            commands::save_story_entity,
            commands::create_story_entity,
            commands::update_story_entity,
            commands::archive_story_entity,
            commands::get_story_entity,
            commands::create_source_reference,
            commands::list_source_references,
            commands::create_bible_update_run,
            commands::list_bible_update_runs,
            commands::list_bible_proposals,
            commands::save_bible_proposals,
            commands::review_bible_proposal,
            commands::complete_bible_review,
            commands::database_info,
            commands::check_local_languagetool,
            commands::provider_status,
            commands::get_ai_provider_settings,
            commands::save_ai_provider_settings,
            commands::get_codex_provider_status,
            commands::run_codex_task,
            commands::cancel_codex_task,
            commands::get_lore_metadata,
            commands::save_lore_metadata,
            commands::get_character_profile,
            commands::save_character_profile,
            commands::list_character_scene_states,
            commands::save_character_scene_state,
            commands::get_project_style,
            commands::save_project_style,
            commands::list_style_references,
            commands::create_style_reference,
            commands::update_style_reference,
            commands::delete_style_reference,
            commands::create_lore_entry,
            commands::list_story_entity_relations,
            commands::create_story_entity_relation,
            commands::delete_story_entity_relation
        ])
        .run(tauri::generate_context!())
        .expect("error while running StoryMemory");
}
