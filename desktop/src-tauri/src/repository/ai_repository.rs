use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use rusqlite::{params, OptionalExtension};

use crate::{
    app_error::AppError,
    database::Database,
    domain::{
        ai::{
            validate_concept_assistant_settings, validate_settings, validate_title_generation_settings,
            AiCoreError, AiResult, AiRole, AiSettings, ConceptAssistantSettings,
            StoredAiCitation, StoredAiMessage, TitleGenerationSettings,
        },
        card::CardStatus,
    },
};

pub const TITLE_GENERATION_SETTINGS_KEY: &str = "ai.title_generation.v1";

pub const AI_SETTINGS_KEY: &str = "ai.settings.v1";
pub const CONCEPT_ASSISTANT_SETTINGS_KEY: &str = "ai.concept_assistant.v1";

pub struct AiSettingsRepository<'a> {
    database: &'a Database,
}

pub struct TitleGenerationSettingsRepository<'a> {
    database: &'a Database,
}

pub struct ConceptAssistantSettingsRepository<'a> {
    database: &'a Database,
}

pub struct AiConversationRepository<'a> {
    database: &'a Database,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRegenerationTarget {
    pub question: String,
    pub web_search_enabled: bool,
    pub history: Vec<StoredAiMessage>,
    pub replace_in_place: bool,
}

impl<'a> AiConversationRepository<'a> {
    pub fn new(database: &'a Database) -> Self {
        Self { database }
    }

    pub fn list_for_card(&self, card_id: &str) -> Result<Vec<StoredAiMessage>, AppError> {
        self.database.with_connection(|connection| {
            let card_exists = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM cards WHERE id = ?1)",
                params![card_id],
                |row| row.get::<_, bool>(0),
            )?;
            if !card_exists {
                return Err(card_not_found_error());
            }
            let thread_id = connection
                .query_row(
                    "SELECT id FROM card_ai_threads WHERE card_id = ?1",
                    params![card_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let Some(thread_id) = thread_id else {
                return Ok(Vec::new());
            };
            load_messages(connection, &thread_id)
        })
    }

