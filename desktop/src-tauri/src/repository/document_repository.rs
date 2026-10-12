use std::{
    collections::{BTreeSet, HashMap},
    fs::File,
    io::Read,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    app_error::AppError,
    database::Database,
    domain::{
        card::{
            fallback_title, Attachment, AttachmentKind, CaptureMethod, Card, CardStatus,
            ExtractionStatus, SourceType,
        },
        document::{validate_document, PendingImage},
    },
    repository::{
        attachment_repository::{AttachmentClass, AttachmentRepository},
        card_repository::CreateCardResult,
    },
};

const MAX_INLINE_IMAGE_LOAD_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardDocument {
    pub card_id: String,
    pub document: Value,
    pub schema_version: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedCardDocument {
    pub card_id: String,
    pub document: Value,
    pub original_content: String,
    pub status: CardStatus,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineImageData {
    pub mime_type: String,
    pub data_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateRichCardInput {
    pub title: String,
    pub note: String,
    pub source_type: SourceType,
    pub capture_method: CaptureMethod,
    pub source_name: String,
    pub source_uri: String,
    pub document: Value,
}

pub struct DocumentRepository<'a> {
    database: &'a Database,
    attachments: &'a AttachmentRepository,
}

impl<'a> DocumentRepository<'a> {
    pub fn new(database: &'a Database, attachments: &'a AttachmentRepository) -> Self {
        Self {
            database,
            attachments,
        }
    }

    pub fn get_card_document(&self, card_id: &str) -> Result<CardDocument, AppError> {
        self.database.with_connection(|connection| {
            let stored = connection
                .query_row(
                    "SELECT document_json, schema_version, updated_at
                     FROM card_documents
                     WHERE card_id = ?1",
                    params![card_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((document_json, schema_version, updated_at)) = stored {
                let document = serde_json::from_str(&document_json)
                    .map_err(|_| database_data_invalid_error())?;
                return Ok(CardDocument {
                    card_id: card_id.to_owned(),
                    document,
                    schema_version,
                    updated_at,
                });
            }

            let card = connection
                .query_row(
                    "SELECT original_content, updated_at FROM cards WHERE id = ?1",
                    params![card_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
                .ok_or_else(card_not_found_error)?;
            Ok(CardDocument {
                card_id: card_id.to_owned(),
                document: plain_text_document(&card.0),
                schema_version: 1,
                updated_at: card.1,
            })
        })
    }

    pub fn save_card_document(
        &self,
        card_id: &str,
        document: Value,
    ) -> Result<SavedCardDocument, AppError> {
        let validated = validate_document(&document, true)?;
        let mut new_relative_paths = Vec::new();
        let result = self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let (status, previous_updated_at) = transaction
                .query_row(
                    "SELECT status, updated_at FROM cards WHERE id = ?1",
                    params![card_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
                .ok_or_else(card_not_found_error)?;
            let status = CardStatus::from_sql(&status).ok_or_else(database_data_invalid_error)?;
            if status == CardStatus::Archived {
                return Err(card_archived_read_only_error());
            }
            let updated_at = monotonic_utc_timestamp(&previous_updated_at, Utc::now())?;
            validate_persisted_images(&transaction, card_id, &validated.persisted_image_ids)?;
            validate_concept_mentions(&transaction, card_id, &validated.concept_mentions)?;
            let old_images = load_inline_images(&transaction, card_id)?;
            let retained_ids = validated
                .persisted_image_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            let removed_images = old_images
                .into_iter()
                .filter(|image| !retained_ids.contains(&image.attachment_id))
                .collect::<Vec<_>>();
            let removed_paths = removed_images
                .iter()
                .map(|image| image.relative_path.clone())
                .collect::<Vec<_>>();
            let staged_removal = self.attachments.stage_removal(&removed_paths)?;
            let stored_images = store_pending_images(
                &transaction,
                self.attachments,
                card_id,
                &validated.pending_images,
                &updated_at,
                &mut new_relative_paths,
            )?;
            let mut persisted_document = validated.document.clone();
            replace_pending_images(&mut persisted_document, &stored_images.replacements)?;
            let persisted = validate_document(&persisted_document, false)?;
            validate_persisted_images(&transaction, card_id, &persisted.persisted_image_ids)?;
            synchronize_concept_mentions(&transaction, card_id, &persisted.concept_mentions)?;
            let content_hash = hex::encode(Sha256::digest(persisted.plain_text.trim().as_bytes()));
            let document_json =
                serde_json::to_string(&persisted.document).map_err(|_| document_save_error())?;

            transaction.execute(
                "UPDATE cards
                 SET original_content = ?1, content_hash = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![persisted.plain_text, content_hash, updated_at, card_id],
            )?;
            transaction.execute(
                "INSERT INTO card_documents (card_id, document_json, schema_version, updated_at)
                 VALUES (?1, ?2, 1, ?3)
                 ON CONFLICT(card_id) DO UPDATE SET
                    document_json = excluded.document_json,
                    schema_version = excluded.schema_version,
                    updated_at = excluded.updated_at",
                params![card_id, document_json, updated_at],
            )?;
            transaction.execute(
                "DELETE FROM card_document_images WHERE card_id = ?1",
                params![card_id],
            )?;
            for (position, attachment_id) in persisted.persisted_image_ids.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO card_document_images (card_id, attachment_id, position)
                     VALUES (?1, ?2, ?3)",
                    params![
                        card_id,
                        attachment_id,
                        i64::try_from(position).map_err(|_| database_data_invalid_error())?
                    ],
                )?;
            }
            for image in &removed_images {
                transaction.execute(
                    "DELETE FROM attachments WHERE id = ?1 AND card_id = ?2",
                    params![image.attachment_id, card_id],
                )?;
            }
            transaction.commit()?;
            staged_removal.commit();

            Ok(SavedCardDocument {
                card_id: card_id.to_owned(),
                document: persisted.document,
                original_content: persisted.plain_text,
                status,
                updated_at,
            })
        });
        if result.is_err() {
            cleanup_new_files(self.attachments, &new_relative_paths)?;
        }
        result
    }

    pub fn load_inline_image(&self, attachment_id: &str) -> Result<InlineImageData, AppError> {
        let (relative_path, mime_type, byte_size) =
            self.database.with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT relative_path, mime_type, byte_size
                         FROM attachments
                         WHERE id = ?1 AND kind = 'image'",
                        params![attachment_id],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i64>(2)?,
                            ))
                        },
                    )
                    .optional()?
                    .ok_or_else(inline_image_not_found_error)
            })?;
        let byte_size = u64::try_from(byte_size).map_err(|_| database_data_invalid_error())?;
        if byte_size > MAX_INLINE_IMAGE_LOAD_BYTES {
            return Err(inline_image_too_large_error());
        }
        if !valid_image_mime_type(&mime_type) {
            return Err(inline_image_type_invalid_error());
        }

        let path = self.attachments.resolve(&relative_path)?;
        let file = File::open(path).map_err(|_| inline_image_load_error())?;
        let mut bytes = Vec::with_capacity(
            usize::try_from(byte_size).map_err(|_| inline_image_too_large_error())?,
        );
        file.take(MAX_INLINE_IMAGE_LOAD_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| inline_image_load_error())?;
        if u64::try_from(bytes.len()).map_err(|_| inline_image_too_large_error())?
            > MAX_INLINE_IMAGE_LOAD_BYTES
        {
            return Err(inline_image_too_large_error());
        }
        Ok(InlineImageData {
            mime_type: mime_type.clone(),
            data_url: format!("data:{mime_type};base64,{}", STANDARD.encode(bytes)),
        })
    }

    pub fn create_rich_card(
        &self,
        input: CreateRichCardInput,
    ) -> Result<CreateCardResult, AppError> {
        let validated = validate_document(&input.document, true)?;
        if validated.plain_text.trim().is_empty()
            && validated.persisted_image_ids.is_empty()
            && validated.pending_images.is_empty()
        {
            return Err(card_content_required_error());
        }
        let mut new_relative_paths = Vec::new();
        let result = self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let card_id = uuid::Uuid::new_v4().to_string();
            let timestamp = utc_timestamp();
            let content_hash = hex::encode(Sha256::digest(validated.plain_text.trim().as_bytes()));
            let duplicate_card_ids =
                duplicate_card_ids(&transaction, &content_hash, &input.source_uri)?;
            let title = if input.title.trim().is_empty() {
                if validated.plain_text.trim().is_empty() {
                    "图片记录".to_owned()
                } else {
                    fallback_title(&validated.plain_text)
                }
            } else {
                input.title.clone()
            };

            transaction.execute(
                "INSERT INTO cards (
                    id, title, original_content, note, source_type, capture_method,
                    source_name, source_uri, status, content_hash, created_at, updated_at,
                    archived_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, NULL)",
                params![
                    card_id,
                    title,
                    validated.plain_text,
                    input.note,
                    input.source_type.as_sql(),
                    input.capture_method.as_sql(),
                    input.source_name,
                    input.source_uri,
                    CardStatus::Unprocessed.as_sql(),
                    content_hash,
                    timestamp,
                ],
            )?;
            let stored_images = store_pending_images(
                &transaction,
                self.attachments,
                &card_id,
                &validated.pending_images,
                &timestamp,
                &mut new_relative_paths,
            )?;
            let mut persisted_document = validated.document.clone();
            replace_pending_images(&mut persisted_document, &stored_images.replacements)?;
            let persisted = validate_document(&persisted_document, false)?;
            validate_persisted_images(&transaction, &card_id, &persisted.persisted_image_ids)?;
            validate_concept_mentions(&transaction, &card_id, &persisted.concept_mentions)?;
            let document_json =
                serde_json::to_string(&persisted.document).map_err(|_| document_save_error())?;
            transaction.execute(
                "INSERT INTO card_documents (card_id, document_json, schema_version, updated_at)
                 VALUES (?1, ?2, 1, ?3)",
                params![card_id, document_json, timestamp],
            )?;
            for (position, attachment_id) in persisted.persisted_image_ids.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO card_document_images (card_id, attachment_id, position)
                     VALUES (?1, ?2, ?3)",
                    params![
                        card_id,
                        attachment_id,
                        i64::try_from(position).map_err(|_| database_data_invalid_error())?
                    ],
                )?;
            }
            transaction.commit()?;

            let mut card_attachments = stored_images.attachments;
            card_attachments.sort_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.id.cmp(&right.id))
            });
            Ok(CreateCardResult {
                card: Card {
                    id: card_id,
                    title,
                    original_content: persisted.plain_text,
                    note: input.note.clone(),
                    source_type: input.source_type,
                    capture_method: input.capture_method,
                    source_name: input.source_name.clone(),
                    source_uri: input.source_uri.clone(),
                    status: CardStatus::Unprocessed,
                    attachments: card_attachments,
                    created_at: timestamp.clone(),
                    updated_at: timestamp,
                    archived_at: None,
                },
                duplicate_card_ids,
            })
        });
        if result.is_err() {
            cleanup_new_files(self.attachments, &new_relative_paths)?;
        }
        result
    }
}

