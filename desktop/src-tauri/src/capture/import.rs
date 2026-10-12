use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    app_error::AppError,
    capture::{
        extractors::{
            office::extract_office, pdf::extract_pdf, text::extract_txt, ExtractionResult,
        },
        file_kind::{classify_path, ExtractionStrategy, FileClassification, MAX_BATCH},
    },
    database::Database,
    domain::card::{
        AttachmentKind, CaptureMethod, Card, ExtractionStatus, NewCardInput, SourceType,
    },
    native::client::{NativeClient, NativeTransport},
    repository::{
        attachment_repository::{AttachmentClass, AttachmentRepository, StoredAttachment},
        card_repository::{CardRepository, NewAttachmentInput},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileImportStatus {
    Saved,
    SavedWithExtractionFailure,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileImportResult {
    pub original_name: String,
    pub status: FileImportStatus,
    pub card: Option<Card>,
    pub error_code: Option<String>,
}

struct Preflight {
    path: PathBuf,
    original_name: String,
    classification: FileClassification,
}

pub struct FileImportService<'a, T> {
    database: &'a Database,
    attachments: &'a AttachmentRepository,
    native: NativeClient<T>,
}

impl<'a, T: NativeTransport> FileImportService<'a, T> {
    pub fn new(
        database: &'a Database,
        attachments: &'a AttachmentRepository,
        native: NativeClient<T>,
    ) -> Self {
        Self {
            database,
            attachments,
            native,
        }
    }

    pub async fn import_paths(&self, paths: &[PathBuf]) -> Result<Vec<FileImportResult>, AppError> {
        if paths.len() > MAX_BATCH {
            return Err(AppError::new(
                "file_batch_too_large",
                "一次最多导入 20 个文件",
                "减少选择的文件后重试",
            ));
        }

        let preflight = paths
            .iter()
            .map(|path| self.preflight(path))
            .collect::<Vec<_>>();
        let mut results = Vec::with_capacity(paths.len());
        for item in preflight {
            match item {
                Ok(item) => results.push(self.import_one(item).await),
                Err((original_name, error)) => results.push(FileImportResult {
                    original_name,
                    status: FileImportStatus::Failed,
                    card: None,
                    error_code: Some(error.code.to_owned()),
                }),
            }
        }
        Ok(results)
    }

    fn preflight(&self, path: &Path) -> Result<Preflight, (String, AppError)> {
        let original_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_owned();
        let classification = classify_path(path).map_err(|error| (original_name.clone(), error))?;
        let metadata = path.metadata().map_err(|_| {
            (
                original_name.clone(),
                AppError::new(
                    "attachment_source_missing",
                    "找不到要导入的文件",
                    "重新选择文件后再试",
                ),
            )
        })?;
        if !metadata.is_file() {
            return Err((
                original_name,
                AppError::new(
                    "attachment_source_missing",
                    "找不到要导入的文件",
                    "重新选择文件后再试",
                ),
            ));
        }
        if metadata.len() > classification.max_bytes {
            return Err((
                original_name,
                AppError::new(
                    "attachment_too_large",
                    "文件超过大小限制",
                    "选择更小的文件后再试",
                ),
            ));
        }
        Ok(Preflight {
            path: path.to_owned(),
            original_name,
            classification,
        })
    }

    async fn import_one(&self, item: Preflight) -> FileImportResult {
        let class = if matches!(
            item.classification.attachment_kind,
            AttachmentKind::Audio | AttachmentKind::Video
        ) {
            AttachmentClass::Media
        } else {
            AttachmentClass::DocumentOrImage
        };
        let stored = match self.attachments.store(&item.path, class) {
            Ok(stored) => stored,
            Err(error) => return failed_result(item.original_name, error),
        };
        let stored_path = match self.attachments.resolve(&stored.relative_path) {
            Ok(path) => path,
            Err(error) => {
                let _ = self.attachments.remove_stored(&stored.relative_path);
                return failed_result(item.original_name, error);
            }
        };
        let extraction = self
            .extract(&stored_path, item.classification.extraction_strategy)
            .await;
        let (extraction, extraction_failed) = match extraction {
            Ok(result) => (result, false),
            Err(error) => (
                ExtractionResult {
                    status: ExtractionStatus::Failed,
                    text: String::new(),
                    error: Some(error.code.to_owned()),
                },
                true,
            ),
        };

        let result = self.save_card(&item, &stored, &extraction);
        match result {
            Ok(card) => FileImportResult {
                original_name: item.original_name,
                status: if extraction_failed {
                    FileImportStatus::SavedWithExtractionFailure
                } else {
                    FileImportStatus::Saved
                },
                card: Some(card),
                error_code: extraction.error,
            },
            Err(error) => {
                let _ = self.attachments.remove_stored(&stored.relative_path);
                failed_result(item.original_name, error)
            }
        }
    }

    async fn extract(
        &self,
        path: &Path,
        strategy: ExtractionStrategy,
    ) -> Result<ExtractionResult, AppError> {
        match strategy {
            ExtractionStrategy::Text => extract_txt(path),
            ExtractionStrategy::Pdf => extract_pdf(path),
            ExtractionStrategy::Docx | ExtractionStrategy::Pptx | ExtractionStrategy::Xlsx => {
                extract_office(path)
            }
            ExtractionStrategy::Ocr => {
                let text = self.native.ocr(path.to_owned()).await?;
                let status = if text.trim().is_empty() {
                    ExtractionStatus::Empty
                } else {
                    ExtractionStatus::Completed
                };
                Ok(ExtractionResult {
                    status,
                    text,
                    error: None,
                })
            }
            ExtractionStrategy::None => Ok(ExtractionResult {
                status: ExtractionStatus::NotApplicable,
                text: String::new(),
                error: None,
            }),
        }
    }

    fn save_card(
        &self,
        item: &Preflight,
        stored: &StoredAttachment,
        extraction: &ExtractionResult,
    ) -> Result<Card, AppError> {
        let title = item
            .path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(&item.original_name)
            .to_owned();
        let input = NewCardInput {
            title,
            original_content: extraction.text.clone(),
            note: String::new(),
            source_type: SourceType::Document,
            capture_method: CaptureMethod::File,
            source_name: item.original_name.clone(),
            source_uri: String::new(),
        };
        let attachment = NewAttachmentInput {
            kind: item.classification.attachment_kind,
            original_name: item.original_name.clone(),
            relative_path: stored.relative_path.clone(),
            mime_type: stored.mime_type.clone(),
            byte_size: stored.byte_size,
            checksum: stored.checksum.clone(),
            extraction_status: extraction.status,
            extraction_error: extraction.error.clone().unwrap_or_default(),
            extracted_text: extraction.text.clone(),
        };
        CardRepository::new(self.database)
            .create_with_attachment(input, attachment)
            .map(|result| result.card)
    }
}

fn failed_result(original_name: String, error: AppError) -> FileImportResult {
    FileImportResult {
        original_name,
        status: FileImportStatus::Failed,
        card: None,
        error_code: Some(error.code.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, sync::Mutex};

    use async_trait::async_trait;
    use tempfile::tempdir;

    use super::{FileImportService, FileImportStatus};
    use crate::{
        app_error::AppError,
        database::Database,
        domain::card::{AttachmentKind, ExtractionStatus, SourceType},
        native::client::{NativeClient, NativeRequest, NativeResponse, NativeTransport},
        repository::attachment_repository::AttachmentRepository,
    };

    struct FakeNativeTransport {
        response: Mutex<Option<NativeResponse>>,
    }

    impl FakeNativeTransport {
        fn new() -> Self {
            Self {
                response: Mutex::new(Some(NativeResponse {
                    ok: true,
                    text: Some("图片文字".to_owned()),
                    path: None,
                    permission: None,
                    capture_kind: None,
                    error_code: None,
                    message: None,
                    cancelled: None,
                })),
            }
        }
    }

    #[async_trait]
    impl NativeTransport for FakeNativeTransport {
        async fn request(&self, _request: NativeRequest) -> Result<NativeResponse, AppError> {
            Ok(self.response.lock().unwrap().take().unwrap())
        }
    }

    #[test]
    fn imports_valid_files_and_keeps_corrupt_office_attachments() {
        tauri::async_runtime::block_on(async {
            let directory = tempdir().unwrap();
            let text = directory.path().join("notes.txt");
            let corrupt = directory.path().join("broken.docx");
            let video = directory.path().join("meeting.mp4");
            fs::write(&text, "第一行\n第二行").unwrap();
            fs::write(&corrupt, b"not a zip").unwrap();
            fs::write(&video, b"video attachment").unwrap();
            let database = Database::open_in_memory().unwrap();
            let attachments = AttachmentRepository::new(
                directory.path().join("attachments"),
                directory.path().join("cache"),
            );
            let service = FileImportService::new(
                &database,
                &attachments,
                NativeClient::new(FakeNativeTransport::new()),
            );

            let results = service
                .import_paths(&[text.clone(), corrupt.clone(), video.clone()])
                .await
                .unwrap();

            assert_eq!(results.len(), 3);
            assert_eq!(results[0].status, FileImportStatus::Saved);
            let text_card = results[0].card.as_ref().unwrap();
            assert_eq!(text_card.source_type, SourceType::Document);
            assert_eq!(text_card.original_content, "第一行\n第二行");
            assert_eq!(text_card.attachments.len(), 1);
            assert_eq!(
                text_card.attachments[0].extraction_status,
                ExtractionStatus::Completed
            );

            assert_eq!(
                results[1].status,
                FileImportStatus::SavedWithExtractionFailure
            );
            let corrupt_card = results[1].card.as_ref().unwrap();
            assert_eq!(corrupt_card.attachments.len(), 1);
            assert_eq!(
                corrupt_card.attachments[0].extraction_status,
                ExtractionStatus::Failed
            );
            assert_eq!(
                corrupt_card.attachments[0].extraction_error,
                "file_extraction_failed"
            );
            assert!(attachments
                .resolve(&corrupt_card.attachments[0].relative_path)
                .unwrap()
                .is_file());

            let video_card = results[2].card.as_ref().unwrap();
            assert_eq!(video_card.source_type, SourceType::Document);
            assert_eq!(video_card.attachments[0].kind, AttachmentKind::Video);
        });
    }

    #[test]
    fn returns_per_file_failures_without_copying_unsupported_files() {
        tauri::async_runtime::block_on(async {
            let directory = tempdir().unwrap();
            let unsupported = directory.path().join("archive.bin");
            fs::write(&unsupported, b"data").unwrap();
            let database = Database::open_in_memory().unwrap();
            let attachment_root = directory.path().join("attachments");
            let attachments =
                AttachmentRepository::new(&attachment_root, directory.path().join("cache"));
            let service = FileImportService::new(
                &database,
                &attachments,
                NativeClient::new(FakeNativeTransport::new()),
            );

            let results = service.import_paths(&[unsupported]).await.unwrap();

            assert_eq!(results[0].status, FileImportStatus::Failed);
            assert_eq!(
                results[0].error_code.as_deref(),
                Some("unsupported_file_type")
            );
            assert!(!attachment_root.exists());
        });
    }

    #[test]
    fn rejects_more_than_twenty_files_before_reading_paths() {
        tauri::async_runtime::block_on(async {
            let directory = tempdir().unwrap();
            let database = Database::open_in_memory().unwrap();
            let attachment_root = directory.path().join("attachments");
            let attachments =
                AttachmentRepository::new(&attachment_root, directory.path().join("cache"));
            let service = FileImportService::new(
                &database,
                &attachments,
                NativeClient::new(FakeNativeTransport::new()),
            );
            let paths = (0..21)
                .map(|index| PathBuf::from(format!("missing-{index}.txt")))
                .collect::<Vec<_>>();

            let error = service.import_paths(&paths).await.unwrap_err();

            assert_eq!(error.code, "file_batch_too_large");
            assert!(!attachment_root.exists());
        });
    }
}
