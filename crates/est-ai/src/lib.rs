//! AI Gateway providers (architecture v0.3, §8.1).
//!
//! Three wire protocols cover the supported providers:
//!
//! * [`Protocol::OpenAiCompatible`] — `/chat/completions` as spoken by
//!   Ollama, the llama.cpp server, Gemini, DeepSeek, Kimi, Qwen and others;
//! * [`Protocol::OpenAi`] — the same endpoint on the OpenAI platform, whose
//!   reasoning models take `max_completion_tokens` and no sampling options;
//! * [`Protocol::Anthropic`] — the Claude Messages API.
//!
//! The rest of EST only sees the [`LlmProvider`] port and never depends on a
//! vendor. Presets for the built-in providers live in [`catalog`].

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use est_application::ports::{DeltaSink, LlmProvider, LlmRequest, LlmResponse, ProviderInfo};
use est_application::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::Url;

pub mod catalog;
mod sse;

use sse::SseParser;

/// Wire protocol of a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Protocol {
    #[default]
    #[serde(rename = "openai-compatible")]
    OpenAiCompatible,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "anthropic")]
    Anthropic,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openai-compatible",
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
        }
    }
}

/// Claude Messages API version sent in `anthropic-version`.
pub const ANTHROPIC_VERSION: &str = "2026-10-21";

/// Extra output tokens for models that think before answering: their
/// reasoning counts against the limit, and the answer would otherwise come
/// back empty. The policy's code budget is still enforced on the answer
/// itself (`check_answer`), so this does not loosen any limit.
const REASONING_HEADROOM: u32 = 8000;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    /// Base URL ending before `/chat/completions` (or `/messages`), e.g.
    /// `http://127.0.0.1:11434/v1`.
    pub base_url: String,
    pub model: String,
    /// Name of the environment variable holding the API key, if any. The key
    /// itself never goes into the config file.
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub protocol: Protocol,
    /// Display name (e.g. "Claude"); defaults to the protocol name.
    #[serde(default)]
    pub name: Option<String>,
}

fn default_timeout() -> u64 {
    120
}

impl ProviderConfig {
    /// The key from [`ProviderConfig::api_key_env`], if that variable is set.
    pub fn env_key(&self) -> Option<String> {
        self.api_key_env
            .as_deref()
            .and_then(|v| std::env::var(v).ok())
            .filter(|k| !k.trim().is_empty())
    }
}

/// Whether `url` points outside this machine (shown to the user, §20).
pub fn is_remote(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(d)) => d != "localhost",
        Some(url::Host::Ipv4(ip)) => !ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => !ip.is_loopback(),
        None => true,
    }
}

/// Parses a base URL: http(s) only, and https whenever data leaves the
/// machine. Returns the URL without a trailing slash and whether it is remote.
pub fn parse_base_url(base_url: &str) -> AppResult<(Url, bool)> {
    let base = Url::parse(base_url.trim().trim_end_matches('/'))
        .map_err(|e| AppError::Invalid(format!("llm.base_url: {e}")))?;
    if !matches!(base.scheme(), "http" | "https") {
        return Err(AppError::Invalid("llm.base_url must be http(s)".into()));
    }
    if !base.username().is_empty() || base.password().is_some() {
        return Err(AppError::Invalid(
            "llm.base_url must not contain credentials".into(),
        ));
    }
    let remote = is_remote(&base);
    if remote && base.scheme() != "https" {
        return Err(AppError::Invalid(
            "a remote LLM provider must use https".into(),
        ));
    }
    Ok((base, remote))
}

fn join(base: &Url, path: &str) -> AppResult<Url> {
    Url::parse(&format!("{}/{path}", base.as_str().trim_end_matches('/')))
        .map_err(|e| AppError::Invalid(e.to_string()))
}

fn http_client(timeout_secs: u64) -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| AppError::Infrastructure(e.to_string()))
}

