use std::{
    fmt::{self, Debug, Formatter},
    path::Path,
    sync::Mutex,
};

use rusqlite::{params, Connection};

use crate::app_error::AppError;

const INITIAL_MIGRATION_SQL: &str = include_str!("../migrations/0001_initial.sql");
const STRUCTURED_CARD_AI_MIGRATION_SQL: &str =
    include_str!("../migrations/0002_structured_card_ai.sql");
const AI_RESPONSE_DETAILS_MIGRATION_SQL: &str =
    include_str!("../migrations/0003_ai_response_details.sql");
const SUPPORTED_SCHEMA_VERSION: i64 = 3;

type SchemaObject = (String, String, String, Option<String>);

pub struct Database {
    connection: Mutex<Connection>,
}

impl Debug for Database {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Database").finish_non_exhaustive()
    }
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        let connection = Connection::open(path).map_err(|_| database_open_error())?;
        Self::initialize(connection)
    }

    pub fn open_in_memory() -> Result<Self, AppError> {
        let connection = Connection::open_in_memory().map_err(|_| database_open_error())?;
        Self::initialize(connection)
    }

    pub fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let connection = self.connection.lock().map_err(|_| database_lock_error())?;
        operation(&connection)
    }

    fn initialize(mut connection: Connection) -> Result<Self, AppError> {
        Self::initialize_with(&mut connection, Self::migrate)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn initialize_with(
        connection: &mut Connection,
        migrate: impl FnOnce(&mut Connection) -> Result<(), AppError>,
    ) -> Result<(), AppError> {
        Self::enable_foreign_keys(connection)?;
        migrate(connection)
    }

    fn enable_foreign_keys(connection: &Connection) -> Result<(), AppError> {
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|_| database_open_error())?;
        let enabled = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, bool>(0))
            .map_err(|_| database_open_error())?;
        if !enabled {
            return Err(database_open_error());
        }
        Ok(())
    }

    fn migrate(connection: &mut Connection) -> Result<(), AppError> {
        Self::migrate_with_sqls(
            connection,
            INITIAL_MIGRATION_SQL,
            STRUCTURED_CARD_AI_MIGRATION_SQL,
            AI_RESPONSE_DETAILS_MIGRATION_SQL,
        )
    }

    #[cfg(test)]
    fn migrate_with_initial_sql(
        connection: &mut Connection,
        initial_migration_sql: &str,
    ) -> Result<(), AppError> {
        Self::migrate_with_sqls(
            connection,
            initial_migration_sql,
            STRUCTURED_CARD_AI_MIGRATION_SQL,
            AI_RESPONSE_DETAILS_MIGRATION_SQL,
        )
    }

    fn migrate_with_sqls(
        connection: &mut Connection,
        initial_migration_sql: &str,
        structured_card_ai_migration_sql: &str,
        ai_response_details_migration_sql: &str,
    ) -> Result<(), AppError> {
        if !Self::schema_object_exists(connection, "table", "schema_migrations")? {
            Self::reject_partial_schema(connection)?;
            return Self::apply_fresh_migrations(
                connection,
                initial_migration_sql,
                structured_card_ai_migration_sql,
                ai_response_details_migration_sql,
            );
        }

        let migrations = Self::applied_migrations(connection)?;
        if migrations
            .iter()
            .any(|(version, _)| *version > SUPPORTED_SCHEMA_VERSION)
        {
            return Err(database_version_unsupported_error());
        }
        match migrations.as_slice() {
            [(1, name)] if name == "initial" => {
                Self::apply_structured_card_ai_migration(
                    connection,
                    initial_migration_sql,
                    structured_card_ai_migration_sql,
                    ai_response_details_migration_sql,
                )?;
            }
            [(1, first), (2, second)] if first == "initial" && second == "structured_card_ai" => {
                Self::apply_ai_response_details_migration(
                    connection,
                    initial_migration_sql,
                    structured_card_ai_migration_sql,
                    ai_response_details_migration_sql,
                )?;
            }
            [(1, first), (2, second), (3, third)]
                if first == "initial"
                    && second == "structured_card_ai"
                    && third == "ai_response_details" => {}
            _ => return Err(database_migration_error()),
        }

        Self::validate_current_schema_with_sqls(
            connection,
            initial_migration_sql,
            structured_card_ai_migration_sql,
            ai_response_details_migration_sql,
        )
    }

    #[cfg(test)]
    fn apply_initial_migration(
        connection: &mut Connection,
        migration_sql: &str,
    ) -> Result<(), AppError> {
        let transaction = connection
            .transaction()
            .map_err(|_| database_migration_error())?;
        transaction
            .execute_batch(migration_sql)
            .map_err(|_| database_migration_error())?;
        transaction.commit().map_err(|_| database_migration_error())
    }

    fn apply_fresh_migrations(
        connection: &mut Connection,
        initial_migration_sql: &str,
        structured_card_ai_migration_sql: &str,
        ai_response_details_migration_sql: &str,
    ) -> Result<(), AppError> {
        let transaction = connection
            .transaction()
            .map_err(|_| database_migration_error())?;
        transaction
            .execute_batch(initial_migration_sql)
            .map_err(|_| database_migration_error())?;
        transaction
            .execute_batch(structured_card_ai_migration_sql)
            .map_err(|_| database_migration_error())?;
        transaction
            .execute_batch(ai_response_details_migration_sql)
            .map_err(|_| database_migration_error())?;
        Self::validate_current_schema_with_sqls(
            &transaction,
            initial_migration_sql,
            structured_card_ai_migration_sql,
            ai_response_details_migration_sql,
        )?;
        transaction.commit().map_err(|_| database_migration_error())
    }

    fn apply_structured_card_ai_migration(
        connection: &mut Connection,
        initial_migration_sql: &str,
        structured_card_ai_migration_sql: &str,
        ai_response_details_migration_sql: &str,
    ) -> Result<(), AppError> {
        let transaction = connection
            .transaction()
            .map_err(|_| database_migration_error())?;
        transaction
            .execute_batch(structured_card_ai_migration_sql)
            .map_err(|_| database_migration_error())?;
        transaction
            .execute_batch(ai_response_details_migration_sql)
            .map_err(|_| database_migration_error())?;
        Self::validate_current_schema_with_sqls(
            &transaction,
            initial_migration_sql,
            structured_card_ai_migration_sql,
            ai_response_details_migration_sql,
        )?;
        transaction.commit().map_err(|_| database_migration_error())
    }

    fn apply_ai_response_details_migration(
        connection: &mut Connection,
        initial_migration_sql: &str,
        structured_card_ai_migration_sql: &str,
        ai_response_details_migration_sql: &str,
    ) -> Result<(), AppError> {
        let transaction = connection
            .transaction()
            .map_err(|_| database_migration_error())?;
        transaction
            .execute_batch(ai_response_details_migration_sql)
            .map_err(|_| database_migration_error())?;
        Self::validate_current_schema_with_sqls(
            &transaction,
            initial_migration_sql,
            structured_card_ai_migration_sql,
            ai_response_details_migration_sql,
        )?;
        transaction.commit().map_err(|_| database_migration_error())
    }

    fn reject_partial_schema(connection: &Connection) -> Result<(), AppError> {
        if !Self::schema_snapshot(connection)?.is_empty() {
            return Err(database_migration_error());
        }
        Ok(())
    }

    fn validate_current_schema_with_sqls(
        connection: &Connection,
        initial_migration_sql: &str,
        structured_card_ai_migration_sql: &str,
        ai_response_details_migration_sql: &str,
    ) -> Result<(), AppError> {
        let migrations = Self::applied_migrations(connection)?;
        if migrations
            != [
                (1, "initial".to_string()),
                (2, "structured_card_ai".to_string()),
                (3, "ai_response_details".to_string()),
            ]
        {
            return Err(database_migration_error());
        }

        let expected_schema = Self::expected_schema_with_sqls(
            initial_migration_sql,
            structured_card_ai_migration_sql,
            ai_response_details_migration_sql,
        )?;
        let actual_schema = Self::schema_snapshot(connection)?;
        if actual_schema != expected_schema {
            return Err(database_migration_error());
        }
        Ok(())
    }

    fn expected_schema_with_sqls(
        initial_migration_sql: &str,
        structured_card_ai_migration_sql: &str,
        ai_response_details_migration_sql: &str,
    ) -> Result<Vec<SchemaObject>, AppError> {
        let connection = Connection::open_in_memory().map_err(|_| database_migration_error())?;
        connection
            .execute_batch(initial_migration_sql)
            .map_err(|_| database_migration_error())?;
        connection
            .execute_batch(structured_card_ai_migration_sql)
            .map_err(|_| database_migration_error())?;
        connection
            .execute_batch(ai_response_details_migration_sql)
            .map_err(|_| database_migration_error())?;
        Self::schema_snapshot(&connection)
    }

    fn schema_snapshot(connection: &Connection) -> Result<Vec<SchemaObject>, AppError> {
        let mut statement = connection
            .prepare(
                "SELECT type, name, tbl_name, sql
                 FROM sqlite_schema
                 WHERE name NOT GLOB 'sqlite_*'
                 ORDER BY type, name, tbl_name, sql",
            )
            .map_err(|_| database_migration_error())?;
        let schema = statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(|_| database_migration_error())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| database_migration_error())?;
        Ok(schema)
    }

    fn applied_migrations(connection: &Connection) -> Result<Vec<(i64, String)>, AppError> {
        let mut statement = connection
            .prepare("SELECT version, name FROM schema_migrations ORDER BY version")
            .map_err(|_| database_migration_error())?;
        let migrations = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|_| database_migration_error())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| database_migration_error())?;
        Ok(migrations)
    }

    fn schema_object_exists(
        connection: &Connection,
        object_type: &str,
        name: &str,
    ) -> Result<bool, AppError> {
        connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_schema WHERE type = ?1 AND name = ?2
                 )",
                params![object_type, name],
                |row| row.get(0),
            )
            .map_err(|_| database_migration_error())
    }

    #[cfg(test)]
    fn table_names(&self) -> Result<Vec<String>, AppError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT name FROM sqlite_schema
                 WHERE type = 'table' AND name NOT GLOB 'sqlite_*' ORDER BY name",
            )?;
            let tables = statement
                .query_map([], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(tables)
        })
    }

    #[cfg(test)]
    fn foreign_keys_enabled(&self) -> Result<bool, AppError> {
        self.with_connection(|connection| {
            Ok(connection.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?)
        })
    }

    #[cfg(test)]
    fn open_with_migration_sql(path: &Path, sql: &str) -> Result<Self, AppError> {
        let mut connection = Connection::open(path).map_err(|_| database_open_error())?;
        Self::initialize_with(&mut connection, |connection| {
            Self::migrate_with_initial_sql(connection, sql)
        })?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    #[cfg(test)]
    fn open_with_structured_card_ai_migration_sql(
        path: &Path,
        sql: &str,
    ) -> Result<Self, AppError> {
        let mut connection = Connection::open(path).map_err(|_| database_open_error())?;
        Self::initialize_with(&mut connection, |connection| {
            Self::migrate_with_sqls(
                connection,
                INITIAL_MIGRATION_SQL,
                sql,
                AI_RESPONSE_DETAILS_MIGRATION_SQL,
            )
        })?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
}

fn database_open_error() -> AppError {
    AppError::new(
        "database_open_failed",
        "无法打开数据库",
        "检查数据目录权限后重新打开应用",
    )
}

fn database_migration_error() -> AppError {
    AppError::new(
        "database_migration_failed",
        "数据库升级失败",
        "保留现有数据并重新打开应用；问题持续时检查数据目录",
    )
}

fn database_version_unsupported_error() -> AppError {
    AppError::new(
        "database_version_unsupported",
        "数据库版本高于当前应用支持范围",
        "请更新 IdeaCard 后重试",
    )
}

fn database_lock_error() -> AppError {
    AppError::new("database_lock_failed", "数据库暂时不可用", "重新打开应用")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        panic::{catch_unwind, AssertUnwindSafe},
    };

    use rusqlite::{params, Connection, ErrorCode};
    use tempfile::tempdir;

    use super::{Database, INITIAL_MIGRATION_SQL, STRUCTURED_CARD_AI_MIGRATION_SQL};
    use crate::app_error::AppError;

    const TABLES: [&str; 18] = [
        "cards",
        "attachments",
        "analysis_sessions",
        "analysis_session_cards",
        "analysis_messages",
        "message_card_citations",
        "message_web_citations",
        "insights",
        "insight_messages",
        "app_settings",
        "schema_migrations",
        "card_documents",
        "card_document_images",
        "concepts",
        "concept_mentions",
        "card_ai_threads",
        "card_ai_messages",
        "card_ai_citations",
    ];

    const INDEXES: [&str; 18] = [
        "cards_status_updated_at_idx",
        "cards_source_type_updated_at_idx",
        "cards_content_hash_idx",
        "cards_source_uri_idx",
        "attachments_card_id_idx",
        "attachments_checksum_idx",
        "analysis_session_cards_card_id_idx",
        "analysis_messages_session_id_idx",
        "message_card_citations_card_id_idx",
        "message_web_citations_message_id_idx",
        "insights_session_id_idx",
        "insights_saved_message_id_idx",
        "insight_messages_message_id_idx",
        "card_document_images_attachment_id_idx",
        "concepts_updated_at_idx",
        "concept_mentions_concept_id_idx",
        "concept_mentions_card_id_idx",
        "card_ai_messages_thread_id_idx",
    ];

    const INVALID_MIGRATION: &str = "
        CREATE TABLE migration_sentinel (id INTEGER PRIMARY KEY);
        THIS IS NOT VALID SQL;
    ";

    fn insert_card(
        connection: &Connection,
        source_type: &str,
        capture_method: &str,
        status: &str,
    ) -> rusqlite::Result<()> {
        connection.execute(
            "INSERT INTO cards (
                id, title, original_content, source_type, capture_method, status,
                content_hash, created_at, updated_at
             ) VALUES (?1, 'title', 'content', ?2, ?3, ?4, 'hash', ?5, ?5)",
            params![
                "card-1",
                source_type,
                capture_method,
                status,
                "2026-07-14T00:00:00Z"
            ],
        )?;
        Ok(())
    }

    #[test]
    fn upgrades_a_version_one_database_without_changing_existing_records() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(INITIAL_MIGRATION_SQL).unwrap();
        connection
            .execute(
                "INSERT INTO cards (
                    id, title, original_content, note, source_type, capture_method,
                    source_name, source_uri, status, content_hash, created_at, updated_at
                 ) VALUES (
                    'card-v1', '旧卡片', '原始正文', '旧想法', 'document', 'file',
                    '旧资料.pdf', '', 'annotated', 'hash-v1', ?1, ?1
                 )",
                ["2026-07-19T00:00:00Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO attachments (
                    id, card_id, kind, original_name, relative_path, mime_type,
                    byte_size, checksum, extraction_status, created_at
                 ) VALUES (
                    'attachment-v1', 'card-v1', 'document', '旧资料.pdf',
                    'attachment-v1.pdf', 'application/pdf', 8, 'checksum-v1',
                    'completed', ?1
                 )",
                ["2026-07-19T00:00:00Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO analysis_sessions (
                    id, title, question, web_search_enabled, status, created_at, updated_at
                 ) VALUES (
                    'analysis-v1', '旧分析', '旧问题', 0, 'draft', ?1, ?1
                 )",
                ["2026-07-19T00:00:00Z"],
            )
            .unwrap();
        drop(connection);

        let database = Database::open(&path).unwrap();

        database
            .with_connection(|connection| {
                let migrations = connection
                    .prepare("SELECT version, name FROM schema_migrations ORDER BY version")?
                    .query_map([], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                assert_eq!(
                    migrations,
                    vec![
                        (1, "initial".to_owned()),
                        (2, "structured_card_ai".to_owned()),
                        (3, "ai_response_details".to_owned()),
                    ]
                );
                assert_eq!(
                    connection.query_row(
                        "SELECT
                            title, original_content, note, source_type, capture_method,
                            source_name, source_uri, status, content_hash, created_at, updated_at,
                            archived_at
                         FROM cards WHERE id = 'card-v1'",
                        [],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                                row.get::<_, String>(5)?,
                                row.get::<_, String>(6)?,
                                row.get::<_, String>(7)?,
                                row.get::<_, String>(8)?,
                                row.get::<_, String>(9)?,
                                row.get::<_, String>(10)?,
                                row.get::<_, Option<String>>(11)?,
                            ))
                        },
                    )?,
                    (
                        "旧卡片".to_owned(),
                        "原始正文".to_owned(),
                        "旧想法".to_owned(),
                        "document".to_owned(),
                        "file".to_owned(),
                        "旧资料.pdf".to_owned(),
                        String::new(),
                        "annotated".to_owned(),
                        "hash-v1".to_owned(),
                        "2026-07-19T00:00:00Z".to_owned(),
                        "2026-07-19T00:00:00Z".to_owned(),
                        None,
                    )
                );
                assert_eq!(
                    connection.query_row(
                        "SELECT
                            card_id, kind, original_name, relative_path, mime_type,
                            byte_size, checksum, extraction_status, extraction_error,
                            extracted_text, created_at
                         FROM attachments WHERE id = 'attachment-v1'",
                        [],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                                row.get::<_, i64>(5)?,
                                row.get::<_, String>(6)?,
                                row.get::<_, String>(7)?,
                                row.get::<_, String>(8)?,
                                row.get::<_, String>(9)?,
                                row.get::<_, String>(10)?,
                            ))
                        },
                    )?,
                    (
                        "card-v1".to_owned(),
                        "document".to_owned(),
                        "旧资料.pdf".to_owned(),
                        "attachment-v1.pdf".to_owned(),
                        "application/pdf".to_owned(),
                        8,
                        "checksum-v1".to_owned(),
                        "completed".to_owned(),
                        String::new(),
                        String::new(),
                        "2026-07-19T00:00:00Z".to_owned(),
                    )
                );
                assert_eq!(
                    connection.query_row(
                        "SELECT
                            title, question, web_search_enabled, status, created_at, updated_at
                         FROM analysis_sessions WHERE id = 'analysis-v1'",
                        [],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i64>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                                row.get::<_, String>(5)?,
                            ))
                        },
                    )?,
                    (
                        "旧分析".to_owned(),
                        "旧问题".to_owned(),
                        0,
                        "draft".to_owned(),
                        "2026-07-19T00:00:00Z".to_owned(),
                        "2026-07-19T00:00:00Z".to_owned(),
                    )
                );
                for table in [
                    "card_documents",
                    "card_document_images",
                    "concepts",
                    "concept_mentions",
                    "card_ai_threads",
                    "card_ai_messages",
                    "card_ai_citations",
                ] {
                    assert!(
                        Database::schema_object_exists(connection, "table", table)?,
                        "missing upgraded table {table}"
                    );
                }
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn upgrades_a_version_two_database_and_preserves_existing_ai_messages() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(INITIAL_MIGRATION_SQL).unwrap();
        connection
            .execute_batch(STRUCTURED_CARD_AI_MIGRATION_SQL)
            .unwrap();
        insert_card(&connection, "personal", "quick", "unprocessed").unwrap();
        connection
            .execute_batch(
                "INSERT INTO card_ai_threads (id, card_id, created_at, updated_at)
                 VALUES ('thread-v2', 'card-1', '2026-08-12T00:00:00Z', '2026-08-12T00:00:00Z');
                 INSERT INTO card_ai_messages (id, thread_id, role, content, created_at)
                 VALUES ('message-v2', 'thread-v2', 'assistant', '旧回答', '2026-08-12T00:00:00Z');",
            )
            .unwrap();
        drop(connection);

        let database = Database::open(&path).unwrap();
        database
            .with_connection(|connection| {
                let migrations = connection
                    .prepare("SELECT version, name FROM schema_migrations ORDER BY version")?
                    .query_map([], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                assert_eq!(
                    migrations,
                    vec![
                        (1, "initial".to_owned()),
                        (2, "structured_card_ai".to_owned()),
                        (3, "ai_response_details".to_owned()),
                    ]
                );
                assert_eq!(
                    connection.query_row(
                        "SELECT content, reasoning_content, web_search_enabled
                         FROM card_ai_messages WHERE id = 'message-v2'",
                        [],
                        |row| Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                        )),
                    )?,
                    ("旧回答".to_owned(), String::new(), 0)
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn rolls_back_a_failed_version_two_migration_and_keeps_version_one_data() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(INITIAL_MIGRATION_SQL).unwrap();
        insert_card(&connection, "personal", "quick", "unprocessed").unwrap();
        drop(connection);

        let error = Database::open_with_structured_card_ai_migration_sql(
            &path,
            "CREATE TABLE migration_v2_sentinel (id INTEGER PRIMARY KEY);
             THIS IS NOT VALID SQL;",
        )
        .unwrap_err();

        assert_sanitized(&error, "database_migration_failed", "THIS IS NOT VALID SQL");
        let connection = Connection::open(&path).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT version, name FROM schema_migrations", [], |row| Ok(
                    (row.get::<_, i64>(0)?, row.get::<_, String>(1)?)
                ),)
                .unwrap(),
            (1, "initial".to_owned())
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert!(
            !Database::schema_object_exists(&connection, "table", "migration_v2_sentinel").unwrap()
        );
    }

    #[test]
    fn rolls_back_version_two_when_final_schema_validation_fails() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(INITIAL_MIGRATION_SQL).unwrap();
        insert_card(&connection, "personal", "quick", "unprocessed").unwrap();
        connection
            .execute("DROP INDEX cards_content_hash_idx", [])
            .unwrap();
        drop(connection);

        let error = Database::open(&path).unwrap_err();

        assert_eq!(error.code, "database_migration_failed");
        let connection = Connection::open(&path).unwrap();
        let migrations = connection
            .prepare("SELECT version, name FROM schema_migrations ORDER BY version")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(migrations, vec![(1, "initial".to_owned())]);
        assert!(!Database::schema_object_exists(&connection, "table", "card_documents").unwrap());
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    fn insert_analysis_session(connection: &Connection, status: &str) -> rusqlite::Result<()> {
        connection.execute(
            "INSERT INTO analysis_sessions (
                id, title, question, web_search_enabled, status, created_at, updated_at
             ) VALUES (?1, 'title', 'question', 1, ?2, ?3, ?3)",
            params!["session-1", status, "2026-07-14T00:00:00Z"],
        )?;
        Ok(())
    }

    fn assert_constraint_violation(error: rusqlite::Error) {
        match error {
            rusqlite::Error::SqliteFailure(details, _) => {
                assert_eq!(details.code, ErrorCode::ConstraintViolation);
            }
            other => panic!("expected constraint violation, got {other:?}"),
        }
    }

    fn assert_sanitized(error: &AppError, expected_code: &str, sentinel: &str) {
        assert_eq!(error.code, expected_code);
        assert!(!error.to_string().contains(sentinel));
        assert!(!format!("{error:?}").contains(sentinel));
        assert!(!serde_json::to_string(error).unwrap().contains(sentinel));
    }

    #[test]
    fn creates_all_initial_tables_and_enables_foreign_keys() {
        let database = Database::open_in_memory().unwrap();
        let tables = database.table_names().unwrap();

        for table in TABLES {
            assert!(tables.contains(&table.to_string()), "missing {table}");
        }
        assert!(database.foreign_keys_enabled().unwrap());
    }

    #[test]
    fn records_initial_migration_version_and_creates_all_indexes() {
        let database = Database::open_in_memory().unwrap();

        database
            .with_connection(|connection| {
                let versions = connection.query_row(
                    "SELECT COUNT(*), MAX(version) FROM schema_migrations",
                    [],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )?;
                let mut statement = connection.prepare(
                    "SELECT name FROM sqlite_schema WHERE type = 'index' AND name NOT GLOB 'sqlite_*'",
                )?;
                let indexes = statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;

                assert_eq!(versions, (3, 3));
                for index in INDEXES {
                    assert!(indexes.contains(&index.to_string()), "missing {index}");
                }
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn reopening_the_same_database_preserves_data_and_skips_applied_migrations() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");

        {
            let database = Database::open(&path).unwrap();
            database
                .with_connection(|connection| {
                    insert_card(connection, "personal", "quick", "unprocessed")?;
                    Ok(())
                })
                .unwrap();
        }

        let reopened = Database::open(&path).unwrap();
        reopened
            .with_connection(|connection| {
                let card_count = connection.query_row(
                    "SELECT COUNT(*) FROM cards WHERE id = 'card-1'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?;
                let migration_count =
                    connection.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                assert_eq!(card_count, 1);
                assert_eq!(migration_count, 3);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn invalid_source_type_is_rejected_by_check_constraint() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let error = insert_card(connection, "forum", "quick", "unprocessed").unwrap_err();
                assert_constraint_violation(error);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn invalid_capture_method_is_rejected_by_check_constraint() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let error =
                    insert_card(connection, "personal", "manual", "unprocessed").unwrap_err();
                assert_constraint_violation(error);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn invalid_card_status_is_rejected_by_check_constraint() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let error = insert_card(connection, "personal", "quick", "deleted").unwrap_err();
                assert_constraint_violation(error);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn invalid_attachment_kind_is_rejected_by_check_constraint() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                insert_card(connection, "personal", "quick", "unprocessed")?;
                let error = connection
                    .execute(
                        "INSERT INTO attachments (
                            id, card_id, kind, original_name, relative_path, mime_type,
                            byte_size, checksum, extraction_status, created_at
                         ) VALUES (
                            'attachment-1', 'card-1', 'archive', 'file.zip', 'attachment-1.zip',
                            'application/zip', 1, 'checksum', 'not_applicable', ?1
                         )",
                        ["2026-07-14T00:00:00Z"],
                    )
                    .unwrap_err();
                assert_constraint_violation(error);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn invalid_extraction_status_is_rejected_by_check_constraint() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                insert_card(connection, "personal", "quick", "unprocessed")?;
                let error = connection
                    .execute(
                        "INSERT INTO attachments (
                            id, card_id, kind, original_name, relative_path, mime_type,
                            byte_size, checksum, extraction_status, created_at
                         ) VALUES (
                            'attachment-1', 'card-1', 'document', 'file.txt', 'attachment-1.txt',
                            'text/plain', 1, 'checksum', 'running', ?1
                         )",
                        ["2026-07-14T00:00:00Z"],
                    )
                    .unwrap_err();
                assert_constraint_violation(error);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn invalid_analysis_status_is_rejected_by_check_constraint() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let error = insert_analysis_session(connection, "paused").unwrap_err();
                assert_constraint_violation(error);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn invalid_message_role_is_rejected_by_check_constraint() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                insert_analysis_session(connection, "draft")?;
                let error = connection
                    .execute(
                        "INSERT INTO analysis_messages (
                            id, analysis_session_id, role, content, status, created_at
                         ) VALUES ('message-1', 'session-1', 'system', 'content', 'completed', ?1)",
                        ["2026-07-14T00:00:00Z"],
                    )
                    .unwrap_err();
                assert_constraint_violation(error);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn foreign_key_references_and_delete_restrictions_are_enforced() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let missing_parent_error = connection
                    .execute(
                        "INSERT INTO attachments (
                            id, card_id, kind, original_name, relative_path, mime_type,
                            byte_size, checksum, extraction_status, created_at
                         ) VALUES (
                            'attachment-1', 'missing-card', 'document', 'file.txt',
                            'attachment-1.txt', 'text/plain', 1, 'checksum', 'completed', ?1
                         )",
                        ["2026-07-14T00:00:00Z"],
                    )
                    .unwrap_err();
                assert_constraint_violation(missing_parent_error);

                insert_card(connection, "personal", "quick", "unprocessed")?;
                connection.execute(
                    "INSERT INTO attachments (
                        id, card_id, kind, original_name, relative_path, mime_type,
                        byte_size, checksum, extraction_status, created_at
                     ) VALUES (
                        'attachment-1', 'card-1', 'document', 'file.txt',
                        'attachment-1.txt', 'text/plain', 1, 'checksum', 'completed', ?1
                     )",
                    ["2026-07-14T00:00:00Z"],
                )?;
                let restricted_delete_error = connection
                    .execute("DELETE FROM cards WHERE id = 'card-1'", [])
                    .unwrap_err();
                assert_constraint_violation(restricted_delete_error);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn all_business_foreign_keys_use_delete_restrict() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let mut foreign_key_count = 0;
                for table in TABLES {
                    let mut statement =
                        connection.prepare(&format!("PRAGMA foreign_key_list('{table}')"))?;
                    let actions = statement
                        .query_map([], |row| row.get::<_, String>(6))?
                        .collect::<rusqlite::Result<Vec<_>>>()?;
                    for action in actions {
                        foreign_key_count += 1;
                        assert_eq!(
                            action, "RESTRICT",
                            "unexpected ON DELETE action for {table}"
                        );
                    }
                }
                assert_eq!(foreign_key_count, 19);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn invalid_migration_rolls_back_the_entire_transaction() {
        let mut connection = Connection::open_in_memory().unwrap();

        let error =
            Database::apply_initial_migration(&mut connection, INVALID_MIGRATION).unwrap_err();

        assert_sanitized(&error, "database_migration_failed", "THIS IS NOT VALID SQL");
        let sentinel_count = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'migration_sentinel'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(sentinel_count, 0);
    }

    #[test]
    fn migration_failure_does_not_create_a_fallback_database() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");

        let error = Database::open_with_migration_sql(&path, INVALID_MIGRATION).unwrap_err();

        assert_sanitized(&error, "database_migration_failed", "THIS IS NOT VALID SQL");
        let files = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(files, vec!["ideacard.sqlite3"]);
    }

    #[test]
    fn rejects_database_versions_newer_than_supported() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                 );
                 INSERT INTO schema_migrations (version, name, applied_at)
                 VALUES (4, 'future', '2026-07-14T00:00:00Z');",
            )
            .unwrap();
        drop(connection);

        let error = Database::open(&path).unwrap_err();

        assert_sanitized(
            &error,
            "database_version_unsupported",
            path.to_str().unwrap(),
        );
    }

    #[test]
    fn rejects_a_database_with_an_incomplete_supported_schema() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");
        drop(Database::open(&path).unwrap());
        let connection = Connection::open(&path).unwrap();
        connection
            .execute("DROP INDEX cards_content_hash_idx", [])
            .unwrap();
        drop(connection);

        let error = Database::open(&path).unwrap_err();

        assert_sanitized(&error, "database_migration_failed", path.to_str().unwrap());
        let files = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(files, vec!["ideacard.sqlite3"]);
    }

    #[test]
    fn rejects_an_expected_index_recreated_with_the_wrong_definition() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");
        drop(Database::open(&path).unwrap());
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "DROP INDEX cards_content_hash_idx;
                 CREATE INDEX cards_content_hash_idx ON cards(title);",
            )
            .unwrap();
        drop(connection);

        let error = Database::open(&path).unwrap_err();

        assert_sanitized(&error, "database_migration_failed", path.to_str().unwrap());
    }

    #[test]
    fn rejects_a_table_with_expected_columns_but_tampered_constraints() {
        const TAMPERED_CARDS_SCHEMA: &str = "CREATE TABLE cards (
          id TEXT PRIMARY KEY,
          title TEXT,
          original_content TEXT,
          note TEXT,
          source_type TEXT,
          capture_method TEXT,
          source_name TEXT,
          source_uri TEXT,
          status TEXT,
          content_hash TEXT,
          created_at TEXT,
          updated_at TEXT,
          archived_at TEXT
        )";

        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");
        drop(Database::open(&path).unwrap());
        let connection = Connection::open(&path).unwrap();
        let schema_version = connection
            .query_row("PRAGMA schema_version", [], |row| row.get::<_, i64>(0))
            .unwrap();
        connection
            .execute_batch("PRAGMA writable_schema = ON;")
            .unwrap();
        connection
            .execute(
                "UPDATE sqlite_schema SET sql = ?1 WHERE type = 'table' AND name = 'cards'",
                [TAMPERED_CARDS_SCHEMA],
            )
            .unwrap();
        connection
            .execute_batch("PRAGMA writable_schema = OFF;")
            .unwrap();
        connection
            .pragma_update(None, "schema_version", schema_version + 1)
            .unwrap();
        drop(connection);

        let error = Database::open(&path).unwrap_err();

        assert_sanitized(&error, "database_migration_failed", path.to_str().unwrap());
    }

    #[test]
    fn rejects_unknown_tables_views_and_indexes_in_a_migrated_database() {
        for (case, sql) in [
            ("table", "CREATE TABLE extra_table (id INTEGER)"),
            ("view", "CREATE VIEW extra_view AS SELECT id FROM cards"),
            ("index", "CREATE INDEX extra_index ON cards(title)"),
        ] {
            let directory = tempdir().unwrap();
            let path = directory.path().join("ideacard.sqlite3");
            drop(Database::open(&path).unwrap());
            let connection = Connection::open(&path).unwrap();
            connection.execute_batch(sql).unwrap();
            drop(connection);

            let error = match Database::open(&path) {
                Ok(_) => panic!("unknown {case} was accepted"),
                Err(error) => error,
            };

            assert_sanitized(&error, "database_migration_failed", path.to_str().unwrap());
        }
    }

    #[test]
    fn rejects_partial_schema_without_creating_remaining_objects_or_fallback_database() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute("CREATE TABLE cards (id TEXT PRIMARY KEY)", [])
            .unwrap();
        drop(connection);

        let error = Database::open(&path).unwrap_err();

        assert_sanitized(&error, "database_migration_failed", path.to_str().unwrap());
        let connection = Connection::open(&path).unwrap();
        let mut statement = connection
            .prepare(
                "SELECT name FROM sqlite_schema
                 WHERE type = 'table' AND name NOT GLOB 'sqlite_*' ORDER BY name",
            )
            .unwrap();
        let tables = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(tables, vec!["cards"]);
        drop(statement);
        drop(connection);

        let files = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(files, vec!["ideacard.sqlite3"]);
    }

    #[test]
    fn rejects_a_foreign_database_without_creating_ideacard_schema_or_fallback_database() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute("CREATE TABLE foreign_table (id TEXT PRIMARY KEY)", [])
            .unwrap();
        drop(connection);

        let error = Database::open(&path).unwrap_err();

        assert_sanitized(&error, "database_migration_failed", path.to_str().unwrap());
        let connection = Connection::open(&path).unwrap();
        let mut statement = connection
            .prepare(
                "SELECT name FROM sqlite_schema
                 WHERE type = 'table' AND name NOT GLOB 'sqlite_*' ORDER BY name",
            )
            .unwrap();
        let tables = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(tables, vec!["foreign_table"]);
        drop(statement);
        drop(connection);

        let files = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(files, vec!["ideacard.sqlite3"]);
    }

    #[test]
    fn rejects_a_foreign_table_whose_name_only_resembles_the_sqlite_internal_prefix() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ideacard.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute("CREATE TABLE sqliteX_foreign (id TEXT PRIMARY KEY)", [])
            .unwrap();
        drop(connection);

        let error = Database::open(&path).unwrap_err();

        assert_sanitized(&error, "database_migration_failed", path.to_str().unwrap());
        let connection = Connection::open(&path).unwrap();
        let mut statement = connection
            .prepare(
                "SELECT name FROM sqlite_schema
                 WHERE type = 'table' AND name NOT GLOB 'sqlite_*' ORDER BY name",
            )
            .unwrap();
        let tables = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(tables, vec!["sqliteX_foreign"]);
        drop(statement);
        drop(connection);

        let files = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(files, vec!["ideacard.sqlite3"]);
    }

    #[test]
    fn database_open_errors_are_stable_and_sanitized() {
        let directory = tempdir().unwrap();
        let path = directory.path();

        let error = Database::open(path).unwrap_err();

        assert_sanitized(&error, "database_open_failed", path.to_str().unwrap());
    }

    #[test]
    fn poisoned_connection_lock_returns_a_stable_sanitized_error() {
        const SENTINEL: &str = "mutex poison SQL /private/ideacard.sqlite3";
        let database = Database::open_in_memory().unwrap();
        let panic_result = catch_unwind(AssertUnwindSafe(|| {
            let _ = database.with_connection::<()>(|_| panic!("{SENTINEL}"));
        }));
        assert!(panic_result.is_err());

        let error = database.with_connection(|_| Ok(())).unwrap_err();

        assert_sanitized(&error, "database_lock_failed", SENTINEL);
    }
}
