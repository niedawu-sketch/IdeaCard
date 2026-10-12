use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::{
    ai::{
        generate,
        keychain::SecretStore,
        provider::{AiTransport, ReqwestTransport},
        service::{discuss_streaming, explain_concept_streaming, load_card_context},
    },
    app_error::AppError,
    domain::ai::{
        validate_settings, AiCitation, AiDeltaKind, AiInputMessage, AiProvider, AiRequest, AiRole,
        AiSettings, ApiKey, ConceptAssistantSettings, TitleGenerationSettings, StoredAiMessage,
    },
    repository::ai_repository::{
        AiConversationRepository, AiSettingsRepository, ConceptAssistantSettingsRepository, TitleGenerationSettingsRepository,
    },
    AppState,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettingsStatus {
    pub provider: AiProvider,
    pub base_url: String,
    pub model: String,
    pub web_search_enabled: bool,
    pub has_api_key: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiSettingsInput {
    pub provider: AiProvider,
    pub base_url: String,
    pub model: String,
    pub web_search_enabled: bool,
    pub api_key: Option<String>,
}

impl std::fmt::Debug for AiSettingsInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AiSettingsInput")
            .field("provider", &self.provider)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("web_search_enabled", &self.web_search_enabled)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl AiSettingsInput {
    fn settings(&self) -> AiSettings {
        AiSettings {
            provider: self.provider,
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            web_search_enabled: self.web_search_enabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConnectionResult {
    pub status: &'static str,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendCardAiInput {
    pub card_id: String,
    pub content: String,
    pub web_search_enabled: bool,
    pub request_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegenerateCardAiInput {
    pub card_id: String,
    pub message_id: String,
    pub request_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExplainConceptInput {
    pub card_id: String,
    pub selected_text: String,
    pub web_search_enabled: bool,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConceptExplanation {
    pub selected_text: String,
    pub explanation: String,
    pub citations: Vec<AiCitation>,
}

const AI_TEXT_DELTA_EVENT: &str = "ai://text-delta";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiTextDelta {
    request_id: String,
    kind: AiDeltaKind,
    delta: String,
}

#[tauri::command]
pub(crate) fn get_ai_settings(state: State<'_, AppState>) -> Result<AiSettingsStatus, AppError> {
    settings_status(&state, AiSettingsRepository::new(&state.database).get()?)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn save_ai_settings(
    input: AiSettingsInput,
    state: State<'_, AppState>,
) -> Result<AiSettingsStatus, AppError> {
    let settings = validate_settings(input.settings()).map_err(core_error_to_app)?;
    if let Some(value) = input.api_key.as_deref() {
        let key = ApiKey::new(value.to_owned()).map_err(core_error_to_app)?;
        let account = provider_account(settings.provider, &settings.base_url);
        state.ai_keychain.set(&account, key.expose())?;
    }
    let saved = AiSettingsRepository::new(&state.database).save(settings)?;
    settings_status(&state, saved)
}

#[tauri::command]
pub(crate) fn get_concept_assistant_settings(
    state: State<'_, AppState>,
) -> Result<ConceptAssistantSettings, AppError> {
    ConceptAssistantSettingsRepository::new(&state.database).get()
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn save_concept_assistant_settings(
    input: ConceptAssistantSettings,
    state: State<'_, AppState>,
) -> Result<ConceptAssistantSettings, AppError> {
    ConceptAssistantSettingsRepository::new(&state.database).save(input)
}

#[tauri::command]
pub(crate) fn get_title_generation_settings(
    state: State<'_, AppState>,
) -> Result<TitleGenerationSettings, AppError> {
    TitleGenerationSettingsRepository::new(&state.database).get()
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn save_title_generation_settings(
    input: TitleGenerationSettings,
    state: State<'_, AppState>,
) -> Result<TitleGenerationSettings, AppError> {
    TitleGenerationSettingsRepository::new(&state.database).save(input)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn remove_ai_key(
    provider: AiProvider,
    state: State<'_, AppState>,
) -> Result<AiSettingsStatus, AppError> {
    let settings = AiSettingsRepository::new(&state.database).get()?;
    let account = provider_account(provider, &settings.base_url);
    state.ai_keychain.remove(&account)?;
    settings_status(&state, settings)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn test_ai_connection(
    input: AiSettingsInput,
    state: State<'_, AppState>,
) -> Result<AiConnectionResult, AppError> {
    let settings = validate_settings(input.settings()).map_err(core_error_to_app)?;
    let api_key = input
        .api_key
        .map(ApiKey::new)
        .transpose()
        .map_err(core_error_to_app)?
        .map_or_else(
            || {
                let account = provider_account(settings.provider, &settings.base_url);
                state
                    .ai_keychain
                    .get(&account)
                    .and_then(|value| ApiKey::new(value).map_err(core_error_to_app))
            },
            Ok,
        )?;
    let transport = ReqwestTransport::new().map_err(core_error_to_app)?;
    test_connection_with(&transport, &settings, &api_key)?;
    Ok(AiConnectionResult {
        status: "connected",
    })
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn list_card_ai_messages(
    card_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<StoredAiMessage>, AppError> {
    AiConversationRepository::new(&state.database).list_for_card(&card_id)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn send_card_ai_message(
    input: SendCardAiInput,
    app: AppHandle,
) -> Result<Vec<StoredAiMessage>, AppError> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = worker_app.state::<AppState>();
        send_card_ai_message_blocking(input, &state, &worker_app)
    })
    .await
    .map_err(|_| ai_request_join_error())?
}

fn send_card_ai_message_blocking(
    input: SendCardAiInput,
    state: &AppState,
    app: &AppHandle,
) -> Result<Vec<StoredAiMessage>, AppError> {
    prepare_request(state, &input.request_id)?;
    let settings = AiSettingsRepository::new(&state.database).get()?;
    let api_key = stored_api_key(state, &settings)?;
    let context = load_card_context(&state.database, &input.card_id)?;
    let repository = AiConversationRepository::new(&state.database);
    let history = repository.list_for_card(&input.card_id)?;
    let transport = ReqwestTransport::new().map_err(core_error_to_app)?;
    let request_id = input.request_id.clone();
    let mut on_delta = |kind: AiDeltaKind, delta: &str| {
        let _ = app.emit(
            AI_TEXT_DELTA_EVENT,
            AiTextDelta {
                request_id: request_id.clone(),
                kind,
                delta: delta.to_owned(),
            },
        );
    };
    let is_cancelled = || request_is_cancelled(state, &input.request_id);
    let result = discuss_streaming(
        &transport,
        &settings,
        &api_key,
        &context,
        &history,
        &input.content,
        input.web_search_enabled,
        &mut on_delta,
        &is_cancelled,
    );
    ensure_not_cancelled(state, &input.request_id)?;
    let result = result?;
    repository.save_exchange(
        &input.card_id,
        &input.content,
        input.web_search_enabled,
        &result,
    )
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn regenerate_card_ai_message(
    input: RegenerateCardAiInput,
    app: AppHandle,
) -> Result<Vec<StoredAiMessage>, AppError> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = worker_app.state::<AppState>();
        regenerate_card_ai_message_blocking(input, &state, &worker_app)
    })
    .await
    .map_err(|_| ai_request_join_error())?
}

fn regenerate_card_ai_message_blocking(
    input: RegenerateCardAiInput,
    state: &AppState,
    app: &AppHandle,
) -> Result<Vec<StoredAiMessage>, AppError> {
    prepare_request(state, &input.request_id)?;
    let settings = AiSettingsRepository::new(&state.database).get()?;
    let api_key = stored_api_key(state, &settings)?;
    let context = load_card_context(&state.database, &input.card_id)?;
    let repository = AiConversationRepository::new(&state.database);
    let target = repository.regeneration_target(&input.card_id, &input.message_id)?;
    let transport = ReqwestTransport::new().map_err(core_error_to_app)?;
    let request_id = input.request_id.clone();
    let mut on_delta = |kind: AiDeltaKind, delta: &str| {
        let _ = app.emit(
            AI_TEXT_DELTA_EVENT,
            AiTextDelta {
                request_id: request_id.clone(),
                kind,
                delta: delta.to_owned(),
            },
        );
    };
    let is_cancelled = || request_is_cancelled(state, &input.request_id);
    let result = discuss_streaming(
        &transport,
        &settings,
        &api_key,
        &context,
        &target.history,
        &target.question,
        target.web_search_enabled,
        &mut on_delta,
        &is_cancelled,
    );
    ensure_not_cancelled(state, &input.request_id)?;
    let result = result?;
    if target.replace_in_place {
        repository.replace_latest_response(&input.card_id, &input.message_id, &result)
    } else {
        repository.save_exchange(
            &input.card_id,
            &target.question,
            target.web_search_enabled,
            &result,
        )
    }
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn explain_card_concept(
    input: ExplainConceptInput,
    app: AppHandle,
) -> Result<ConceptExplanation, AppError> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = worker_app.state::<AppState>();
        explain_card_concept_blocking(input, &state, &worker_app)
    })
    .await
    .map_err(|_| ai_request_join_error())?
}

fn explain_card_concept_blocking(
    input: ExplainConceptInput,
    state: &AppState,
    app: &AppHandle,
) -> Result<ConceptExplanation, AppError> {
    prepare_request(state, &input.request_id)?;
    let settings = AiSettingsRepository::new(&state.database).get()?;
    let api_key = stored_api_key(state, &settings)?;
    let context = load_card_context(&state.database, &input.card_id)?;
    let concept_assistant = ConceptAssistantSettingsRepository::new(&state.database).get()?;
    let transport = ReqwestTransport::new().map_err(core_error_to_app)?;
    let request_id = input.request_id.clone();
    let mut on_delta = |kind: AiDeltaKind, delta: &str| {
        if kind != AiDeltaKind::Answer {
            return;
        }
        let _ = app.emit(
            AI_TEXT_DELTA_EVENT,
            AiTextDelta {
                request_id: request_id.clone(),
                kind,
                delta: delta.to_owned(),
            },
        );
    };
    let is_cancelled = || request_is_cancelled(state, &input.request_id);
    let result = explain_concept_streaming(
        &transport,
        &settings,
        &api_key,
        &context,
        &concept_assistant.system_prompt,
        &input.selected_text,
        input.web_search_enabled,
        &mut on_delta,
        &is_cancelled,
    );
    ensure_not_cancelled(state, &input.request_id)?;
    let result = result?;
    Ok(ConceptExplanation {
        selected_text: input.selected_text,
        explanation: result.text,
        citations: result.citations,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn cancel_ai_request(
    request_id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    uuid::Uuid::parse_str(&request_id).map_err(|_| ai_request_id_invalid())?;
    state
        .cancelled_ai_requests
        .lock()
        .map_err(|_| ai_request_state_error())?
        .insert(request_id);
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn open_external_url(url: String, app: AppHandle) -> Result<(), AppError> {
    let parsed = url::Url::parse(&url).map_err(|_| external_url_invalid())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(external_url_invalid());
    }
    app.opener()
        .open_url(url, None::<String>)
        .map_err(|_| AppError::new("external_url_open_failed", "无法打开网页", "稍后重试"))
}

fn test_connection_with(
    transport: &dyn AiTransport,
    settings: &AiSettings,
    api_key: &ApiKey,
) -> Result<(), AppError> {
    generate(
        transport,
        settings,
        api_key,
        &AiRequest {
            system_prompt: "这是连接测试。".to_owned(),
            messages: vec![AiInputMessage {
                role: AiRole::User,
                content: "请只回复：连接成功".to_owned(),
            }],
            web_search_enabled: false,
        },
    )
    .map(|_| ())
    .map_err(core_error_to_app)
}

fn settings_status(state: &AppState, settings: AiSettings) -> Result<AiSettingsStatus, AppError> {
    let account = provider_account(settings.provider, &settings.base_url);
    let has_api_key = state.ai_keychain.exists(&account)?;
    Ok(AiSettingsStatus {
        provider: settings.provider,
        base_url: settings.base_url,
        model: settings.model,
        web_search_enabled: settings.web_search_enabled,
        has_api_key,
    })
}

pub(crate) fn stored_api_key(state: &AppState, settings: &AiSettings) -> Result<ApiKey, AppError> {
    let account = provider_account(settings.provider, &settings.base_url);
    state
        .ai_keychain
        .get(&account)
        .and_then(|value| ApiKey::new(value).map_err(core_error_to_app))
}

fn provider_account(provider: AiProvider, base_url: &str) -> String {
    let provider_name = match provider {
        AiProvider::OpenAi => "openAi",
        AiProvider::DeepSeek => "deepSeek",
        AiProvider::OpenAiCompatible => "openAiCompatible",
        AiProvider::Anthropic => "anthropic",
    };
    let digest = hex::encode(Sha256::digest(
        format!("{provider_name}:{}", base_url.trim().trim_end_matches('/')).as_bytes(),
    ));
    format!("{provider_name}-{}", &digest[..32])
}

fn prepare_request(state: &AppState, request_id: &str) -> Result<(), AppError> {
    uuid::Uuid::parse_str(request_id).map_err(|_| ai_request_id_invalid())?;
    state
        .cancelled_ai_requests
        .lock()
        .map_err(|_| ai_request_state_error())?
        .remove(request_id);
    Ok(())
}

fn ensure_not_cancelled(state: &AppState, request_id: &str) -> Result<(), AppError> {
    if state
        .cancelled_ai_requests
        .lock()
        .map_err(|_| ai_request_state_error())?
        .remove(request_id)
    {
        return Err(AppError::new(
            "ai_request_cancelled",
            "已取消 AI 请求",
            "需要时可重新发送",
        ));
    }
    Ok(())
}

fn request_is_cancelled(state: &AppState, request_id: &str) -> bool {
    state
        .cancelled_ai_requests
        .lock()
        .map(|requests| requests.contains(request_id))
        .unwrap_or(true)
}

fn core_error_to_app(error: crate::domain::ai::AiCoreError) -> AppError {
    AppError::new(error.code, error.message, error.recovery)
}

fn ai_request_id_invalid() -> AppError {
    AppError::new(
        "ai_request_id_invalid",
        "AI 请求标识无效",
        "重新打开卡片后再试",
    )
}

fn ai_request_state_error() -> AppError {
    AppError::new("ai_request_failed", "无法处理 AI 请求", "重新发送问题")
}

fn ai_request_join_error() -> AppError {
    AppError::new("ai_request_failed", "AI 请求意外中断", "重新发送问题")
}

fn external_url_invalid() -> AppError {
    AppError::new("external_url_invalid", "网页地址无效", "检查引用地址后重试")
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::{provider_account, test_connection_with};
    use crate::{
        ai::provider::{AiTransport, HttpRequest, HttpResponse, TransportFailure},
        domain::ai::{AiProvider, AiSettings, ApiKey},
    };

    struct FakeTransport {
        request: Mutex<Option<HttpRequest>>,
    }

    impl AiTransport for FakeTransport {
        fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportFailure> {
            *self.request.lock().unwrap() = Some(request);
            Ok(HttpResponse::json(
                200,
                json!({"choices": [{"message": {"content": "连接成功"}}]}),
            ))
        }
    }

    #[test]
    fn connection_test_uses_the_protocol_without_enabling_web_search() {
        let transport = FakeTransport {
            request: Mutex::new(None),
        };
        test_connection_with(
            &transport,
            &AiSettings {
                provider: AiProvider::OpenAiCompatible,
                base_url: "https://models.example/v1".to_owned(),
                model: "test-model".to_owned(),
                web_search_enabled: false,
            },
            &ApiKey::new("test-key").unwrap(),
        )
        .unwrap();

        let request = transport.request.lock().unwrap().clone().unwrap();
        assert!(request.url.ends_with("/chat/completions"));
        assert!(format!("{request:?}").contains("[REDACTED]"));
    }

    #[test]
    fn keychain_accounts_are_isolated_by_provider_and_base_url() {
        let first = provider_account(AiProvider::OpenAiCompatible, "https://one.example/v1");
        let second = provider_account(AiProvider::OpenAiCompatible, "https://two.example/v1");
        let official = provider_account(AiProvider::OpenAi, "https://api.openai.com/v1");
        let deepseek = provider_account(AiProvider::DeepSeek, "https://api.deepseek.com");

        assert_ne!(first, second);
        assert_ne!(first, official);
        assert_ne!(first, deepseek);
        assert_ne!(official, deepseek);
        assert!(!first.contains("https://"));
        assert!(first.len() <= 100);
    }
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn generate_card_title(
    card_id: String,
    original_content: String,
    app: AppHandle,
) -> Result<String, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        load_card_context(&state.database, &card_id)?;
        crate::ai::title::generate_title(&state, &original_content)
    }).await.map_err(|_| AppError::new("ai_title_failed", "标题生成失败", "稍后重试"))?
}
