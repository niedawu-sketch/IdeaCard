use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Personal,
    Web,
    Document,
    Image,
    Media,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMethod {
    Quick,
    Clipboard,
    Screenshot,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardStatus {
    Unprocessed,
    Annotated,
    PendingAnalysis,
    Concluded,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionStatus {
    Pending,
    Completed,
    Empty,
    Failed,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Document,
    Image,
    Audio,
    Video,
}

macro_rules! impl_sql_enum {
    ($enum_type:ty, { $($variant:path => $value:literal),+ $(,)? }) => {
        impl $enum_type {
            pub const fn as_sql(self) -> &'static str {
                match self {
                    $($variant => $value),+
                }
            }

            pub fn from_sql(value: &str) -> Option<Self> {
                match value {
                    $($value => Some($variant),)+
                    _ => None,
                }
            }
        }
    };
}

impl_sql_enum!(SourceType, {
    SourceType::Personal => "personal",
    SourceType::Web => "web",
    SourceType::Document => "document",
    SourceType::Image => "image",
    SourceType::Media => "media",
});

impl_sql_enum!(CaptureMethod, {
    CaptureMethod::Quick => "quick",
    CaptureMethod::Clipboard => "clipboard",
    CaptureMethod::Screenshot => "screenshot",
    CaptureMethod::File => "file",
});

impl_sql_enum!(CardStatus, {
    CardStatus::Unprocessed => "unprocessed",
    CardStatus::Annotated => "annotated",
    CardStatus::PendingAnalysis => "pending_analysis",
    CardStatus::Concluded => "concluded",
    CardStatus::Archived => "archived",
});

impl_sql_enum!(ExtractionStatus, {
    ExtractionStatus::Pending => "pending",
    ExtractionStatus::Completed => "completed",
    ExtractionStatus::Empty => "empty",
    ExtractionStatus::Failed => "failed",
    ExtractionStatus::NotApplicable => "not_applicable",
});

impl_sql_enum!(AttachmentKind, {
    AttachmentKind::Document => "document",
    AttachmentKind::Image => "image",
    AttachmentKind::Audio => "audio",
    AttachmentKind::Video => "video",
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub card_id: String,
    pub kind: AttachmentKind,
    pub original_name: String,
    pub relative_path: String,
    pub mime_type: String,
    pub byte_size: u64,
    pub extraction_status: ExtractionStatus,
    pub extraction_error: String,
    pub extracted_text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    pub title: String,
    pub original_content: String,
    pub note: String,
    pub source_type: SourceType,
    pub capture_method: CaptureMethod,
    pub source_name: String,
    pub source_uri: String,
    pub status: CardStatus,
    pub attachments: Vec<Attachment>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCardInput {
    pub title: String,
    pub original_content: String,
    pub note: String,
    pub source_type: SourceType,
    pub capture_method: CaptureMethod,
    pub source_name: String,
    pub source_uri: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<SourceType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<CardStatus>,
}

pub fn fallback_title(content: &str) -> String {
    let first_line = content
        .split_once('\n')
        .map_or(content, |(first_line, _)| first_line);
    let normalized = first_line
        .split(|character: char| character.is_whitespace() || character == '\u{feff}')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    normalized.graphemes(true).take(30).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde_json::json;

    fn serialized_values<T: Serialize>(values: &[T]) -> Vec<String> {
        values
            .iter()
            .map(|value| {
                serde_json::to_value(value)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn enums_serialize_to_stable_values() {
        assert_eq!(
            serialized_values(&[
                SourceType::Personal,
                SourceType::Web,
                SourceType::Document,
                SourceType::Image,
                SourceType::Media,
            ]),
            ["personal", "web", "document", "image", "media"]
        );
        assert_eq!(
            serialized_values(&[
                CaptureMethod::Quick,
                CaptureMethod::Clipboard,
                CaptureMethod::Screenshot,
                CaptureMethod::File,
            ]),
            ["quick", "clipboard", "screenshot", "file"]
        );
        assert_eq!(
            serialized_values(&[
                CardStatus::Unprocessed,
                CardStatus::Annotated,
                CardStatus::PendingAnalysis,
                CardStatus::Concluded,
                CardStatus::Archived,
            ]),
            [
                "unprocessed",
                "annotated",
                "pending_analysis",
                "concluded",
                "archived",
            ]
        );
        assert_eq!(
            serialized_values(&[
                ExtractionStatus::Pending,
                ExtractionStatus::Completed,
                ExtractionStatus::Empty,
                ExtractionStatus::Failed,
                ExtractionStatus::NotApplicable,
            ]),
            ["pending", "completed", "empty", "failed", "not_applicable"]
        );
        assert_eq!(
            serialized_values(&[
                AttachmentKind::Document,
                AttachmentKind::Image,
                AttachmentKind::Audio,
                AttachmentKind::Video,
            ]),
            ["document", "image", "audio", "video"]
        );
    }

    #[test]
    fn source_type_serializes_to_stable_values() {
        assert_eq!(serde_json::to_string(&SourceType::Web).unwrap(), "\"web\"");
    }

    #[test]
    fn enums_convert_to_and_from_stable_sql_values() {
        for (value, sql) in [
            (SourceType::Personal, "personal"),
            (SourceType::Web, "web"),
            (SourceType::Document, "document"),
            (SourceType::Image, "image"),
            (SourceType::Media, "media"),
        ] {
            assert_eq!(value.as_sql(), sql);
            assert_eq!(SourceType::from_sql(sql), Some(value));
        }
        for (value, sql) in [
            (CaptureMethod::Quick, "quick"),
            (CaptureMethod::Clipboard, "clipboard"),
            (CaptureMethod::Screenshot, "screenshot"),
            (CaptureMethod::File, "file"),
        ] {
            assert_eq!(value.as_sql(), sql);
            assert_eq!(CaptureMethod::from_sql(sql), Some(value));
        }
        for (value, sql) in [
            (CardStatus::Unprocessed, "unprocessed"),
            (CardStatus::Annotated, "annotated"),
            (CardStatus::PendingAnalysis, "pending_analysis"),
            (CardStatus::Concluded, "concluded"),
            (CardStatus::Archived, "archived"),
        ] {
            assert_eq!(value.as_sql(), sql);
            assert_eq!(CardStatus::from_sql(sql), Some(value));
        }
        for (value, sql) in [
            (AttachmentKind::Document, "document"),
            (AttachmentKind::Image, "image"),
            (AttachmentKind::Audio, "audio"),
            (AttachmentKind::Video, "video"),
        ] {
            assert_eq!(value.as_sql(), sql);
            assert_eq!(AttachmentKind::from_sql(sql), Some(value));
        }
        for (value, sql) in [
            (ExtractionStatus::Pending, "pending"),
            (ExtractionStatus::Completed, "completed"),
            (ExtractionStatus::Empty, "empty"),
            (ExtractionStatus::Failed, "failed"),
            (ExtractionStatus::NotApplicable, "not_applicable"),
        ] {
            assert_eq!(value.as_sql(), sql);
            assert_eq!(ExtractionStatus::from_sql(sql), Some(value));
        }
    }

    #[test]
    fn invalid_sql_enum_values_are_rejected_without_fallbacks() {
        assert_eq!(SourceType::from_sql("Personal"), None);
        assert_eq!(CaptureMethod::from_sql("QUICK"), None);
        assert_eq!(CardStatus::from_sql("pending-analysis"), None);
        assert_eq!(AttachmentKind::from_sql("pdf"), None);
        assert_eq!(ExtractionStatus::from_sql("unknown"), None);
    }

    #[test]
    fn new_card_input_serializes_with_typescript_field_names() {
        let input = NewCardInput {
            title: "标题".to_owned(),
            original_content: "正文".to_owned(),
            note: "想法".to_owned(),
            source_type: SourceType::Personal,
            capture_method: CaptureMethod::Quick,
            source_name: String::new(),
            source_uri: String::new(),
        };

        let value = serde_json::to_value(input).unwrap();

        assert_eq!(value["originalContent"], "正文");
        assert_eq!(value["sourceType"], "personal");
        assert_eq!(value["captureMethod"], "quick");
        assert_eq!(value["sourceName"], "");
        assert_eq!(value["sourceUri"], "");
        assert!(value.get("original_content").is_none());
    }

    #[test]
    fn card_patch_rejects_unknown_fields() {
        let result = serde_json::from_str::<CardPatch>(r#"{"soruceType":"web"}"#);

        assert!(result.is_err());
    }

    #[test]
    fn card_and_attachment_round_trip_exact_json_with_null_archived_at() {
        let card = Card {
            id: "card-1".to_owned(),
            title: "标题".to_owned(),
            original_content: "正文".to_owned(),
            note: "想法".to_owned(),
            source_type: SourceType::Document,
            capture_method: CaptureMethod::File,
            source_name: "资料.pdf".to_owned(),
            source_uri: String::new(),
            status: CardStatus::Annotated,
            attachments: vec![Attachment {
                id: "attachment-1".to_owned(),
                card_id: "card-1".to_owned(),
                kind: AttachmentKind::Document,
                original_name: "资料.pdf".to_owned(),
                relative_path: "attachments/attachment-1.pdf".to_owned(),
                mime_type: "application/pdf".to_owned(),
                byte_size: 1024,
                extraction_status: ExtractionStatus::Completed,
                extraction_error: String::new(),
                extracted_text: "正文".to_owned(),
                created_at: "2026-07-14T06:00:00Z".to_owned(),
            }],
            created_at: "2026-07-14T06:00:00Z".to_owned(),
            updated_at: "2026-07-14T06:30:00Z".to_owned(),
            archived_at: None,
        };
        let expected = json!({
            "id": "card-1",
            "title": "标题",
            "originalContent": "正文",
            "note": "想法",
            "sourceType": "document",
            "captureMethod": "file",
            "sourceName": "资料.pdf",
            "sourceUri": "",
            "status": "annotated",
            "attachments": [{
                "id": "attachment-1",
                "cardId": "card-1",
                "kind": "document",
                "originalName": "资料.pdf",
                "relativePath": "attachments/attachment-1.pdf",
                "mimeType": "application/pdf",
                "byteSize": 1024,
                "extractionStatus": "completed",
                "extractionError": "",
                "extractedText": "正文",
                "createdAt": "2026-07-14T06:00:00Z"
            }],
            "createdAt": "2026-07-14T06:00:00Z",
            "updatedAt": "2026-07-14T06:30:00Z",
            "archivedAt": null
        });

        assert_eq!(serde_json::to_value(&card).unwrap(), expected);
        assert_eq!(serde_json::from_value::<Card>(expected).unwrap(), card);
    }

    #[test]
    fn card_patch_round_trips_exact_json() {
        let patch = CardPatch {
            title: Some("新标题".to_owned()),
            note: Some("新想法".to_owned()),
            source_type: Some(SourceType::Web),
            source_uri: Some("https://example.com".to_owned()),
            status: Some(CardStatus::PendingAnalysis),
            ..CardPatch::default()
        };
        let expected = json!({
            "title": "新标题",
            "note": "新想法",
            "sourceType": "web",
            "sourceUri": "https://example.com",
            "status": "pending_analysis"
        });

        assert_eq!(serde_json::to_value(&patch).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<CardPatch>(expected).unwrap(),
            patch
        );
    }

    #[test]
    fn fallback_title_uses_first_normalized_line() {
        assert_eq!(
            fallback_title("  第一行   有空格\n第二行  "),
            "第一行 有空格"
        );
        assert_eq!(fallback_title(&"一".repeat(35)), "一".repeat(30));
    }

    #[test]
    fn fallback_title_counts_emoji_as_one_character_without_splitting_it() {
        assert_eq!(fallback_title(&"😀".repeat(31)), "😀".repeat(30));
    }

    #[test]
    fn fallback_title_truncates_user_perceived_characters() {
        for grapheme in ["👨‍👩‍👧‍👦", "🇨🇳", "👍🏽", "e\u{0301}"] {
            assert_eq!(fallback_title(&grapheme.repeat(31)), grapheme.repeat(30));
        }
    }

    #[test]
    fn fallback_title_normalizes_bom_and_nel_whitespace() {
        assert_eq!(
            fallback_title("\u{feff}第一\u{feff}\u{feff}第二"),
            "第一 第二"
        );
        assert_eq!(
            fallback_title("\u{0085}第一\u{0085}\u{0085}第二"),
            "第一 第二"
        );
    }
}
