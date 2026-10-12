use std::{
    collections::{BTreeSet, HashMap},
    slice::Chunks,
};

use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    app_error::AppError,
    database::Database,
    domain::card::{
        fallback_title, Attachment, AttachmentKind, CaptureMethod, Card, CardPatch, CardStatus,
        ExtractionStatus, NewCardInput, SourceType,
    },
};

const CARD_SELECT: &str = "
    SELECT id, title, original_content, note, source_type, capture_method,
           source_name, source_uri, status, created_at, updated_at, archived_at
    FROM cards
";
const ATTACHMENT_QUERY_BATCH_SIZE: usize = 500;
const DUPLICATE_CONTENT_HASH_SQL: &str = "
    SELECT id
    FROM cards INDEXED BY cards_content_hash_idx
    WHERE content_hash = ?1
";
const DUPLICATE_SOURCE_URI_SQL: &str = "
    SELECT id
    FROM cards INDEXED BY cards_source_uri_idx
    WHERE source_uri = ?1 AND source_uri <> ''
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DuplicateLookup {
    ContentHash,
    SourceUri,
}

impl DuplicateLookup {
    const fn sql(self) -> &'static str {
        match self {
            Self::ContentHash => DUPLICATE_CONTENT_HASH_SQL,
            Self::SourceUri => DUPLICATE_SOURCE_URI_SQL,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCardResult {
    pub card: Card,
    pub duplicate_card_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAttachmentInput {
    pub kind: AttachmentKind,
    pub original_name: String,
    pub relative_path: String,
    pub mime_type: String,
    pub byte_size: u64,
    pub checksum: String,
    pub extraction_status: ExtractionStatus,
    pub extraction_error: String,
    pub extracted_text: String,
}

pub struct CardRepository<'a> {
    database: &'a Database,
}

impl<'a> CardRepository<'a> {
    pub fn new(database: &'a Database) -> Self {
        Self { database }
    }

    pub fn list(&self, include_archived: bool) -> Result<Vec<Card>, AppError> {
        self.database.with_connection(|connection| {
            let sql = format!(
                "{CARD_SELECT}
                 WHERE (?1 = 1 OR status <> ?2)
                 ORDER BY updated_at DESC, id ASC"
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement
                .query_map(
                    params![include_archived, CardStatus::Archived.as_sql()],
                    card_row,
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            let mut cards = rows
                .into_iter()
                .map(build_card)
                .collect::<Result<Vec<_>, _>>()?;
            let card_ids = cards.iter().map(|card| card.id.clone()).collect::<Vec<_>>();
            let mut attachments = load_attachments_for_cards(connection, &card_ids)?;
            for card in &mut cards {
                card.attachments = attachments.remove(&card.id).unwrap_or_default();
            }
            Ok(cards)
        })
    }

    pub fn get(&self, id: &str) -> Result<Card, AppError> {
        self.database.with_connection(|connection| {
            load_card(connection, id)?.ok_or_else(card_not_found_error)
        })
    }

    pub fn create(&self, input: NewCardInput) -> Result<CreateCardResult, AppError> {
        self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let content_hash = content_hash(&input.original_content);
            let duplicate_card_ids =
                duplicate_card_ids(&transaction, &content_hash, &input.source_uri)?;
            let id = Uuid::new_v4().to_string();
            let timestamp = utc_timestamp();
            let title = if input.title.trim().is_empty() {
                fallback_title(&input.original_content)
            } else {
                input.title
            };

            transaction.execute(
                "INSERT INTO cards (
                    id, title, original_content, note, source_type, capture_method,
                    source_name, source_uri, status, content_hash, created_at, updated_at,
                    archived_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, NULL)",
                params![
                    id,
                    title,
                    input.original_content,
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

            let card = load_card(&transaction, &id)?.ok_or_else(card_not_found_error)?;
            transaction.commit()?;
            Ok(CreateCardResult {
                card,
                duplicate_card_ids,
            })
        })
    }

    pub fn create_with_attachment(
        &self,
        input: NewCardInput,
        attachment: NewAttachmentInput,
    ) -> Result<CreateCardResult, AppError> {
        self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let content_hash = content_hash(&input.original_content);
            let duplicate_card_ids =
                duplicate_card_ids(&transaction, &content_hash, &input.source_uri)?;
            let card_id = Uuid::new_v4().to_string();
            let timestamp = utc_timestamp();
            let title = if input.title.trim().is_empty() {
                fallback_title(&input.original_content)
            } else {
                input.title
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
                    input.original_content,
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
            transaction.execute(
                "INSERT INTO attachments (
                    id, card_id, kind, original_name, relative_path, mime_type, byte_size,
                    checksum, extraction_status, extraction_error, extracted_text, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    Uuid::new_v4().to_string(),
                    card_id,
                    attachment.kind.as_sql(),
                    attachment.original_name,
                    attachment.relative_path,
                    attachment.mime_type,
                    i64::try_from(attachment.byte_size)
                        .map_err(|_| database_data_invalid_error())?,
                    attachment.checksum,
                    attachment.extraction_status.as_sql(),
                    attachment.extraction_error,
                    attachment.extracted_text,
                    timestamp,
                ],
            )?;

            let card = load_card(&transaction, &card_id)?.ok_or_else(card_not_found_error)?;
            transaction.commit()?;
            Ok(CreateCardResult {
                card,
                duplicate_card_ids,
            })
        })
    }

    pub fn update(&self, id: &str, patch: CardPatch) -> Result<Card, AppError> {
        if patch_is_empty(&patch) {
            return self.get(id);
        }

        self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let current = load_card(&transaction, id)?.ok_or_else(card_not_found_error)?;
            if patch.original_content.is_some() {
                let has_structured_document = transaction.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM card_documents WHERE card_id = ?1
                     )",
                    params![id],
                    |row| row.get::<_, bool>(0),
                )?;
                if has_structured_document {
                    return Err(structured_document_required_error());
                }
            }
            let CardPatch {
                title,
                original_content,
                note,
                source_type,
                source_name,
                source_uri,
                status: explicit_status,
            } = patch;
            let should_annotate = explicit_status.is_none()
                && current.status == CardStatus::Unprocessed
                && note
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty());
            let status = explicit_status.unwrap_or(if should_annotate {
                CardStatus::Annotated
            } else {
                current.status
            });
            let timestamp = monotonic_utc_timestamp(&current.updated_at, Utc::now())?;
            let archived_at = match explicit_status {
                Some(CardStatus::Archived) => current
                    .archived_at
                    .clone()
                    .or_else(|| Some(timestamp.clone())),
                Some(_) => None,
                None => current.archived_at.clone(),
            };
            let title = title.unwrap_or(current.title);
            let original_content = original_content.unwrap_or(current.original_content);
            let note = note.unwrap_or(current.note);
            let source_type = source_type.unwrap_or(current.source_type);
            let source_name = source_name.unwrap_or(current.source_name);
            let source_uri = source_uri.unwrap_or(current.source_uri);
            let content_hash = content_hash(&original_content);

            transaction.execute(
                "UPDATE cards
                 SET title = ?1,
                     original_content = ?2,
                     note = ?3,
                     source_type = ?4,
                     source_name = ?5,
                     source_uri = ?6,
                     status = ?7,
                     content_hash = ?8,
                     updated_at = ?9,
                     archived_at = ?10
                 WHERE id = ?11",
                params![
                    title,
                    original_content,
                    note,
                    source_type.as_sql(),
                    source_name,
                    source_uri,
                    status.as_sql(),
                    content_hash,
                    timestamp,
                    archived_at,
                    id,
                ],
            )?;

            let card = load_card(&transaction, id)?.ok_or_else(card_not_found_error)?;
            transaction.commit()?;
            Ok(card)
        })
    }

    pub fn restore(&self, id: &str) -> Result<Card, AppError> {
        self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let current = load_card(&transaction, id)?.ok_or_else(card_not_found_error)?;
            if current.status != CardStatus::Archived {
                return Err(card_not_archived_error());
            }
            let status = if current.note.trim().is_empty() {
                CardStatus::Unprocessed
            } else {
                CardStatus::Annotated
            };
            let timestamp = monotonic_utc_timestamp(&current.updated_at, Utc::now())?;
            transaction.execute(
                "UPDATE cards
                 SET status = ?1, updated_at = ?2, archived_at = NULL
                 WHERE id = ?3",
                params![status.as_sql(), timestamp, id],
            )?;
            let card = load_card(&transaction, id)?.ok_or_else(card_not_found_error)?;
            transaction.commit()?;
            Ok(card)
        })
    }

    pub fn delete_archived(&self, id: &str) -> Result<Vec<String>, AppError> {
        self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let current = load_card(&transaction, id)?.ok_or_else(card_not_found_error)?;
            if current.status != CardStatus::Archived {
                return Err(card_not_archived_error());
            }
            let referenced = transaction.query_row(
                "SELECT
                    EXISTS(SELECT 1 FROM analysis_session_cards WHERE card_id = ?1)
                    OR EXISTS(SELECT 1 FROM message_card_citations WHERE card_id = ?1)",
                params![id],
                |row| row.get::<_, bool>(0),
            )?;
            if referenced {
                return Err(card_delete_referenced_error());
            }
            let mut statement = transaction.prepare(
                "SELECT relative_path
                 FROM attachments
                 WHERE card_id = ?1
                 ORDER BY created_at ASC, id ASC",
            )?;
            let paths = statement
                .query_map(params![id], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(statement);
            transaction.execute("DELETE FROM attachments WHERE card_id = ?1", params![id])?;
            transaction.execute("DELETE FROM cards WHERE id = ?1", params![id])?;
            transaction.commit()?;
            Ok(paths)
        })
    }
}

