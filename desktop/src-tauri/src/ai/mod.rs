pub mod anthropic;
pub mod keychain;
pub mod openai;
pub mod openai_compatible;
pub mod provider;
pub mod service;
pub(crate) mod title;

use crate::domain::ai::{
    map_provider_failure, validate_settings, AiCoreError, AiDeltaKind, AiProvider, AiRequest,
    AiResult, AiSettings, ApiKey, ProviderFailure,
};
use provider::AiTransport;

pub fn generate(
    transport: &dyn AiTransport,
    settings: &AiSettings,
    api_key: &ApiKey,
    request: &AiRequest,
) -> Result<AiResult, AiCoreError> {
    let settings = validate_settings(settings.clone())?;
    if request.messages.is_empty()
        || request
            .messages
            .iter()
            .any(|message| message.content.trim().is_empty())
    {
        return Err(map_provider_failure(ProviderFailure::InvalidRequest));
    }
    match settings.provider {
        AiProvider::OpenAi => openai::generate(transport, &settings, api_key, request),
        AiProvider::DeepSeek => openai::generate(transport, &settings, api_key, request),
        AiProvider::OpenAiCompatible => {
            openai_compatible::generate(transport, &settings, api_key, request)
        }
        AiProvider::Anthropic => anthropic::generate(transport, &settings, api_key, request),
    }
}

pub fn generate_streaming(
    transport: &dyn AiTransport,
    settings: &AiSettings,
    api_key: &ApiKey,
    request: &AiRequest,
    on_delta: &mut dyn FnMut(AiDeltaKind, &str),
    is_cancelled: &dyn Fn() -> bool,
) -> Result<AiResult, AiCoreError> {
    let settings = validate_settings(settings.clone())?;
    if request.messages.is_empty()
        || request
            .messages
            .iter()
            .any(|message| message.content.trim().is_empty())
    {
        return Err(map_provider_failure(ProviderFailure::InvalidRequest));
    }
    match settings.provider {
        AiProvider::OpenAi => openai::generate_streaming(
            transport,
            &settings,
            api_key,
            request,
            on_delta,
            is_cancelled,
        ),
        AiProvider::DeepSeek => openai::generate_streaming(
            transport,
            &settings,
            api_key,
            request,
            on_delta,
            is_cancelled,
        ),
        AiProvider::OpenAiCompatible => openai_compatible::generate_streaming(
            transport,
            &settings,
            api_key,
            request,
            on_delta,
            is_cancelled,
        ),
        AiProvider::Anthropic => anthropic::generate_streaming(
            transport,
            &settings,
            api_key,
            request,
            on_delta,
            is_cancelled,
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::{
        generate,
        provider::{AiTransport, HttpRequest, HttpResponse, TransportFailure},
    };
    use crate::domain::ai::{AiInputMessage, AiProvider, AiRequest, AiRole, AiSettings, ApiKey};

    struct FakeTransport {
        response: HttpResponse,
        request: Mutex<Option<HttpRequest>>,
    }

    impl AiTransport for FakeTransport {
        fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportFailure> {
            *self.request.lock().unwrap() = Some(request);
            Ok(self.response.clone())
        }
    }

    #[test]
    fn provider_neutral_entrypoint_selects_the_configured_protocol() {
        let transport = FakeTransport {
            response: HttpResponse::json(
                200,
                json!({"choices": [{"message": {"content": "统一结果"}}]}),
            ),
            request: Mutex::new(None),
        };
        let settings = AiSettings {
            provider: AiProvider::OpenAiCompatible,
            base_url: "https://models.example/v1".to_owned(),
            model: "custom-model".to_owned(),
            web_search_enabled: false,
        };
        let request = AiRequest {
            system_prompt: String::new(),
            messages: vec![AiInputMessage {
                role: AiRole::User,
                content: "问题".to_owned(),
            }],
            web_search_enabled: false,
        };

        let result = generate(
            &transport,
            &settings,
            &ApiKey::new("secret").unwrap(),
            &request,
        )
        .unwrap();

        assert_eq!(result.text, "统一结果");
        assert!(transport
            .request
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .url
            .ends_with("/chat/completions"));
    }
}
