use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::app_error::AppError;

const MAX_DOCUMENT_BYTES: usize = 32 * 1024 * 1024;
const MAX_DOCUMENT_DEPTH: usize = 64;
const MAX_DOCUMENT_NODES: usize = 50_000;
const MAX_PENDING_IMAGES: usize = 20;
const MAX_INLINE_IMAGE_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingImage {
    pub temp_id: String,
    pub mime_type: String,
    pub extension: String,
    pub bytes: Vec<u8>,
    pub alt: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentMention {
    pub concept_id: String,
    pub mention_id: String,
    pub selected_text: String,
}

#[derive(Clone, PartialEq)]
pub struct ValidatedDocument {
    pub document: Value,
    pub plain_text: String,
    pub persisted_image_ids: Vec<String>,
    pub pending_images: Vec<PendingImage>,
    pub concept_mentions: Vec<DocumentMention>,
}

impl Debug for ValidatedDocument {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedDocument")
            .field("plain_text_length", &self.plain_text.chars().count())
            .field("persisted_image_ids", &self.persisted_image_ids)
            .field("pending_image_count", &self.pending_images.len())
            .field("concept_mentions", &self.concept_mentions)
            .finish()
    }
}

#[derive(Default)]
struct ValidationState {
    node_count: usize,
    persisted_image_ids: Vec<String>,
    pending_images: Vec<PendingImage>,
    concept_mentions: Vec<DocumentMention>,
    mention_indexes: HashMap<String, usize>,
}

pub fn validate_document(
    document: &Value,
    allow_pending_images: bool,
) -> Result<ValidatedDocument, AppError> {
    let encoded = serde_json::to_vec(document).map_err(|_| document_invalid())?;
    if encoded.len() > MAX_DOCUMENT_BYTES {
        return Err(document_too_large());
    }

    let root = document.as_object().ok_or_else(document_invalid)?;
    require_keys(root, &["type", "content"])?;
    if string_field(root, "type")? != "doc" {
        return Err(document_invalid());
    }
    let content = array_field(root, "content")?;
    let mut state = ValidationState::default();
    let mut plain_blocks = Vec::new();
    for child in content {
        let child = child.as_object().ok_or_else(document_invalid)?;
        match string_field(child, "type")? {
            "paragraph" => {
                plain_blocks.push(validate_paragraph(child, 1, &mut state)?);
            }
            "image" => {
                validate_image(child, 1, allow_pending_images, &mut state)?;
            }
            _ => return Err(document_invalid()),
        }
    }
    let plain_text = plain_blocks
        .into_iter()
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(ValidatedDocument {
        document: document.clone(),
        plain_text,
        persisted_image_ids: state.persisted_image_ids,
        pending_images: state.pending_images,
        concept_mentions: state.concept_mentions,
    })
}

fn validate_paragraph(
    node: &Map<String, Value>,
    depth: usize,
    state: &mut ValidationState,
) -> Result<String, AppError> {
    visit_node(depth, state)?;
    require_optional_keys(node, &["type"], &["content"])?;
    let mut plain_text = String::new();
    for child in optional_array_field(node, "content")? {
        let child = child.as_object().ok_or_else(document_invalid)?;
        match string_field(child, "type")? {
            "text" => plain_text.push_str(&validate_text(child, depth + 1, state)?),
            "hardBreak" => {
                visit_node(depth + 1, state)?;
                require_keys(child, &["type"])?;
                plain_text.push('\n');
            }
            _ => return Err(document_invalid()),
        }
    }
    Ok(plain_text)
}

fn validate_text(
    node: &Map<String, Value>,
    depth: usize,
    state: &mut ValidationState,
) -> Result<String, AppError> {
    visit_node(depth, state)?;
    require_optional_keys(node, &["type", "text"], &["marks"])?;
    let text = string_field(node, "text")?;
    let marks = optional_array_field(node, "marks")?;
    if marks.len() > 1 {
        return Err(document_invalid());
    }
    if let Some(mark) = marks.first() {
        let mark = mark.as_object().ok_or_else(document_invalid)?;
        require_keys(mark, &["type", "attrs"])?;
        if string_field(mark, "type")? != "concept" {
            return Err(document_invalid());
        }
        let attrs = object_field(mark, "attrs")?;
        require_keys(attrs, &["conceptId", "mentionId"])?;
        let concept_id = valid_uuid_field(attrs, "conceptId")?;
        let mention_id = valid_uuid_field(attrs, "mentionId")?;
        if text.trim().is_empty() {
            return Err(document_invalid());
        }
        if let Some(index) = state.mention_indexes.get(&mention_id).copied() {
            let existing = &mut state.concept_mentions[index];
            if existing.concept_id != concept_id {
                return Err(document_invalid());
            }
            existing.selected_text.push_str(text);
        } else {
            state
                .mention_indexes
                .insert(mention_id.clone(), state.concept_mentions.len());
            state.concept_mentions.push(DocumentMention {
                concept_id,
                mention_id,
                selected_text: text.to_owned(),
            });
        }
    }
    Ok(text.to_owned())
}