    pub fn save_exchange(
        &self,
        card_id: &str,
        question: &str,
        web_search_enabled: bool,
        result: &AiResult,
    ) -> Result<Vec<StoredAiMessage>, AppError> {
        let question = question.trim();
        if question.is_empty() || result.text.trim().is_empty() {
            return Err(ai_message_invalid_error());
        }
        self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let status = transaction
                .query_row(
                    "SELECT status FROM cards WHERE id = ?1",
                    params![card_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or_else(card_not_found_error)?;
            if status == CardStatus::Archived.as_sql() {
                return Err(card_archived_read_only_error());
            }
            let existing_thread = transaction
                .query_row(
                    "SELECT id, updated_at FROM card_ai_threads WHERE card_id = ?1",
                    params![card_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            let (thread_id, previous_timestamp) = existing_thread
                .map(|(id, timestamp)| (id, Some(timestamp)))
                .unwrap_or_else(|| (uuid::Uuid::new_v4().to_string(), None));
            let user_created_at = next_timestamp(previous_timestamp.as_deref())?;
            let assistant_created_at = next_timestamp(Some(&user_created_at))?;
            if previous_timestamp.is_none() {
                transaction.execute(
                    "INSERT INTO card_ai_threads (id, card_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![thread_id, card_id, user_created_at, assistant_created_at],
                )?;
            } else {
                transaction.execute(
                    "UPDATE card_ai_threads SET updated_at = ?1 WHERE id = ?2",
                    params![assistant_created_at, thread_id],
                )?;
            }
            let user_id = uuid::Uuid::new_v4().to_string();
            let assistant_id = uuid::Uuid::new_v4().to_string();
            transaction.execute(
                "INSERT INTO card_ai_messages (
                   id, thread_id, role, content, reasoning_content, web_search_enabled, created_at
                 ) VALUES (?1, ?2, 'user', ?3, '', ?4, ?5)",
                params![
                    user_id,
                    thread_id,
                    question,
                    web_search_enabled,
                    user_created_at
                ],
            )?;
            transaction.execute(
                "INSERT INTO card_ai_messages (
                   id, thread_id, role, content, reasoning_content, web_search_enabled, created_at
                 ) VALUES (?1, ?2, 'assistant', ?3, ?4, 0, ?5)",
                params![
                    assistant_id,
                    thread_id,
                    result.text.trim(),
                    result.reasoning.trim(),
                    assistant_created_at
                ],
            )?;
            for (position, citation) in result.citations.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO card_ai_citations (message_id, url, title, position)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        assistant_id,
                        citation.url,
                        citation.title,
                        i64::try_from(position).map_err(|_| ai_message_invalid_error())?
                    ],
                )?;
            }
            let messages = load_messages(&transaction, &thread_id)?;
            transaction.commit()?;
            Ok(messages)
        })
    }

    pub fn regeneration_target(
        &self,
        card_id: &str,
        assistant_message_id: &str,
    ) -> Result<AiRegenerationTarget, AppError> {
        let messages = self.list_for_card(card_id)?;
        let assistant_index = messages
            .iter()
            .position(|message| {
                message.id == assistant_message_id && message.role == AiRole::Assistant
            })
            .ok_or_else(ai_message_not_found_error)?;
        let user_index = assistant_index
            .checked_sub(1)
            .filter(|index| messages[*index].role == AiRole::User)
            .ok_or_else(invalid_stored_messages)?;
        let replace_in_place = assistant_index + 1 == messages.len();
        let history = if replace_in_place {
            messages[..user_index].to_vec()
        } else {
            messages.clone()
        };
        Ok(AiRegenerationTarget {
            question: messages[user_index].content.clone(),
            web_search_enabled: messages[user_index].web_search_enabled,
            history,
            replace_in_place,
        })
    }

    pub fn replace_latest_response(
        &self,
        card_id: &str,
        assistant_message_id: &str,
        result: &AiResult,
    ) -> Result<Vec<StoredAiMessage>, AppError> {
        if result.text.trim().is_empty() {
            return Err(ai_message_invalid_error());
        }
        self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let (thread_id, status, updated_at) = transaction
                .query_row(
                    "SELECT card_ai_threads.id, cards.status, card_ai_threads.updated_at
                     FROM card_ai_threads
                     JOIN cards ON cards.id = card_ai_threads.card_id
                     WHERE card_ai_threads.card_id = ?1",
                    params![card_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(ai_message_not_found_error)?;
            if status == CardStatus::Archived.as_sql() {
                return Err(card_archived_read_only_error());
            }
            let latest = transaction
                .query_row(
                    "SELECT id, role FROM card_ai_messages
                     WHERE thread_id = ?1
                     ORDER BY created_at DESC, id DESC LIMIT 1",
                    params![thread_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
                .ok_or_else(ai_message_not_found_error)?;
            if latest.0 != assistant_message_id || latest.1 != "assistant" {
                return Err(ai_message_not_latest_error());
            }
            transaction.execute(
                "UPDATE card_ai_messages
                 SET content = ?1, reasoning_content = ?2
                 WHERE id = ?3",
                params![
                    result.text.trim(),
                    result.reasoning.trim(),
                    assistant_message_id
                ],
            )?;
            transaction.execute(
                "DELETE FROM card_ai_citations WHERE message_id = ?1",
                params![assistant_message_id],
            )?;
            for (position, citation) in result.citations.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO card_ai_citations (message_id, url, title, position)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        assistant_message_id,
                        citation.url,
                        citation.title,
                        i64::try_from(position).map_err(|_| ai_message_invalid_error())?
                    ],
                )?;
            }
            transaction.execute(
                "UPDATE card_ai_threads SET updated_at = ?1 WHERE id = ?2",
                params![next_timestamp(Some(&updated_at))?, thread_id],
            )?;
            let messages = load_messages(&transaction, &thread_id)?;
            transaction.commit()?;
            Ok(messages)
        })
    }
}

