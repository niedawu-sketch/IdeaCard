use std::{
    fmt::{self, Debug, Formatter},
    io::{BufRead, BufReader},
    time::Duration,
};

use serde_json::Value;
use url::Url;

use crate::domain::ai::{map_provider_failure, AiCoreError, ApiKey, ProviderFailure};

#[derive(Clone, PartialEq)]
pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

impl HttpRequest {
    pub fn post(url: impl Into<String>, headers: Vec<(String, String)>, body: Value) -> Self {
        Self {
            method: "POST",
            url: url.into(),
            headers,
            body,
        }
    }
}

impl Debug for HttpRequest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &"[REDACTED]")
            .field("body", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Value,
}

impl HttpResponse {
    pub fn json(status: u16, body: Value) -> Self {
        Self { status, body }
    }
}

impl Debug for HttpResponse {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("body", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportFailure {
    Cancelled,
    Timeout,
    Network,
    InvalidResponse,
}

pub trait AiTransport: Send + Sync {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportFailure>;

    fn send_stream(
        &self,
        request: HttpRequest,
        on_data: &mut dyn FnMut(&str),
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<u16, TransportFailure> {
        if is_cancelled() {
            return Err(TransportFailure::Cancelled);
        }
        let response = self.send(request)?;
        if (200..300).contains(&response.status) {
            on_data(&response.body.to_string());
        }
        Ok(response.status)
    }
}

pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub fn new() -> Result<Self, AiCoreError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| map_provider_failure(ProviderFailure::Network))?;
        Ok(Self { client })
    }
}

impl AiTransport for ReqwestTransport {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportFailure> {
        let mut builder = self.client.post(&request.url).json(&request.body);
        for (name, value) in request.headers {
            builder = builder.header(name, value);
        }
        let response = builder.send().map_err(|error| {
            if error.is_timeout() {
                TransportFailure::Timeout
            } else {
                TransportFailure::Network
            }
        })?;
        let status = response.status().as_u16();
        let body = response
            .json::<Value>()
            .map_err(|_| TransportFailure::InvalidResponse)?;
        Ok(HttpResponse::json(status, body))
    }

    fn send_stream(
        &self,
        request: HttpRequest,
        on_data: &mut dyn FnMut(&str),
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<u16, TransportFailure> {
        if is_cancelled() {
            return Err(TransportFailure::Cancelled);
        }
        let mut builder = self.client.post(&request.url).json(&request.body);
        for (name, value) in request.headers {
            builder = builder.header(name, value);
        }
        let response = builder.send().map_err(|error| {
            if error.is_timeout() {
                TransportFailure::Timeout
            } else {
                TransportFailure::Network
            }
        })?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Ok(status);
        }
        parse_sse(BufReader::new(response), on_data, is_cancelled)?;
        Ok(status)
    }
}

pub fn build_endpoint(base_url: &str, resource: &str) -> Result<String, AiCoreError> {
    let parsed = Url::parse(base_url).map_err(|_| invalid_base_url())?;
    let loopback_http = parsed.scheme() == "http"
        && cfg!(test)
        && parsed.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        });
    if parsed.scheme() != "https" && !loopback_http {
        return Err(invalid_base_url());
    }
    if parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(invalid_base_url());
    }
    let resource = resource.trim_matches('/');
    if resource.is_empty() {
        return Err(invalid_base_url());
    }
    Ok(format!("{}/{}", base_url.trim_end_matches('/'), resource))
}

pub fn bearer_header(key: &ApiKey) -> (String, String) {
    (
        "authorization".to_owned(),
        format!("Bearer {}", key.expose()),
    )
}

pub fn json_content_type_header() -> (String, String) {
    ("content-type".to_owned(), "application/json".to_owned())
}

pub fn execute(transport: &dyn AiTransport, request: HttpRequest) -> Result<Value, AiCoreError> {
    let response = transport.send(request).map_err(|failure| {
        map_provider_failure(match failure {
            TransportFailure::Cancelled => ProviderFailure::Cancelled,
            TransportFailure::Timeout => ProviderFailure::Timeout,
            TransportFailure::Network => ProviderFailure::Network,
            TransportFailure::InvalidResponse => ProviderFailure::InvalidResponse,
        })
    })?;
    if (200..300).contains(&response.status) {
        return Ok(response.body);
    }
    Err(map_http_failure(response.status))
}

pub fn execute_stream(
    transport: &dyn AiTransport,
    request: HttpRequest,
    on_data: &mut dyn FnMut(&str),
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), AiCoreError> {
    let status = transport
        .send_stream(request, on_data, is_cancelled)
        .map_err(map_transport_failure)?;
    if (200..300).contains(&status) {
        return Ok(());
    }
    Err(map_http_failure(status))
}

fn parse_sse(
    reader: impl BufRead,
    on_data: &mut dyn FnMut(&str),
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), TransportFailure> {
    let mut data_lines = Vec::new();
    for line in reader.lines() {
        if is_cancelled() {
            return Err(TransportFailure::Cancelled);
        }
        let line = line.map_err(|error| match error.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
                TransportFailure::Timeout
            }
            std::io::ErrorKind::InvalidData => TransportFailure::InvalidResponse,
            _ => TransportFailure::Network,
        })?;
        if line.is_empty() {
            dispatch_sse_data(&mut data_lines, on_data);
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.strip_prefix(' ').unwrap_or(data).to_owned());
        }
    }
    if is_cancelled() {
        return Err(TransportFailure::Cancelled);
    }
    dispatch_sse_data(&mut data_lines, on_data);
    Ok(())
}

fn dispatch_sse_data(data_lines: &mut Vec<String>, on_data: &mut dyn FnMut(&str)) {
    if data_lines.is_empty() {
        return;
    }
    let data = data_lines.join("\n");
    data_lines.clear();
    on_data(&data);
}

