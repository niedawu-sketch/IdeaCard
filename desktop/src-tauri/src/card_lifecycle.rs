use rusqlite::{params, OptionalExtension};

use crate::{
    app_error::AppError,
    database::Database,
    domain::card::{Card, CardPatch, CardStatus},
    repository::{attachment_repository::AttachmentRepository, card_repository::CardRepository},
};

pub struct CardLifecycleService<'a> {
    database: &'a Database,
    attachments: &'a AttachmentRepository,
}

impl<'a> CardLifecycleService<'a> {
    pub fn new(database: &'a Database, attachments: &'a AttachmentRepository) -> Self {
        Self {
            database,
            attachments,
        }
    }

    pub fn archive(&self, id: &str) -> Result<Card, AppError> {
        CardRepository::new(self.database).update(
            id,
            CardPatch {
                status: Some(CardStatus::Archived),
                ..CardPatch::default()
            },
        )
    }

    pub fn restore(&self, id: &str) -> Result<Card, AppError> {
        CardRepository::new(self.database).restore(id)
    }

    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        let repository = CardRepository::new(self.database);
        let card = repository.get(id)?;
        let paths = card
            .attachments
            .iter()
            .map(|attachment| attachment.relative_path.clone())
            .collect::<Vec<_>>();
        let mut staged = self.attachments.stage_removal(&paths)?;
        let result = self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction
                .query_row("SELECT 1 FROM cards WHERE id = ?1", params![id], |row| {
                    row.get::<_, i64>(0)
                })
                .optional()?
                .ok_or_else(card_not_found_error)?;
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

            transaction.execute(
                "DELETE FROM card_ai_citations
                 WHERE message_id IN (
                    SELECT messages.id
                    FROM card_ai_messages AS messages
                    JOIN card_ai_threads AS threads ON threads.id = messages.thread_id
                    WHERE threads.card_id = ?1
                 )",
                params![id],
            )?;
            transaction.execute(
                "DELETE FROM card_ai_messages
                 WHERE thread_id IN (
                    SELECT id FROM card_ai_threads WHERE card_id = ?1
                 )",
                params![id],
            )?;
            transaction.execute(
                "DELETE FROM card_ai_threads WHERE card_id = ?1",
                params![id],
            )?;
            transaction.execute(
                "DELETE FROM concept_mentions WHERE card_id = ?1",
                params![id],
            )?;
            transaction.execute(
                "DELETE FROM card_document_images WHERE card_id = ?1",
                params![id],
            )?;
            transaction.execute("DELETE FROM card_documents WHERE card_id = ?1", params![id])?;
            transaction.execute("DELETE FROM attachments WHERE card_id = ?1", params![id])?;
            transaction.execute("DELETE FROM cards WHERE id = ?1", params![id])?;
            transaction.commit()?;
            Ok(())
        });
        match result {
            Ok(_) => {
                staged.commit();
                Ok(())
            }
            Err(error) => {
                staged.rollback()?;
                Err(error)
            }
        }
    }
}

fn card_not_found_error() -> AppError {
    AppError::new(
        "card_not_found",
        "找不到这张卡片",
        "返回收集箱后刷新卡片列表",
    )
}

