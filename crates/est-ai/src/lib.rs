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
use est_application::ports::{LlmProvider, LlmRequest, LlmResponse, ProviderInfo};
use est_application::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::Url;

pub mod catalog;

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
                "stream": false,
            }),
            _ => json!({
                "model": self.model,
                "messages": messages,
                "max_tokens": req.max_tokens,
                "temperature": req.temperature,
                "stream": false,
            }),
        };
        let mut http = self.client.post(self.endpoint.clone()).json(&body);
        if let Some(key) = &self.api_key {
            http = http.bearer_auth(key);
        }
        let res = http
            .send()
            .await
            .map_err(|e| AppError::Infrastructure(format!("LLM provider unreachable: {e}")))?;
        if !res.status().is_success() {
            return Err(failure(res).await);
        }
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
            .ok_or_else(|| AppError::Infrastructure("LLM response has no content".into()))?;
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
        let messages: Vec<Value> = req
            .messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect();
        // Sampling options are not part of this API version; thinking is
        // decided by the model, so the limit gets reasoning headroom.
        let body = json!({
            "model": self.model,
            "system": req.system,
            "messages": messages,
            "max_tokens": req.max_tokens + REASONING_HEADROOM,
        });
        let res = self
            .client
            .post(self.endpoint.clone())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Infrastructure(format!("LLM provider unreachable: {e}")))?;
        if !res.status().is_success() {
            return Err(failure(res).await);
        }
        let m: AnthropicMessage = res
            .json()
            .await
            .map_err(|e| AppError::Infrastructure(format!("unexpected LLM response: {e}")))?;
        if m.stop_reason.as_deref() == Some("refusal") {
            return Err(AppError::Infrastructure(
                "the model declined to answer this request".into(),
            ));
        }
        let content: Vec<&str> = m
            .content
            .iter()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect();
        let content = content.join("\n\n");
        if content.trim().is_empty() {
            return Err(AppError::Infrastructure(
                "LLM response has no content".into(),
            ));
        }
        Ok(LlmResponse {
            content,
            model: m.model.unwrap_or_else(|| self.model.clone()),
        })
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
}
