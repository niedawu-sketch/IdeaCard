use serde_json::{json, Value};

use crate::{
    ai::provider::{
        bearer_header, build_endpoint, execute, execute_stream, json_content_type_header,
        AiTransport, HttpRequest,
    },
    domain::ai::{
        map_provider_failure, AiCoreError, AiDeltaKind, AiRequest, AiResult, AiRole, AiSettings,
        ApiKey, ProviderFailure,
    },
};

pub fn generate(
    transport: &dyn AiTransport,
    settings: &AiSettings,
    api_key: &ApiKey,
    request: &AiRequest,
) -> Result<AiResult, AiCoreError> {
    let endpoint = build_endpoint(&settings.base_url, "chat/completions")?;
    let response = execute(
        transport,
        HttpRequest::post(
            endpoint,
            vec![json_content_type_header(), bearer_header(api_key)],
            request_body(settings, request),
        ),
    )?;
    let text = response
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(invalid_response)?;
    Ok(AiResult {
        text: text.to_owned(),
        reasoning: String::new(),
        citations: Vec::new(),
    })
}

pub fn generate_streaming(
    transport: &dyn AiTransport,
    settings: &AiSettings,
    api_key: &ApiKey,
    request: &AiRequest,
    on_delta: &mut dyn FnMut(AiDeltaKind, &str),
    is_cancelled: &dyn Fn() -> bool,
) -> Result<AiResult, AiCoreError> {
    let endpoint = build_endpoint(&settings.base_url, "chat/completions")?;
    let mut body = request_body(settings, request);
    body["stream"] = json!(true);
    let mut text = String::new();
    let mut completed = false;
    let mut stream_error = false;
    execute_stream(
        transport,
        HttpRequest::post(
            endpoint,
            vec![json_content_type_header(), bearer_header(api_key)],
            body,
        ),
        &mut |data| {
            if stream_error {
                return;
            }
            if data.trim() == "[DONE]" {
                completed = true;
                return;
            }
            let Ok(event) = serde_json::from_str::<Value>(data) else {
                stream_error = true;
                return;
            };
            if event.get("error").is_some() {
                stream_error = true;
                return;
            }
            let Some(choices) = event.get("choices").and_then(Value::as_array) else {
                stream_error = true;
                return;
            };
            for choice in choices {
                if choice
                    .get("finish_reason")
                    .is_some_and(|reason| !reason.is_null())
                {
                    completed = true;
                }
                if let Some(delta) = choice
                    .get("delta")
                    .and_then(|delta| delta.get("content"))
                    .and_then(Value::as_str)
                {
                    text.push_str(delta);
                    on_delta(AiDeltaKind::Answer, delta);
                }
            }
        },
        is_cancelled,
    )?;
    let text = text.trim();
    if stream_error || !completed || text.is_empty() {
        return Err(invalid_response());
    }
    Ok(AiResult {
        text: text.to_owned(),
        reasoning: String::new(),
        citations: Vec::new(),
    })
}

fn request_body(settings: &AiSettings, request: &AiRequest) -> Value {
    let mut messages = Vec::new();
    if !request.system_prompt.trim().is_empty() {
        messages.push(json!({
            "role": "system",
            "content": request.system_prompt,
        }));
    }
    messages.extend(request.messages.iter().map(|message| {
        json!({
            "role": match message.role {
                AiRole::User => "user",
                AiRole::Assistant => "assistant",
            },
            "content": message.content,
        })
    }));
    json!({
        "model": settings.model,
        "messages": messages,
    })
}