fn card_delete_referenced_error() -> AppError {
    AppError::new(
        "card_delete_referenced",
        "这张卡片仍被分析内容引用",
        "保留这张卡片，或先处理相关分析内容",
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::params;
    use tempfile::tempdir;

    use super::CardLifecycleService;
    use crate::{
        database::Database,
        domain::card::{CaptureMethod, CardPatch, CardStatus, NewCardInput, SourceType},
        repository::{
            attachment_repository::AttachmentRepository,
            card_repository::CardRepository,
            document_repository::{CreateRichCardInput, DocumentRepository},
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

    fn insert_attachment(database: &Database, card_id: &str, relative_path: &str) {
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO attachments (
                        id, card_id, kind, original_name, relative_path, mime_type, byte_size,
                        checksum, extraction_status, extraction_error, extracted_text, created_at
                     ) VALUES (
                        'attachment-lifecycle', ?1, 'document', '资料.txt', ?2,
                        'text/plain', 7, 'checksum', 'completed', '', 'content', ?3
                     )",
                    params![card_id, relative_path, "2026-07-14T00:00:00Z"],
                )?;
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn archive_and_restore_delegate_to_the_repository_rules() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachments = AttachmentRepository::new(
            directory.path().join("attachments"),
            directory.path().join("cache"),
        );
        let card = CardRepository::new(&database)
            .create(new_card("生命周期"))
            .unwrap()
            .card;
        let service = CardLifecycleService::new(&database, &attachments);

        let archived = service.archive(&card.id).unwrap();
        let restored = service.restore(&archived.id).unwrap();

        assert_eq!(archived.status, CardStatus::Archived);
        assert!(archived.archived_at.is_some());
        assert_eq!(restored.status, CardStatus::Unprocessed);
        assert!(restored.archived_at.is_none());
    }

    #[test]
    fn delete_restores_staged_files_when_database_delete_fails() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachment_root = directory.path().join("attachments");
        let attachments =
            AttachmentRepository::new(&attachment_root, directory.path().join("cache"));
        fs::create_dir_all(&attachment_root).unwrap();
        fs::write(attachment_root.join("card-file.txt"), b"content").unwrap();
        let repository = CardRepository::new(&database);
        let card = repository.create(new_card("被引用")).unwrap().card;
        insert_attachment(&database, &card.id, "card-file.txt");
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
                     ) VALUES ('analysis-lifecycle', '会话', '问题', 0, 'draft', ?1, ?1)",
                    params!["2026-07-14T00:00:00Z"],
                )?;
                connection.execute(
                    "INSERT INTO analysis_session_cards (
                        analysis_session_id, card_id, position
                     ) VALUES ('analysis-lifecycle', ?1, 0)",
                    params![archived.id],
                )?;
                Ok(())
            })
            .unwrap();
        let service = CardLifecycleService::new(&database, &attachments);

        let error = service.delete(&archived.id).unwrap_err();

        assert_eq!(error.code, "card_delete_referenced");
        assert_eq!(
            fs::read(attachment_root.join("card-file.txt")).unwrap(),
            b"content"
        );
        assert!(repository.get(&archived.id).is_ok());
    }

    #[test]
    fn delete_removes_an_active_card_and_its_attachment_file() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachment_root = directory.path().join("attachments");
        let attachments =
            AttachmentRepository::new(&attachment_root, directory.path().join("cache"));
        fs::create_dir_all(&attachment_root).unwrap();
        fs::write(attachment_root.join("card-file.txt"), b"content").unwrap();
        let repository = CardRepository::new(&database);
        let card = repository.create(new_card("永久删除")).unwrap().card;
        insert_attachment(&database, &card.id, "card-file.txt");
        let service = CardLifecycleService::new(&database, &attachments);

        service.delete(&card.id).unwrap();

        assert!(!attachment_root.join("card-file.txt").exists());
        assert_eq!(repository.get(&card.id).unwrap_err().code, "card_not_found");
    }

    #[test]
    fn delete_cleans_rich_document_concept_mentions_and_ai_rows_but_keeps_concept() {
        let directory = tempdir().unwrap();
        let database = Database::open_in_memory().unwrap();
        let attachment_root = directory.path().join("attachments");
        let attachments =
            AttachmentRepository::new(&attachment_root, directory.path().join("cache"));
        let created = DocumentRepository::new(&database, &attachments)
            .create_rich_card(CreateRichCardInput {
                title: String::new(),
                note: String::new(),
                source_type: SourceType::Personal,
                capture_method: CaptureMethod::Clipboard,
                source_name: String::new(),
                source_uri: String::new(),
                document: serde_json::json!({
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
            .unwrap()
            .card;
        let concept_id = "8ee5e2b0-6f71-4c31-9d47-34c6974f8c01";
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO concepts (
                        id, name, normalized_name, explanation, created_at, updated_at
                     ) VALUES (?1, '概念', '概念', '解释', ?2, ?2)",
                    params![concept_id, "2026-07-19T00:00:00Z"],
                )?;
                connection.execute(
                    "INSERT INTO concept_mentions (
                        id, concept_id, card_id, selected_text, created_at
                     ) VALUES (?1, ?2, ?3, '概念', ?4)",
                    params![
                        "491261f4-7a25-4987-a9ac-7f12e56e9fc3",
                        concept_id,
                        created.id,
                        "2026-07-19T00:00:00Z"
                    ],
                )?;
                connection.execute(
                    "INSERT INTO card_ai_threads (id, card_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?3)",
                    params!["thread-delete", created.id, "2026-07-19T00:00:00Z"],
                )?;
                connection.execute(
                    "INSERT INTO card_ai_messages (id, thread_id, role, content, created_at)
                     VALUES (?1, ?2, 'assistant', '回答', ?3)",
                    params!["message-delete", "thread-delete", "2026-07-19T00:00:00Z"],
                )?;
                connection.execute(
                    "INSERT INTO card_ai_citations (message_id, url, title, position)
                     VALUES (?1, 'https://example.com', '来源', 0)",
                    params!["message-delete"],
                )?;
                Ok(())
            })
            .unwrap();
        let archived = CardRepository::new(&database)
            .update(
                &created.id,
                CardPatch {
                    status: Some(CardStatus::Archived),
                    ..CardPatch::default()
                },
            )
            .unwrap();
        let file_path = attachment_root.join(&archived.attachments[0].relative_path);
        assert!(file_path.is_file());

        CardLifecycleService::new(&database, &attachments)
            .delete(&archived.id)
            .unwrap();

        assert!(!file_path.exists());
        database
            .with_connection(|connection| {
                for (table, column) in [
                    ("cards", "id"),
                    ("attachments", "card_id"),
                    ("card_documents", "card_id"),
                    ("card_document_images", "card_id"),
                    ("concept_mentions", "card_id"),
                    ("card_ai_threads", "card_id"),
                ] {
                    let count = connection.query_row(
                        &format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?1"),
                        [&archived.id],
                        |row| row.get::<_, i64>(0),
                    )?;
                    assert_eq!(count, 0, "{table} was not cleaned");
                }
                let message_count = connection.query_row(
                    "SELECT COUNT(*) FROM card_ai_messages WHERE id = 'message-delete'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?;
                let citation_count = connection.query_row(
                    "SELECT COUNT(*) FROM card_ai_citations
                     WHERE message_id = 'message-delete'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?;
                let concept_count = connection.query_row(
                    "SELECT COUNT(*) FROM concepts WHERE id = ?1",
                    [concept_id],
                    |row| row.get::<_, i64>(0),
                )?;
                assert_eq!((message_count, citation_count, concept_count), (0, 0, 1));
                Ok(())
            })
            .unwrap();
    }
}
