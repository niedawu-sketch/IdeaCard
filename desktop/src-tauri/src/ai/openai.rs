use std::collections::HashSet;

use serde_json::{json, Value};

use crate::{
    ai::provider::{
        bearer_header, build_endpoint, execute, execute_stream, json_content_type_header,
        AiTransport, HttpRequest,
    },
    domain::ai::{
        map_provider_failure, AiCitation, AiCoreError, AiDeltaKind, AiProvider, AiRequest,
        AiResult, AiRole, AiSettings, ApiKey, ProviderFailure,
    },
};

pub fn generate(
    transport: &dyn AiTransport,
    settings: &AiSettings,
    api_key: &ApiKey,
    request: &AiRequest,
) -> Result<AiResult, AiCoreError> {
    let endpoint = build_endpoint(&settings.base_url, "responses")?;
    let response = execute(
        transport,
        HttpRequest::post(
            endpoint,
            vec![json_content_type_header(), bearer_header(api_key)],
            request_body(settings, request),
        ),
    )?;
    parse_response(&response)
}

pub fn generate_streaming(
    transport: &dyn AiTransport,
    settings: &AiSettings,
    api_key: &ApiKey,
    request: &AiRequest,
    on_delta: &mut dyn FnMut(AiDeltaKind, &str),
    is_cancelled: &dyn Fn() -> bool,
) -> Result<AiResult, AiCoreError> {
    let endpoint = build_endpoint(&settings.base_url, "responses")?;
    let mut body = request_body(settings, request);
    body["stream"] = json!(true);
    let mut completed_response = None;
    let mut stream_error = false;
    let mut completed = false;
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
            let Ok(event) = serde_json::from_str::<Value>(data) else {
                stream_error = true;
                return;
            };
            match event.get("type").and_then(Value::as_str) {
                Some("response.output_text.delta") => {
                    if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                        on_delta(AiDeltaKind::Answer, delta);
                    } else {
                        stream_error = true;
                    }
                }
                Some("response.reasoning_text.delta") => {
                    if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                        on_delta(AiDeltaKind::Reasoning, delta);
                    } else {
                        stream_error = true;
                    }
                }
                Some("response.completed") => {
                    completed = true;
                    completed_response = event.get("response").cloned();
                }
                Some("response.failed" | "response.incomplete" | "error") => {
                    stream_error = true;
                }
                _ => {}
            }
        },
        is_cancelled,
    )?;
    if stream_error || !completed {
        return Err(invalid_response());
    }
    completed_response
        .as_ref()
        .ok_or_else(invalid_response)
        .and_then(parse_response)
}

fn request_body(settings: &AiSettings, request: &AiRequest) -> Value {
    let mut input = Vec::new();
    if !request.system_prompt.trim().is_empty() {
        input.push(json!({
            "role": "system",
            "content": [{"type": "input_text", "text": request.system_prompt}]
        }));
    }
    input.extend(request.messages.iter().map(|message| {
        json!({
            "role": match message.role {
                AiRole::User => "user",
                AiRole::Assistant => "assistant",
            },
            "content": [{"type": "input_text", "text": message.content}]
        })
    }));
    let mut body = json!({
        "model": settings.model,
        "input": input,
    });
    if settings.web_search_enabled && request.web_search_enabled {
        body["tools"] = json!([{"type": "web_search"}]);
        if settings.provider == AiProvider::DeepSeek {
            body["tool_choice"] = json!({"type": "web_search"});
        }
    }
    body
}

fn parse_response(response: &Value) -> Result<AiResult, AiCoreError> {
    let output = response
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(invalid_response)?;
    let mut output_parts = Vec::new();
    let mut citations = Vec::new();
    let mut seen_urls = HashSet::new();

    for item in output {
        let item_type = item.get("type").and_then(Value::as_str);
        if item_type == Some("web_search_call") {
            collect_web_search_citations(item, &mut citations, &mut seen_urls);
            continue;
        }
        let Some(content) = item.get("content").and_then(Value::as_array) else {
            continue;
        };
        let expected_part_type = match item_type {
            Some("reasoning") => "reasoning_text",
            Some("message") => "output_text",
            _ => continue,
        };
        let mut item_text = Vec::new();
        for part in content {
            if part.get("type").and_then(Value::as_str) != Some(expected_part_type) {
                continue;
            }
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                let text = text.trim();
                if !text.is_empty() {
                    item_text.push(text.to_owned());
                }
            }
            if expected_part_type != "output_text" {
                continue;
            }
            let Some(annotations) = part.get("annotations").and_then(Value::as_array) else {
                continue;
            };
            for annotation in annotations {
                if annotation.get("type").and_then(Value::as_str) != Some("url_citation") {
                    continue;
                }
                let Some(url) = annotation
                    .get("url")
                    .and_then(Value::as_str)
                    .filter(|url| is_web_url(url))
                else {
                    continue;
                };
                let title = annotation
                    .get("title")
                    .and_then(Value::as_str)
                    .filter(|title| !title.trim().is_empty())
                    .map(str::to_owned);
                push_citation(url, title.as_deref(), &mut citations, &mut seen_urls);
            }
        }
        if !item_text.is_empty() {
            output_parts.push((item_type == Some("message"), item_text.join("\n\n")));
        }
    }

    let final_index = output_parts
        .iter()
        .rposition(|(is_message, text)| *is_message && !text.trim().is_empty())
        .ok_or_else(invalid_response)?;
    let text = output_parts[final_index].1.clone();
    let reasoning = output_parts
        .into_iter()
        .enumerate()
        .filter_map(|(index, (_, text))| (index != final_index).then_some(text))
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok(AiResult {
        text,
        reasoning,
        citations,
    })
}