fn validate_image(
    node: &Map<String, Value>,
    depth: usize,
    allow_pending_images: bool,
    state: &mut ValidationState,
) -> Result<(), AppError> {
    visit_node(depth, state)?;
    require_keys(node, &["type", "attrs"])?;
    let attrs = object_field(node, "attrs")?;
    require_optional_keys(
        attrs,
        &[],
        &["attachmentId", "tempId", "data", "alt", "title"],
    )?;
    let alt = optional_string_field(attrs, "alt")?.unwrap_or_default();
    let title = optional_string_field(attrs, "title")?.unwrap_or_default();
    let attachment_id = optional_string_field(attrs, "attachmentId")?;
    let temp_id = optional_string_field(attrs, "tempId")?;
    let data = optional_string_field(attrs, "data")?;

    match (attachment_id, temp_id, data) {
        (Some(attachment_id), None, None) => {
            Uuid::parse_str(&attachment_id).map_err(|_| document_invalid())?;
            if state.persisted_image_ids.contains(&attachment_id) {
                return Err(document_invalid());
            }
            state.persisted_image_ids.push(attachment_id);
            Ok(())
        }
        (None, Some(temp_id), Some(data)) if allow_pending_images => {
            Uuid::parse_str(&temp_id).map_err(|_| document_invalid())?;
            if state.pending_images.len() >= MAX_PENDING_IMAGES
                || state
                    .pending_images
                    .iter()
                    .any(|pending| pending.temp_id == temp_id)
            {
                return Err(document_invalid());
            }
            let (mime_type, extension, bytes) = decode_image_data_url(&data)?;
            state.pending_images.push(PendingImage {
                temp_id,
                mime_type,
                extension,
                bytes,
                alt,
                title,
            });
            Ok(())
        }
        _ => Err(document_invalid()),
    }
}

fn decode_image_data_url(data: &str) -> Result<(String, String, Vec<u8>), AppError> {
    let (prefix, encoded) = data.split_once(',').ok_or_else(inline_image_invalid)?;
    let (mime_type, extension) = match prefix {
        "data:image/png;base64" => ("image/png", "png"),
        "data:image/jpeg;base64" => ("image/jpeg", "jpg"),
        "data:image/heic;base64" => ("image/heic", "heic"),
        "data:image/webp;base64" => ("image/webp", "webp"),
        _ => return Err(inline_image_invalid()),
    };
    let estimated_size = encoded.len().saturating_mul(3) / 4;
    if estimated_size > MAX_INLINE_IMAGE_BYTES {
        return Err(inline_image_too_large());
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| inline_image_invalid())?;
    if bytes.is_empty() {
        return Err(inline_image_invalid());
    }
    if bytes.len() > MAX_INLINE_IMAGE_BYTES {
        return Err(inline_image_too_large());
    }
    if !matches_image_signature(mime_type, &bytes) {
        return Err(inline_image_invalid());
    }
    Ok((mime_type.to_owned(), extension.to_owned(), bytes))
}

fn matches_image_signature(mime_type: &str, bytes: &[u8]) -> bool {
    match mime_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        "image/heic" => {
            if bytes.len() < 12 || &bytes[4..8] != b"ftyp" {
                return false;
            }
            matches!(
                &bytes[8..12],
                b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"mif1" | b"msf1"
            )
        }
        _ => false,
    }
}

fn visit_node(depth: usize, state: &mut ValidationState) -> Result<(), AppError> {
    if depth > MAX_DOCUMENT_DEPTH || state.node_count >= MAX_DOCUMENT_NODES {
        return Err(document_invalid());
    }
    state.node_count += 1;
    Ok(())
}

fn require_keys(object: &Map<String, Value>, required: &[&str]) -> Result<(), AppError> {
    require_optional_keys(object, required, &[])
}

fn require_optional_keys(
    object: &Map<String, Value>,
    required: &[&str],
    optional: &[&str],
) -> Result<(), AppError> {
    if required.iter().any(|key| !object.contains_key(*key))
        || object
            .keys()
            .any(|key| !required.contains(&key.as_str()) && !optional.contains(&key.as_str()))
    {
        return Err(document_invalid());
    }
    Ok(())
}

fn string_field<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, AppError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(document_invalid)
}

fn optional_string_field(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, AppError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(document_invalid()),
    }
}

fn valid_uuid_field(object: &Map<String, Value>, key: &str) -> Result<String, AppError> {
    let value = string_field(object, key)?;
    Uuid::parse_str(value).map_err(|_| document_invalid())?;
    Ok(value.to_owned())
}

fn object_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Map<String, Value>, AppError> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(document_invalid)
}

fn array_field<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Vec<Value>, AppError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(document_invalid)
}

fn optional_array_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a [Value], AppError> {
    match object.get(key) {
        None => Ok(&[]),
        Some(Value::Array(values)) => Ok(values),
        Some(_) => Err(document_invalid()),
    }
}

fn document_invalid() -> AppError {
    AppError::new("document_invalid", "正文格式无效", "重新打开卡片后再试")
}

