use std::{fs, path::PathBuf};

use rusqlite::OptionalExtension;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

use crate::{
    app_error::AppError,
    capture::{
        clipboard::{ClipboardService, SystemClipboard},
        import::{FileImportResult, FileImportService},
        screenshot::{ScreenshotCaptureResult, ScreenshotCaptureService},
        service::CaptureDraft,
    },
    domain::card::{AttachmentKind, Card, ExtractionStatus, NewCardInput},
    menu::{CaptureAction, SHORTCUTS},
    native::client::{NativeClient, TauriNativeTransport},
    repository::{
        attachment_repository::AttachmentClass,
        card_repository::{CardRepository, NewAttachmentInput},
    },
    AppState,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptureSystemStatus {
    screen_permission: String,
    shortcuts: Vec<ShortcutSystemStatus>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShortcutSystemStatus {
    shortcut: &'static str,
    action: CaptureAction,
    registered: bool,
}

#[tauri::command]
pub(crate) async fn read_clipboard_draft(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CaptureDraft, AppError> {
    let clipboard = SystemClipboard::open()?;
    let native = NativeClient::new(TauriNativeTransport::new(app));
    let mut service =
        ClipboardService::new(state.capture_cache_directory.clone(), clipboard, native);
    service.read().await
}

#[tauri::command]
pub(crate) async fn save_capture_draft(
    draft: CaptureDraft,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Card, AppError> {
    let card = save_capture_draft_from_state(draft, &state)?;
    Ok(crate::ai::title::initialize_title(app, card).await)
}

fn save_capture_draft_from_state(draft: CaptureDraft, state: &AppState) -> Result<Card, AppError> {
    if draft.content.trim().is_empty() && draft.temporary_attachment_ids.is_empty() {
        return Err(AppError::new(
            "capture_empty",
            "记录内容不能为空",
            "补充内容或附件后再保存",
        ));
    }
    let input = NewCardInput {
        title: draft.title,
        original_content: draft.content.clone(),
        note: draft.note,
        source_type: draft.source_type,
        capture_method: draft.kind,
        source_name: draft.source_name,
        source_uri: draft.source_uri,
    };
    let Some(attachment_id) = draft.temporary_attachment_ids.first() else {
        return CardRepository::new(&state.database)
            .create(input)
            .map(|result| result.card);
    };
    if draft.temporary_attachment_ids.len() != 1 || Uuid::parse_str(attachment_id).is_err() {
        return Err(temporary_attachment_invalid());
    }
    let temporary_path = state
        .capture_cache_directory
        .join(format!("{attachment_id}.png"));
    if !temporary_path.is_file() {
        return Err(temporary_attachment_invalid());
    }
    let stored = state
        .attachment_repository
        .store(&temporary_path, AttachmentClass::DocumentOrImage)?;
    let extraction_status = if draft.content.trim().is_empty() {
        ExtractionStatus::Empty
    } else {
        ExtractionStatus::Completed
    };
    let attachment = NewAttachmentInput {
        kind: AttachmentKind::Image,
        original_name: "剪贴板图片.png".to_owned(),
        relative_path: stored.relative_path.clone(),
        mime_type: stored.mime_type,
        byte_size: stored.byte_size,
        checksum: stored.checksum,
        extraction_status,
        extraction_error: String::new(),
        extracted_text: draft.content,
    };
    match CardRepository::new(&state.database).create_with_attachment(input, attachment) {
        Ok(result) => {
            let _ = fs::remove_file(temporary_path);
            Ok(result.card)
        }
        Err(error) => {
            let _ = state
                .attachment_repository
                .remove_stored(&stored.relative_path);
            Err(error)
        }
    }
}

#[tauri::command]
pub(crate) async fn import_files(
    paths: Vec<PathBuf>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<FileImportResult>, AppError> {
    let mut results = FileImportService::new(
        &state.database,
        &state.attachment_repository,
        NativeClient::new(TauriNativeTransport::new(app.clone())),
    )
    .import_paths(&paths)
    .await?;
    let mut pending = Vec::new();
    for (index, result) in results.iter_mut().enumerate() {
        if let Some(card) = result.card.take() {
            let fallback = card.clone();
            pending.push((index, fallback, tauri::async_runtime::spawn(
                crate::ai::title::initialize_title(app.clone(), card),
            )));
        }
    }
    for (index, fallback, task) in pending {
        results[index].card = Some(task.await.unwrap_or(fallback));
    }
    Ok(results)
}

#[tauri::command]
pub(crate) async fn capture_screenshot(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ScreenshotCaptureResult, AppError> {
    let Some(guard) = ScreenshotSessionGuard::acquire(&state.screenshot_in_progress) else {
        return Ok(ScreenshotCaptureResult::Cancelled);
    };
    let mut result = ScreenshotCaptureService::new(
        state.capture_cache_directory.clone(),
        &state.database,
        &state.attachment_repository,
        NativeClient::new(TauriNativeTransport::new(app.clone())),
    )
    .capture()
    .await;
    if let Ok(ScreenshotCaptureResult::Saved { card }) = &mut result {
        **card = crate::ai::title::initialize_title(app.clone(), (**card).clone()).await;
    }
    drop(guard);
    match &result {
        Ok(ScreenshotCaptureResult::Saved { .. }) => {
            let _ = app
                .notification()
                .builder()
                .title("IdeaCard")
                .body("截图已保存到 IdeaCard")
                .show();
        }
        Err(error) => {
            if error.code == "accessibility_not_trusted" {
                // Prompted by the main app so the system registers a proper
                // "IdeaCard" entry in the Accessibility list; the helper
                // inherits that trust as a child process.
                crate::native::accessibility::prompt_for_trust();
            }
            let _ = app
                .notification()
                .builder()
                .title("IdeaCard 截图失败")
                .body(screenshot_failure_notification(error))
                .show();
        }
        Ok(ScreenshotCaptureResult::Cancelled) => {}
    }
    result
}

fn screenshot_failure_notification(error: &AppError) -> String {
    if error.recovery.is_empty() || error.recovery.starts_with("x-apple.systempreferences:") {
        error.message.clone()
    } else {
        format!("{}；{}", error.message, error.recovery)
    }
}

struct ScreenshotSessionGuard<'a>(&'a std::sync::atomic::AtomicBool);

impl<'a> ScreenshotSessionGuard<'a> {
    fn acquire(flag: &'a std::sync::atomic::AtomicBool) -> Option<Self> {
        flag.compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        )
        .map(|_| Self(flag))
        .ok()
    }
}

impl Drop for ScreenshotSessionGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}

#[tauri::command]
pub(crate) async fn get_capture_status(app: AppHandle) -> Result<CaptureSystemStatus, AppError> {
    let screen_permission = NativeClient::new(TauriNativeTransport::new(app.clone()))
        .screen_permission()
        .await?;
    let shortcuts = SHORTCUTS
        .into_iter()
        .map(|(shortcut, action)| ShortcutSystemStatus {
            shortcut,
            action,
            registered: app.global_shortcut().is_registered(shortcut),
        })
        .collect();
    Ok(CaptureSystemStatus {
        screen_permission,
        shortcuts,
    })
}

#[tauri::command]
pub(crate) fn reveal_attachment(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let path = resolve_attachment_path(&state.database, &state.attachment_repository, &id)?;
    app.opener()
        .open_path(path.to_string_lossy().into_owned(), None::<String>)
        .map_err(|_| {
            AppError::new(
                "attachment_open_failed",
                "无法打开附件",
                "检查默认应用后重试",
            )
        })
}

fn resolve_attachment_path(
    database: &crate::database::Database,
    attachments: &crate::repository::attachment_repository::AttachmentRepository,
    id: &str,
) -> Result<PathBuf, AppError> {
    let relative_path = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT relative_path FROM attachments WHERE id = ?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(AppError::from)?
            .ok_or_else(|| {
                AppError::new("attachment_not_found", "找不到这个附件", "刷新卡片后重试")
            })
    })?;
    attachments.resolve(&relative_path)
}

fn temporary_attachment_invalid() -> AppError {
    AppError::new(
        "temporary_attachment_invalid",
        "临时附件已经失效",
        "重新捕获内容后再保存",
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::atomic::AtomicBool};

    use tempfile::tempdir;

    use super::{resolve_attachment_path, screenshot_failure_notification, ScreenshotSessionGuard};
    use crate::{
        app_error::AppError,
        database::Database,
        domain::card::{AttachmentKind, CaptureMethod, ExtractionStatus, NewCardInput, SourceType},
        repository::{
            attachment_repository::{AttachmentClass, AttachmentRepository},
            card_repository::{CardRepository, NewAttachmentInput},
        },
    };

    #[test]
    fn resolves_only_known_attachment_ids_through_the_repository() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("source.txt");
        fs::write(&source, "内容").unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachments = AttachmentRepository::new(
            directory.path().join("attachments"),
            directory.path().join("cache"),
        );
        let stored = attachments
            .store(&source, AttachmentClass::DocumentOrImage)
            .unwrap();
        let card = CardRepository::new(&database)
            .create_with_attachment(
                NewCardInput {
                    title: "材料".to_owned(),
                    original_content: "内容".to_owned(),
                    note: String::new(),
                    source_type: SourceType::Document,
                    capture_method: CaptureMethod::File,
                    source_name: "source.txt".to_owned(),
                    source_uri: String::new(),
                },
                NewAttachmentInput {
                    kind: AttachmentKind::Document,
                    original_name: "source.txt".to_owned(),
                    relative_path: stored.relative_path,
                    mime_type: stored.mime_type,
                    byte_size: stored.byte_size,
                    checksum: stored.checksum,
                    extraction_status: ExtractionStatus::Completed,
                    extraction_error: String::new(),
                    extracted_text: "内容".to_owned(),
                },
            )
            .unwrap()
            .card;

        let resolved =
            resolve_attachment_path(&database, &attachments, &card.attachments[0].id).unwrap();
        assert!(resolved.is_file());
        assert_eq!(
            resolve_attachment_path(&database, &attachments, "missing")
                .unwrap_err()
                .code,
            "attachment_not_found"
        );
    }

    #[test]
    fn screenshot_guard_ignores_overlapping_sessions_and_recovers_after_drop() {
        let flag = AtomicBool::new(false);
        let first = ScreenshotSessionGuard::acquire(&flag).unwrap();

        assert!(ScreenshotSessionGuard::acquire(&flag).is_none());
        drop(first);
        assert!(ScreenshotSessionGuard::acquire(&flag).is_some());
    }

    #[test]
    fn screenshot_failure_notification_explains_why_no_card_was_created() {
        let error = AppError::new(
            "scrolling_capture_unreliable",
            "页面内容无法可靠拼接，未生成卡片",
            "等待页面停止变化后重试；滚动速度会由 IdeaCard 自动限制",
        );

        assert_eq!(
            screenshot_failure_notification(&error),
            "页面内容无法可靠拼接，未生成卡片；等待页面停止变化后重试；滚动速度会由 IdeaCard 自动限制"
        );
    }
}