#[derive(Debug)]
struct InlineImageRow {
    attachment_id: String,
    relative_path: String,
}

fn load_inline_images(
    connection: &rusqlite::Connection,
    card_id: &str,
) -> Result<Vec<InlineImageRow>, AppError> {
    let mut statement = connection.prepare(
        "SELECT images.attachment_id, attachments.relative_path
         FROM card_document_images AS images
         JOIN attachments ON attachments.id = images.attachment_id
         WHERE images.card_id = ?1
         ORDER BY images.position ASC",
    )?;
    let rows = statement
        .query_map(params![card_id], |row| {
            Ok(InlineImageRow {
                attachment_id: row.get(0)?,
                relative_path: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn validate_persisted_images(
    connection: &rusqlite::Connection,
    card_id: &str,
    attachment_ids: &[String],
) -> Result<(), AppError> {
    for attachment_id in attachment_ids {
        let valid = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM attachments
                WHERE id = ?1 AND card_id = ?2 AND kind = 'image'
             )",
            params![attachment_id, card_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !valid {
            return Err(inline_image_attachment_invalid_error());
        }
    }
    Ok(())
}

fn validate_concept_mentions(
    connection: &rusqlite::Connection,
    card_id: &str,
    mentions: &[crate::domain::document::DocumentMention],
) -> Result<(), AppError> {
    for mention in mentions {
        let valid = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM concept_mentions
                WHERE id = ?1 AND card_id = ?2 AND concept_id = ?3
             )",
            params![mention.mention_id, card_id, mention.concept_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !valid {
            return Err(concept_mention_invalid_error());
        }
    }
    Ok(())
}

fn synchronize_concept_mentions(
    connection: &rusqlite::Connection,
    card_id: &str,
    mentions: &[crate::domain::document::DocumentMention],
) -> Result<(), AppError> {
    let retained_ids = mentions
        .iter()
        .map(|mention| mention.mention_id.as_str())
        .collect::<BTreeSet<_>>();
    for mention in mentions {
        let changed = connection.execute(
            "UPDATE concept_mentions
             SET selected_text = ?1
             WHERE id = ?2 AND card_id = ?3 AND concept_id = ?4",
            params![
                mention.selected_text,
                mention.mention_id,
                card_id,
                mention.concept_id
            ],
        )?;
        if changed != 1 {
            return Err(concept_mention_invalid_error());
        }
    }

    let mut statement = connection.prepare("SELECT id FROM concept_mentions WHERE card_id = ?1")?;
    let stored_ids = statement
        .query_map([card_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    for mention_id in stored_ids {
        if !retained_ids.contains(mention_id.as_str()) {
            connection.execute(
                "DELETE FROM concept_mentions WHERE id = ?1 AND card_id = ?2",
                params![mention_id, card_id],
            )?;
        }
    }
    Ok(())
}

fn store_pending_images(
    connection: &rusqlite::Connection,
    attachments: &AttachmentRepository,
    card_id: &str,
    pending_images: &[PendingImage],
    timestamp: &str,
    new_relative_paths: &mut Vec<String>,
) -> Result<StoredPendingImages, AppError> {
    let mut replacements = HashMap::with_capacity(pending_images.len());
    let mut attachment_records = Vec::with_capacity(pending_images.len());
    for (index, pending) in pending_images.iter().enumerate() {
        let stored = attachments.store_bytes(
            &pending.bytes,
            &pending.extension,
            AttachmentClass::DocumentOrImage,
        )?;
        new_relative_paths.push(stored.relative_path.clone());
        let attachment_id = uuid::Uuid::new_v4().to_string();
        let original_name = inline_image_name(index, &pending.extension);
        connection.execute(
            "INSERT INTO attachments (
                id, card_id, kind, original_name, relative_path, mime_type, byte_size,
                checksum, extraction_status, extraction_error, extracted_text, created_at
             ) VALUES (?1, ?2, 'image', ?3, ?4, ?5, ?6, ?7, 'not_applicable', '', '', ?8)",
            params![
                attachment_id,
                card_id,
                original_name,
                stored.relative_path,
                pending.mime_type,
                i64::try_from(stored.byte_size).map_err(|_| database_data_invalid_error())?,
                stored.checksum,
                timestamp,
            ],
        )?;
        attachment_records.push(Attachment {
            id: attachment_id.clone(),
            card_id: card_id.to_owned(),
            kind: AttachmentKind::Image,
            original_name,
            relative_path: stored.relative_path,
            mime_type: pending.mime_type.clone(),
            byte_size: stored.byte_size,
            extraction_status: ExtractionStatus::NotApplicable,
            extraction_error: String::new(),
            extracted_text: String::new(),
            created_at: timestamp.to_owned(),
        });
        replacements.insert(pending.temp_id.clone(), attachment_id);
    }
    Ok(StoredPendingImages {
        replacements,
        attachments: attachment_records,
    })
}

struct StoredPendingImages {
    replacements: HashMap<String, String>,
    attachments: Vec<Attachment>,
}

fn inline_image_name(index: usize, extension: &str) -> String {
    if index == 0 {
        format!("正文图片.{extension}")
    } else {
        format!("正文图片 {}.{extension}", index + 1)
    }
}

fn replace_pending_images(
    document: &mut Value,
    replacements: &HashMap<String, String>,
) -> Result<(), AppError> {
    let content = document
        .get_mut("content")
        .and_then(Value::as_array_mut)
        .ok_or_else(document_save_error)?;
    for node in content {
        let Some(node) = node.as_object_mut() else {
            return Err(document_save_error());
        };
        if node.get("type").and_then(Value::as_str) != Some("image") {
            continue;
        }
        let attrs = node
            .get_mut("attrs")
            .and_then(Value::as_object_mut)
            .ok_or_else(document_save_error)?;
        let Some(temp_id) = attrs.get("tempId").and_then(Value::as_str) else {
            continue;
        };
        let attachment_id = replacements
            .get(temp_id)
            .ok_or_else(document_save_error)?
            .clone();
        attrs.remove("tempId");
        attrs.remove("data");
        attrs.insert("attachmentId".to_owned(), Value::String(attachment_id));
    }
    Ok(())
}

fn cleanup_new_files(
    attachments: &AttachmentRepository,
    relative_paths: &[String],
) -> Result<(), AppError> {
    for relative_path in relative_paths.iter().rev() {
        attachments
            .remove_stored(relative_path)
            .map_err(|_| document_rollback_error())?;
    }
    Ok(())
}

fn duplicate_card_ids(
    connection: &rusqlite::Connection,
    content_hash: &str,
    source_uri: &str,
) -> Result<Vec<String>, AppError> {
    let mut ids = BTreeSet::new();
    let mut content_statement = connection.prepare(
        "SELECT id FROM cards INDEXED BY cards_content_hash_idx WHERE content_hash = ?1",
    )?;
    let content_matches = content_statement
        .query_map(params![content_hash], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ids.extend(content_matches);
    if !source_uri.is_empty() {
        let mut source_statement = connection.prepare(
            "SELECT id FROM cards INDEXED BY cards_source_uri_idx
             WHERE source_uri = ?1 AND source_uri <> ''",
        )?;
        let source_matches = source_statement
            .query_map(params![source_uri], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.extend(source_matches);
    }
    Ok(ids.into_iter().collect())
}

fn plain_text_document(content: &str) -> Value {
    let paragraphs = content
        .split('\n')
        .map(|paragraph| {
            if paragraph.is_empty() {
                json!({ "type": "paragraph" })
            } else {
                json!({
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": paragraph }]
                })
            }
        })
        .collect::<Vec<_>>();
    json!({ "type": "doc", "content": paragraphs })
}

fn card_not_found_error() -> AppError {
    AppError::new(
        "card_not_found",
        "找不到这张卡片",
        "返回收集箱后刷新卡片列表",
    )
}

fn card_content_required_error() -> AppError {
    AppError::new(
        "card_content_required",
        "记录内容不能为空",
        "输入文字或粘贴一张图片后再保存",
    )
}

fn database_data_invalid_error() -> AppError {
    AppError::new(
        "database_data_invalid",
        "数据库中的正文数据无效",
        "重新打开应用；问题持续时检查数据目录",
    )
}

fn card_archived_read_only_error() -> AppError {
    AppError::new(
        "card_archived_read_only",
        "已归档卡片不能修改",
        "先恢复卡片后再编辑",
    )
}

fn document_save_error() -> AppError {
    AppError::new("document_save_failed", "无法保存正文", "保留当前内容并重试")
}

fn inline_image_attachment_invalid_error() -> AppError {
    AppError::new(
        "inline_image_attachment_invalid",
        "正文图片不可用",
        "移除该图片后重新粘贴",
    )
}

fn concept_mention_invalid_error() -> AppError {
    AppError::new(
        "concept_mention_invalid",
        "正文中的概念注释无效",
        "重新打开卡片后再试",
    )
}

fn document_rollback_error() -> AppError {
    AppError::new(
        "document_rollback_failed",
        "正文保存失败且无法清理临时图片",
        "重新打开应用后检查附件目录",
    )
}

fn inline_image_not_found_error() -> AppError {
    AppError::new(
        "inline_image_not_found",
        "找不到正文图片",
        "重新打开卡片后再试",
    )
}

fn inline_image_type_invalid_error() -> AppError {
    AppError::new(
        "inline_image_type_invalid",
        "正文图片类型无效",
        "移除该图片后重新粘贴",
    )
}

fn inline_image_too_large_error() -> AppError {
    AppError::new(
        "inline_image_too_large",
        "正文图片超过 20 MB",
        "压缩图片后重新粘贴",
    )
}

fn inline_image_load_error() -> AppError {
    AppError::new(
        "inline_image_load_failed",
        "无法读取正文图片",
        "重新打开卡片后再试",
    )
}

fn valid_image_mime_type(mime_type: &str) -> bool {
    mime_type.starts_with("image/")
        && mime_type
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'+' | b'-' | b'.'))
}

fn utc_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true)
}

fn monotonic_utc_timestamp(previous: &str, candidate: DateTime<Utc>) -> Result<String, AppError> {
    let previous = DateTime::parse_from_rfc3339(previous)
        .map_err(|_| database_data_invalid_error())?
        .with_timezone(&Utc);
    let timestamp = if candidate > previous {
        candidate
    } else {
        previous
            .checked_add_signed(TimeDelta::nanoseconds(1))
            .ok_or_else(database_data_invalid_error)?
    };
    Ok(timestamp.to_rfc3339_opts(SecondsFormat::Nanos, true))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use super::{CreateRichCardInput, DocumentRepository};
    use crate::{
        database::Database,
        domain::{
            card::{CaptureMethod, CardStatus, NewCardInput, SourceType},
            concept::SaveConceptAnnotationInput,
        },
        repository::{
            attachment_repository::AttachmentRepository, card_repository::CardRepository,
            concept_repository::ConceptRepository,
        },
    };

    fn new_card(content: &str) -> NewCardInput {
        NewCardInput {
            title: String::new(),
            original_content: content.to_owned(),
            note: String::new(),
            source_type: SourceType::Personal,
            capture_method: CaptureMethod::Quick,
            source_name: String::new(),
            source_uri: String::new(),
        }
    }

    #[test]
    fn legacy_card_is_projected_to_paragraph_json_without_writing_a_document_row() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachments = AttachmentRepository::new(
            directory.path().join("attachments"),
            directory.path().join("cache"),
        );
        let card = CardRepository::new(&database)
            .create(new_card("第一段\n第二段"))
            .unwrap()
            .card;

        let loaded = DocumentRepository::new(&database, &attachments)
            .get_card_document(&card.id)
            .unwrap();

        assert_eq!(loaded.card_id, card.id);
        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.updated_at, card.updated_at);
        assert_eq!(
            loaded.document,
            json!({
                "type": "doc",
                "content": [{
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "第一段" }]
                }, {
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "第二段" }]
                }]
            })
        );
        database
            .with_connection(|connection| {
                let count = connection.query_row(
                    "SELECT COUNT(*) FROM card_documents WHERE card_id = ?1",
                    [&loaded.card_id],
                    |row| row.get::<_, i64>(0),
                )?;
                assert_eq!(count, 0);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn save_text_document_updates_projection_hash_timestamp_and_document_atomically() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachments = AttachmentRepository::new(
            directory.path().join("attachments"),
            directory.path().join("cache"),
        );
        let card = CardRepository::new(&database)
            .create(new_card("旧正文"))
            .unwrap()
            .card;
        let document = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{ "type": "text", "text": "新的正文" }]
            }, {
                "type": "paragraph",
                "content": [{ "type": "text", "text": "第二段" }]
            }]
        });

        let saved = DocumentRepository::new(&database, &attachments)
            .save_card_document(&card.id, document.clone())
            .unwrap();

        assert_eq!(saved.card_id, card.id);
        assert_eq!(saved.document, document);
        assert_eq!(saved.original_content, "新的正文\n第二段");
        assert_eq!(saved.status, CardStatus::Unprocessed);
        assert!(saved.updated_at > card.updated_at);
        let loaded_card = CardRepository::new(&database).get(&card.id).unwrap();
        assert_eq!(loaded_card.original_content, saved.original_content);
        assert_eq!(loaded_card.updated_at, saved.updated_at);
        database
            .with_connection(|connection| {
                let (document_json, schema_version, document_updated_at) = connection.query_row(
                    "SELECT document_json, schema_version, updated_at
                     FROM card_documents WHERE card_id = ?1",
                    [&card.id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )?;
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&document_json).unwrap(),
                    document
                );
                assert_eq!(schema_version, 1);
                assert_eq!(document_updated_at, saved.updated_at);
                let content_hash = connection.query_row(
                    "SELECT content_hash FROM cards WHERE id = ?1",
                    [&card.id],
                    |row| row.get::<_, String>(0),
                )?;
                assert_eq!(
                    content_hash,
                    hex::encode(Sha256::digest(saved.original_content.trim().as_bytes()))
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn save_document_deletes_a_concept_mention_after_its_mark_is_removed() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachments = AttachmentRepository::new(
            directory.path().join("attachments"),
            directory.path().join("cache"),
        );
        let card = CardRepository::new(&database)
            .create(new_card("原概念"))
            .unwrap()
            .card;
        let concept_id = "8ee5e2b0-6f71-4c31-9d47-34c6974f8c01";
        let mention_id = "7fd7d15d-6672-471f-afc8-a8b4cdf1ed0f";
        ConceptRepository::new(&database)
            .save_concept_annotation(SaveConceptAnnotationInput {
                card_id: card.id.clone(),
                concept_id: concept_id.to_owned(),
                mention_id: mention_id.to_owned(),
                selected_text: "原概念".to_owned(),
                name: "原概念".to_owned(),
                explanation: "已有解释".to_owned(),
                overwrite_existing: false,
                document: json!({
                    "type": "doc",
                    "content": [{
                        "type": "paragraph",
                        "content": [{
                            "type": "text",
                            "text": "原概念",
                            "marks": [{
                                "type": "concept",
                                "attrs": {
                                    "conceptId": concept_id,
                                    "mentionId": mention_id
                                }
                            }]
                        }]
                    }]
                }),
            })
            .unwrap();

        DocumentRepository::new(&database, &attachments)
            .save_card_document(
                &card.id,
                json!({
                    "type": "doc",
                    "content": [{
                        "type": "paragraph",
                        "content": [{"type": "text", "text": "原概念"}]
                    }]
                }),
            )
            .unwrap();

        database
            .with_connection(|connection| {
                let mention_count = connection.query_row(
                    "SELECT COUNT(*) FROM concept_mentions WHERE card_id = ?1",
                    [&card.id],
                    |row| row.get::<_, i64>(0),
                )?;
                let concept_count = connection.query_row(
                    "SELECT COUNT(*) FROM concepts WHERE id = ?1",
                    [concept_id],
                    |row| row.get::<_, i64>(0),
                )?;
                assert_eq!((mention_count, concept_count), (0, 1));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn save_document_updates_selected_text_to_match_the_remaining_mark() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachments = AttachmentRepository::new(
            directory.path().join("attachments"),
            directory.path().join("cache"),
        );
        let card = CardRepository::new(&database)
            .create(new_card("原概念"))
            .unwrap()
            .card;
        let concept_id = "8ee5e2b0-6f71-4c31-9d47-34c6974f8c01";
        let mention_id = "7fd7d15d-6672-471f-afc8-a8b4cdf1ed0f";
        let marked_document = |text: &str| {
            json!({
                "type": "doc",
                "content": [{
                    "type": "paragraph",
                    "content": [{
                        "type": "text",
                        "text": text,
                        "marks": [{
                            "type": "concept",
                            "attrs": {
                                "conceptId": concept_id,
                                "mentionId": mention_id
                            }
                        }]
                    }]
                }]
            })
        };
        ConceptRepository::new(&database)
            .save_concept_annotation(SaveConceptAnnotationInput {
                card_id: card.id.clone(),
                concept_id: concept_id.to_owned(),
                mention_id: mention_id.to_owned(),
                selected_text: "原概念".to_owned(),
                name: "原概念".to_owned(),
                explanation: "已有解释".to_owned(),
                overwrite_existing: false,
                document: marked_document("原概念"),
            })
            .unwrap();

        DocumentRepository::new(&database, &attachments)
            .save_card_document(&card.id, marked_document("扩展后的概念"))
            .unwrap();

        database
            .with_connection(|connection| {
                let selected_text = connection.query_row(
                    "SELECT selected_text FROM concept_mentions WHERE id = ?1",
                    [mention_id],
                    |row| row.get::<_, String>(0),
                )?;
                assert_eq!(selected_text, "扩展后的概念");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn save_converts_pending_data_url_to_owned_image_attachment_without_persisting_data() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachment_root = directory.path().join("attachments");
        let attachments =
            AttachmentRepository::new(&attachment_root, directory.path().join("cache"));
        let card = CardRepository::new(&database)
            .create(new_card("粘贴图片"))
            .unwrap()
            .card;
        let document = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{ "type": "text", "text": "图片前" }]
            }, {
                "type": "image",
                "attrs": {
                    "tempId": "7b73c44e-cff8-4fb4-97e7-d6a03c3dfd10",
                    "data": "data:image/png;base64,iVBORw0KGgppbmxpbmU=",
                    "alt": "粘贴图",
                    "title": "示例"
                }
            }]
        });

        let saved = DocumentRepository::new(&database, &attachments)
            .save_card_document(&card.id, document)
            .unwrap();

        let attrs = saved.document["content"][1]["attrs"].as_object().unwrap();
        let attachment_id = attrs["attachmentId"].as_str().unwrap();
        assert_eq!(attrs["alt"], "粘贴图");
        assert_eq!(attrs["title"], "示例");
        assert!(attrs.get("tempId").is_none());
        assert!(attrs.get("data").is_none());
        assert_eq!(saved.original_content, "图片前");
        database
            .with_connection(|connection| {
                let (owner, kind, relative_path, mime_type, byte_size) = connection.query_row(
                    "SELECT card_id, kind, relative_path, mime_type, byte_size
                     FROM attachments WHERE id = ?1",
                    [attachment_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, i64>(4)?,
                        ))
                    },
                )?;
                assert_eq!(owner, card.id);
                assert_eq!(kind, "image");
                assert_eq!(mime_type, "image/png");
                assert_eq!(byte_size, 14);
                let position = connection.query_row(
                    "SELECT position FROM card_document_images
                     WHERE card_id = ?1 AND attachment_id = ?2",
                    [&card.id, attachment_id],
                    |row| row.get::<_, i64>(0),
                )?;
                assert_eq!(position, 0);
                let stored_json = connection.query_row(
                    "SELECT document_json FROM card_documents WHERE card_id = ?1",
                    [&card.id],
                    |row| row.get::<_, String>(0),
                )?;
                assert!(!stored_json.contains("data:image"));
                assert!(!stored_json.contains("tempId"));
                assert_eq!(
                    std::fs::read(attachment_root.join(relative_path)).unwrap(),
                    b"\x89PNG\r\n\x1a\ninline"
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn load_inline_image_returns_only_mime_and_data_url() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachments = AttachmentRepository::new(
            directory.path().join("private/attachments"),
            directory.path().join("cache"),
        );
        let card = CardRepository::new(&database)
            .create(new_card("图片"))
            .unwrap()
            .card;
        let saved = DocumentRepository::new(&database, &attachments)
            .save_card_document(
                &card.id,
                json!({
                    "type": "doc",
                    "content": [{
                        "type": "image",
                        "attrs": {
                            "tempId": "7b73c44e-cff8-4fb4-97e7-d6a03c3dfd10",
                            "data": "data:image/png;base64,iVBORw0KGgppbmxpbmU=",
                            "alt": "",
                            "title": ""
                        }
                    }]
                }),
            )
            .unwrap();
        let attachment_id = saved.document["content"][0]["attrs"]["attachmentId"]
            .as_str()
            .unwrap();

        let loaded = DocumentRepository::new(&database, &attachments)
            .load_inline_image(attachment_id)
            .unwrap();

        assert_eq!(loaded.mime_type, "image/png");
        assert_eq!(
            loaded.data_url,
            "data:image/png;base64,iVBORw0KGgppbmxpbmU="
        );
        let serialized = serde_json::to_string(&loaded).unwrap();
        assert!(!serialized.contains(directory.path().to_string_lossy().as_ref()));
        assert!(!serialized.contains("relativePath"));
        assert_eq!(
            serde_json::to_value(loaded).unwrap(),
            json!({
                "mimeType": "image/png",
                "dataUrl": "data:image/png;base64,iVBORw0KGgppbmxpbmU="
            })
        );
    }

    #[test]
    fn create_rich_image_only_card_uses_image_title_and_commits_document_and_attachment() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachments = AttachmentRepository::new(
            directory.path().join("attachments"),
            directory.path().join("cache"),
        );
        let repository = DocumentRepository::new(&database, &attachments);

        let result = repository
            .create_rich_card(CreateRichCardInput {
                title: String::new(),
                note: String::new(),
                source_type: SourceType::Personal,
                capture_method: CaptureMethod::Clipboard,
                source_name: String::new(),
                source_uri: String::new(),
                document: json!({
                    "type": "doc",
                    "content": [{
                        "type": "image",
                        "attrs": {
                            "tempId": "7b73c44e-cff8-4fb4-97e7-d6a03c3dfd10",
                            "data": "data:image/png;base64,iVBORw0KGgppbmxpbmU=",
                            "alt": "",
                            "title": ""
                        }
                    }]
                }),
            })
            .unwrap();

        assert_eq!(result.card.title, "图片记录");
        assert_eq!(result.card.original_content, "");
        assert_eq!(result.card.status, CardStatus::Unprocessed);
        assert_eq!(result.card.capture_method, CaptureMethod::Clipboard);
        assert_eq!(result.card.attachments.len(), 1);
        assert_eq!(
            result.card.attachments[0].kind,
            crate::domain::card::AttachmentKind::Image
        );
        assert!(result.duplicate_card_ids.is_empty());
        let loaded = repository.get_card_document(&result.card.id).unwrap();
        assert_eq!(
            loaded.document["content"][0]["attrs"]["attachmentId"],
            result.card.attachments[0].id
        );
        assert!(loaded.document["content"][0]["attrs"].get("data").is_none());
        assert_eq!(
            CardRepository::new(&database).get(&result.card.id).unwrap(),
            result.card
        );
    }

    #[test]
    fn create_rich_card_rejects_a_document_without_text_or_images() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachments = AttachmentRepository::new(
            directory.path().join("attachments"),
            directory.path().join("cache"),
        );
        let repository = DocumentRepository::new(&database, &attachments);

        let error = repository
            .create_rich_card(CreateRichCardInput {
                title: String::new(),
                note: String::new(),
                source_type: SourceType::Personal,
                capture_method: CaptureMethod::Quick,
                source_name: String::new(),
                source_uri: String::new(),
                document: json!({
                    "type": "doc",
                    "content": [{ "type": "paragraph" }]
                }),
            })
            .unwrap_err();

        assert_eq!(error.code, "card_content_required");
        assert!(CardRepository::new(&database)
            .list(true)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn save_rejects_persisted_images_owned_by_another_card_or_with_a_non_image_kind() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachments = AttachmentRepository::new(
            directory.path().join("attachments"),
            directory.path().join("cache"),
        );
        let repository = CardRepository::new(&database);
        let target = repository.create(new_card("目标正文")).unwrap().card;
        let other = repository.create(new_card("另一张卡")).unwrap().card;
        let other_image = "550e8400-e29b-41d4-a716-446655440000";
        let target_document = "550e8400-e29b-41d4-a716-446655440001";
        database
            .with_connection(|connection| {
                for (id, card_id, kind, path, mime) in [
                    (
                        other_image,
                        other.id.as_str(),
                        "image",
                        "other.png",
                        "image/png",
                    ),
                    (
                        target_document,
                        target.id.as_str(),
                        "document",
                        "target.pdf",
                        "application/pdf",
                    ),
                ] {
                    connection.execute(
                        "INSERT INTO attachments (
                            id, card_id, kind, original_name, relative_path, mime_type, byte_size,
                            checksum, extraction_status, extraction_error, extracted_text, created_at
                         ) VALUES (?1, ?2, ?3, '文件', ?4, ?5, 1, 'hash',
                                   'not_applicable', '', '', ?6)",
                        rusqlite::params![
                            id,
                            card_id,
                            kind,
                            path,
                            mime,
                            "2026-07-19T00:00:00Z"
                        ],
                    )?;
                }
                Ok(())
            })
            .unwrap();

        for attachment_id in [other_image, target_document] {
            let error = DocumentRepository::new(&database, &attachments)
                .save_card_document(
                    &target.id,
                    json!({
                        "type": "doc",
                        "content": [{
                            "type": "image",
                            "attrs": {
                                "attachmentId": attachment_id,
                                "alt": "",
                                "title": ""
                            }
                        }]
                    }),
                )
                .unwrap_err();
            assert_eq!(error.code, "inline_image_attachment_invalid");
        }
        assert_eq!(
            repository.get(&target.id).unwrap().original_content,
            "目标正文"
        );
        database
            .with_connection(|connection| {
                let count = connection.query_row(
                    "SELECT COUNT(*) FROM card_documents WHERE card_id = ?1",
                    [&target.id],
                    |row| row.get::<_, i64>(0),
                )?;
                assert_eq!(count, 0);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn saving_without_an_old_inline_image_removes_its_row_and_file() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachment_root = directory.path().join("attachments");
        let attachments =
            AttachmentRepository::new(&attachment_root, directory.path().join("cache"));
        let card = CardRepository::new(&database)
            .create(new_card("图片正文"))
            .unwrap()
            .card;
        let repository = DocumentRepository::new(&database, &attachments);
        let with_image = repository
            .save_card_document(
                &card.id,
                json!({
                    "type": "doc",
                    "content": [{
                        "type": "image",
                        "attrs": {
                            "tempId": "7b73c44e-cff8-4fb4-97e7-d6a03c3dfd10",
                            "data": "data:image/png;base64,iVBORw0KGgppbmxpbmU=",
                            "alt": "",
                            "title": ""
                        }
                    }]
                }),
            )
            .unwrap();
        let attachment_id = with_image.document["content"][0]["attrs"]["attachmentId"]
            .as_str()
            .unwrap()
            .to_owned();
        let relative_path = database
            .with_connection(|connection| {
                Ok(connection.query_row(
                    "SELECT relative_path FROM attachments WHERE id = ?1",
                    [&attachment_id],
                    |row| row.get::<_, String>(0),
                )?)
            })
            .unwrap();
        assert!(attachment_root.join(&relative_path).is_file());

        repository
            .save_card_document(
                &card.id,
                json!({
                    "type": "doc",
                    "content": [{
                        "type": "paragraph",
                        "content": [{ "type": "text", "text": "只保留文字" }]
                    }]
                }),
            )
            .unwrap();

        assert!(!attachment_root.join(relative_path).exists());
        database
            .with_connection(|connection| {
                let attachment_count = connection.query_row(
                    "SELECT COUNT(*) FROM attachments WHERE id = ?1",
                    [&attachment_id],
                    |row| row.get::<_, i64>(0),
                )?;
                let relation_count = connection.query_row(
                    "SELECT COUNT(*) FROM card_document_images WHERE attachment_id = ?1",
                    [&attachment_id],
                    |row| row.get::<_, i64>(0),
                )?;
                assert_eq!((attachment_count, relation_count), (0, 0));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn sql_failure_rolls_back_rows_and_cleans_new_image_files_without_leaking_details() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachment_root = directory.path().join("attachments");
        let attachments =
            AttachmentRepository::new(&attachment_root, directory.path().join("cache"));
        let card = CardRepository::new(&database)
            .create(new_card("事务前正文"))
            .unwrap()
            .card;
        let sentinel = "data:image/png;base64,SENSITIVE /Users/private/ideacard.sqlite3";
        database
            .with_connection(|connection| {
                connection.execute_batch(&format!(
                    "CREATE TRIGGER fail_document_insert
                     BEFORE INSERT ON card_documents
                     BEGIN
                       SELECT RAISE(FAIL, '{sentinel}');
                     END;"
                ))?;
                Ok(())
            })
            .unwrap();

        let error = DocumentRepository::new(&database, &attachments)
            .save_card_document(
                &card.id,
                json!({
                    "type": "doc",
                    "content": [{
                        "type": "image",
                        "attrs": {
                            "tempId": "7b73c44e-cff8-4fb4-97e7-d6a03c3dfd10",
                            "data": "data:image/png;base64,iVBORw0KGgppbmxpbmU=",
                            "alt": "",
                            "title": ""
                        }
                    }]
                }),
            )
            .unwrap_err();

        assert_eq!(error.code, "database_operation_failed");
        let serialized = serde_json::to_string(&error).unwrap();
        assert!(!serialized.contains("SENSITIVE"));
        assert!(!serialized.contains("data:image"));
        assert!(!serialized.contains("/Users/private"));
        assert_eq!(
            CardRepository::new(&database)
                .get(&card.id)
                .unwrap()
                .original_content,
            "事务前正文"
        );
        database
            .with_connection(|connection| {
                let counts = (
                    connection.query_row(
                        "SELECT COUNT(*) FROM attachments WHERE card_id = ?1",
                        [&card.id],
                        |row| row.get::<_, i64>(0),
                    )?,
                    connection.query_row(
                        "SELECT COUNT(*) FROM card_documents WHERE card_id = ?1",
                        [&card.id],
                        |row| row.get::<_, i64>(0),
                    )?,
                );
                assert_eq!(counts, (0, 0));
                Ok(())
            })
            .unwrap();
        assert!(
            !attachment_root.exists()
                || std::fs::read_dir(attachment_root).unwrap().next().is_none()
        );
    }

    #[test]
    fn failed_update_restores_an_old_inline_image_and_its_database_rows() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachment_root = directory.path().join("attachments");
        let attachments =
            AttachmentRepository::new(&attachment_root, directory.path().join("cache"));
        let card = CardRepository::new(&database)
            .create(new_card("原正文"))
            .unwrap()
            .card;
        let repository = DocumentRepository::new(&database, &attachments);
        let first = repository
            .save_card_document(
                &card.id,
                json!({
                    "type": "doc",
                    "content": [{
                        "type": "image",
                        "attrs": {
                            "tempId": "7b73c44e-cff8-4fb4-97e7-d6a03c3dfd10",
                            "data": "data:image/png;base64,iVBORw0KGgppbmxpbmU=",
                            "alt": "",
                            "title": ""
                        }
                    }]
                }),
            )
            .unwrap();
        let attachment_id = first.document["content"][0]["attrs"]["attachmentId"]
            .as_str()
            .unwrap()
            .to_owned();
        let relative_path = database
            .with_connection(|connection| {
                connection.execute_batch(
                    "CREATE TRIGGER fail_document_update
                     BEFORE UPDATE ON card_documents
                     BEGIN
                       SELECT RAISE(FAIL, 'forced update failure');
                     END;",
                )?;
                Ok(connection.query_row(
                    "SELECT relative_path FROM attachments WHERE id = ?1",
                    [&attachment_id],
                    |row| row.get::<_, String>(0),
                )?)
            })
            .unwrap();

        let error = repository
            .save_card_document(
                &card.id,
                json!({
                    "type": "doc",
                    "content": [{
                        "type": "paragraph",
                        "content": [{ "type": "text", "text": "删除图片" }]
                    }]
                }),
            )
            .unwrap_err();

        assert_eq!(error.code, "database_operation_failed");
        assert_eq!(
            std::fs::read(attachment_root.join(relative_path)).unwrap(),
            b"\x89PNG\r\n\x1a\ninline"
        );
        database
            .with_connection(|connection| {
                let attachment_count = connection.query_row(
                    "SELECT COUNT(*) FROM attachments WHERE id = ?1",
                    [&attachment_id],
                    |row| row.get::<_, i64>(0),
                )?;
                let relation_count = connection.query_row(
                    "SELECT COUNT(*) FROM card_document_images WHERE attachment_id = ?1",
                    [&attachment_id],
                    |row| row.get::<_, i64>(0),
                )?;
                assert_eq!((attachment_count, relation_count), (1, 1));
                Ok(())
            })
            .unwrap();
        assert_eq!(
            repository.get_card_document(&card.id).unwrap().document,
            first.document
        );
    }

    #[test]
    fn create_failure_rolls_back_card_document_attachment_and_image_file() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachment_root = directory.path().join("attachments");
        let attachments =
            AttachmentRepository::new(&attachment_root, directory.path().join("cache"));
        database
            .with_connection(|connection| {
                connection.execute_batch(
                    "CREATE TRIGGER fail_rich_document_insert
                     BEFORE INSERT ON card_documents
                     BEGIN
                       SELECT RAISE(FAIL, 'forced create failure');
                     END;",
                )?;
                Ok(())
            })
            .unwrap();

        let error = DocumentRepository::new(&database, &attachments)
            .create_rich_card(CreateRichCardInput {
                title: String::new(),
                note: String::new(),
                source_type: SourceType::Personal,
                capture_method: CaptureMethod::Clipboard,
                source_name: String::new(),
                source_uri: String::new(),
                document: json!({
                    "type": "doc",
                    "content": [{
                        "type": "image",
                        "attrs": {
                            "tempId": "7b73c44e-cff8-4fb4-97e7-d6a03c3dfd10",
                            "data": "data:image/png;base64,iVBORw0KGgppbmxpbmU=",
                            "alt": "",
                            "title": ""
                        }
                    }]
                }),
            })
            .unwrap_err();

        assert_eq!(error.code, "database_operation_failed");
        database
            .with_connection(|connection| {
                for table in [
                    "cards",
                    "attachments",
                    "card_documents",
                    "card_document_images",
                ] {
                    let count = connection.query_row(
                        &format!("SELECT COUNT(*) FROM {table}"),
                        [],
                        |row| row.get::<_, i64>(0),
                    )?;
                    assert_eq!(count, 0, "{table} should roll back");
                }
                Ok(())
            })
            .unwrap();
        assert!(
            !attachment_root.exists()
                || std::fs::read_dir(attachment_root).unwrap().next().is_none()
        );
    }

    #[test]
    fn load_rejects_non_images_and_enforces_the_file_limit_even_if_database_size_is_wrong() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachment_root = directory.path().join("attachments");
        std::fs::create_dir_all(&attachment_root).unwrap();
        let oversized_path = attachment_root.join("oversized.png");
        let oversized = std::fs::File::create(&oversized_path).unwrap();
        oversized
            .set_len(super::MAX_INLINE_IMAGE_LOAD_BYTES + 1)
            .unwrap();
        let attachments =
            AttachmentRepository::new(&attachment_root, directory.path().join("cache"));
        let card = CardRepository::new(&database)
            .create(new_card("附件"))
            .unwrap()
            .card;
        let document_id = "550e8400-e29b-41d4-a716-446655440000";
        let image_id = "550e8400-e29b-41d4-a716-446655440001";
        database
            .with_connection(|connection| {
                for (id, kind, path, mime) in [
                    (
                        document_id,
                        "document",
                        "not-image.pdf",
                        "application/pdf",
                    ),
                    (image_id, "image", "oversized.png", "image/png"),
                ] {
                    connection.execute(
                        "INSERT INTO attachments (
                            id, card_id, kind, original_name, relative_path, mime_type, byte_size,
                            checksum, extraction_status, extraction_error, extracted_text, created_at
                         ) VALUES (?1, ?2, ?3, '附件', ?4, ?5, 1, 'hash',
                                   'not_applicable', '', '', ?6)",
                        rusqlite::params![
                            id,
                            card.id,
                            kind,
                            path,
                            mime,
                            "2026-07-19T00:00:00Z"
                        ],
                    )?;
                }
                Ok(())
            })
            .unwrap();
        let repository = DocumentRepository::new(&database, &attachments);

        assert_eq!(
            repository.load_inline_image(document_id).unwrap_err().code,
            "inline_image_not_found"
        );
        assert_eq!(
            repository.load_inline_image(image_id).unwrap_err().code,
            "inline_image_too_large"
        );
    }

    #[test]
    fn create_rich_text_card_uses_text_title_and_existing_duplicate_contract() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachments = AttachmentRepository::new(
            directory.path().join("attachments"),
            directory.path().join("cache"),
        );
        let existing = CardRepository::new(&database)
            .create(new_card("重复正文"))
            .unwrap()
            .card;

        let created = DocumentRepository::new(&database, &attachments)
            .create_rich_card(CreateRichCardInput {
                title: String::new(),
                note: String::new(),
                source_type: SourceType::Personal,
                capture_method: CaptureMethod::Quick,
                source_name: String::new(),
                source_uri: String::new(),
                document: json!({
                    "type": "doc",
                    "content": [{
                        "type": "paragraph",
                        "content": [{ "type": "text", "text": "  重复正文  " }]
                    }]
                }),
            })
            .unwrap();

        assert_eq!(created.card.title, "重复正文");
        assert_eq!(created.card.original_content, "  重复正文  ");
        assert_eq!(created.duplicate_card_ids, vec![existing.id]);
    }

    #[test]
    fn archived_card_document_is_read_only_and_keeps_its_existing_content() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachments = AttachmentRepository::new(
            directory.path().join("attachments"),
            directory.path().join("cache"),
        );
        let repository = CardRepository::new(&database);
        let card = repository.create(new_card("归档正文")).unwrap().card;
        repository
            .update(
                &card.id,
                crate::domain::card::CardPatch {
                    status: Some(CardStatus::Archived),
                    ..crate::domain::card::CardPatch::default()
                },
            )
            .unwrap();

        let error = DocumentRepository::new(&database, &attachments)
            .save_card_document(
                &card.id,
                json!({
                    "type": "doc",
                    "content": [{
                        "type": "paragraph",
                        "content": [{ "type": "text", "text": "不应保存" }]
                    }]
                }),
            )
            .unwrap_err();

        assert_eq!(error.code, "card_archived_read_only");
        assert_eq!(
            repository.get(&card.id).unwrap().original_content,
            "归档正文"
        );
    }
}