fn load_messages(
    connection: &rusqlite::Connection,
    thread_id: &str,
) -> Result<Vec<StoredAiMessage>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, role, content, reasoning_content, web_search_enabled, created_at
         FROM card_ai_messages
         WHERE thread_id = ?1
         ORDER BY created_at ASC, id ASC",
    )?;
    let rows = statement
        .query_map(params![thread_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, bool>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);

    rows.into_iter()
        .map(
            |(id, role, content, reasoning_content, web_search_enabled, created_at)| {
                let role = match role.as_str() {
                    "user" => AiRole::User,
                    "assistant" => AiRole::Assistant,
                    _ => return Err(invalid_stored_messages()),
                };
                let mut citations = connection.prepare(
                    "SELECT url, title, position
                 FROM card_ai_citations
                 WHERE message_id = ?1
                 ORDER BY position ASC",
                )?;
                let citations = citations
                    .query_map(params![id], |row| {
                        Ok(StoredAiCitation {
                            url: row.get(0)?,
                            title: row.get(1)?,
                            position: row.get(2)?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(StoredAiMessage {
                    id,
                    role,
                    content,
                    reasoning_content,
                    web_search_enabled,
                    citations,
                    created_at,
                })
            },
        )
        .collect()
}

fn next_timestamp(previous: Option<&str>) -> Result<String, AppError> {
    let now = Utc::now();
    let timestamp = if let Some(previous) = previous {
        let previous = DateTime::parse_from_rfc3339(previous)
            .map_err(|_| invalid_stored_messages())?
            .with_timezone(&Utc);
        if now <= previous {
            previous
                .checked_add_signed(TimeDelta::nanoseconds(1))
                .ok_or_else(invalid_stored_messages)?
        } else {
            now
        }
    } else {
        now
    };
    Ok(timestamp.to_rfc3339_opts(SecondsFormat::Nanos, true))
}

impl<'a> AiSettingsRepository<'a> {
    pub fn new(database: &'a Database) -> Self {
        Self { database }
    }

    pub fn get(&self) -> Result<AiSettings, AppError> {
        self.database.with_connection(|connection| {
            let stored = connection
                .query_row(
                    "SELECT value FROM app_settings WHERE key = ?1",
                    [AI_SETTINGS_KEY],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let Some(stored) = stored else {
                return Ok(AiSettings::default());
            };
            let settings = serde_json::from_str::<AiSettings>(&stored)
                .map_err(|_| invalid_stored_settings())?;
            validate_settings(settings).map_err(core_error_to_app)
        })
    }

    pub fn save(&self, settings: AiSettings) -> Result<AiSettings, AppError> {
        let settings = validate_settings(settings).map_err(core_error_to_app)?;
        let value = serde_json::to_string(&settings).map_err(|_| invalid_stored_settings())?;
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO app_settings (key, value, updated_at)
                 VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![AI_SETTINGS_KEY, value],
            )?;
            Ok(settings)
        })
    }
}

impl<'a> ConceptAssistantSettingsRepository<'a> {
    pub fn new(database: &'a Database) -> Self {
        Self { database }
    }

    pub fn get(&self) -> Result<ConceptAssistantSettings, AppError> {
        self.database.with_connection(|connection| {
            let stored = connection
                .query_row(
                    "SELECT value FROM app_settings WHERE key = ?1",
                    [CONCEPT_ASSISTANT_SETTINGS_KEY],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let Some(stored) = stored else {
                return Ok(ConceptAssistantSettings::default());
            };
            let settings = serde_json::from_str::<ConceptAssistantSettings>(&stored)
                .map_err(|_| invalid_concept_assistant_settings())?;
            validate_concept_assistant_settings(settings).map_err(core_error_to_app)
        })
    }

    pub fn save(
        &self,
        settings: ConceptAssistantSettings,
    ) -> Result<ConceptAssistantSettings, AppError> {
        let settings = validate_concept_assistant_settings(settings).map_err(core_error_to_app)?;
        let value =
            serde_json::to_string(&settings).map_err(|_| invalid_concept_assistant_settings())?;
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO app_settings (key, value, updated_at)
                 VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![CONCEPT_ASSISTANT_SETTINGS_KEY, value],
            )?;
            Ok(settings)
        })
    }
}