fn invalid_response() -> AiCoreError {
    map_provider_failure(ProviderFailure::InvalidResponse)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::{generate, generate_streaming};
    use crate::{
        ai::provider::{AiTransport, HttpRequest, HttpResponse, TransportFailure},
        domain::ai::{AiInputMessage, AiProvider, AiRequest, AiRole, AiSettings, ApiKey},
    };

    struct FakeTransport {
        response: HttpResponse,
        request: Mutex<Option<HttpRequest>>,
    }

    struct StreamingTransport {
        events: Vec<String>,
        request: Mutex<Option<HttpRequest>>,
    }

    impl AiTransport for StreamingTransport {
        fn send(&self, _request: HttpRequest) -> Result<HttpResponse, TransportFailure> {
            Err(TransportFailure::InvalidResponse)
        }

        fn send_stream(
            &self,
            request: HttpRequest,
            on_data: &mut dyn FnMut(&str),
            is_cancelled: &dyn Fn() -> bool,
        ) -> Result<u16, TransportFailure> {
            *self.request.lock().unwrap() = Some(request);
            for event in &self.events {
                if is_cancelled() {
                    return Err(TransportFailure::Cancelled);
                }
                on_data(event);
            }
            Ok(200)
        }
    }

    fn settings() -> AiSettings {
        AiSettings {
            provider: AiProvider::OpenAiCompatible,
            base_url: "https://compatible.example/v1".to_owned(),
            model: "compatible-model".to_owned(),
            web_search_enabled: false,
        }
    }

    fn request() -> AiRequest {
        AiRequest {
            system_prompt: "系统指令".to_owned(),
            messages: vec![AiInputMessage {
                role: AiRole::User,
                content: "问题".to_owned(),
            }],
            web_search_enabled: false,
        }
    }

    impl AiTransport for FakeTransport {
        fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportFailure> {
            *self.request.lock().unwrap() = Some(request);
            Ok(self.response.clone())
        }
    }

    #[test]
    fn sends_chat_completions_contract_and_parses_assistant_text() {
        let transport = FakeTransport {
            response: HttpResponse::json(
                200,
                json!({"choices": [{"message": {"role": "assistant", "content": "兼容结果"}}]}),
            ),
            request: Mutex::new(None),
        };
        let settings = AiSettings {
            provider: AiProvider::OpenAiCompatible,
            base_url: "https://compatible.example/v1".to_owned(),
            model: "compatible-model".to_owned(),
            web_search_enabled: false,
        };
        let input = AiRequest {
            system_prompt: "系统指令".to_owned(),
            messages: vec![
                AiInputMessage {
                    role: AiRole::User,
                    content: "问题".to_owned(),
                },
                AiInputMessage {
                    role: AiRole::Assistant,
                    content: "历史回答".to_owned(),
                },
            ],
            web_search_enabled: false,
        };

        let result = generate(
            &transport,
            &settings,
            &ApiKey::new("compatible-key").unwrap(),
            &input,
        )
        .unwrap();

        assert_eq!(result.text, "兼容结果");
        assert!(result.citations.is_empty());
        let captured = transport.request.lock().unwrap().clone().unwrap();
        assert_eq!(
            captured.url,
            "https://compatible.example/v1/chat/completions"
        );
        assert_eq!(captured.body["model"], "compatible-model");
        assert_eq!(
            captured.body["messages"],
            json!([
                {"role": "system", "content": "系统指令"},
                {"role": "user", "content": "问题"},
                {"role": "assistant", "content": "历史回答"}
            ])
        );
        assert_eq!(
            captured
                .headers
                .iter()
                .find(|header| header.0 == "authorization")
                .unwrap()
                .1,
            "Bearer compatible-key"
        );
    }

    #[test]
    fn streams_chat_completion_deltas_until_done() {
        let transport = StreamingTransport {
            events: vec![
                json!({
                    "choices": [{"delta": {"content": "兼容"}}]
                })
                .to_string(),
                json!({
                    "choices": [{"delta": {"content": "流式"}}]
                })
                .to_string(),
                "[DONE]".to_owned(),
            ],
            request: Mutex::new(None),
        };
        let mut deltas = Vec::new();

        let result = generate_streaming(
            &transport,
            &settings(),
            &ApiKey::new("compatible-key").unwrap(),
            &request(),
            &mut |_, delta| deltas.push(delta.to_owned()),
            &|| false,
        )
        .unwrap();

        assert_eq!(deltas, vec!["兼容", "流式"]);
        assert_eq!(result.text, "兼容流式");
        assert_eq!(
            transport.request.lock().unwrap().as_ref().unwrap().body["stream"],
            true
        );
    }

    #[test]
    fn does_not_accept_a_chat_stream_without_a_completion_marker() {
        let transport = StreamingTransport {
            events: vec![json!({
                "choices": [{"delta": {"content": "未完成"}}]
            })
            .to_string()],
            request: Mutex::new(None),
        };

        let error = generate_streaming(
            &transport,
            &settings(),
            &ApiKey::new("compatible-key").unwrap(),
            &request(),
            &mut |_, _| {},
            &|| false,
        )
        .unwrap_err();

        assert_eq!(error.code, "ai_response_invalid");
    }
}
