use serde_json::{json, Value};

use crate::{
    ai::provider::{
        build_endpoint, execute, execute_stream, json_content_type_header, AiTransport, HttpRequest,
    },
    domain::ai::{
        map_provider_failure, AiCoreError, AiDeltaKind, AiRequest, AiResult, AiRole, AiSettings,
        ApiKey, ProviderFailure,
    },
};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 2_048;

pub fn generate(
    transport: &dyn AiTransport,
    settings: &AiSettings,
    api_key: &ApiKey,
    request: &AiRequest,
) -> Result<AiResult, AiCoreError> {
    let endpoint = build_endpoint(&settings.base_url, "messages")?;
    let response = execute(
        transport,
        HttpRequest::post(
            endpoint,
            vec![
                json_content_type_header(),
                ("x-api-key".to_owned(), api_key.expose().to_owned()),
                ("anthropic-version".to_owned(), ANTHROPIC_VERSION.to_owned()),
            ],
            request_body(settings, request),
        ),
    )?;
    let content = response
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(invalid_response)?;
    let text = content
        .iter()
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if text.is_empty() {
        return Err(invalid_response());
    }
    Ok(AiResult {
        text,
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
    let endpoint = build_endpoint(&settings.base_url, "messages")?;
    let mut body = request_body(settings, request);
    body["stream"] = json!(true);
    let mut text = String::new();
    let mut completed = false;
    let mut stream_error = false;
    execute_stream(
        transport,
        HttpRequest::post(
            endpoint,
            vec![
                json_content_type_header(),
                ("x-api-key".to_owned(), api_key.expose().to_owned()),
                ("anthropic-version".to_owned(), ANTHROPIC_VERSION.to_owned()),
            ],
            body,
        ),
        &mut |data| {
            if stream_error {
                return;
            }
            let Ok(event) = serde_json::from_str::<Value>(data) else {
                stream_error = true;
                return;
            };
            match event.get("type").and_then(Value::as_str) {
                Some("content_block_delta") => {
                    let delta = event.get("delta");
                    if delta
                        .and_then(|delta| delta.get("type"))
                        .and_then(Value::as_str)
                        == Some("text_delta")
                    {
                        if let Some(value) = delta
                            .and_then(|delta| delta.get("text"))
                            .and_then(Value::as_str)
                        {
                            text.push_str(value);
                            on_delta(AiDeltaKind::Answer, value);
                        } else {
                            stream_error = true;
                        }
                    }
                }
                Some("message_stop") => completed = true,
                Some("error") => stream_error = true,
                _ => {}
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
    let messages = request
        .messages
        .iter()
        .map(|message| {
            json!({
                "role": match message.role {
                    AiRole::User => "user",
                    AiRole::Assistant => "assistant",
                },
                "content": message.content,
            })
        })
        .collect::<Vec<_>>();
    let mut body = json!({
        "model": settings.model,
        "max_tokens": DEFAULT_MAX_TOKENS,
        "messages": messages,
    });
    if !request.system_prompt.trim().is_empty() {
        body["system"] = json!(request.system_prompt);
    }
    body
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
            provider: AiProvider::Anthropic,
            base_url: "https://api.anthropic.com/v1".to_owned(),
            model: "claude-model".to_owned(),
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
    fn sends_messages_contract_and_parses_text_blocks() {
        let transport = FakeTransport {
            response: HttpResponse::json(
                200,
                json!({
                    "content": [
                        {"type": "text", "text": "第一段"},
                        {"type": "thinking", "thinking": "隐藏"},
                        {"type": "text", "text": "第二段"}
                    ]
                }),
            ),
            request: Mutex::new(None),
        };
        let settings = AiSettings {
            provider: AiProvider::Anthropic,
            base_url: "https://api.anthropic.com/v1".to_owned(),
            model: "claude-model".to_owned(),
            web_search_enabled: false,
        };
        let input = AiRequest {
            system_prompt: "系统指令".to_owned(),
            messages: vec![AiInputMessage {
                role: AiRole::User,
                content: "问题".to_owned(),
            }],
            web_search_enabled: false,
        };

        let result = generate(
            &transport,
            &settings,
            &ApiKey::new("claude-key").unwrap(),
            &input,
        )
        .unwrap();

        assert_eq!(result.text, "第一段\n\n第二段");
        assert!(result.citations.is_empty());
        let captured = transport.request.lock().unwrap().clone().unwrap();
        assert_eq!(captured.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(captured.body["system"], "系统指令");
        assert_eq!(
            captured.body["messages"],
            json!([{"role": "user", "content": "问题"}])
        );
        assert_eq!(
            captured
                .headers
                .iter()
                .find(|header| header.0 == "x-api-key")
                .unwrap()
                .1,
            "claude-key"
        );
        assert_eq!(
            captured
                .headers
                .iter()
                .find(|header| header.0 == "anthropic-version")
                .unwrap()
                .1,
            "2023-06-01"
        );
    }

    #[test]
    fn streams_anthropic_text_deltas_until_message_stop() {
        let transport = StreamingTransport {
            events: vec![
                json!({
                    "type": "content_block_delta",
                    "delta": {"type": "text_delta", "text": "Claude "}
                })
                .to_string(),
                json!({
                    "type": "content_block_delta",
                    "delta": {"type": "text_delta", "text": "流式"}
                })
                .to_string(),
                json!({"type": "message_stop"}).to_string(),
            ],
            request: Mutex::new(None),
        };
        let mut deltas = Vec::new();

        let result = generate_streaming(
            &transport,
            &settings(),
            &ApiKey::new("claude-key").unwrap(),
            &request(),
            &mut |_, delta| deltas.push(delta.to_owned()),
            &|| false,
        )
        .unwrap();

        assert_eq!(deltas, vec!["Claude ", "流式"]);
        assert_eq!(result.text, "Claude 流式");
        assert_eq!(
            transport.request.lock().unwrap().as_ref().unwrap().body["stream"],
            true
        );
    }

    #[test]
    fn does_not_accept_an_anthropic_stream_without_message_stop() {
        let transport = StreamingTransport {
            events: vec![json!({
                "type": "content_block_delta",
                "delta": {"type": "text_delta", "text": "未完成"}
            })
            .to_string()],
            request: Mutex::new(None),
        };

        let error = generate_streaming(
            &transport,
            &settings(),
            &ApiKey::new("claude-key").unwrap(),
            &request(),
            &mut |_, _| {},
            &|| false,
        )
        .unwrap_err();

        assert_eq!(error.code, "ai_response_invalid");
    }
}
