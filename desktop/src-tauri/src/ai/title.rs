//! Automatic initialization and explicit user-requested title generation.
use rusqlite::params;
use tauri::{AppHandle, Manager};

use crate::{
    ai::{
        generate,
        provider::{AiTransport, ReqwestTransport},
    },
    app_error::AppError,
    commands::ai_commands::stored_api_key,
    database::Database,
    domain::{
        ai::{AiInputMessage, AiRequest, AiRole, AiSettings, ApiKey},
        card::Card,
    },
    repository::{
        ai_repository::{AiSettingsRepository, TitleGenerationSettingsRepository},
        card_repository::CardRepository,
    },
    AppState,
};

pub(crate) async fn initialize_title(app: AppHandle, card: Card) -> Card {
    if card.original_content.trim().is_empty() {
        return card;
    }
    let fallback = card.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let attempt = || generate_title(&state, &card.original_content);
        finish_creation(&state.database, card.clone(), attempt())
    })
    .await
    .unwrap_or(fallback)
}

pub(crate) fn generate_title(state: &AppState, content: &str) -> Result<String, AppError> {
    if content.trim().is_empty() {
        return Err(AppError::new("ai_title_content_empty", "没有可用于生成标题的原始内容", "补充原始内容后重试"));
    }
    let settings = AiSettingsRepository::new(&state.database).get()?;
    let key = stored_api_key(state, &settings)?;
    let transport = ReqwestTransport::new()
        .map_err(|error| AppError::new(error.code, error.message, error.recovery))?;
    let prompt = TitleGenerationSettingsRepository::new(&state.database).get()?;
    summarize_title(&transport, &settings, &key, content, &prompt.system_prompt)
}

fn finish_creation(database: &Database, card: Card, title: Result<String, AppError>) -> Card {
    if let Ok(title) = title {
        // A failed title attempt must never turn a successful capture into an error.
        let _ = apply_title(database, &card, &title);
    }
    CardRepository::new(database).get(&card.id).unwrap_or(card)
}

fn summarize_title(
    transport: &dyn AiTransport,
    settings: &AiSettings,
    key: &ApiKey,
    content: &str,
    system_prompt: &str,
) -> Result<String, AppError> {
    let mut settings = settings.clone();
    settings.web_search_enabled = false;
    let result = generate(
        transport,
        &settings,
        key,
        &AiRequest {
            system_prompt: system_prompt.to_owned(),
            messages: vec![AiInputMessage {
                role: AiRole::User,
                content: content.chars().take(12_000).collect(),
            }],
            web_search_enabled: false,
        },
    )
    .map_err(|error| AppError::new(error.code, error.message, error.recovery))?;
    normalize_title(&result.text)
        .ok_or_else(|| AppError::new("ai_title_invalid", "AI 未返回有效标题", "保留原有标题"))
}

fn normalize_title(text: &str) -> Option<String> {
    let title = text
        .trim()
        .trim_matches(['"', '\'', '“', '”', '「', '」'])
        .trim();
    if title.is_empty()
        || title.chars().count() > 40
        || title.chars().any(char::is_control)
        || title.starts_with(['#', '`'])
    {
        return None;
    }
    Some(title.to_owned())
}

