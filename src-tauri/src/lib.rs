#![allow(dead_code)]

mod commands;
mod database;
mod models;
mod providers;
mod services;

use database::DbState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = DbState::open(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_workspace,
            commands::create_project,
            commands::create_chapter,
            commands::create_scene,
            commands::update_scene,
            commands::list_scene_versions,
            commands::restore_scene_version,
            commands::get_editor_preferences,
            commands::save_editor_preferences,
            commands::list_story_entities,
            commands::save_story_entity,
            commands::database_info,
            commands::check_local_languagetool,
            commands::provider_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running StoryMemory");
}
