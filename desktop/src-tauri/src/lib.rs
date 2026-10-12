use std::{
    collections::HashSet,
    fs,
    path::Path,
    sync::{atomic::AtomicBool, Mutex},
};

use ai::keychain::{MacOsKeychain, SystemKeychainAdapter};
use app_error::AppError;
use database::Database;
use native::voice_input_state::VoiceInputState;
use repository::attachment_repository::AttachmentRepository;
use rusqlite::params;
use tauri::Manager;

pub mod ai;
pub mod app_error;
pub mod capture;
pub mod card_lifecycle;
pub mod commands;
pub mod database;
pub mod domain;
pub mod menu;
pub mod native;
pub mod repository;

pub struct AppState {
    pub(crate) database: Database,
    pub(crate) attachment_repository: AttachmentRepository,
    pub(crate) capture_cache_directory: std::path::PathBuf,
    pub(crate) cancelled_ai_requests: Mutex<HashSet<String>>,
    pub(crate) ai_keychain: MacOsKeychain<SystemKeychainAdapter>,
    pub(crate) screenshot_in_progress: AtomicBool,
    pub(crate) voice_input: Mutex<VoiceInputState>,
}

impl AppState {
    pub(crate) fn new(database: Database, attachment_repository: AttachmentRepository) -> Self {
        let capture_cache_directory = attachment_repository.cache_directory().join("capture");
        Self {
            database,
            attachment_repository,
            capture_cache_directory,
            cancelled_ai_requests: Mutex::new(HashSet::new()),
            ai_keychain: MacOsKeychain::new(SystemKeychainAdapter),
            screenshot_in_progress: AtomicBool::new(false),
            voice_input: Mutex::new(VoiceInputState::default()),
        }
    }

    fn open(data_directory: &Path, cache_directory: &Path) -> Result<Self, AppError> {
        let attachment_directory = data_directory.join("attachments");
        let attachment_cache_directory = cache_directory.join("attachments");
        fs::create_dir_all(&attachment_directory).map_err(|_| app_directory_error())?;
        fs::create_dir_all(&attachment_cache_directory).map_err(|_| app_directory_error())?;
        let attachment_repository =
            AttachmentRepository::new(attachment_directory, attachment_cache_directory);
        let database = Database::open(&data_directory.join("ideacard.sqlite3"))?;
        attachment_repository.cleanup_staged_removals(|relative_path| {
            database.with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT EXISTS(
                            SELECT 1 FROM attachments WHERE relative_path = ?1
                         )",
                        params![relative_path],
                        |row| row.get::<_, bool>(0),
                    )
                    .map_err(AppError::from)
            })
        })?;

        Ok(Self::new(database, attachment_repository))
    }
}

fn app_directory_error() -> AppError {
    AppError::new(
        "app_directory_unavailable",
        "无法准备应用数据目录",
        "检查磁盘空间和目录权限后重新打开应用",
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let data_directory = app
                .path()
                .app_data_dir()
                .map_err(|_| app_directory_error())?;
            let cache_directory = app
                .path()
                .app_cache_dir()
                .map_err(|_| app_directory_error())?;
            app.manage(AppState::open(&data_directory, &cache_directory)?);
            menu::setup_menu(app.handle())?;
            menu::register_shortcuts(app.handle());
            native::voice_input::install(app.handle());

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && matches!(event, tauri::WindowEvent::CloseRequested { .. })
            {
                let _ = window.hide();
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::card_commands::list_cards,
            commands::card_commands::get_card,
            commands::card_commands::create_card,
            commands::card_commands::update_card,
            commands::card_commands::archive_card,
            commands::card_commands::restore_card,
            commands::card_commands::delete_card,
            commands::document_commands::get_card_document,
            commands::document_commands::save_card_document,
            commands::document_commands::load_inline_image,
            commands::document_commands::create_rich_card,
            commands::concept_commands::list_concepts,
            commands::concept_commands::get_concept,
            commands::concept_commands::list_card_concepts,
            commands::concept_commands::delete_concept,
            commands::concept_commands::save_concept_annotation,
            commands::ai_commands::get_ai_settings,
            commands::ai_commands::save_ai_settings,
            commands::ai_commands::generate_card_title,
            commands::ai_commands::get_title_generation_settings,
            commands::ai_commands::save_title_generation_settings,
            commands::ai_commands::get_concept_assistant_settings,
            commands::ai_commands::save_concept_assistant_settings,
            commands::ai_commands::remove_ai_key,
            commands::ai_commands::test_ai_connection,
            commands::ai_commands::list_card_ai_messages,
            commands::ai_commands::send_card_ai_message,
            commands::ai_commands::regenerate_card_ai_message,
            commands::ai_commands::explain_card_concept,
            commands::ai_commands::cancel_ai_request,
            commands::ai_commands::open_external_url,
            commands::capture_commands::read_clipboard_draft,
            commands::capture_commands::save_capture_draft,
            commands::capture_commands::import_files,
            commands::capture_commands::capture_screenshot,
            commands::capture_commands::get_capture_status,
            commands::capture_commands::reveal_attachment,
            commands::clipboard_commands::read_clipboard_text,
            commands::clipboard_commands::write_clipboard_text,
            commands::native_text_area_commands::sync_native_text_area,
            commands::native_text_area_commands::remove_native_text_area,
            commands::native_text_area_commands::focus_native_text_area,
            commands::native_text_field_commands::sync_native_text_field,
            commands::native_text_field_commands::remove_native_text_field,
            commands::native_text_field_commands::focus_native_text_field,
            commands::voice_input_commands::set_voice_input_target,
            commands::voice_input_commands::finish_voice_input_session,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        if matches!(event, tauri::RunEvent::Reopen { .. }) {
            menu::restore_main_window(app);
        }

        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::AppState;

    #[test]
    fn opening_app_state_prepares_all_local_directories() {
        let root = tempdir().unwrap();
        let data_directory = root.path().join("data");
        let cache_directory = root.path().join("cache");
        let abandoned = data_directory.join("attachments/.deleting/old/file.txt");
        std::fs::create_dir_all(abandoned.parent().unwrap()).unwrap();
        std::fs::write(&abandoned, b"old").unwrap();

        let _state = AppState::open(&data_directory, &cache_directory).unwrap();

        assert!(data_directory.join("ideacard.sqlite3").is_file());
        assert!(data_directory.join("attachments").is_dir());
        assert!(cache_directory.join("attachments").is_dir());
        assert!(!data_directory.join("attachments/.deleting").exists());
    }
}