fn apply_title(database: &Database, created: &Card, title: &str) -> Result<bool, AppError> {
    database.with_connection(|connection| {
        // Compare and write under the same database lock. Do not alter ordering or status.
        let changed = connection.execute(
            "UPDATE cards SET title = ?1
             WHERE id = ?2 AND title = ?3 AND original_content = ?4
               AND updated_at = ?5 AND status <> 'archived'",
            params![
                title,
                created.id,
                created.title,
                created.original_content,
                created.updated_at
            ],
        )?;
        Ok(changed == 1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ai::provider::{HttpRequest, HttpResponse, TransportFailure},
        domain::{
            ai::AiProvider,
            card::{CaptureMethod, CardPatch, NewCardInput, SourceType},
        },
    };
    use serde_json::json;
    use std::sync::Mutex;

    struct FakeTransport {
        requests: Mutex<Vec<HttpRequest>>,
        response: Result<HttpResponse, TransportFailure>,
    }

    impl AiTransport for FakeTransport {
        fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportFailure> {
            self.requests.lock().unwrap().push(request);
            self.response.clone()
        }
    }

    fn create(database: &Database) -> Card {
        CardRepository::new(database)
            .create(NewCardInput {
                title: "截图 2026-09-19".to_owned(),
                original_content: "AI 产品从演示到落地仍然需要工程能力".to_owned(),
                note: "不应发送的私人备注".to_owned(),
                source_type: SourceType::Personal,
                capture_method: CaptureMethod::Screenshot,
                source_name: String::new(),
                source_uri: String::new(),
            })
            .unwrap()
            .card
    }

    fn settings() -> AiSettings {
        AiSettings {
            provider: AiProvider::OpenAiCompatible,
            base_url: "https://example.com/v1".to_owned(),
            model: "test-model".to_owned(),
            web_search_enabled: true,
        }
    }

    #[test]
    fn summarizes_original_once_and_preserves_later_manual_changes() {
        let database = Database::open_in_memory().unwrap();
        let card = create(&database);
        let transport = FakeTransport {
            requests: Mutex::new(Vec::new()),
            response: Ok(HttpResponse::json(
                200,
                json!({
                    "choices": [{"message": {"content": "“AI 产品落地的工程门槛”"}}]
                }),
            )),
        };
        let title = summarize_title(
            &transport,
            &settings(),
            &ApiKey::new("test-key").unwrap(),
            &card.original_content,
            "自定义标题规则",
        );
        let initialized = finish_creation(&database, card.clone(), title);
        assert_eq!(initialized.title, "AI 产品落地的工程门槛");
        assert_eq!(initialized.note, card.note);
        assert_eq!(initialized.status, card.status);
        let repository = CardRepository::new(&database);
        repository
            .update(
                &card.id,
                CardPatch {
                    title: Some("我自己的标题".to_owned()),
                    ..Default::default()
                },
            )
            .unwrap();
        repository
            .update(
                &card.id,
                CardPatch {
                    note: Some("再次自动保存".to_owned()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(repository.get(&card.id).unwrap().title, "我自己的标题");
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body["messages"][0]["content"], "自定义标题规则");
        assert_eq!(
            requests[0].body["messages"][1]["content"],
            card.original_content
        );
        assert!(!requests[0].body.to_string().contains(&card.note));
        assert!(requests[0].body.get("tools").is_none());
    }

    #[test]
    fn rejects_late_results_after_edits_archive_or_deletion() {
        let database = Database::open_in_memory().unwrap();
        let repository = CardRepository::new(&database);
        for patch in [
            CardPatch {
                title: Some("手动标题".to_owned()),
                ..Default::default()
            },
            CardPatch {
                original_content: Some("用户更新后的内容".to_owned()),
                ..Default::default()
            },
            CardPatch {
                note: Some("用户更新后的备注".to_owned()),
                ..Default::default()
            },
        ] {
            let card = create(&database);
            let updated = repository.update(&card.id, patch).unwrap();
            assert!(!apply_title(&database, &card, "迟到的 AI 标题").unwrap());
            assert_eq!(repository.get(&card.id).unwrap(), updated);
        }
        let card = create(&database);
        database
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE cards SET status = 'archived' WHERE id = ?1",
                    [&card.id],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(!apply_title(&database, &card, "迟到的 AI 标题").unwrap());
        database
            .with_connection(|connection| {
                connection.execute("DELETE FROM cards WHERE id = ?1", [&card.id])?;
                Ok(())
            })
            .unwrap();
        assert!(!apply_title(&database, &card, "迟到的 AI 标题").unwrap());
    }

    #[test]
    fn model_failure_keeps_successfully_created_card() {
        let database = Database::open_in_memory().unwrap();
        let card = create(&database);
        let transport = FakeTransport {
            requests: Mutex::new(Vec::new()),
            response: Err(TransportFailure::Timeout),
        };
        let title = summarize_title(
            &transport,
            &settings(),
            &ApiKey::new("test-key").unwrap(),
            &card.original_content,
            "自定义标题规则",
        );
        assert!(title.is_err());
        assert_eq!(finish_creation(&database, card.clone(), title), card);
        assert_eq!(transport.requests.lock().unwrap().len(), 1);
    }

    #[test]
    fn invalid_titles_are_not_applied() {
        for text in [
            "",
            " \n ",
            "标题\n解释",
            "# 标题",
            "```标题```",
            &"长".repeat(41),
        ] {
            assert_eq!(normalize_title(text), None);
        }
        assert_eq!(normalize_title(" “短标题” "), Some("短标题".to_owned()));
    }
}