#[derive(Debug)]
struct CardRow {
    id: String,
    title: String,
    original_content: String,
    note: String,
    source_type: String,
    capture_method: String,
    source_name: String,
    source_uri: String,
    status: String,
    created_at: String,
    updated_at: String,
    archived_at: Option<String>,
}

#[derive(Debug)]
struct AttachmentRow {
    id: String,
    card_id: String,
    kind: String,
    original_name: String,
    relative_path: String,
    mime_type: String,
    byte_size: i64,
    extraction_status: String,
    extraction_error: String,
    extracted_text: String,
    created_at: String,
}

fn card_row(row: &Row<'_>) -> rusqlite::Result<CardRow> {
    Ok(CardRow {
        id: row.get(0)?,
        title: row.get(1)?,
        original_content: row.get(2)?,
        note: row.get(3)?,
        source_type: row.get(4)?,
        capture_method: row.get(5)?,
        source_name: row.get(6)?,
        source_uri: row.get(7)?,
        status: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        archived_at: row.get(11)?,
    })
}

fn attachment_row(row: &Row<'_>) -> rusqlite::Result<AttachmentRow> {
    Ok(AttachmentRow {
        id: row.get(0)?,
        card_id: row.get(1)?,
        kind: row.get(2)?,
        original_name: row.get(3)?,
        relative_path: row.get(4)?,
        mime_type: row.get(5)?,
        byte_size: row.get(6)?,
        extraction_status: row.get(7)?,
        extraction_error: row.get(8)?,
        extracted_text: row.get(9)?,
        created_at: row.get(10)?,
    })
}

