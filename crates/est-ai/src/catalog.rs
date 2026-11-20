//! Built-in presets for cloud and local AI providers.
//!
//! A preset only pre-fills the form: protocol, base URL, suggested models and
//! where to get a key. Every value stays editable, and the live model list is
//! read from the provider (`GET /models`) when a key is available.

use serde::Serialize;

use crate::Protocol;

/// How the learner pays for / authenticates with a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    /// Pay-as-you-go platform API key.
    ApiKey,
    /// A subscription plan that issues its own API key for a dedicated
    /// endpoint (coding plans).
    Subscription,
    /// Local server, no key.
    None,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: &'static str,
    /// Product name shown in the UI.
    pub name: &'static str,
    pub vendor: &'static str,
    pub protocol: Protocol,
    pub auth: AuthKind,
    pub base_url: &'static str,
    /// Suggestions; the provider's own `/models` list wins when reachable.
    pub models: &'static [&'static str],
    /// Conventional environment variable for the key.
    pub api_key_env: Option<&'static str>,
    /// Where the learner creates a key or buys the plan.
    pub key_url: Option<&'static str>,
    /// Where the API is documented.
    pub docs_url: &'static str,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        id: "claude",
        name: "Claude",
        vendor: "Anthropic",
        protocol: Protocol::Anthropic,
        auth: AuthKind::ApiKey,
        base_url: "https://api.anthropic.com/v1",
        models: &["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"],
        api_key_env: Some("ANTHROPIC_API_KEY"),
        key_url: Some("https://console.anthropic.com/settings/keys"),
        docs_url: "https://docs.claude.com/en/api/messages",
    },
    Preset {
        id: "chatgpt",
        name: "ChatGPT",
        vendor: "OpenAI",
        protocol: Protocol::OpenAi,
        auth: AuthKind::ApiKey,
        base_url: "https://api.openai.com/v1",
        models: &["gpt-5", "gpt-5-mini", "gpt-4.1"],
        api_key_env: Some("OPENAI_API_KEY"),
        key_url: Some("https://platform.openai.com/api-keys"),
        docs_url: "https://platform.openai.com/docs/api-reference/chat",
    },
    Preset {
        id: "gemini",
        name: "Gemini",
        vendor: "Google",
        protocol: Protocol::OpenAiCompatible,
        auth: AuthKind::ApiKey,
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        models: &["gemini-2.5-pro", "gemini-2.5-flash"],
        api_key_env: Some("GEMINI_API_KEY"),
        key_url: Some("https://aistudio.google.com/apikey"),
        docs_url: "https://ai.google.dev/gemini-api/docs/openai",
    },
    Preset {
        id: "deepseek",
        name: "DeepSeek",
        vendor: "DeepSeek",
        protocol: Protocol::OpenAiCompatible,
        auth: AuthKind::ApiKey,
        base_url: "https://api.deepseek.com/v1",
        models: &["deepseek-chat", "deepseek-reasoner"],
        api_key_env: Some("DEEPSEEK_API_KEY"),
        key_url: Some("https://platform.deepseek.com/api_keys"),
        docs_url: "https://api-docs.deepseek.com/",
    },
    Preset {
        id: "kimi",
        name: "Kimi",
        vendor: "Moonshot AI",
        protocol: Protocol::OpenAiCompatible,
        auth: AuthKind::ApiKey,
        base_url: "https://api.moonshot.ai/v1",
        models: &[
            "kimi-k2-turbo-preview",
            "kimi-k2-0905-preview",
            "kimi-latest",
        ],
        api_key_env: Some("MOONSHOT_API_KEY"),
        key_url: Some("https://platform.moonshot.ai/console/api-keys"),
        docs_url: "https://platform.moonshot.ai/docs",
    },
    Preset {
        id: "qwen",
        name: "Qwen",
        vendor: "Alibaba Cloud",
        protocol: Protocol::OpenAiCompatible,
        auth: AuthKind::ApiKey,
        base_url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
        models: &["qwen3-coder-plus", "qwen-max", "qwen-plus"],
        api_key_env: Some("DASHSCOPE_API_KEY"),
        key_url: Some("https://modelstudio.console.alibabacloud.com/"),
        docs_url: "https://www.alibabacloud.com/help/en/model-studio/compatibility-of-openai-with-dashscope",
    },
    Preset {
        id: "kimi-code",
        name: "Kimi Code",
        vendor: "Moonshot AI",
        protocol: Protocol::OpenAiCompatible,
        auth: AuthKind::Subscription,
        base_url: "https://api.kimi.com/coding/v1",
        models: &["kimi-for-coding"],
        api_key_env: Some("KIMI_CODE_API_KEY"),
        key_url: Some("https://www.kimi.com/code"),
        docs_url: "https://www.kimi.com/code/docs/en/",
    },
    Preset {
        id: "qwen-coding-plan",
        name: "Qwen Coding Plan",
        vendor: "Alibaba Cloud",
        protocol: Protocol::OpenAiCompatible,
        auth: AuthKind::Subscription,
        base_url: "https://coding-intl.dashscope.aliyuncs.com/v1",
        models: &["qwen3-coder-plus", "qwen3.5-plus"],
        api_key_env: Some("QWEN_CODING_PLAN_API_KEY"),
        key_url: Some("https://www.alibabacloud.com/help/en/model-studio/coding-plan"),
        docs_url: "https://www.alibabacloud.com/help/en/model-studio/coding-plan-quickstart",
    },
    Preset {
        id: "ollama",
        name: "Ollama",
        vendor: "local",
        protocol: Protocol::OpenAiCompatible,
        auth: AuthKind::None,
        base_url: "http://127.0.0.1:11434/v1",
        models: &["qwen2.5-coder:7b"],
        api_key_env: None,
        key_url: None,
        docs_url: "https://github.com/ollama/ollama/blob/main/docs/openai.md",
    },
    Preset {
        id: "custom",
        name: "OpenAI-compatible",
        vendor: "custom",
        protocol: Protocol::OpenAiCompatible,
        auth: AuthKind::ApiKey,
        base_url: "",
        models: &[],
        api_key_env: None,
        key_url: None,
        docs_url: "https://platform.openai.com/docs/api-reference/chat",
    },
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_unique_and_remote_ones_use_https() {
        let mut ids: Vec<_> = PRESETS.iter().map(|p| p.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), PRESETS.len());
        for p in PRESETS {
            if p.auth != AuthKind::None && !p.base_url.is_empty() {
                assert!(p.base_url.starts_with("https://"), "{}", p.id);
            }
        }
        for must in ["claude", "chatgpt", "gemini", "deepseek", "kimi", "qwen"] {
            assert!(preset(must).is_some(), "{must}");
        }
    }
}