fn map_transport_failure(failure: TransportFailure) -> AiCoreError {
    map_provider_failure(match failure {
        TransportFailure::Cancelled => ProviderFailure::Cancelled,
        TransportFailure::Timeout => ProviderFailure::Timeout,
        TransportFailure::Network => ProviderFailure::Network,
        TransportFailure::InvalidResponse => ProviderFailure::InvalidResponse,
    })
}

fn map_http_failure(status: u16) -> AiCoreError {
    let failure = match status {
        400 | 404 | 405 | 409 | 422 => ProviderFailure::InvalidRequest,
        401 | 403 => ProviderFailure::Unauthorized,
        429 => ProviderFailure::RateLimited,
        500..=599 => ProviderFailure::Unavailable,
        _ => ProviderFailure::InvalidResponse,
    };
    map_provider_failure(failure)
}

fn invalid_base_url() -> AiCoreError {
    AiCoreError::new(
        "ai_base_url_invalid",
        "AI 服务地址无效",
        "使用有效的 HTTPS 地址",
    )
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, sync::Mutex};

    use serde_json::json;

    use super::{
        build_endpoint, execute, parse_sse, AiTransport, HttpRequest, HttpResponse,
        TransportFailure,
    };
    use crate::domain::ai::ApiKey;

    const SECRET: &str = "test-provider-secret";
    const RESPONSE_SECRET: &str = "provider-body-must-not-leak";

    struct FakeTransport {
        response: Result<HttpResponse, TransportFailure>,
        requests: Mutex<Vec<HttpRequest>>,
    }

    impl AiTransport for FakeTransport {
        fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportFailure> {
            self.requests.lock().unwrap().push(request);
            self.response.clone()
        }
    }

    #[test]
    fn endpoint_requires_https_but_allows_test_loopback_http() {
        assert_eq!(
            build_endpoint("https://api.openai.com/v1/", "responses").unwrap(),
            "https://api.openai.com/v1/responses"
        );
        assert_eq!(
            build_endpoint("http://127.0.0.1:4312/v1", "responses").unwrap(),
            "http://127.0.0.1:4312/v1/responses"
        );
        assert_eq!(
            build_endpoint("http://localhost:4312/v1", "responses").unwrap(),
            "http://localhost:4312/v1/responses"
        );

        let insecure = build_endpoint("http://example.com/v1", "responses").unwrap_err();
        assert_eq!(insecure.code, "ai_base_url_invalid");
        assert!(build_endpoint("https://user:pass@example.com", "responses").is_err());
    }

    #[test]
    fn request_debug_redacts_credentials_and_body() {
        let request = HttpRequest::post(
            "https://example.com/v1/responses",
            vec![
                ("authorization".to_owned(), format!("Bearer {SECRET}")),
                ("x-api-key".to_owned(), SECRET.to_owned()),
            ],
            json!({"input": SECRET}),
        );

        let debug = format!("{request:?}");
        assert!(!debug.contains(SECRET));
        assert!(!debug.contains("Bearer"));
        assert!(!debug.contains("input"));
    }

    #[test]
    fn execute_maps_http_status_without_exposing_provider_body() {
        let cases = [
            (400, "ai_request_invalid"),
            (401, "ai_authentication_failed"),
            (403, "ai_authentication_failed"),
            (429, "ai_rate_limited"),
            (500, "ai_provider_unavailable"),
        ];

        for (status, code) in cases {
            let transport = FakeTransport {
                response: Ok(HttpResponse::json(
                    status,
                    json!({"error": RESPONSE_SECRET}),
                )),
                requests: Mutex::new(Vec::new()),
            };
            let error = execute(
                &transport,
                HttpRequest::post("https://example.com", Vec::new(), json!({})),
            )
            .unwrap_err();

            assert_eq!(error.code, code);
            assert!(!format!("{error:?}").contains(RESPONSE_SECRET));
        }
    }

    #[test]
    fn transport_failures_use_the_same_stable_error_contract() {
        let transport = FakeTransport {
            response: Err(TransportFailure::Timeout),
            requests: Mutex::new(Vec::new()),
        };
        let error = execute(
            &transport,
            HttpRequest::post("https://example.com", Vec::new(), json!({})),
        )
        .unwrap_err();

        assert_eq!(error.code, "ai_request_timeout");
    }

    #[test]
    fn sse_parser_dispatches_complete_data_events_and_ignores_metadata() {
        let body = concat!(
            ": keep-alive\n",
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\n",
            "data: \"delta\":\"第一段\"}\n\n",
            "data: [DONE]\n\n",
        );
        let mut events = Vec::new();

        parse_sse(
            Cursor::new(body),
            &mut |data| events.push(data.to_owned()),
            &|| false,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![
                "{\"type\":\"response.output_text.delta\",\n\"delta\":\"第一段\"}",
                "[DONE]",
            ]
        );
    }

    #[test]
    fn sse_parser_stops_before_dispatch_when_cancelled() {
        let mut called = false;
        let error = parse_sse(
            Cursor::new("data: first\n\ndata: second\n\n"),
            &mut |_| called = true,
            &|| true,
        )
        .unwrap_err();

        assert_eq!(error, TransportFailure::Cancelled);
        assert!(!called);
    }

    #[test]
    fn authorization_header_is_built_from_redacted_key_only_at_dispatch_boundary() {
        let key = ApiKey::new(SECRET).unwrap();
        let header = super::bearer_header(&key);

        assert_eq!(header.0, "authorization");
        assert_eq!(header.1, format!("Bearer {SECRET}"));
        assert!(!format!("{key:?}").contains(SECRET));
    }
}