fn load_card(connection: &Connection, id: &str) -> Result<Option<Card>, AppError> {
    let sql = format!("{CARD_SELECT} WHERE id = ?1");
    let row = connection
        .query_row(&sql, params![id], card_row)
        .optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    let mut card = build_card(row)?;
    let mut attachments = load_attachments_for_cards(connection, std::slice::from_ref(&card.id))?;
    card.attachments = attachments.remove(&card.id).unwrap_or_default();
    Ok(Some(card))
}

fn build_card(row: CardRow) -> Result<Card, AppError> {
    Ok(Card {
        id: row.id,
        title: row.title,
        original_content: row.original_content,
        note: row.note,
        source_type: SourceType::from_sql(&row.source_type)
            .ok_or_else(database_data_invalid_error)?,
        capture_method: CaptureMethod::from_sql(&row.capture_method)
            .ok_or_else(database_data_invalid_error)?,
        source_name: row.source_name,
        source_uri: row.source_uri,
        status: CardStatus::from_sql(&row.status).ok_or_else(database_data_invalid_error)?,
        attachments: Vec::new(),
        created_at: row.created_at,
        updated_at: row.updated_at,
        archived_at: row.archived_at,
    })
}

fn load_attachments_for_cards(
    connection: &Connection,
    card_ids: &[String],
) -> Result<HashMap<String, Vec<Attachment>>, AppError> {
    let mut attachments_by_card = HashMap::<String, Vec<Attachment>>::new();
    for batch in attachment_query_batches(card_ids) {
        let placeholders = (1..=batch.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, card_id, kind, original_name, relative_path, mime_type, byte_size,
                    extraction_status, extraction_error, extracted_text, created_at
             FROM attachments
             WHERE card_id IN ({placeholders})
             ORDER BY card_id ASC, created_at ASC, id ASC"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement
            .query_map(params_from_iter(batch.iter()), attachment_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for row in rows {
            let attachment = build_attachment(row)?;
            attachments_by_card
                .entry(attachment.card_id.clone())
                .or_default()
                .push(attachment);
        }
    }
    Ok(attachments_by_card)
}

fn build_attachment(row: AttachmentRow) -> Result<Attachment, AppError> {
    Ok(Attachment {
        id: row.id,
        card_id: row.card_id,
        kind: AttachmentKind::from_sql(&row.kind).ok_or_else(database_data_invalid_error)?,
        original_name: row.original_name,
        relative_path: row.relative_path,
        mime_type: row.mime_type,
        byte_size: u64::try_from(row.byte_size).map_err(|_| database_data_invalid_error())?,
        extraction_status: ExtractionStatus::from_sql(&row.extraction_status)
            .ok_or_else(database_data_invalid_error)?,
        extraction_error: row.extraction_error,
        extracted_text: row.extracted_text,
        created_at: row.created_at,
    })
}

fn attachment_query_batches(card_ids: &[String]) -> Chunks<'_, String> {
    card_ids.chunks(ATTACHMENT_QUERY_BATCH_SIZE)
}

fn duplicate_card_ids(
    connection: &Connection,
    content_hash: &str,
    source_uri: &str,
) -> Result<Vec<String>, AppError> {
    let mut ids = BTreeSet::new();
    for lookup in duplicate_lookups(source_uri) {
        let value = match lookup {
            DuplicateLookup::ContentHash => content_hash,
            DuplicateLookup::SourceUri => source_uri,
        };
        let mut statement = connection.prepare(lookup.sql())?;
        let matches = statement
            .query_map(params![value], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.extend(matches);
    }
    Ok(ids.into_iter().collect())
}

fn duplicate_lookups(source_uri: &str) -> Vec<DuplicateLookup> {
    let mut lookups = vec![DuplicateLookup::ContentHash];
    if !source_uri.is_empty() {
        lookups.push(DuplicateLookup::SourceUri);
    }
    lookups
}

fn content_hash(original_content: &str) -> String {
    hex::encode(Sha256::digest(original_content.trim().as_bytes()))
}

fn structured_document_required_error() -> AppError {
    AppError::new(
        "structured_document_required",
        "结构化正文不能通过旧接口修改",
        "刷新卡片后使用正文编辑器保存",
    )
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

fn patch_is_empty(patch: &CardPatch) -> bool {
    patch.title.is_none()
        && patch.original_content.is_none()
        && patch.note.is_none()
        && patch.source_type.is_none()
        && patch.source_name.is_none()
        && patch.source_uri.is_none()
        && patch.status.is_none()
}

fn card_not_found_error() -> AppError {
    AppError::new(
        "card_not_found",
        "找不到这张卡片",
        "返回收集箱后刷新卡片列表",
    )
}

fn card_not_archived_error() -> AppError {
    AppError::new(
        "card_not_archived",
        "只能删除已归档卡片",
        "先将卡片归档后再删除",
    )
}

fn card_delete_referenced_error() -> AppError {
    AppError::new(
        "card_delete_referenced",
        "这张卡片仍被分析内容引用",
        "保留归档卡片，或先处理相关分析内容",
    )
}

fn database_data_invalid_error() -> AppError {
    AppError::new(
        "database_data_invalid",
        "数据库中的卡片数据无效",
        "重新打开应用；问题持续时检查数据目录",
    )
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::{Arc, Barrier},
        thread,
    };

    use chrono::{DateTime, Utc};
    use rusqlite::{params, ErrorCode};

    use super::{
        attachment_query_batches, duplicate_lookups, monotonic_utc_timestamp, CardRepository,
        DuplicateLookup,
    };
    use crate::{
        database::Database,
        domain::card::{
            Attachment, AttachmentKind, CaptureMethod, CardPatch, CardStatus, ExtractionStatus,
            NewCardInput, SourceType,
        },
    };

    fn new_card(original_content: &str) -> NewCardInput {
        NewCardInput {
            title: String::new(),
            original_content: original_content.to_owned(),
            note: String::new(),
            source_type: SourceType::Personal,
            capture_method: CaptureMethod::Quick,
            source_name: String::new(),
            source_uri: String::new(),
        }
    }

    #[test]
    fn creates_and_lists_a_quick_card() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let input = new_card(" 第一行\n第二行 ");

        let result = repository.create(input.clone()).unwrap();
        let cards = repository.list(false).unwrap();

        assert_eq!(result.card.title, "第一行");
        assert_eq!(result.card.original_content, input.original_content);
        assert_eq!(result.card.source_type, SourceType::Personal);
        assert_eq!(result.card.status, CardStatus::Unprocessed);
        assert!(result.card.attachments.is_empty());
        assert!(result.duplicate_card_ids.is_empty());
        assert_eq!(cards, vec![result.card.clone()]);

        let created_at = DateTime::parse_from_rfc3339(&result.card.created_at).unwrap();
        let updated_at = DateTime::parse_from_rfc3339(&result.card.updated_at).unwrap();
        assert_eq!(created_at.offset().local_minus_utc(), 0);
        assert_eq!(updated_at.offset().local_minus_utc(), 0);
    }

    #[test]
    fn note_changes_unprocessed_card_to_annotated() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let created = repository.create(new_card("正文")).unwrap().card;

        let updated = repository
            .update(
                &created.id,
                CardPatch {
                    note: Some("新的想法".to_owned()),
                    ..CardPatch::default()
                },
            )
            .unwrap();

        assert_eq!(updated.note, "新的想法");
        assert_eq!(updated.status, CardStatus::Annotated);
        assert!(updated.updated_at > created.updated_at);
    }

    #[test]
    fn rejects_legacy_original_content_updates_for_a_structured_document() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let created = repository.create(new_card("原始正文")).unwrap().card;
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO card_documents (
                        card_id, document_json, schema_version, updated_at
                     ) VALUES (?1, ?2, 1, ?3)",
                    params![
                        created.id,
                        r#"{"type":"doc","content":[{"type":"paragraph"}]}"#,
                        created.updated_at
                    ],
                )?;
                Ok(())
            })
            .unwrap();

        let error = repository
            .update(
                &created.id,
                CardPatch {
                    original_content: Some("绕过编辑器".to_owned()),
                    ..CardPatch::default()
                },
            )
            .unwrap_err();

        assert_eq!(error.code, "structured_document_required");
        assert_eq!(
            repository.get(&created.id).unwrap().original_content,
            "原始正文"
        );
    }

    #[test]
    fn same_clock_tick_still_produces_a_strictly_newer_timestamp() {
        let previous = "2026-07-14T00:00:00.000000123Z";
        let candidate = DateTime::parse_from_rfc3339(previous)
            .unwrap()
            .with_timezone(&Utc);

        let timestamp = monotonic_utc_timestamp(previous, candidate).unwrap();

        assert_eq!(timestamp, "2026-07-14T00:00:00.000000124Z");
    }

    #[test]
    fn duplicate_content_is_reported_but_second_card_can_be_saved() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let first = repository.create(new_card("重复正文")).unwrap();

        let second = repository.create(new_card("  重复正文  ")).unwrap();

        assert_ne!(second.card.id, first.card.id);
        assert_eq!(second.duplicate_card_ids, vec![first.card.id]);
        assert_eq!(repository.list(false).unwrap().len(), 2);
    }

    #[test]
    fn invalid_source_value_is_rejected_by_the_database() {
        let database = Database::open_in_memory().unwrap();

        database
            .with_connection(|connection| {
                let error = connection
                    .execute(
                        "INSERT INTO cards (
                            id, title, original_content, source_type, capture_method, status,
                            content_hash, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                        params![
                            "card-invalid",
                            "标题",
                            "正文",
                            "forum",
                            "quick",
                            "unprocessed",
                            "hash",
                            "2026-07-14T00:00:00Z"
                        ],
                    )
                    .unwrap_err();
                match error {
                    rusqlite::Error::SqliteFailure(details, _) => {
                        assert_eq!(details.code, ErrorCode::ConstraintViolation);
                    }
                    other => panic!("expected constraint violation, got {other:?}"),
                }
                let count = connection
                    .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get::<_, i64>(0))?;
                assert_eq!(count, 0);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn duplicate_source_uri_is_reported_but_second_card_can_be_saved() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let mut first_input = new_card("第一篇正文");
        first_input.source_uri = "https://example.com/article".to_owned();
        let first = repository.create(first_input).unwrap();
        let mut second_input = new_card("完全不同的正文");
        second_input.source_uri = "https://example.com/article".to_owned();

        let second = repository.create(second_input).unwrap();

        assert_ne!(second.card.id, first.card.id);
        assert_eq!(second.duplicate_card_ids, vec![first.card.id]);
    }

    #[test]
    fn duplicate_matches_are_deduplicated_and_sorted_by_id() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let mut both_input = new_card("共享正文");
        both_input.source_uri = "https://example.com/shared".to_owned();
        let both = repository.create(both_input).unwrap().card;
        let mut uri_input = new_card("不同正文");
        uri_input.source_uri = "https://example.com/shared".to_owned();
        let uri = repository.create(uri_input).unwrap().card;
        let mut hash_input = new_card("共享正文");
        hash_input.source_uri = "https://example.com/other".to_owned();
        let hash = repository.create(hash_input).unwrap().card;
        let mut target_input = new_card(" 共享正文 ");
        target_input.source_uri = "https://example.com/shared".to_owned();
        let mut expected = vec![both.id, uri.id, hash.id];
        expected.sort();

        let result = repository.create(target_input).unwrap();

        assert_eq!(result.duplicate_card_ids, expected);
    }

    #[test]
    fn duplicate_lookup_plans_use_each_index_without_scanning_cards() {
        let database = Database::open_in_memory().unwrap();

        database
            .with_connection(|connection| {
                let nonempty_lookups = duplicate_lookups("https://example.com/article");
                assert_eq!(
                    nonempty_lookups,
                    vec![DuplicateLookup::ContentHash, DuplicateLookup::SourceUri]
                );
                let nonempty_plans = nonempty_lookups
                    .into_iter()
                    .map(|lookup| {
                        let mut statement =
                            connection.prepare(&format!("EXPLAIN QUERY PLAN {}", lookup.sql()))?;
                        let details = statement
                            .query_map(params!["lookup-value"], |row| row.get::<_, String>(3))?
                            .collect::<rusqlite::Result<Vec<_>>>()?;
                        Ok((lookup, details))
                    })
                    .collect::<Result<Vec<_>, crate::app_error::AppError>>()?;

                assert!(nonempty_plans[0]
                    .1
                    .iter()
                    .any(|detail| detail.contains("cards_content_hash_idx")));
                assert!(nonempty_plans[1]
                    .1
                    .iter()
                    .any(|detail| detail.contains("cards_source_uri_idx")));
                assert!(nonempty_plans
                    .iter()
                    .flat_map(|(_, details)| details)
                    .all(|detail| !detail.contains("SCAN cards")));

                let empty_lookups = duplicate_lookups("");
                assert_eq!(empty_lookups, vec![DuplicateLookup::ContentHash]);
                let mut statement = connection
                    .prepare(&format!("EXPLAIN QUERY PLAN {}", empty_lookups[0].sql()))?;
                let empty_plan = statement
                    .query_map(params!["lookup-value"], |row| row.get::<_, String>(3))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                assert!(empty_plan
                    .iter()
                    .any(|detail| detail.contains("cards_content_hash_idx")));
                assert!(empty_plan
                    .iter()
                    .all(|detail| !detail.contains("cards_source_uri_idx")));
                assert!(empty_plan
                    .iter()
                    .all(|detail| !detail.contains("SCAN cards")));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn list_filters_archived_cards_unless_requested() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let created = repository.create(new_card("待归档")).unwrap().card;
        let archived = repository
            .update(
                &created.id,
                CardPatch {
                    status: Some(CardStatus::Archived),
                    ..CardPatch::default()
                },
            )
            .unwrap();

        assert!(archived.archived_at.is_some());
        assert!(repository.list(false).unwrap().is_empty());
        assert_eq!(repository.list(true).unwrap(), vec![archived]);
    }

    #[test]
    fn restore_archived_card_derives_status_from_note() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let empty_note = repository.create(new_card("空想法")).unwrap().card;
        let with_note = repository.create(new_card("有想法")).unwrap().card;
        let empty_note = repository
            .update(
                &empty_note.id,
                CardPatch {
                    status: Some(CardStatus::Archived),
                    ..CardPatch::default()
                },
            )
            .unwrap();
        let with_note = repository
            .update(
                &with_note.id,
                CardPatch {
                    note: Some("已经整理".to_owned()),
                    status: Some(CardStatus::Archived),
                    ..CardPatch::default()
                },
            )
            .unwrap();

        let restored_empty = repository.restore(&empty_note.id).unwrap();
        let restored_with_note = repository.restore(&with_note.id).unwrap();

        assert_eq!(restored_empty.status, CardStatus::Unprocessed);
        assert_eq!(restored_with_note.status, CardStatus::Annotated);
        assert!(restored_empty.archived_at.is_none());
        assert!(restored_with_note.archived_at.is_none());
    }

    #[test]
    fn delete_rejects_unarchived_card() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let card = repository.create(new_card("不能删除")).unwrap().card;

        let error = repository.delete_archived(&card.id).unwrap_err();

        assert_eq!(error.code, "card_not_archived");
        assert_eq!(repository.get(&card.id).unwrap(), card);
    }

    #[test]
    fn delete_rejects_cards_referenced_by_analysis() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let card = repository.create(new_card("被引用")).unwrap().card;
        let archived = repository
            .update(
                &card.id,
                CardPatch {
                    status: Some(CardStatus::Archived),
                    ..CardPatch::default()
                },
            )
            .unwrap();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO analysis_sessions (
                        id, title, question, web_search_enabled, status, created_at, updated_at
                     ) VALUES ('analysis-1', '会话', '问题', 0, 'draft', ?1, ?1)",
                    params!["2026-07-14T00:00:00Z"],
                )?;
                connection.execute(
                    "INSERT INTO analysis_session_cards (
                        analysis_session_id, card_id, position
                     ) VALUES ('analysis-1', ?1, 0)",
                    params![archived.id],
                )?;
                Ok(())
            })
            .unwrap();

        let error = repository.delete_archived(&archived.id).unwrap_err();

        assert_eq!(error.code, "card_delete_referenced");
        assert_eq!(repository.get(&archived.id).unwrap(), archived);
    }

    #[test]
    fn delete_removes_attachment_rows_and_card_in_one_transaction() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let card = repository.create(new_card("带附件")).unwrap().card;
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO attachments (
                        id, card_id, kind, original_name, relative_path, mime_type, byte_size,
                        checksum, extraction_status, extraction_error, extracted_text, created_at
                     ) VALUES (
                        'attachment-delete', ?1, 'document', '资料.pdf', 'attachment-delete.pdf',
                        'application/pdf', 12, 'checksum', 'completed', '', '正文', ?2
                     )",
                    params![card.id, "2026-07-14T00:00:00Z"],
                )?;
                Ok(())
            })
            .unwrap();
        let archived = repository
            .update(
                &card.id,
                CardPatch {
                    status: Some(CardStatus::Archived),
                    ..CardPatch::default()
                },
            )
            .unwrap();

        let paths = repository.delete_archived(&archived.id).unwrap();

        assert_eq!(paths, vec!["attachment-delete.pdf"]);
        assert_eq!(
            repository.get(&archived.id).unwrap_err().code,
            "card_not_found"
        );
        database
            .with_connection(|connection| {
                let count = connection.query_row(
                    "SELECT COUNT(*) FROM attachments WHERE card_id = ?1",
                    params![archived.id],
                    |row| row.get::<_, i64>(0),
                )?;
                assert_eq!(count, 0);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn list_sorts_by_updated_at_descending_then_id_ascending() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let oldest = repository.create(new_card("旧卡片")).unwrap().card;
        let tied_a = repository.create(new_card("并列 A")).unwrap().card;
        let tied_b = repository.create(new_card("并列 B")).unwrap().card;
        database
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE cards SET updated_at = ?1 WHERE id = ?2",
                    params!["2026-07-14T00:00:00.000000000Z", oldest.id],
                )?;
                connection.execute(
                    "UPDATE cards SET updated_at = ?1 WHERE id IN (?2, ?3)",
                    params!["2026-07-14T01:00:00.000000000Z", tied_a.id, tied_b.id],
                )?;
                Ok(())
            })
            .unwrap();
        let mut tied_ids = [tied_a.id, tied_b.id];
        tied_ids.sort();

        let ids = repository
            .list(false)
            .unwrap()
            .into_iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![tied_ids[0].clone(), tied_ids[1].clone(), oldest.id]
        );
    }

    #[test]
    fn get_returns_card_not_found_for_unknown_id() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);

        let error = repository.get("missing-card").unwrap_err();

        assert_eq!(error.code, "card_not_found");
    }

    #[test]
    fn empty_patch_returns_the_existing_card_without_touching_updated_at() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let created = repository.create(new_card("正文")).unwrap().card;

        let updated = repository
            .update(&created.id, CardPatch::default())
            .unwrap();

        assert_eq!(updated, created);
    }

    #[test]
    fn explicit_status_takes_priority_over_note_annotation() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let created = repository.create(new_card("正文")).unwrap().card;

        let updated = repository
            .update(
                &created.id,
                CardPatch {
                    note: Some("新的想法".to_owned()),
                    status: Some(CardStatus::PendingAnalysis),
                    ..CardPatch::default()
                },
            )
            .unwrap();

        assert_eq!(updated.status, CardStatus::PendingAnalysis);
    }

    #[test]
    fn note_edit_does_not_regress_other_statuses() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let created = repository.create(new_card("正文")).unwrap().card;
        let pending = repository
            .update(
                &created.id,
                CardPatch {
                    status: Some(CardStatus::PendingAnalysis),
                    ..CardPatch::default()
                },
            )
            .unwrap();

        let updated = repository
            .update(
                &pending.id,
                CardPatch {
                    note: Some("补充想法".to_owned()),
                    ..CardPatch::default()
                },
            )
            .unwrap();

        assert_eq!(updated.status, CardStatus::PendingAnalysis);
    }

    #[test]
    fn update_supports_every_card_patch_field() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let created = repository.create(new_card("正文")).unwrap().card;

        let updated = repository
            .update(
                &created.id,
                CardPatch {
                    title: Some("新标题".to_owned()),
                    original_content: Some("新正文".to_owned()),
                    note: Some("新想法".to_owned()),
                    source_type: Some(SourceType::Web),
                    source_name: Some("示例网站".to_owned()),
                    source_uri: Some("https://example.com/new".to_owned()),
                    status: Some(CardStatus::Concluded),
                },
            )
            .unwrap();

        assert_eq!(updated.title, "新标题");
        assert_eq!(updated.original_content, "新正文");
        assert_eq!(updated.note, "新想法");
        assert_eq!(updated.source_type, SourceType::Web);
        assert_eq!(updated.capture_method, CaptureMethod::Quick);
        assert_eq!(updated.source_name, "示例网站");
        assert_eq!(updated.source_uri, "https://example.com/new");
        assert_eq!(updated.status, CardStatus::Concluded);

        let duplicate = repository.create(new_card("  新正文 ")).unwrap();
        assert_eq!(duplicate.duplicate_card_ids, vec![created.id]);
    }

    #[test]
    fn get_and_list_load_all_attachment_fields_in_stable_order() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        let created = repository.create(new_card("附件正文")).unwrap().card;
        let another = repository.create(new_card("另一张附件正文")).unwrap().card;
        database
            .with_connection(|connection| {
                let rows = [
                    (
                        "attachment-b",
                        created.id.as_str(),
                        "image",
                        "后图.png",
                        "attachments/attachment-b.png",
                        "image/png",
                        2048_i64,
                        "checksum-b",
                        "failed",
                        "OCR 失败",
                        "",
                        "2026-07-14T01:00:00Z",
                    ),
                    (
                        "attachment-c",
                        created.id.as_str(),
                        "document",
                        "先文.pdf",
                        "attachments/attachment-c.pdf",
                        "application/pdf",
                        1024_i64,
                        "checksum-c",
                        "completed",
                        "",
                        "提取正文",
                        "2026-07-14T00:00:00Z",
                    ),
                    (
                        "attachment-a",
                        created.id.as_str(),
                        "audio",
                        "前音.m4a",
                        "attachments/attachment-a.m4a",
                        "audio/mp4",
                        4096_i64,
                        "checksum-a",
                        "not_applicable",
                        "",
                        "",
                        "2026-07-14T01:00:00Z",
                    ),
                    (
                        "attachment-other",
                        another.id.as_str(),
                        "video",
                        "视频.mp4",
                        "attachments/attachment-other.mp4",
                        "video/mp4",
                        8192_i64,
                        "checksum-other",
                        "pending",
                        "",
                        "",
                        "2026-07-14T00:30:00Z",
                    ),
                ];
                for row in rows {
                    connection.execute(
                        "INSERT INTO attachments (
                            id, card_id, kind, original_name, relative_path, mime_type, byte_size,
                            checksum, extraction_status, extraction_error, extracted_text, created_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                        params![
                            row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8,
                            row.9, row.10, row.11
                        ],
                    )?;
                }
                Ok(())
            })
            .unwrap();

        let from_get = repository.get(&created.id).unwrap();
        let listed = repository.list(false).unwrap();
        let from_list = listed.iter().find(|card| card.id == created.id).unwrap();
        let other_from_list = listed.iter().find(|card| card.id == another.id).unwrap();
        let expected = vec![
            Attachment {
                id: "attachment-c".to_owned(),
                card_id: created.id.clone(),
                kind: AttachmentKind::Document,
                original_name: "先文.pdf".to_owned(),
                relative_path: "attachments/attachment-c.pdf".to_owned(),
                mime_type: "application/pdf".to_owned(),
                byte_size: 1024,
                extraction_status: ExtractionStatus::Completed,
                extraction_error: String::new(),
                extracted_text: "提取正文".to_owned(),
                created_at: "2026-07-14T00:00:00Z".to_owned(),
            },
            Attachment {
                id: "attachment-a".to_owned(),
                card_id: created.id.clone(),
                kind: AttachmentKind::Audio,
                original_name: "前音.m4a".to_owned(),
                relative_path: "attachments/attachment-a.m4a".to_owned(),
                mime_type: "audio/mp4".to_owned(),
                byte_size: 4096,
                extraction_status: ExtractionStatus::NotApplicable,
                extraction_error: String::new(),
                extracted_text: String::new(),
                created_at: "2026-07-14T01:00:00Z".to_owned(),
            },
            Attachment {
                id: "attachment-b".to_owned(),
                card_id: created.id.clone(),
                kind: AttachmentKind::Image,
                original_name: "后图.png".to_owned(),
                relative_path: "attachments/attachment-b.png".to_owned(),
                mime_type: "image/png".to_owned(),
                byte_size: 2048,
                extraction_status: ExtractionStatus::Failed,
                extraction_error: "OCR 失败".to_owned(),
                extracted_text: String::new(),
                created_at: "2026-07-14T01:00:00Z".to_owned(),
            },
        ];

        assert_eq!(from_get.attachments, expected);
        assert_eq!(from_list.attachments, expected);
        assert_eq!(other_from_list.attachments.len(), 1);
        assert_eq!(other_from_list.attachments[0].id, "attachment-other");
    }

    #[test]
    fn attachment_query_batches_are_bounded_instead_of_per_card() {
        let three_ids = (0..3)
            .map(|index| format!("card-{index}"))
            .collect::<Vec<_>>();
        assert_eq!(
            attachment_query_batches(&three_ids)
                .map(<[String]>::len)
                .collect::<Vec<_>>(),
            vec![3]
        );

        let many_ids = (0..1001)
            .map(|index| format!("card-{index}"))
            .collect::<Vec<_>>();
        assert_eq!(
            attachment_query_batches(&many_ids)
                .map(<[String]>::len)
                .collect::<Vec<_>>(),
            vec![500, 500, 1]
        );
    }

    #[test]
    fn concurrent_creates_with_an_arc_database_are_serialized_safely() {
        let database = Arc::new(Database::open_in_memory().unwrap());
        let barrier = Arc::new(Barrier::new(8));
        let handles = (0..8)
            .map(|_| {
                let database = Arc::clone(&database);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    CardRepository::new(database.as_ref())
                        .create(new_card("并发重复正文"))
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();

        let ids = results
            .iter()
            .map(|result| result.card.id.clone())
            .collect::<HashSet<_>>();
        let mut duplicate_counts = results
            .iter()
            .map(|result| result.duplicate_card_ids.len())
            .collect::<Vec<_>>();
        duplicate_counts.sort_unstable();

        assert_eq!(ids.len(), 8);
        assert_eq!(duplicate_counts, (0..8).collect::<Vec<_>>());
        assert_eq!(
            CardRepository::new(database.as_ref())
                .list(false)
                .unwrap()
                .len(),
            8
        );
    }

    #[test]
    fn create_rolls_back_when_post_insert_loading_fails() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                connection.execute_batch(
                    "PRAGMA ignore_check_constraints = ON;
                     CREATE TRIGGER add_invalid_attachment_after_card
                     AFTER INSERT ON cards
                     BEGIN
                       INSERT INTO attachments (
                         id, card_id, kind, original_name, relative_path, mime_type, byte_size,
                         checksum, extraction_status, created_at
                       ) VALUES (
                         'invalid-attachment', NEW.id, 'invalid_kind', 'bad',
                         'attachments/bad', 'application/octet-stream', 1, 'checksum',
                         'completed', NEW.created_at
                       );
                     END;",
                )?;
                Ok(())
            })
            .unwrap();
        let repository = CardRepository::new(&database);

        let error = repository.create(new_card("事务正文")).unwrap_err();

        assert_eq!(error.code, "database_data_invalid");
        database
            .with_connection(|connection| {
                let card_count = connection
                    .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get::<_, i64>(0))?;
                let attachment_count =
                    connection.query_row("SELECT COUNT(*) FROM attachments", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                assert_eq!((card_count, attachment_count), (0, 0));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn malformed_sql_enum_returns_sanitized_database_data_invalid() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                connection.execute_batch("PRAGMA ignore_check_constraints = ON;")?;
                connection.execute(
                    "INSERT INTO cards (
                        id, title, original_content, source_type, capture_method, status,
                        content_hash, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                    params![
                        "malformed-card",
                        "标题",
                        "正文",
                        "private-sentinel-source",
                        "quick",
                        "unprocessed",
                        "hash",
                        "2026-07-14T00:00:00Z"
                    ],
                )?;
                Ok(())
            })
            .unwrap();
        let repository = CardRepository::new(&database);

        let error = repository.get("malformed-card").unwrap_err();

        assert_eq!(error.code, "database_data_invalid");
        assert!(!format!("{error:?}").contains("private-sentinel-source"));
    }
}