/// Builds the provider for `cfg`. `key` (from the local key store) wins over
/// the environment variable named in the config.
pub fn connect(cfg: &ProviderConfig, key: Option<String>) -> AppResult<Arc<dyn LlmProvider>> {
    let key = key.or_else(|| cfg.env_key());
    Ok(match cfg.protocol {
        Protocol::Anthropic => Arc::new(AnthropicMessages::new(cfg, key)?),
        _ => Arc::new(OpenAiCompatible::with_key(cfg, key)?),
    })
}

fn unreachable(e: reqwest::Error) -> AppError {
    AppError::Infrastructure(format!("LLM provider unreachable: {e}"))
}

fn no_content() -> AppError {
    AppError::Infrastructure("LLM response has no content".into())
}

fn is_event_stream(res: &reqwest::Response) -> bool {
    res.headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("text/event-stream"))
}

/// Reads server-sent events, calling `on_event` with each `data` payload
/// until it returns `false`.
async fn read_events(
    mut res: reqwest::Response,
    mut on_event: impl FnMut(&str) -> AppResult<bool>,
) -> AppResult<()> {
    let mut parser = SseParser::default();
    while let Some(chunk) = res.chunk().await.map_err(unreachable)? {
        for data in parser.push(&chunk) {
            if !on_event(&data)? {
                return Ok(());
            }
        }
    }
    for data in parser.finish() {
        if !on_event(&data)? {
            break;
        }
    }
    Ok(())
}

/// Turns a failed HTTP response into an error without echoing headers.
async fn failure(res: reqwest::Response) -> AppError {
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    let body: String = body.chars().take(300).collect();
    let hint = match status.as_u16() {
        401 | 403 => " (check the API key and that it matches this endpoint)",
        404 => " (check the base URL and the model name)",
        429 => " (rate limit or plan quota reached)",
        _ => "",
    };
    AppError::Infrastructure(format!("LLM provider returned {status}{hint}: {body}"))
}

// ---- OpenAI and compatible ---------------------------------------------------

pub struct OpenAiCompatible {
    client: reqwest::Client,
    endpoint: Url,
    model: String,
    api_key: Option<String>,
    remote: bool,
    protocol: Protocol,
    name: String,
}

impl OpenAiCompatible {
    /// Reads the key from the environment variable named in `cfg`; a named
    /// but unset variable is an error.
    pub fn new(cfg: &ProviderConfig) -> AppResult<Self> {
        let api_key = match &cfg.api_key_env {
            Some(var) => Some(std::env::var(var).map_err(|_| {
                AppError::Invalid(format!(
                    "environment variable {var} with the LLM API key is not set"
                ))
            })?),
            None => None,
        };
        Self::with_key(cfg, api_key)
    }

    pub fn with_key(cfg: &ProviderConfig, api_key: Option<String>) -> AppResult<Self> {
        let (base, remote) = parse_base_url(&cfg.base_url)?;
        if cfg.model.trim().is_empty() {
            return Err(AppError::Invalid("llm.model must not be empty".into()));
        }
        Ok(Self {
            client: http_client(cfg.timeout_secs)?,
            endpoint: join(&base, "chat/completions")?,
            model: cfg.model.trim().to_owned(),
            api_key,
            remote,
            protocol: cfg.protocol,
            name: cfg
                .name
                .clone()
                .unwrap_or_else(|| cfg.protocol.as_str().to_owned()),
        })
    }
}