fn collect_web_search_citations(
    item: &Value,
    citations: &mut Vec<AiCitation>,
    seen_urls: &mut HashSet<String>,
) {
    let Some(action) = item.get("action") else {
        return;
    };
    if let Some(sources) = action.get("sources").and_then(Value::as_array) {
        for source in sources {
            if source
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|source_type| source_type != "url")
            {
                continue;
            }
            if let Some(url) = source.get("url").and_then(Value::as_str) {
                let title = source
                    .get("title")
                    .and_then(Value::as_str)
                    .filter(|title| !title.trim().is_empty());
                push_citation(url, title, citations, seen_urls);
            }
        }
    }
    if let Some(url) = action.get("url").and_then(Value::as_str) {
        push_citation(url, None, citations, seen_urls);
    }
}

fn push_citation(
    url: &str,
    title: Option<&str>,
    citations: &mut Vec<AiCitation>,
    seen_urls: &mut HashSet<String>,
) {
    if !is_web_url(url) || !seen_urls.insert(url.to_owned()) {
        return;
    }
    let fallback_title = url::Url::parse(url)
        .ok()
        .and_then(|value| value.host_str().map(str::to_owned))
        .unwrap_or_else(|| url.to_owned());
    citations.push(AiCitation {
        url: url.to_owned(),
        title: title
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&fallback_title)
            .to_owned(),
    });
}

