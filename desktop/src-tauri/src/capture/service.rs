use serde::{Deserialize, Serialize};

use crate::domain::card::{CaptureMethod, SourceType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureDraft {
    pub kind: CaptureMethod,
    pub title: String,
    pub content: String,
    pub note: String,
    pub source_type: SourceType,
    pub source_name: String,
    pub source_uri: String,
    pub temporary_attachment_ids: Vec<String>,
}

impl CaptureDraft {
    pub fn clipboard_text(
        content: impl Into<String>,
        source_type: SourceType,
        source_name: impl Into<String>,
        source_uri: impl Into<String>,
    ) -> Self {
        Self {
            kind: CaptureMethod::Clipboard,
            title: String::new(),
            content: content.into(),
            note: String::new(),
            source_type,
            source_name: source_name.into(),
            source_uri: source_uri.into(),
            temporary_attachment_ids: Vec::new(),
        }
    }

    pub fn clipboard_image(content: impl Into<String>, attachment_id: String) -> Self {
        Self {
            kind: CaptureMethod::Clipboard,
            title: String::new(),
            content: content.into(),
            note: String::new(),
            source_type: SourceType::Personal,
            source_name: "剪贴板图片".to_owned(),
            source_uri: String::new(),
            temporary_attachment_ids: vec![attachment_id],
        }
    }
}