#[derive(Deserialize)]
struct Completion {
    choices: Vec<Choice>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

#[async_trait]
impl LlmProvider for OpenAiCompatible {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: self.name.clone(),
            model: self.model.clone(),
            endpoint: self.endpoint.to_string(),
            remote: self.remote,
        }
    }

    async fn generate(&self, req: LlmRequest) -> AppResult<LlmResponse> {
        let res = self.send(&req, false).await?;
        self.completion(res).await
    }

    async fn generate_stream(
        &self,
        req: LlmRequest,
        on_delta: DeltaSink<'_>,
    ) -> AppResult<LlmResponse> {
        let res = self.send(&req, true).await?;
        // A server that ignores `stream` answers with one JSON document.
        if !is_event_stream(&res) {
            let r = self.completion(res).await?;
            on_delta(&r.content);
            return Ok(r);
        }
        let mut content = String::new();
        let mut model = None;
        read_events(res, |data| {
            if data.trim() == "[DONE]" {
                return Ok(false);
            }
            let v: Value = serde_json::from_str(data).map_err(|e| {
                AppError::Infrastructure(format!("unexpected LLM stream event: {e}"))
            })?;
            if let Some(err) = v.get("error") {
                let msg = err["message"].as_str().unwrap_or("unknown error");
                return Err(AppError::Infrastructure(format!(
                    "LLM provider failed while answering: {msg}"
                )));
            }
            if model.is_none() {
                model = v["model"].as_str().map(str::to_owned);
            }
            if let Some(text) = v["choices"][0]["delta"]["content"].as_str()
                && !text.is_empty()
            {
                content.push_str(text);
                on_delta(text);
            }
            Ok(true)
        })
        .await?;
        if content.trim().is_empty() {
            return Err(no_content());
        }
        Ok(LlmResponse {
            content,
            model: model.unwrap_or_else(|| self.model.clone()),
        })
    }
}

impl OpenAiCompatible {
    async fn send(&self, req: &LlmRequest, stream: bool) -> AppResult<reqwest::Response> {
        let mut messages = vec![json!({ "role": "system", "content": req.system })];
        messages.extend(
            req.messages
                .iter()
                .map(|m| json!({ "role": m.role, "content": m.content })),
        );
        let body = match self.protocol {
            // Reasoning models on the OpenAI platform reject `max_tokens` and
            // non-default sampling.
            Protocol::OpenAi => json!({
                "model": self.model,
                "messages": messages,
                "max_completion_tokens": req.max_tokens + REASONING_HEADROOM,
                "stream": stream,
            }),
            _ => json!({
                "model": self.model,
                "messages": messages,
                "max_tokens": req.max_tokens,
                "temperature": req.temperature,
                "stream": stream,
            }),
        };
        let mut http = self.client.post(self.endpoint.clone()).json(&body);
        if let Some(key) = &self.api_key {
            http = http.bearer_auth(key);
        }
        let res = http.send().await.map_err(unreachable)?;
        if !res.status().is_success() {
            return Err(failure(res).await);
        }
        Ok(res)
    }

    async fn completion(&self, res: reqwest::Response) -> AppResult<LlmResponse> {
        let c: Completion = res
            .json()
            .await
            .map_err(|e| AppError::Infrastructure(format!("unexpected LLM response: {e}")))?;
        let content = c
            .choices
            .into_iter()
            .next()
            .and_then(|ch| ch.message.content)
            .filter(|c| !c.trim().is_empty())
            .ok_or_else(no_content)?;
        Ok(LlmResponse {
            content,
            model: c.model.unwrap_or_else(|| self.model.clone()),
        })
    }
}

// ---- Claude Messages API -----------------------------------------------------

pub struct AnthropicMessages {
    client: reqwest::Client,
    endpoint: Url,
    model: String,
    api_key: String,
    remote: bool,
    name: String,
}

impl AnthropicMessages {
    pub fn new(cfg: &ProviderConfig, api_key: Option<String>) -> AppResult<Self> {
        let (base, remote) = parse_base_url(&cfg.base_url)?;
        if cfg.model.trim().is_empty() {
            return Err(AppError::Invalid("llm.model must not be empty".into()));
        }
        let api_key = api_key
            .ok_or_else(|| AppError::Invalid("the Claude API requires an API key".into()))?;
        Ok(Self {
            client: http_client(cfg.timeout_secs)?,
            endpoint: join(&base, "messages")?,
            model: cfg.model.trim().to_owned(),
            api_key,
            remote,
            name: cfg.name.clone().unwrap_or_else(|| "anthropic".into()),
        })
    }
}

