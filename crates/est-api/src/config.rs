//! Daemon configuration: `~/.est/config/config.toml`, overridden by `EST_*`
//! environment variables.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use est_ai::ProviderConfig;
use est_dependency::DecayParams;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Address of the local daemon.
    pub bind: SocketAddr,
    /// Editor command used by "open workspace", e.g. `code` or `codium -n`.
    pub editor: Option<String>,
    /// Directory with the built React frontend (`frontend/dist`).
    pub frontend_dir: Option<PathBuf>,
    /// Extra Host header values accepted by the API (for non-loopback binds).
    pub allowed_hosts: Vec<String>,
    /// AI Gateway provider (`[llm]`); without it the mentor applies the
    /// policy but cannot generate answers.
    pub llm: Option<ProviderConfig>,
    /// ADI double-decay parameters (`[adi]`).
    pub adi: Option<DecayParams>,
    /// Sandbox Manager (`[sandbox]`): runtime, images and limits.
    pub sandbox: est_sandbox::SandboxConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: SocketAddr::from(([127, 0, 0, 1], 7878)),
            editor: None,
            frontend_dir: None,
            allowed_hosts: Vec::new(),
            llm: None,
            adi: None,
            sandbox: est_sandbox::SandboxConfig::default(),
        }
    }
}

impl Config {
    /// Reads `path` if it exists, then applies environment overrides.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let mut cfg: Config = match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(e) => return Err(e.into()),
        };
        if let Ok(bind) = std::env::var("EST_BIND") {
            cfg.bind = bind.parse()?;
        }
        if let Ok(editor) = std::env::var("EST_EDITOR") {
            cfg.editor = Some(editor);
        }
        if let Some(dir) = std::env::var_os("EST_FRONTEND_DIR") {
            cfg.frontend_dir = Some(dir.into());
        }
        Ok(cfg)
    }

    /// Host header values the API accepts (DNS-rebinding protection).
    pub fn allowed_hosts(&self) -> Vec<String> {
        let port = self.bind.port();
        let mut hosts = vec![
            format!("localhost:{port}"),
            format!("127.0.0.1:{port}"),
            format!("[::1]:{port}"),
            "localhost".into(),
            "127.0.0.1".into(),
        ];
        if !self.bind.ip().is_loopback() && !self.bind.ip().is_unspecified() {
            hosts.push(self.bind.to_string());
        }
        hosts.extend(self.allowed_hosts.iter().cloned());
        hosts
    }

    pub const TEMPLATE: &'static str = r#"# Engineering Skills Trainer — local configuration.

# Address of the local daemon (`est serve`).
bind = "127.0.0.1:7878"

# Editor used to open task workspaces. Any command that accepts a directory.
# editor = "code"

# Built React frontend served by the daemon.
# frontend_dir = "/path/to/Engineer-Craft/frontend/dist"

# AI Gateway: any OpenAI-compatible endpoint. Local Ollama:
# [llm]
# base_url = "http://127.0.0.1:11434/v1"
# model = "qwen2.5-coder:7b"
# A remote provider must use https; the key is read from an environment
# variable and never stored here:
# api_key_env = "CRAFT_LLM_API_KEY"

# Sandbox for build/test runs: Podman or Docker. Network is off unless you
# allow it for a run; the workspace is mounted read-only.
# [sandbox]
# runtime = "auto"          # auto | podman | docker
# cpus = 2.0
# memory_mb = 2048
# timeout_secs = 600
# [sandbox.images]
# rust = "docker.io/library/rust:1-slim"

# ADI double decay (preliminary MVP values).
# [adi]
# half_life_days = 30.0
# half_life_issues = 10.0
# min_issues_for_status = 5
"#;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_parses() {
        let cfg: Config = toml::from_str(Config::TEMPLATE).unwrap();
        assert_eq!(cfg.bind.port(), 7878);
        assert!(cfg.allowed_hosts().contains(&"localhost:7878".to_owned()));
    }

    #[test]
    fn parses_llm_and_adi() {
        let cfg: Config = toml::from_str(
            "[llm]\nbase_url = \"http://127.0.0.1:11434/v1\"\nmodel = \"m\"\n[adi]\nhalf_life_days = 7.0\nhalf_life_issues = 3.0\nmin_issues_for_status = 2\n",
        )
        .unwrap();
        assert_eq!(cfg.llm.unwrap().model, "m");
        assert_eq!(cfg.adi.unwrap().half_life_days, 7.0);
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(toml::from_str::<Config>("bnid = \"x\"").is_err());
    }
}
