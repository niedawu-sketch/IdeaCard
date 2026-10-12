use std::path::Path;

use crate::{app_error::AppError, domain::card::AttachmentKind};

const DOCUMENTS: &[&str] = &["pdf", "docx", "pptx", "xlsx", "md", "markdown", "txt"];
const IMAGES: &[&str] = &["png", "jpg", "jpeg", "heic"];
const AUDIO: &[&str] = &["mp3", "m4a", "wav", "aac", "flac"];
const VIDEO: &[&str] = &["mp4", "mov", "m4v", "webm"];
const DOCUMENT_LIMIT: u64 = 50 * 1024 * 1024;
const MEDIA_LIMIT: u64 = 500 * 1024 * 1024;
pub const MAX_BATCH: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionStrategy {
    Text,
    Pdf,
    Docx,
    Pptx,
    Xlsx,
    Ocr,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileClassification {
    pub attachment_kind: AttachmentKind,
    pub mime_type: &'static str,
    pub extraction_strategy: ExtractionStrategy,
    pub max_bytes: u64,
}

pub fn classify_path(path: &Path) -> Result<FileClassification, AppError> {
    if path.file_name().is_none() {
        return Err(unsupported_file_type());
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(unsupported_file_type)?;

    let (attachment_kind, extraction_strategy, mime_type, max_bytes) = match extension.as_str() {
        "pdf" => document(ExtractionStrategy::Pdf, "application/pdf"),
        "docx" => document(
            ExtractionStrategy::Docx,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ),
        "pptx" => document(
            ExtractionStrategy::Pptx,
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        ),
        "xlsx" => document(
            ExtractionStrategy::Xlsx,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
        "md" | "markdown" => document(ExtractionStrategy::Text, "text/markdown"),
        "txt" => document(ExtractionStrategy::Text, "text/plain"),
        "png" => image("image/png"),
        "jpg" | "jpeg" => image("image/jpeg"),
        "heic" => image("image/heic"),
        "mp3" => audio("audio/mpeg"),
        "m4a" => audio("audio/mp4"),
        "wav" => audio("audio/wav"),
        "aac" => audio("audio/aac"),
        "flac" => audio("audio/flac"),
        "mp4" => video("video/mp4"),
        "mov" => video("video/quicktime"),
        "m4v" => video("video/x-m4v"),
        "webm" => video("video/webm"),
        _ => return Err(unsupported_file_type()),
    };

    debug_assert!(
        DOCUMENTS.contains(&extension.as_str())
            || IMAGES.contains(&extension.as_str())
            || AUDIO.contains(&extension.as_str())
            || VIDEO.contains(&extension.as_str())
    );

    Ok(FileClassification {
        attachment_kind,
        mime_type,
        extraction_strategy,
        max_bytes,
    })
}

fn document(
    extraction_strategy: ExtractionStrategy,
    mime_type: &'static str,
) -> (AttachmentKind, ExtractionStrategy, &'static str, u64) {
    (
        AttachmentKind::Document,
        extraction_strategy,
        mime_type,
        DOCUMENT_LIMIT,
    )
}

fn image(mime_type: &'static str) -> (AttachmentKind, ExtractionStrategy, &'static str, u64) {
    (
        AttachmentKind::Image,
        ExtractionStrategy::Ocr,
        mime_type,
        DOCUMENT_LIMIT,
    )
}

fn audio(mime_type: &'static str) -> (AttachmentKind, ExtractionStrategy, &'static str, u64) {
    (
        AttachmentKind::Audio,
        ExtractionStrategy::None,
        mime_type,
        MEDIA_LIMIT,
    )
}

fn video(mime_type: &'static str) -> (AttachmentKind, ExtractionStrategy, &'static str, u64) {
    (
        AttachmentKind::Video,
        ExtractionStrategy::None,
        mime_type,
        MEDIA_LIMIT,
    )
}

fn unsupported_file_type() -> AppError {
    AppError::new(
        "unsupported_file_type",
        "不支持这种文件类型",
        "请选择 PDF、Office、Markdown、文本、图片、音频或视频文件",
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::domain::card::AttachmentKind;

    use super::{classify_path, ExtractionStrategy, MAX_BATCH};

    #[test]
    fn maps_supported_extensions_to_attachment_kind_and_limits() {
        let pdf = classify_path(Path::new("report.pdf")).unwrap();
        assert_eq!(pdf.attachment_kind, AttachmentKind::Document);
        assert_eq!(pdf.mime_type, "application/pdf");
        assert_eq!(pdf.extraction_strategy, ExtractionStrategy::Pdf);
        assert_eq!(pdf.max_bytes, 50 * 1024 * 1024);

        let image = classify_path(Path::new("screen.HEIC")).unwrap();
        assert_eq!(image.attachment_kind, AttachmentKind::Image);
        assert_eq!(image.extraction_strategy, ExtractionStrategy::Ocr);

        let video = classify_path(Path::new("meeting.mp4")).unwrap();
        assert_eq!(video.attachment_kind, AttachmentKind::Video);
        assert_eq!(video.extraction_strategy, ExtractionStrategy::None);
        assert_eq!(video.max_bytes, 500 * 1024 * 1024);
        assert_eq!(MAX_BATCH, 20);
    }

    #[test]
    fn maps_text_office_and_audio_extraction_strategies() {
        assert_eq!(
            classify_path(Path::new("notes.md"))
                .unwrap()
                .extraction_strategy,
            ExtractionStrategy::Text
        );
        assert_eq!(
            classify_path(Path::new("proposal.docx"))
                .unwrap()
                .extraction_strategy,
            ExtractionStrategy::Docx
        );
        assert_eq!(
            classify_path(Path::new("slides.pptx"))
                .unwrap()
                .extraction_strategy,
            ExtractionStrategy::Pptx
        );
        assert_eq!(
            classify_path(Path::new("data.xlsx"))
                .unwrap()
                .extraction_strategy,
            ExtractionStrategy::Xlsx
        );
        assert_eq!(
            classify_path(Path::new("voice.m4a"))
                .unwrap()
                .attachment_kind,
            AttachmentKind::Audio
        );
    }

    #[test]
    fn rejects_unknown_or_missing_extensions() {
        assert_eq!(
            classify_path(Path::new("archive.bin")).unwrap_err().code,
            "unsupported_file_type"
        );
        assert_eq!(
            classify_path(Path::new("README")).unwrap_err().code,
            "unsupported_file_type"
        );
        assert_eq!(
            classify_path(Path::new("/")).unwrap_err().code,
            "unsupported_file_type"
        );
    }
}