impl<'a> TitleGenerationSettingsRepository<'a> {
    pub fn new(database: &'a Database) -> Self {
        Self { database }
    }

    pub fn get(&self) -> Result<TitleGenerationSettings, AppError> {
        self.database.with_connection(|connection| {
            let stored = connection
                .query_row(
                    "SELECT value FROM app_settings WHERE key = ?1",
                    [TITLE_GENERATION_SETTINGS_KEY],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let Some(stored) = stored else {
                return Ok(TitleGenerationSettings::default());
            };
            let settings = serde_json::from_str::<TitleGenerationSettings>(&stored)
                .map_err(|_| AppError::new("title_settings_invalid", "标题提示词配置无效", "重新保存标题提示词"))?;
            validate_title_generation_settings(settings).map_err(core_error_to_app)
        })
    }

    pub fn save(
        &self,
        settings: TitleGenerationSettings,
    ) -> Result<TitleGenerationSettings, AppError> {
        let settings = validate_title_generation_settings(settings).map_err(core_error_to_app)?;
        let value =
            serde_json::to_string(&settings).map_err(|_| AppError::new("title_settings_invalid", "标题提示词配置无效", "重新保存标题提示词"))?;
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO app_settings (key, value, updated_at)
                 VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = excluded.updated_at",
                params![TITLE_GENERATION_SETTINGS_KEY, value],
            )?;
            Ok(settings)
        })
    }
}

fn core_error_to_app(error: AiCoreError) -> AppError {
    AppError::new(error.code, error.message, error.recovery)
}

fn invalid_stored_settings() -> AppError {
    AppError::new("ai_settings_invalid", "AI 设置数据无效", "重新保存 AI 设置")
}

fn invalid_concept_assistant_settings() -> AppError {
    AppError::new(
        "concept_assistant_settings_invalid",
        "概念助手设置数据无效",
        "恢复默认 Prompt 后重新保存",
    )
}

fn invalid_stored_messages() -> AppError {
    AppError::new(
        "ai_messages_invalid",
        "AI 对话数据无效",
        "重新打开卡片后再试",
    )
}

fn ai_message_invalid_error() -> AppError {
    AppError::new("ai_message_invalid", "AI 问题不能为空", "输入问题后再发送")
}

fn ai_message_not_found_error() -> AppError {
    AppError::new(
        "ai_message_not_found",
        "找不到这条 AI 回答",
        "重新打开卡片后再试",
    )
}

fn ai_message_not_latest_error() -> AppError {
    AppError::new(
        "ai_message_not_latest",
        "这条回答后已有新对话",
        "它会作为新一轮问题重新生成",
    )
}

fn card_not_found_error() -> AppError {
    AppError::new(
        "card_not_found",
        "找不到这张卡片",
        "返回收集箱后刷新卡片列表",
    )
}