fn is_web_url(value: &str) -> bool {
    url::Url::parse(value)
        .map(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
        .unwrap_or(false)
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
        domain::ai::{
            AiDeltaKind, AiInputMessage, AiProvider, AiRequest, AiRole, AiSettings, ApiKey,
        },
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

    impl AiTransport for FakeTransport {
        fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportFailure> {
            *self.request.lock().unwrap() = Some(request);
            Ok(self.response.clone())
        }
    }

    fn request() -> AiRequest {
        AiRequest {
            system_prompt: "只解释当前概念".to_owned(),
            messages: vec![AiInputMessage {
                role: AiRole::User,
                content: "解释：涌现".to_owned(),
            }],
            web_search_enabled: true,
        }
    }

    #[test]
    fn sends_responses_request_with_optional_web_search_tool() {
        let transport = FakeTransport {
            response: HttpResponse::json(
                200,
                json!({
                    "output": [{
                        "type": "message",
                        "content": [{"type": "output_text", "text": "解释结果", "annotations": []}]
                    }]
                }),
            ),
            request: Mutex::new(None),
        };
        let settings = AiSettings {
            web_search_enabled: true,
            ..AiSettings::default()
        };

        generate(
            &transport,
            &settings,
            &ApiKey::new("test-api-key").unwrap(),
            &request(),
        )
        .unwrap();

        let captured = transport.request.lock().unwrap().clone().unwrap();
        assert_eq!(captured.url, "https://api.openai.com/v1/responses");
        assert_eq!(
            captured
                .headers
                .iter()
                .find(|header| header.0 == "authorization")
                .unwrap()
                .1,
            "Bearer test-api-key"
        );
        assert_eq!(captured.body["model"], "gpt-5.6");
        assert_eq!(captured.body["tools"], json!([{"type": "web_search"}]));
        assert_eq!(captured.body["input"][0]["role"], "system");
        assert_eq!(
            captured.body["input"][0]["content"][0],
            json!({"type": "input_text", "text": "只解释当前概念"})
        );
    }

    #[test]
    fn deepseek_v4_flash_forces_server_side_web_search() {
        let transport = FakeTransport {
            response: HttpResponse::json(
                200,
                json!({
                    "output": [{
                        "type": "message",
                        "content": [{"type": "output_text", "text": "DeepSeek 联网结果", "annotations": []}]
                    }]
                }),
            ),
            request: Mutex::new(None),
        };
        let settings = AiSettings {
            provider: AiProvider::DeepSeek,
            base_url: "https://api.deepseek.com".to_owned(),
            model: "deepseek-v4-flash".to_owned(),
            web_search_enabled: true,
        };

        generate(
            &transport,
            &settings,
            &ApiKey::new("deepseek-key").unwrap(),
            &request(),
        )
        .unwrap();

        let captured = transport.request.lock().unwrap().clone().unwrap();
        assert_eq!(captured.url, "https://api.deepseek.com/responses");
        assert_eq!(captured.body["tools"], json!([{"type": "web_search"}]));
        assert_eq!(captured.body["tool_choice"], json!({"type": "web_search"}));
    }

    #[test]
    fn parses_output_text_and_unique_url_citations_in_source_order() {
        let transport = FakeTransport {
            response: HttpResponse::json(
                200,
                json!({
                    "output": [
                        {"type": "web_search_call", "status": "completed"},
                        {
                            "type": "message",
                            "content": [{
                                "type": "output_text",
                                "text": "第一段",
                                "annotations": [
                                    {
                                        "type": "url_citation",
                                        "url": "https://example.com/one",
                                        "title": "来源一",
                                        "start_index": 0,
                                        "end_index": 3
                                    },
                                    {
                                        "type": "url_citation",
                                        "url": "https://example.com/two",
                                        "title": "来源二"
                                    }
                                ]
                            }, {
                                "type": "output_text",
                                "text": "第二段",
                                "annotations": [{
                                    "type": "url_citation",
                                    "url": "https://example.com/one",
                                    "title": "重复来源"
                                }]
                            }]
                        }
                    ]
                }),
            ),
            request: Mutex::new(None),
        };

        let result = generate(
            &transport,
            &AiSettings::default(),
            &ApiKey::new("test-api-key").unwrap(),
            &request(),
        )
        .unwrap();

        assert_eq!(result.text, "第一段\n\n第二段");
        assert_eq!(result.citations.len(), 2);
        assert_eq!(result.citations[0].title, "来源一");
        assert_eq!(result.citations[1].url, "https://example.com/two");
    }

    #[test]
    fn separates_reasoning_and_intermediate_messages_and_parses_search_sources() {
        let transport = FakeTransport {
            response: HttpResponse::json(
                200,
                json!({
                    "output": [
                        {
                            "type": "reasoning",
                            "content": [{
                                "type": "reasoning_text",
                                "text": "先确认需要查证的时间点。"
                            }]
                        },
                        {
                            "type": "message",
                            "content": [{
                                "type": "output_text",
                                "text": "直接访问被拦截，改用检索结果交叉验证。",
                                "annotations": []
                            }]
                        },
                        {
                            "type": "web_search_call",
                            "status": "completed",
                            "action": {
                                "type": "search",
                                "sources": [
                                    {
                                        "type": "url",
                                        "url": "https://api-docs.deepseek.com/guides/responses_api/",
                                        "title": "DeepSeek Responses API"
                                    },
                                    {
                                        "url": "https://example.com/without-type"
                                    },
                                    {
                                        "type": "url",
                                        "url": "javascript:alert(1)"
                                    }
                                ]
                            }
                        },
                        {
                            "type": "message",
                            "content": [{
                                "type": "output_text",
                                "text": "## 结论\n\n最终回答。",
                                "annotations": [{
                                    "type": "url_citation",
                                    "url": "https://api-docs.deepseek.com/guides/responses_api/",
                                    "title": "重复来源"
                                }]
                            }]
                        }
                    ]
                }),
            ),
            request: Mutex::new(None),
        };

        let result = generate(
            &transport,
            &AiSettings::default(),
            &ApiKey::new("test-api-key").unwrap(),
            &request(),
        )
        .unwrap();

        assert_eq!(result.text, "## 结论\n\n最终回答。");
        assert_eq!(
            result.reasoning,
            "先确认需要查证的时间点。\n\n直接访问被拦截，改用检索结果交叉验证。"
        );
        assert_eq!(result.citations.len(), 2);
        assert_eq!(result.citations[0].title, "DeepSeek Responses API");
        assert_eq!(result.citations[1].title, "example.com");
    }

    #[test]
    fn rejects_success_responses_without_output_text() {
        let transport = FakeTransport {
            response: HttpResponse::json(200, json!({"output": []})),
            request: Mutex::new(None),
        };

        let error = generate(
            &transport,
            &AiSettings::default(),
            &ApiKey::new("test-api-key").unwrap(),
            &request(),
        )
        .unwrap_err();

        assert_eq!(error.code, "ai_response_invalid");
    }

    #[test]
    fn streams_responses_deltas_and_uses_the_completed_response_for_citations() {
        let transport = StreamingTransport {
            events: vec![
                json!({
                    "type": "response.reasoning_text.delta",
                    "delta": "查证"
                })
                .to_string(),
                json!({
                    "type": "response.output_text.delta",
                    "delta": "解释"
                })
                .to_string(),
                json!({
                    "type": "response.output_text.delta",
                    "delta": "结果"
                })
                .to_string(),
                json!({
                    "type": "response.completed",
                    "response": {
                        "output": [
                            {
                                "type": "reasoning",
                                "content": [{
                                    "type": "reasoning_text",
                                    "text": "查证"
                                }]
                            },
                            {
                                "type": "message",
                                "content": [{
                                    "type": "output_text",
                                    "text": "解释结果",
                                    "annotations": [{
                                        "type": "url_citation",
                                        "url": "https://example.com/source",
                                        "title": "参考来源"
                                    }]
                                }]
                            }
                        ]
                    }
                })
                .to_string(),
            ],
            request: Mutex::new(None),
        };
        let mut deltas = Vec::new();

        let result = generate_streaming(
            &transport,
            &AiSettings::default(),
            &ApiKey::new("test-api-key").unwrap(),
            &request(),
            &mut |kind, delta| deltas.push((kind, delta.to_owned())),
            &|| false,
        )
        .unwrap();

        assert_eq!(
            deltas,
            vec![
                (AiDeltaKind::Reasoning, "查证".to_owned()),
                (AiDeltaKind::Answer, "解释".to_owned()),
                (AiDeltaKind::Answer, "结果".to_owned()),
            ]
        );
        assert_eq!(result.text, "解释结果");
        assert_eq!(result.reasoning, "查证");
        assert_eq!(result.citations[0].title, "参考来源");
        assert_eq!(
            transport.request.lock().unwrap().as_ref().unwrap().body["stream"],
            true
        );
    }

    #[test]
    fn deepseek_streams_response_and_preserves_web_citations() {
        let transport = StreamingTransport {
            events: vec![
                json!({
                    "type": "response.output_text.delta",
                    "delta": "联网"
                })
                .to_string(),
                json!({
                    "type": "response.completed",
                    "response": {
                        "output": [{
                            "type": "message",
                            "content": [{
                                "type": "output_text",
                                "text": "联网结果",
                                "annotations": [{
                                    "type": "url_citation",
                                    "url": "https://example.com/deepseek-source",
                                    "title": "DeepSeek 来源"
                                }]
                            }]
                        }]
                    }
                })
                .to_string(),
            ],
            request: Mutex::new(None),
        };
        let settings = AiSettings {
            provider: AiProvider::DeepSeek,
            base_url: "https://api.deepseek.com".to_owned(),
            model: "deepseek-v4-flash".to_owned(),
            web_search_enabled: true,
        };
        let mut deltas = Vec::new();

        let result = generate_streaming(
            &transport,
            &settings,
            &ApiKey::new("deepseek-key").unwrap(),
            &request(),
            &mut |kind, delta| deltas.push((kind, delta.to_owned())),
            &|| false,
        )
        .unwrap();

        let captured = transport.request.lock().unwrap().clone().unwrap();
        assert_eq!(captured.url, "https://api.deepseek.com/responses");
        assert_eq!(captured.body["tools"], json!([{"type": "web_search"}]));
        assert_eq!(captured.body["tool_choice"], json!({"type": "web_search"}));
        assert_eq!(deltas, vec![(AiDeltaKind::Answer, "联网".to_owned())]);
        assert_eq!(result.text, "联网结果");
        assert_eq!(result.citations[0].title, "DeepSeek 来源");
    }

    #[test]
    fn rejects_incomplete_or_cancelled_response_streams() {
        let transport = StreamingTransport {
            events: vec![json!({
                "type": "response.output_text.delta",
                "delta": "未完成"
            })
            .to_string()],
            request: Mutex::new(None),
        };
        let incomplete = generate_streaming(
            &transport,
            &AiSettings::default(),
            &ApiKey::new("test-api-key").unwrap(),
            &request(),
            &mut |_, _| {},
            &|| false,
        )
        .unwrap_err();
        assert_eq!(incomplete.code, "ai_response_invalid");

        let cancelled = generate_streaming(
            &transport,
            &AiSettings::default(),
            &ApiKey::new("test-api-key").unwrap(),
            &request(),
            &mut |_, _| {},
            &|| true,
        )
        .unwrap_err();
        assert_eq!(cancelled.code, "ai_request_cancelled");
    }
}