#[derive(Deserialize)]
struct AnthropicMessage {
    #[serde(default)]
    content: Vec<Value>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[async_trait]
impl LlmProvider for AnthropicMessages {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: self.name.clone(),
            model: self.model.clone(),
            endpoint: self.endpoint.to_string(),
            remote: self.remote,
        }
    }

    async fn generate(&self, req: LlmRequest) -> AppResult<LlmResponse> {
        let res = self.send(&req, false).await?;
        let m: AnthropicMessage = res
            .json()
            .await
            .map_err(|e| AppError::Infrastructure(format!("unexpected LLM response: {e}")))?;
        if m.stop_reason.as_deref() == Some("refusal") {
            return Err(refusal());
        }
        let content: Vec<&str> = m
            .content
            .iter()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect();
        let content = content.join("\n\n");
        if content.trim().is_empty() {
            return Err(no_content());
        }
        Ok(LlmResponse {
            content,
            model: m.model.unwrap_or_else(|| self.model.clone()),
        })
    }

    async fn generate_stream(
        &self,
        req: LlmRequest,
        on_delta: DeltaSink<'_>,
    ) -> AppResult<LlmResponse> {
        let res = self.send(&req, true).await?;
        let mut content = String::new();
        let mut model = None;
        let mut text_blocks = 0;
        let mut stop_reason = None;
        read_events(res, |data| {
            let v: Value = serde_json::from_str(data).map_err(|e| {
                AppError::Infrastructure(format!("unexpected LLM stream event: {e}"))
            })?;
            let mut emit = |text: &str| {
                content.push_str(text);
                on_delta(text);
            };
            match v["type"].as_str().unwrap_or("") {
                "message_start" => model = v["message"]["model"].as_str().map(str::to_owned),
                // Text blocks are joined as in a non-streamed answer.
                "content_block_start" if v["content_block"]["type"] == "text" => {
                    if text_blocks > 0 {
                        emit("\n\n");
                    }
                    text_blocks += 1;
                }
                "content_block_delta" if v["delta"]["type"] == "text_delta" => {
                    if let Some(t) = v["delta"]["text"].as_str() {
                        emit(t);
                    }
                }
                "message_delta" => {
                    if let Some(r) = v["delta"]["stop_reason"].as_str() {
                        stop_reason = Some(r.to_owned());
                    }
                }
                "message_stop" => return Ok(false),
                "error" => {
                    let msg = v["error"]["message"].as_str().unwrap_or("unknown error");
                    return Err(AppError::Infrastructure(format!(
                        "LLM provider failed while answering: {msg}"
                    )));
                }
                _ => {}
            }
            Ok(true)
        })
        .await?;
        if stop_reason.as_deref() == Some("refusal") {
            return Err(refusal());
        }
        if content.trim().is_empty() {
            return Err(no_content());
        }
        Ok(LlmResponse {
            content,
            model: model.unwrap_or_else(|| self.model.clone()),
        })
    }
}

fn refusal() -> AppError {
    AppError::Infrastructure("the model declined to answer this request".into())
}

impl AnthropicMessages {
    async fn send(&self, req: &LlmRequest, stream: bool) -> AppResult<reqwest::Response> {
        let messages: Vec<Value> = req
            .messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect();
        // Sampling options are not part of this API version; thinking is
        // decided by the model, so the limit gets reasoning headroom.
        let mut body = json!({
            "model": self.model,
            "system": req.system,
            "messages": messages,
            "max_tokens": req.max_tokens + REASONING_HEADROOM,
        });
        if stream {
            body["stream"] = json!(true);
        }
        let res = self
            .client
            .post(self.endpoint.clone())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(unreachable)?;
        if !res.status().is_success() {
            return Err(failure(res).await);
        }
        Ok(res)
    }
}