fn card_archived_read_only_error() -> AppError {
    AppError::new(
        "card_archived_read_only",
        "已归档卡片不能继续 AI 讨论",
        "恢复卡片后再发送",
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn title_prompt_persists_independently_and_rejects_invalid_updates() {
        use super::TitleGenerationSettingsRepository;
        use crate::domain::ai::TitleGenerationSettings;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.sqlite3");
        {
            let database = Database::open(&path).unwrap();
            let repository = TitleGenerationSettingsRepository::new(&database);
            assert_eq!(repository.get().unwrap(), TitleGenerationSettings::default());
            repository.save(TitleGenerationSettings { system_prompt: "  提炼核心观点  ".to_owned() }).unwrap();
            for prompt in [" ".to_owned(), "字".repeat(8001)] {
                assert!(repository.save(TitleGenerationSettings { system_prompt: prompt }).is_err());
            }
            assert_eq!(repository.get().unwrap().system_prompt, "提炼核心观点");
            assert_eq!(ConceptAssistantSettingsRepository::new(&database).get().unwrap(), ConceptAssistantSettings::default());
        }
        let reopened = Database::open(&path).unwrap();
        let repository = TitleGenerationSettingsRepository::new(&reopened);
        assert_eq!(repository.get().unwrap().system_prompt, "提炼核心观点");
        repository.save(TitleGenerationSettings::default()).unwrap();
        assert_eq!(repository.get().unwrap(), TitleGenerationSettings::default());
    }

    use serde_json::{json, Value};

    use super::{
        AiConversationRepository, AiSettingsRepository, ConceptAssistantSettingsRepository,
        AI_SETTINGS_KEY, CONCEPT_ASSISTANT_SETTINGS_KEY,
    };
    use crate::{
        database::Database,
        domain::{
            ai::{AiCitation, AiProvider, AiResult, AiRole, AiSettings, ConceptAssistantSettings},
            card::{CaptureMethod, CardPatch, CardStatus, NewCardInput, SourceType},
        },
        repository::card_repository::CardRepository,
    };

    const SECRET: &str = "test-secret-must-not-enter-database";

    fn create_card(database: &Database) -> String {
        CardRepository::new(database)
            .create(NewCardInput {
                title: "测试卡片".to_owned(),
                original_content: "正文".to_owned(),
                note: "想法".to_owned(),
                source_type: SourceType::Personal,
                capture_method: CaptureMethod::Quick,
                source_name: String::new(),
                source_uri: String::new(),
            })
            .unwrap()
            .card
            .id
    }

    #[test]
    fn missing_setting_returns_safe_official_openai_defaults() {
        let database = Database::open_in_memory().unwrap();
        let repository = AiSettingsRepository::new(&database);

        assert_eq!(repository.get().unwrap(), AiSettings::default());
    }

    #[test]
    fn concept_assistant_settings_use_an_independent_app_setting() {
        let database = Database::open_in_memory().unwrap();
        let repository = ConceptAssistantSettingsRepository::new(&database);

        assert_eq!(
            repository.get().unwrap(),
            ConceptAssistantSettings::default()
        );
        repository
            .save(ConceptAssistantSettings {
                system_prompt: "  只解释当前概念  ".to_owned(),
            })
            .unwrap();

        assert_eq!(repository.get().unwrap().system_prompt, "只解释当前概念");
        let keys: Vec<String> = database
            .with_connection(|connection| {
                let mut statement =
                    connection.prepare("SELECT key FROM app_settings ORDER BY key")?;
                let keys = statement
                    .query_map([], |row| row.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(keys)
            })
            .unwrap();
        assert_eq!(keys, vec![CONCEPT_ASSISTANT_SETTINGS_KEY]);
        assert_ne!(CONCEPT_ASSISTANT_SETTINGS_KEY, AI_SETTINGS_KEY);
    }

    #[test]
    fn saves_only_non_secret_configuration_in_app_settings() {
        let database = Database::open_in_memory().unwrap();
        let repository = AiSettingsRepository::new(&database);
        let settings = AiSettings {
            provider: AiProvider::OpenAiCompatible,
            base_url: "https://models.example/v1/".to_owned(),
            model: "custom-model".to_owned(),
            web_search_enabled: false,
        };

        let saved = repository.save(settings).unwrap();
        assert_eq!(saved.base_url, "https://models.example/v1");

        let stored: String = database
            .with_connection(|connection| {
                Ok(connection.query_row(
                    "SELECT value FROM app_settings WHERE key = ?1",
                    [AI_SETTINGS_KEY],
                    |row| row.get(0),
                )?)
            })
            .unwrap();
        let value: Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(
            value,
            json!({
                "provider": "openAiCompatible",
                "baseUrl": "https://models.example/v1",
                "model": "custom-model",
                "webSearchEnabled": false
            })
        );
        assert!(!stored.contains(SECRET));
        assert!(value.get("apiKey").is_none());
    }

    #[test]
    fn saving_again_updates_the_single_setting_row() {
        let database = Database::open_in_memory().unwrap();
        let repository = AiSettingsRepository::new(&database);
        repository.save(AiSettings::default()).unwrap();
        repository
            .save(AiSettings {
                model: "gpt-next".to_owned(),
                ..AiSettings::default()
            })
            .unwrap();

        let count: i64 = database
            .with_connection(|connection| {
                Ok(connection.query_row(
                    "SELECT COUNT(*) FROM app_settings WHERE key = ?1",
                    [AI_SETTINGS_KEY],
                    |row| row.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(repository.get().unwrap().model, "gpt-next");
    }

    #[test]
    fn rejects_insecure_remote_urls_and_accepts_test_loopback() {
        let database = Database::open_in_memory().unwrap();
        let repository = AiSettingsRepository::new(&database);
        let insecure = repository
            .save(AiSettings {
                base_url: "http://models.example/v1".to_owned(),
                ..AiSettings::default()
            })
            .unwrap_err();
        assert_eq!(insecure.code, "ai_base_url_invalid");

        let loopback = repository
            .save(AiSettings {
                base_url: "http://127.0.0.1:8080/v1".to_owned(),
                ..AiSettings::default()
            })
            .unwrap();
        assert_eq!(loopback.base_url, "http://127.0.0.1:8080/v1");
    }

    #[test]
    fn accepts_named_localhost_during_tests() {
        let database = Database::open_in_memory().unwrap();
        let repository = AiSettingsRepository::new(&database);

        let saved = repository
            .save(AiSettings {
                base_url: "http://localhost:8080/v1".to_owned(),
                ..AiSettings::default()
            })
            .unwrap();

        assert_eq!(saved.base_url, "http://localhost:8080/v1");
    }

    #[test]
    fn rejects_web_search_for_protocols_that_do_not_support_it() {
        let database = Database::open_in_memory().unwrap();
        let repository = AiSettingsRepository::new(&database);

        let error = repository
            .save(AiSettings {
                provider: AiProvider::Anthropic,
                base_url: "https://api.anthropic.com/v1".to_owned(),
                model: "claude-model".to_owned(),
                web_search_enabled: true,
            })
            .unwrap_err();

        assert_eq!(error.code, "ai_web_search_unsupported");
    }

    #[test]
    fn corrupt_stored_settings_return_a_stable_error_without_raw_data() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO app_settings (key, value, updated_at)
                     VALUES (?1, ?2, '2026-07-19T00:00:00.000Z')",
                    [AI_SETTINGS_KEY, SECRET],
                )?;
                Ok(())
            })
            .unwrap();

        let error = AiSettingsRepository::new(&database).get().unwrap_err();
        assert_eq!(error.code, "ai_settings_invalid");
        assert!(!format!("{error:?}").contains(SECRET));
    }

    #[test]
    fn saves_a_complete_exchange_with_ordered_citations_atomically() {
        let database = Database::open_in_memory().unwrap();
        let card_id = create_card(&database);
        let repository = AiConversationRepository::new(&database);

        let messages = repository
            .save_exchange(
                &card_id,
                "有哪些反例？",
                true,
                &AiResult {
                    text: "先检查边界条件。".to_owned(),
                    reasoning: "先确认问题边界。".to_owned(),
                    citations: vec![AiCitation {
                        url: "https://example.com/source".to_owned(),
                        title: "来源".to_owned(),
                    }],
                },
            )
            .unwrap();

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, AiRole::User);
        assert!(messages[0].web_search_enabled);
        assert_eq!(messages[1].role, AiRole::Assistant);
        assert_eq!(messages[1].reasoning_content, "先确认问题边界。");
        assert_eq!(messages[1].citations[0].position, 0);
        assert_eq!(repository.list_for_card(&card_id).unwrap(), messages);
    }

    #[test]
    fn regeneration_replaces_the_latest_answer_and_appends_for_history() {
        let database = Database::open_in_memory().unwrap();
        let card_id = create_card(&database);
        let repository = AiConversationRepository::new(&database);
        let first = repository
            .save_exchange(
                &card_id,
                "第一个问题",
                false,
                &AiResult {
                    text: "第一个回答".to_owned(),
                    reasoning: "第一段思考".to_owned(),
                    citations: Vec::new(),
                },
            )
            .unwrap();
        let first_assistant_id = first[1].id.clone();
        let original = repository
            .save_exchange(
                &card_id,
                "第二个问题",
                true,
                &AiResult {
                    text: "第二个回答".to_owned(),
                    reasoning: String::new(),
                    citations: vec![AiCitation {
                        url: "https://example.com/old".to_owned(),
                        title: "旧来源".to_owned(),
                    }],
                },
            )
            .unwrap();
        let latest_assistant_id = original[3].id.clone();

        let latest_target = repository
            .regeneration_target(&card_id, &latest_assistant_id)
            .unwrap();
        assert!(latest_target.replace_in_place);
        assert!(latest_target.web_search_enabled);
        assert_eq!(latest_target.question, "第二个问题");
        assert_eq!(latest_target.history, original[..2]);

        let replaced = repository
            .replace_latest_response(
                &card_id,
                &latest_assistant_id,
                &AiResult {
                    text: "更新后的回答".to_owned(),
                    reasoning: "更新后的思考".to_owned(),
                    citations: vec![AiCitation {
                        url: "https://example.com/new".to_owned(),
                        title: "新来源".to_owned(),
                    }],
                },
            )
            .unwrap();
        assert_eq!(replaced.len(), 4);
        assert_eq!(replaced[3].id, latest_assistant_id);
        assert_eq!(replaced[3].content, "更新后的回答");
        assert_eq!(replaced[3].reasoning_content, "更新后的思考");
        assert_eq!(replaced[3].citations[0].title, "新来源");

        let historical_target = repository
            .regeneration_target(&card_id, &first_assistant_id)
            .unwrap();
        assert!(!historical_target.replace_in_place);
        assert!(!historical_target.web_search_enabled);
        assert_eq!(historical_target.question, "第一个问题");
        assert_eq!(historical_target.history, replaced);

        let appended = repository
            .save_exchange(
                &card_id,
                &historical_target.question,
                historical_target.web_search_enabled,
                &AiResult {
                    text: "第一个问题的新回答".to_owned(),
                    reasoning: String::new(),
                    citations: Vec::new(),
                },
            )
            .unwrap();
        assert_eq!(appended.len(), 6);
        assert_eq!(appended[1].id, first_assistant_id);
        assert_eq!(appended[1].content, "第一个回答");
        assert_eq!(appended[4].content, "第一个问题");
        assert_eq!(appended[5].content, "第一个问题的新回答");
    }

    #[test]
    fn archived_cards_cannot_add_an_ai_exchange() {
        let database = Database::open_in_memory().unwrap();
        let card_id = create_card(&database);
        CardRepository::new(&database)
            .update(
                &card_id,
                CardPatch {
                    status: Some(CardStatus::Archived),
                    ..CardPatch::default()
                },
            )
            .unwrap();

        let error = AiConversationRepository::new(&database)
            .save_exchange(
                &card_id,
                "继续",
                false,
                &AiResult {
                    text: "不应保存".to_owned(),
                    reasoning: String::new(),
                    citations: Vec::new(),
                },
            )
            .unwrap_err();

        assert_eq!(error.code, "card_archived_read_only");
        assert!(AiConversationRepository::new(&database)
            .list_for_card(&card_id)
            .unwrap()
            .is_empty());
    }
}
