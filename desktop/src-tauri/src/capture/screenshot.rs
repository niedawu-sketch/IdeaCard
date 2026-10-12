use std::{fs, path::PathBuf};

use chrono::Local;
use rusqlite::params;
use uuid::Uuid;

use crate::{
    app_error::AppError,
    database::Database,
    domain::card::{
        AttachmentKind, CaptureMethod, Card, ExtractionStatus, NewCardInput, SourceType,
    },
    native::client::{NativeCaptureKind, NativeClient, NativeScreenshot, NativeTransport},
    repository::{
        attachment_repository::{AttachmentClass, AttachmentRepository},
        card_repository::{CardRepository, NewAttachmentInput},
    },
};

const SCREEN_PERMISSION_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ScreenshotCaptureResult {
    Saved { card: Box<Card> },
    Cancelled,
}

pub struct ScreenshotCaptureService<'a, T> {
    cache_directory: PathBuf,
    database: &'a Database,
    attachments: &'a AttachmentRepository,
    native: NativeClient<T>,
}

impl<'a, T: NativeTransport> ScreenshotCaptureService<'a, T> {
    pub fn new(
        cache_directory: PathBuf,
        database: &'a Database,
        attachments: &'a AttachmentRepository,
        native: NativeClient<T>,
    ) -> Self {
        Self {
            cache_directory,
            database,
            attachments,
            native,
        }
    }

    pub async fn capture(&self) -> Result<ScreenshotCaptureResult, AppError> {
        fs::create_dir_all(&self.cache_directory).map_err(|_| screenshot_cache_failed())?;
        let output_path = self.cache_directory.join(format!("{}.png", Uuid::new_v4()));
        let captured = match self.native.screenshot(output_path.clone()).await {
            Ok(Some(capture)) if capture.path == output_path => capture,
            Ok(Some(_)) => return Err(screenshot_result_invalid()),
            Ok(None) => return Ok(ScreenshotCaptureResult::Cancelled),
            Err(error) if error.code == "screen_permission_denied" => {
                return Err(AppError::new(
                    "screen_permission_denied",
                    "没有屏幕录制权限",
                    SCREEN_PERMISSION_SETTINGS_URL,
                ));
            }
            Err(error) => return Err(error),
        };
        let NativeScreenshot {
            path: captured_path,
            kind: captured_kind,
            note,
        } = captured;
        let temp_guard = ScreenshotFileGuard::new(captured_path.clone());

        let (text, extraction_status, extraction_error) =
            match self.native.ocr(captured_path.clone()).await {
                Ok(text) if text.trim().is_empty() => {
                    (text, ExtractionStatus::Empty, String::new())
                }
                Ok(text) => (text, ExtractionStatus::Completed, String::new()),
                Err(error) => (
                    String::new(),
                    ExtractionStatus::Failed,
                    error.code.to_owned(),
                ),
            };
        let (title_prefix, source_name, original_name) = screenshot_metadata(captured_kind);
        let title = unique_screenshot_title(
            self.database,
            &format!(
                "{title_prefix} {}",
                Local::now().format("%Y-%m-%d %H:%M:%S")
            ),
        )?;
        let stored = self
            .attachments
            .store(&captured_path, AttachmentClass::DocumentOrImage)?;
        let input = NewCardInput {
            title,
            original_content: text.clone(),
            note,
            source_type: SourceType::Personal,
            capture_method: CaptureMethod::Screenshot,
            source_name: source_name.to_owned(),
            source_uri: String::new(),
        };
        let attachment = NewAttachmentInput {
            kind: AttachmentKind::Image,
            original_name: original_name.to_owned(),
            relative_path: stored.relative_path.clone(),
            mime_type: stored.mime_type,
            byte_size: stored.byte_size,
            checksum: stored.checksum,
            extraction_status,
            extraction_error,
            extracted_text: text,
        };
        let card =
            match CardRepository::new(self.database).create_with_attachment(input, attachment) {
                Ok(result) => result.card,
                Err(error) => {
                    let _ = self.attachments.remove_stored(&stored.relative_path);
                    return Err(error);
                }
            };
        drop(temp_guard);
        Ok(ScreenshotCaptureResult::Saved {
            card: Box::new(card),
        })
    }
}

fn screenshot_metadata(kind: NativeCaptureKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        NativeCaptureKind::Region => ("截图", "区域截图", "截图.png"),
        NativeCaptureKind::Scrolling => ("滚动截图", "滚动截图", "滚动截图.png"),
    }
}

fn unique_screenshot_title(database: &Database, base: &str) -> Result<String, AppError> {
    database.with_connection(|connection| {
        let mut suffix = 1_u32;
        loop {
            let candidate = if suffix == 1 {
                base.to_owned()
            } else {
                format!("{base} ({suffix})")
            };
            let exists = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM cards WHERE title = ?1)",
                    params![&candidate],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(AppError::from)?;
            if !exists {
                return Ok(candidate);
            }
            suffix = suffix.checked_add(1).ok_or_else(|| {
                AppError::new("screenshot_title_exhausted", "无法生成截图标题", "稍后重试")
            })?;
        }
    })
}

struct ScreenshotFileGuard {
    path: PathBuf,
}

impl ScreenshotFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for ScreenshotFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn screenshot_cache_failed() -> AppError {
    AppError::new(
        "screenshot_cache_failed",
        "无法准备截图缓存目录",
        "检查磁盘空间后重试",
    )
}