// ---- model listing -----------------------------------------------------------

/// Model ids the provider offers (`GET {base}/models`), sorted.
pub async fn list_models(
    protocol: Protocol,
    base_url: &str,
    api_key: Option<&str>,
) -> AppResult<Vec<String>> {
    let (base, _) = parse_base_url(base_url)?;
    let url = join(&base, "models")?;
    let client = http_client(20)?;
    let mut http = client.get(url);
    match (protocol, api_key) {
        (Protocol::Anthropic, Some(key)) => {
            http = http
                .header("x-api-key", key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .query(&[("limit", "100")]);
        }
        (_, Some(key)) => http = http.bearer_auth(key),
        (_, None) => {}
    }
    let res = http
        .send()
        .await
        .map_err(|e| AppError::Infrastructure(format!("LLM provider unreachable: {e}")))?;
    if !res.status().is_success() {
        return Err(failure(res).await);
    }
    let v: Value = res
        .json()
        .await
        .map_err(|e| AppError::Infrastructure(format!("unexpected models response: {e}")))?;
    let mut ids: Vec<String> = v["data"]
        .as_array()
        .or_else(|| v["models"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|m| m["id"].as_str().or_else(|| m["name"].as_str()))
        .map(|id| id.trim_start_matches("models/").to_owned())
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use axum::{
        Json, Router,
        routing::{get, post},
    };
    use est_application::ports::ChatMessage;
    use serde_json::Value;

    #[test]
    fn remote_detection_and_validation() {
        assert!(!is_remote(
            &Url::parse("http://127.0.0.1:11434/v1").unwrap()
        ));
        assert!(!is_remote(&Url::parse("http://localhost:8080").unwrap()));
        assert!(is_remote(
            &Url::parse("https://api.example.com/v1").unwrap()
        ));
        let cfg = |url: &str| ProviderConfig {
            base_url: url.into(),
            model: "m".into(),
            api_key_env: None,
            timeout_secs: 5,
            protocol: Protocol::OpenAiCompatible,
            name: None,
        };
        assert!(
            OpenAiCompatible::new(&cfg("http://api.example.com/v1")).is_err(),
            "remote must be https"
        );
        assert!(OpenAiCompatible::new(&cfg("ftp://localhost")).is_err());
        assert!(
            OpenAiCompatible::new(&cfg("https://user:pw@api.example.com/v1")).is_err(),
            "credentials in the URL"
        );
        let p = OpenAiCompatible::new(&cfg("http://127.0.0.1:11434/v1/")).unwrap();
        assert_eq!(
            p.info().endpoint,
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        assert!(!p.info().remote);
    }

    #[tokio::test]
    async fn talks_openai_protocol() {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|Json(body): Json<Value>| async move {
                assert_eq!(body["messages"][0]["role"], "system");
                assert_eq!(body["messages"][1]["content"], "hello");
                Json(json!({ "model": "tiny", "choices": [{ "message": { "role": "assistant", "content": "What did you try?" } }] }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let p = OpenAiCompatible::new(&ProviderConfig {
            base_url: format!("http://{addr}/v1"),
            model: "tiny".into(),
            api_key_env: None,
            timeout_secs: 5,
            protocol: Protocol::OpenAiCompatible,
            name: None,
        })
        .unwrap();
        let r = p
            .generate(LlmRequest {
                system: "be socratic".into(),
                messages: vec![ChatMessage {
                    role: "user".into(),
                    content: "hello".into(),
                }],
                max_tokens: 100,
                temperature: 0.2,
            })
            .await
            .unwrap();
        assert_eq!(r.content, "What did you try?");
        assert_eq!(r.model, "tiny");
    }

    async fn serve(app: Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/v1")
    }

    fn request() -> LlmRequest {
        LlmRequest {
            system: "be socratic".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "hello".into(),
            }],
            max_tokens: 100,
            temperature: 0.2,
        }
    }

    #[tokio::test]
    async fn openai_platform_omits_sampling() {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|headers: HeaderMap, Json(body): Json<Value>| async move {
                assert_eq!(headers["authorization"], "Bearer sk-test");
                assert!(body.get("temperature").is_none());
                assert!(body.get("max_tokens").is_none());
                assert!(body["max_completion_tokens"].as_u64().unwrap() > 100);
                Json(json!({ "choices": [{ "message": { "content": "Why?" } }] }))
            }),
        );
        let base = serve(app).await;
        let p = connect(
            &ProviderConfig {
                base_url: base,
                model: "gpt".into(),
                api_key_env: None,
                timeout_secs: 5,
                protocol: Protocol::OpenAi,
                name: Some("ChatGPT".into()),
            },
            Some("sk-test".into()),
        )
        .unwrap();
        assert_eq!(p.info().name, "ChatGPT");
        assert_eq!(p.generate(request()).await.unwrap().content, "Why?");
    }

    #[tokio::test]
    async fn talks_claude_messages_api() {
        let app = Router::new()
            .route(
                "/v1/messages",
                post(|headers: HeaderMap, Json(body): Json<Value>| async move {
                    assert_eq!(headers["x-api-key"], "sk-ant");
                    assert_eq!(headers["anthropic-version"], ANTHROPIC_VERSION);
                    assert_eq!(body["system"], "be socratic");
                    assert_eq!(body["messages"][0]["role"], "user");
                    assert!(body.get("temperature").is_none());
                    Json(json!({
                        "model": "claude-test",
                        "stop_reason": "end_turn",
                        "content": [
                            { "type": "thinking", "thinking": "" },
                            { "type": "text", "text": "What did you try?" }
                        ]
                    }))
                }),
            )
            .route(
                "/v1/models",
                get(|| async {
                    Json(json!({ "data": [{ "id": "claude-b" }, { "id": "claude-a" }] }))
                }),
            );
        let base = serve(app).await;
        let cfg = ProviderConfig {
            base_url: base.clone(),
            model: "claude-test".into(),
            api_key_env: None,
            timeout_secs: 5,
            protocol: Protocol::Anthropic,
            name: None,
        };
        assert!(connect(&cfg, None).is_err(), "a key is required");
        let p = connect(&cfg, Some("sk-ant".into())).unwrap();
        let r = p.generate(request()).await.unwrap();
        assert_eq!(r.content, "What did you try?");
        assert_eq!(r.model, "claude-test");
        let models = list_models(Protocol::Anthropic, &base, Some("sk-ant"))
            .await
            .unwrap();
        assert_eq!(models, ["claude-a", "claude-b"]);
    }

    fn sse(events: &[Value]) -> ([(axum::http::HeaderName, &'static str); 1], String) {
        let mut body = String::new();
        for e in events {
            body.push_str(&format!("data: {e}\n\n"));
        }
        (
            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
            body,
        )
    }

    async fn collect(p: &dyn LlmProvider) -> (LlmResponse, Vec<String>) {
        let deltas = std::sync::Mutex::new(Vec::new());
        let sink = |d: &str| deltas.lock().unwrap().push(d.to_owned());
        let r = p.generate_stream(request(), &sink).await.unwrap();
        (r, deltas.into_inner().unwrap())
    }

    fn openai(base: String) -> Arc<dyn LlmProvider> {
        connect(
            &ProviderConfig {
                base_url: base,
                model: "tiny".into(),
                api_key_env: None,
                timeout_secs: 5,
                protocol: Protocol::OpenAiCompatible,
                name: None,
            },
            None,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn streams_openai_protocol() {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|Json(body): Json<Value>| async move {
                assert_eq!(body["stream"], true);
                let mut events: Vec<Value> = ["What ", "did you ", "", "try?"]
                    .iter()
                    .map(|t| json!({ "model": "tiny-1", "choices": [{ "delta": { "content": t } }] }))
                    .collect();
                events.push(json!({ "choices": [{ "delta": {}, "finish_reason": "stop" }] }));
                let (h, mut body) = sse(&events);
                body.push_str("data: [DONE]\n\n");
                (h, body)
            }),
        );
        let (r, deltas) = collect(openai(serve(app).await).as_ref()).await;
        assert_eq!(deltas, ["What ", "did you ", "try?"]);
        assert_eq!(
            (r.content.as_str(), r.model.as_str()),
            ("What did you try?", "tiny-1")
        );
    }

    #[tokio::test]
    async fn falls_back_when_the_server_does_not_stream() {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async { Json(json!({ "choices": [{ "message": { "content": "Why?" } }] })) }),
        );
        let (r, deltas) = collect(openai(serve(app).await).as_ref()).await;
        assert_eq!(
            (r.content.as_str(), deltas),
            ("Why?", vec!["Why?".to_owned()])
        );
    }

    #[tokio::test]
    async fn streams_claude_messages_api() {
        let events = vec![
            json!({ "type": "message_start", "message": { "model": "claude-test" } }),
            json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "thinking" } }),
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "thinking_delta", "thinking": "hmm" } }),
            json!({ "type": "content_block_start", "index": 1, "content_block": { "type": "text" } }),
            json!({ "type": "content_block_delta", "index": 1, "delta": { "type": "text_delta", "text": "What did " } }),
            json!({ "type": "content_block_delta", "index": 1, "delta": { "type": "text_delta", "text": "you try?" } }),
            json!({ "type": "content_block_start", "index": 2, "content_block": { "type": "text" } }),
            json!({ "type": "content_block_delta", "index": 2, "delta": { "type": "text_delta", "text": "And why?" } }),
            json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" } }),
            json!({ "type": "message_stop" }),
        ];
        let app = Router::new().route(
            "/v1/messages",
            post(move |Json(body): Json<Value>| {
                let events = events.clone();
                async move {
                    assert_eq!(body["stream"], true);
                    sse(&events)
                }
            }),
        );
        let cfg = ProviderConfig {
            base_url: serve(app).await,
            model: "claude-test".into(),
            api_key_env: None,
            timeout_secs: 5,
            protocol: Protocol::Anthropic,
            name: None,
        };
        let p = connect(&cfg, Some("sk-ant".into())).unwrap();
        let (r, deltas) = collect(p.as_ref()).await;
        assert_eq!(deltas, ["What did ", "you try?", "\n\n", "And why?"]);
        assert_eq!(r.content, "What did you try?\n\nAnd why?");
        assert_eq!(r.model, "claude-test");
    }

    #[tokio::test]
    async fn stream_refusals_and_errors_fail() {
        let app = Router::new()
            .route(
                "/v1/messages",
                post(|| async {
                    sse(&[
                        json!({ "type": "content_block_start", "content_block": { "type": "text" } }),
                        json!({ "type": "content_block_delta", "delta": { "type": "text_delta", "text": "I" } }),
                        json!({ "type": "message_delta", "delta": { "stop_reason": "refusal" } }),
                        json!({ "type": "message_stop" }),
                    ])
                }),
            )
            .route(
                "/v1/chat/completions",
                post(|| async { sse(&[json!({ "error": { "message": "overloaded" } })]) }),
            );
        let base = serve(app).await;
        let cfg = ProviderConfig {
            base_url: base.clone(),
            model: "c".into(),
            api_key_env: None,
            timeout_secs: 5,
            protocol: Protocol::Anthropic,
            name: None,
        };
        let sink = |_: &str| {};
        let claude = connect(&cfg, Some("k".into())).unwrap();
        let e = claude.generate_stream(request(), &sink).await.unwrap_err();
        assert!(e.to_string().contains("declined"), "{e}");
        let e = openai(base)
            .generate_stream(request(), &sink)
            .await
            .unwrap_err();
        assert!(e.to_string().contains("overloaded"), "{e}");
    }
}