fn document_too_large() -> AppError {
    AppError::new(
        "document_too_large",
        "正文内容过大",
        "减少正文中的图片后再保存",
    )
}

fn inline_image_invalid() -> AppError {
    AppError::new(
        "inline_image_invalid",
        "粘贴的图片格式无效",
        "请粘贴 PNG、JPEG、HEIC 或 WebP 图片",
    )
}

fn inline_image_too_large() -> AppError {
    AppError::new(
        "inline_image_too_large",
        "粘贴的图片超过 20 MB",
        "压缩图片后重新粘贴",
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::validate_document;

    #[test]
    fn validates_the_allowed_tree_and_builds_a_plain_text_projection() {
        let document = json!({
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        { "type": "text", "text": "第一行" },
                        { "type": "hardBreak" },
                        { "type": "text", "text": "继续" }
                    ]
                },
                {
                    "type": "image",
                    "attrs": {
                        "attachmentId": "550e8400-e29b-41d4-a716-446655440000",
                        "alt": "",
                        "title": ""
                    }
                },
                {
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "第二段" }]
                }
            ]
        });

        let validated = validate_document(&document, false).unwrap();

        assert_eq!(validated.plain_text, "第一行\n继续\n第二段");
        assert_eq!(
            validated.persisted_image_ids,
            vec!["550e8400-e29b-41d4-a716-446655440000"]
        );
        assert!(validated.pending_images.is_empty());
        assert!(validated.concept_mentions.is_empty());
    }

    #[test]
    fn collects_pending_images_and_concept_mentions_without_leaking_image_data() {
        let document = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "text",
                    "text": "结构化正文",
                    "marks": [{
                        "type": "concept",
                        "attrs": {
                            "conceptId": "8ee5e2b0-6f71-4c31-9d47-34c6974f8c01",
                            "mentionId": "491261f4-7a25-4987-a9ac-7f12e56e9fc3"
                        }
                    }]
                }]
            }, {
                "type": "image",
                "attrs": {
                    "tempId": "7b73c44e-cff8-4fb4-97e7-d6a03c3dfd10",
                    "data": "data:image/png;base64,iVBORw0KGgo=",
                    "alt": "示例",
                    "title": ""
                }
            }]
        });

        let validated = validate_document(&document, true).unwrap();

        assert_eq!(validated.pending_images.len(), 1);
        assert_eq!(validated.pending_images[0].mime_type, "image/png");
        assert_eq!(validated.pending_images[0].bytes, b"\x89PNG\r\n\x1a\n");
        assert_eq!(validated.concept_mentions.len(), 1);
        assert_eq!(validated.concept_mentions[0].selected_text, "结构化正文");
        assert!(!format!("{validated:?}").contains("iVBORw0KGgo"));
    }

    #[test]
    fn rejects_image_data_that_does_not_match_the_declared_mime_type() {
        let document = json!({
            "type": "doc",
            "content": [{
                "type": "image",
                "attrs": {
                    "tempId": "7b73c44e-cff8-4fb4-97e7-d6a03c3dfd10",
                    "data": "data:image/png;base64,bm90LWEtcG5n",
                    "alt": "",
                    "title": ""
                }
            }]
        });

        assert_eq!(
            validate_document(&document, true).unwrap_err().code,
            "inline_image_invalid"
        );
    }

    #[test]
    fn rejects_unexpected_nodes_marks_and_persisted_data_urls() {
        for document in [
            json!({"type": "doc", "content": [{"type": "heading", "content": []}]}),
            json!({
                "type": "doc",
                "content": [{
                    "type": "paragraph",
                    "content": [{
                        "type": "text",
                        "text": "粗体",
                        "marks": [{"type": "bold"}]
                    }]
                }]
            }),
            json!({
                "type": "doc",
                "content": [{
                    "type": "image",
                    "attrs": {
                        "tempId": "7b73c44e-cff8-4fb4-97e7-d6a03c3dfd10",
                        "data": "data:image/png;base64,iVBORw0KGgo="
                    }
                }]
            }),
        ] {
            let error = validate_document(&document, false).unwrap_err();
            assert_eq!(error.code, "document_invalid");
        }
    }

    #[test]
    fn rejects_invalid_image_mime_and_more_than_twenty_pending_images() {
        let invalid_mime = json!({
            "type": "doc",
            "content": [{
                "type": "image",
                "attrs": {
                    "tempId": "7b73c44e-cff8-4fb4-97e7-d6a03c3dfd10",
                    "data": "data:image/gif;base64,R0lGODlh"
                }
            }]
        });
        assert_eq!(
            validate_document(&invalid_mime, true).unwrap_err().code,
            "inline_image_invalid"
        );

        let images = (0..21)
            .map(|index| {
                json!({
                    "type": "image",
                    "attrs": {
                        "tempId": format!("00000000-0000-4000-8000-{index:012}"),
                        "data": "data:image/png;base64,iVBORw0KGgo="
                    }
                })
            })
            .collect::<Vec<_>>();
        let too_many = json!({ "type": "doc", "content": images });
        assert_eq!(
            validate_document(&too_many, true).unwrap_err().code,
            "document_invalid"
        );
    }
}
