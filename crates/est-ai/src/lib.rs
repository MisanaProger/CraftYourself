//! AI Gateway providers (architecture v0.3, §8.1).
//!
//! [`OpenAiCompatible`] speaks the `/chat/completions` protocol, which
//! covers Ollama (`http://127.0.0.1:11434/v1`), the llama.cpp server and
//! remote OpenAI-compatible APIs. The rest of EST only sees the
//! [`LlmProvider`] port and never depends on a vendor.

use std::time::Duration;

use async_trait::async_trait;
use est_application::ports::{LlmProvider, LlmRequest, LlmResponse, ProviderInfo};
use est_application::{AppError, AppResult};
use serde::Deserialize;
use serde_json::json;
use url::Url;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    /// Base URL ending before `/chat/completions`, e.g. `http://127.0.0.1:11434/v1`.
    pub base_url: String,
    pub model: String,
    /// Name of the environment variable holding the API key, if any. The key
    /// itself never goes into the config file.
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    120
}

pub struct OpenAiCompatible {
    client: reqwest::Client,
    endpoint: Url,
    model: String,
    api_key: Option<String>,
    remote: bool,
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

impl OpenAiCompatible {
    pub fn new(cfg: &ProviderConfig) -> AppResult<Self> {
        let base = Url::parse(cfg.base_url.trim_end_matches('/'))
            .map_err(|e| AppError::Invalid(format!("llm.base_url: {e}")))?;
        if !matches!(base.scheme(), "http" | "https") {
            return Err(AppError::Invalid("llm.base_url must be http(s)".into()));
        }
        let remote = is_remote(&base);
        if remote && base.scheme() != "https" {
            return Err(AppError::Invalid(
                "a remote LLM provider must use https".into(),
            ));
        }
        let endpoint = Url::parse(&format!(
            "{}/chat/completions",
            base.as_str().trim_end_matches('/')
        ))
        .map_err(|e| AppError::Invalid(e.to_string()))?;
        let api_key = match &cfg.api_key_env {
            Some(var) => Some(std::env::var(var).map_err(|_| {
                AppError::Invalid(format!(
                    "environment variable {var} with the LLM API key is not set"
                ))
            })?),
            None => None,
        };
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.timeout_secs))
            .build()
            .map_err(|e| AppError::Infrastructure(e.to_string()))?;
        Ok(Self {
            client,
            endpoint,
            model: cfg.model.clone(),
            api_key,
            remote,
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
            name: "openai-compatible".into(),
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
        let mut http = self.client.post(self.endpoint.clone()).json(&json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": req.max_tokens,
            "temperature": req.temperature,
            "stream": false,
        }));
        if let Some(key) = &self.api_key {
            http = http.bearer_auth(key);
        }
        let res = http
            .send()
            .await
            .map_err(|e| AppError::Infrastructure(format!("LLM provider unreachable: {e}")))?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            let body: String = body.chars().take(300).collect();
            return Err(AppError::Infrastructure(format!(
                "LLM provider returned {status}: {body}"
            )));
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
            .ok_or_else(|| AppError::Infrastructure("LLM response has no content".into()))?;
        Ok(LlmResponse {
            content,
            model: c.model.unwrap_or_else(|| self.model.clone()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, routing::post};
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
        };
        assert!(
            OpenAiCompatible::new(&cfg("http://api.example.com/v1")).is_err(),
            "remote must be https"
        );
        assert!(OpenAiCompatible::new(&cfg("ftp://localhost")).is_err());
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
}
