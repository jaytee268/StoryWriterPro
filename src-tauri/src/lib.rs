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
            commands::get_dashboard_snapshot,
            commands::list_story_entities,
            commands::save_scene,
            commands::create_chapter,
            commands::create_scene,
            commands::save_story_entity,
            commands::check_local_languagetool,
            commands::provider_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running StoryMemory");
}