fn screenshot_result_invalid() -> AppError {
    AppError::new(
        "screenshot_result_invalid",
        "截图程序返回了无效文件",
        "重新截图后重试",
    )
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, path::PathBuf, sync::Mutex};

    use async_trait::async_trait;
    use tempfile::tempdir;

    use super::{
        screenshot_metadata, unique_screenshot_title, ScreenshotCaptureResult,
        ScreenshotCaptureService,
    };
    use crate::{
        app_error::AppError,
        database::Database,
        domain::card::{CaptureMethod, NewCardInput, SourceType},
        native::client::{
            NativeCaptureKind, NativeClient, NativeCommand, NativeRequest, NativeResponse,
            NativeTransport,
        },
        repository::{
            attachment_repository::AttachmentRepository, card_repository::CardRepository,
        },
    };

    struct FakeNativeTransport {
        responses: Mutex<VecDeque<NativeResponse>>,
    }

    impl FakeNativeTransport {
        fn new(responses: Vec<NativeResponse>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
            }
        }
    }

    #[async_trait]
    impl NativeTransport for FakeNativeTransport {
        async fn request(&self, request: NativeRequest) -> Result<NativeResponse, AppError> {
            let mut response = self.responses.lock().unwrap().pop_front().unwrap();
            if request.command == NativeCommand::Screenshot && response.path.is_some() {
                let requested_path = request.path.unwrap();
                std::fs::write(PathBuf::from(&requested_path), b"captured image").unwrap();
                response.path = Some(requested_path);
            }
            Ok(response)
        }
    }

    fn response(
        ok: bool,
        text: Option<&str>,
        path: Option<&str>,
        error_code: Option<&str>,
        cancelled: Option<bool>,
    ) -> NativeResponse {
        NativeResponse {
            ok,
            text: text.map(str::to_owned),
            path: path.map(str::to_owned),
            permission: None,
            capture_kind: path.map(|_| "region".to_owned()),
            error_code: error_code.map(str::to_owned),
            message: None,
            cancelled,
        }
    }

    #[test]
    fn cancellation_does_not_create_a_card_or_file() {
        tauri::async_runtime::block_on(async {
            let directory = tempdir().unwrap();
            let database = Database::open_in_memory().unwrap();
            let attachments = AttachmentRepository::new(
                directory.path().join("attachments"),
                directory.path().join("attachment-cache"),
            );
            let service = ScreenshotCaptureService::new(
                directory.path().join("capture-cache"),
                &database,
                &attachments,
                NativeClient::new(FakeNativeTransport::new(vec![response(
                    true,
                    None,
                    None,
                    None,
                    Some(true),
                )])),
            );

            assert_eq!(
                service.capture().await.unwrap(),
                ScreenshotCaptureResult::Cancelled
            );
            assert!(directory
                .path()
                .join("capture-cache")
                .read_dir()
                .unwrap()
                .next()
                .is_none());
        });
    }

    #[test]
    fn permission_failure_contains_the_system_settings_recovery_url() {
        tauri::async_runtime::block_on(async {
            let directory = tempdir().unwrap();
            let database = Database::open_in_memory().unwrap();
            let attachments = AttachmentRepository::new(
                directory.path().join("attachments"),
                directory.path().join("attachment-cache"),
            );
            let service = ScreenshotCaptureService::new(
                directory.path().join("capture-cache"),
                &database,
                &attachments,
                NativeClient::new(FakeNativeTransport::new(vec![response(
                    false,
                    None,
                    None,
                    Some("screen_permission_denied"),
                    None,
                )])),
            );

            let error = service.capture().await.unwrap_err();

            assert_eq!(error.code, "screen_permission_denied");
            assert_eq!(
                error.recovery,
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
            );
        });
    }

    #[test]
    fn capture_saves_the_optional_note_on_the_card() {
        tauri::async_runtime::block_on(async {
            let directory = tempdir().unwrap();
            let database = Database::open_in_memory().unwrap();
            let attachments = AttachmentRepository::new(
                directory.path().join("attachments"),
                directory.path().join("attachment-cache"),
            );
            let service = ScreenshotCaptureService::new(
                directory.path().join("capture-cache"),
                &database,
                &attachments,
                NativeClient::new(FakeNativeTransport::new(vec![
                    response(
                        true,
                        Some("这是截图时记录的想法"),
                        Some("/placeholder/capture.png"),
                        None,
                        None,
                    ),
                    response(true, Some("截图里的文字"), None, None, None),
                ])),
            );

            let result = service.capture().await.unwrap();

            let ScreenshotCaptureResult::Saved { card } = result else {
                panic!("expected a saved screenshot card");
            };
            assert_eq!(card.note, "这是截图时记录的想法");
            assert_eq!(card.original_content, "截图里的文字");
        });
    }

    #[test]
    fn screenshot_titles_increment_existing_suffixes() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let base = "截图 2026-07-23 14:05:09";

        assert_eq!(unique_screenshot_title(&database, base).unwrap(), base);
        for title in [base.to_owned(), format!("{base} (2)")] {
            repository
                .create(NewCardInput {
                    title,
                    original_content: String::new(),
                    note: String::new(),
                    source_type: SourceType::Personal,
                    capture_method: CaptureMethod::Screenshot,
                    source_name: "区域截图".to_owned(),
                    source_uri: String::new(),
                })
                .unwrap();
        }

        assert_eq!(
            unique_screenshot_title(&database, base).unwrap(),
            format!("{base} (3)")
        );
    }

    #[test]
    fn scrolling_capture_uses_distinct_card_and_attachment_names() {
        assert_eq!(
            screenshot_metadata(NativeCaptureKind::Scrolling),
            ("滚动截图", "滚动截图", "滚动截图.png")
        );
        assert_eq!(
            screenshot_metadata(NativeCaptureKind::Region),
            ("截图", "区域截图", "截图.png")
        );
    }
}
