//! AI providers the learner connects from the settings page.
//!
//! The list lives in `~/.est/config/providers.toml` and holds no secrets:
//! a key entered in the UI goes to the [`KeyStore`] (`~/.est/secrets`, owner
//! only), or the provider names an environment variable. The API never
//! returns a key — only whether one is configured and where it comes from.
//!
//! A provider from `[llm]` in `config.toml` is shown as the read-only entry
//! [`CONFIG_ID`] and stays active until the learner picks another one.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use est_ai::catalog::{self, AuthKind};
use est_ai::{Protocol, ProviderConfig};
use est_application::ports::LlmProvider;
use est_application::{AppError, AppResult};
use est_workspace::{EstHome, KeyStore};
use serde::{Deserialize, Serialize};

/// Id of the provider configured in `config.toml`.
pub const CONFIG_ID: &str = "config";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredProvider {
    pub id: String,
    /// Preset it was created from (`claude`, `chatgpt`, …, `custom`).
    pub preset: String,
    pub name: String,
    pub protocol: Protocol,
    pub auth: AuthKind,
    pub base_url: String,
    pub model: String,
    /// Environment variable with the key, used when no key is stored.
    #[serde(default)]
    pub api_key_env: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvidersFile {
    /// Active provider id; `None` means "no model". A missing file means
    /// "use `[llm]` from config.toml".
    #[serde(default)]
    active: Option<String>,
    #[serde(default)]
    providers: Vec<StoredProvider>,
}

/// Where a provider's key comes from.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KeySource {
    Stored,
    Env,
    Missing,
    NotNeeded,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderView {
    #[serde(flatten)]
    pub provider: StoredProvider,
    pub key: KeySource,
    /// Data leaves this machine.
    pub remote: bool,
    pub active: bool,
    /// From `config.toml`: cannot be edited or removed here.
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvidersView {
    pub active: Option<String>,
    pub providers: Vec<ProviderView>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddProvider {
    pub preset: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    pub model: String,
    /// Only for the `custom` preset.
    #[serde(default)]
    pub protocol: Option<Protocol>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Make it the active provider right away (handled by the route).
    #[serde(default)]
    pub activate: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateProvider {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    /// Empty string clears the variable.
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub clear_key: bool,
}

pub struct ProviderRegistry {
    file: PathBuf,
    keys: KeyStore,
    /// `[llm]` from config.toml.
    config_llm: Option<ProviderConfig>,
    lock: Mutex<()>,
}

fn io(e: impl std::fmt::Display) -> AppError {
    AppError::Infrastructure(format!("providers.toml: {e}"))
}

fn valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit())
}

fn clean_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(AppError::Invalid(
            "provider name must be 1–80 characters".into(),
        ));
    }
    Ok(name.to_owned())
}

fn clean_model(model: &str) -> AppResult<String> {
    let model = model.trim();
    if model.is_empty() || model.len() > 200 || model.chars().any(char::is_whitespace) {
        return Err(AppError::Invalid(
            "model must be a single non-empty id".into(),
        ));
    }
    Ok(model.to_owned())
}

impl ProviderRegistry {
    pub fn new(home: &EstHome, config_llm: Option<ProviderConfig>) -> Self {
        Self {
            file: home.config_dir().join("providers.toml"),
            keys: home.key_store(),
            config_llm,
            lock: Mutex::new(()),
        }
    }

    fn guard(&self) -> std::sync::MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `None` when the file does not exist yet.
    fn read(&self) -> AppResult<Option<ProvidersFile>> {
        match std::fs::read_to_string(&self.file) {
            Ok(text) => Ok(Some(toml::from_str(&text).map_err(io)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io(e)),
        }
    }

    fn write(&self, f: &ProvidersFile) -> AppResult<()> {
        let text = format!(
            "# AI providers connected from the settings page. No keys here:\n\
             # keys entered in the UI live in ~/.est/secrets.\n\n{}",
            toml::to_string_pretty(f).map_err(io)?
        );
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let tmp = self.file.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(io)?;
        std::fs::rename(&tmp, &self.file).map_err(io)
    }

    fn active_id(&self, f: Option<&ProvidersFile>) -> Option<String> {
        match f {
            Some(f) => f.active.clone(),
            None => self.config_llm.as_ref().map(|_| CONFIG_ID.to_owned()),
        }
    }

    fn config_entry(&self) -> Option<StoredProvider> {
        self.config_llm.as_ref().map(|c| StoredProvider {
            id: CONFIG_ID.into(),
            preset: "custom".into(),
            name: c.name.clone().unwrap_or_else(|| "config.toml".into()),
            protocol: c.protocol,
            auth: if c.api_key_env.is_some() {
                AuthKind::ApiKey
            } else {
                AuthKind::None
            },
            base_url: c.base_url.clone(),
            model: c.model.clone(),
            api_key_env: c.api_key_env.clone(),
        })
    }

    fn key_source(&self, p: &StoredProvider) -> KeySource {
        if p.id != CONFIG_ID && self.keys.has(&p.id) {
            return KeySource::Stored;
        }
        let env = p
            .api_key_env
            .as_deref()
            .and_then(|v| std::env::var(v).ok())
            .is_some_and(|k| !k.trim().is_empty());
        match (env, p.auth) {
            (true, _) => KeySource::Env,
            (false, AuthKind::None) => KeySource::NotNeeded,
            (false, _) => KeySource::Missing,
        }
    }

    fn view(&self, p: StoredProvider, active: &Option<String>) -> ProviderView {
        ProviderView {
            key: self.key_source(&p),
            remote: est_ai::parse_base_url(&p.base_url)
                .map(|(_, r)| r)
                .unwrap_or(true),
            active: active.as_deref() == Some(p.id.as_str()),
            read_only: p.id == CONFIG_ID,
            provider: p,
        }
    }

    pub fn list(&self) -> AppResult<ProvidersView> {
        let _g = self.guard();
        let f = self.read()?;
        let active = self.active_id(f.as_ref());
        let mut all: Vec<StoredProvider> = self.config_entry().into_iter().collect();
        all.extend(f.map(|f| f.providers).unwrap_or_default());
        Ok(ProvidersView {
            providers: all.into_iter().map(|p| self.view(p, &active)).collect(),
            active,
        })
    }

    fn find(&self, f: Option<&ProvidersFile>, id: &str) -> AppResult<StoredProvider> {
        if id == CONFIG_ID {
            return self
                .config_entry()
                .ok_or_else(|| AppError::NotFound("no [llm] in config.toml".into()));
        }
        f.and_then(|f| f.providers.iter().find(|p| p.id == id).cloned())
            .ok_or_else(|| AppError::NotFound(format!("AI provider {id}")))
    }

    pub fn add(&self, req: AddProvider) -> AppResult<ProviderView> {
        let preset = catalog::preset(&req.preset)
            .ok_or_else(|| AppError::Invalid(format!("unknown provider preset {}", req.preset)))?;
        let protocol = match (preset.id, req.protocol) {
            ("custom", Some(p)) => p,
            _ => preset.protocol,
        };
        let base_url = req
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .unwrap_or(preset.base_url)
            .trim_end_matches('/')
            .to_owned();
        est_ai::parse_base_url(&base_url)?;
        let api_key_env = match req.api_key_env.as_deref().map(str::trim) {
            Some("") => None,
            Some(v) if valid_env_name(v) => Some(v.to_owned()),
            Some(_) => {
                return Err(AppError::Invalid(
                    "environment variable names use A–Z, 0–9 and _".into(),
                ));
            }
            None => preset.api_key_env.map(str::to_owned),
        };
        let p = StoredProvider {
            id: format!(
                "{}-{}",
                preset.id,
                &uuid::Uuid::new_v4().simple().to_string()[..8]
            ),
            preset: preset.id.into(),
            name: clean_name(req.name.as_deref().unwrap_or(preset.name))?,
            protocol,
            auth: preset.auth,
            base_url,
            model: clean_model(&req.model)?,
            api_key_env,
        };
        let _g = self.guard();
        let mut f = self.read()?.unwrap_or_else(|| ProvidersFile {
            active: self.active_id(None),
            providers: Vec::new(),
        });
        if f.providers.len() >= 50 {
            return Err(AppError::Invalid("too many AI providers".into()));
        }
        if let Some(key) = req.api_key.as_deref().filter(|k| !k.trim().is_empty()) {
            self.keys.set(&p.id, key)?;
        }
        f.providers.push(p.clone());
        self.write(&f)?;
        Ok(self.view(p, &f.active))
    }

    pub fn update(&self, id: &str, req: UpdateProvider) -> AppResult<ProviderView> {
        if id == CONFIG_ID {
            return Err(AppError::Invalid(
                "the provider from config.toml is edited in that file".into(),
            ));
        }
        let _g = self.guard();
        let mut f = self.read()?.unwrap_or_default();
        let p = f
            .providers
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| AppError::NotFound(format!("AI provider {id}")))?;
        if let Some(name) = &req.name {
            p.name = clean_name(name)?;
        }
        if let Some(url) = &req.base_url {
            let url = url.trim().trim_end_matches('/');
            est_ai::parse_base_url(url)?;
            p.base_url = url.to_owned();
        }
        if let Some(model) = &req.model {
            p.model = clean_model(model)?;
        }
        if let Some(env) = req.api_key_env.as_deref().map(str::trim) {
            p.api_key_env = match env {
                "" => None,
                v if valid_env_name(v) => Some(v.to_owned()),
                _ => {
                    return Err(AppError::Invalid(
                        "environment variable names use A–Z, 0–9 and _".into(),
                    ));
                }
            };
        }
        let p = p.clone();
        if req.clear_key {
            self.keys.remove(id)?;
        }
        if let Some(key) = req.api_key.as_deref().filter(|k| !k.trim().is_empty()) {
            self.keys.set(id, key)?;
        }
        self.write(&f)?;
        Ok(self.view(p, &f.active))
    }

    /// Removes the provider and its key. Returns whether it was active.
    pub fn remove(&self, id: &str) -> AppResult<bool> {
        if id == CONFIG_ID {
            return Err(AppError::Invalid(
                "the provider from config.toml is removed in that file".into(),
            ));
        }
        let _g = self.guard();
        let mut f = self.read()?.unwrap_or_default();
        let before = f.providers.len();
        f.providers.retain(|p| p.id != id);
        if f.providers.len() == before {
            return Err(AppError::NotFound(format!("AI provider {id}")));
        }
        let was_active = f.active.as_deref() == Some(id);
        if was_active {
            f.active = None;
        }
        self.keys.remove(id)?;
        self.write(&f)?;
        Ok(was_active)
    }

    fn config_for(&self, p: &StoredProvider) -> ProviderConfig {
        match (&self.config_llm, p.id == CONFIG_ID) {
            (Some(c), true) => c.clone(),
            _ => ProviderConfig {
                base_url: p.base_url.clone(),
                model: p.model.clone(),
                api_key_env: p.api_key_env.clone(),
                timeout_secs: 120,
                protocol: p.protocol,
                name: Some(p.name.clone()),
            },
        }
    }

    fn key_for(&self, p: &StoredProvider) -> AppResult<Option<String>> {
        if p.id == CONFIG_ID {
            return Ok(None);
        }
        self.keys.get(&p.id)
    }

    /// Builds a provider without changing what is active.
    pub fn connect(&self, id: &str) -> AppResult<Arc<dyn LlmProvider>> {
        let _g = self.guard();
        let f = self.read()?;
        let p = self.find(f.as_ref(), id)?;
        let key = self.key_for(&p)?;
        if key.is_none() && p.auth != AuthKind::None && self.key_source(&p) == KeySource::Missing {
            return Err(AppError::Invalid(format!(
                "{} has no API key: add one or set {}",
                p.name,
                p.api_key_env
                    .as_deref()
                    .unwrap_or("an environment variable")
            )));
        }
        est_ai::connect(&self.config_for(&p), key)
    }

    /// Records the active provider (`None`: no model).
    pub fn set_active(&self, id: Option<&str>) -> AppResult<()> {
        let _g = self.guard();
        let mut f = self.read()?.unwrap_or_default();
        if let Some(id) = id {
            self.find(Some(&f), id)?;
        }
        f.active = id.map(str::to_owned);
        self.write(&f)
    }

    /// The provider to use at startup, if any.
    pub fn startup(&self) -> Option<Arc<dyn LlmProvider>> {
        let id = {
            let _g = self.guard();
            match self.read() {
                Ok(f) => self.active_id(f.as_ref())?,
                Err(e) => {
                    tracing::warn!("{e}");
                    return None;
                }
            }
        };
        match self.connect(&id) {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!("AI provider {id} is not available: {e}");
                None
            }
        }
    }

    /// Models offered by a provider: by stored id, or by preset/base URL with
    /// a key typed into the form (not stored).
    pub async fn models(&self, req: ModelsQuery) -> AppResult<Vec<String>> {
        let (protocol, base_url, key) = match &req.provider_id {
            Some(id) => {
                let (p, key) = {
                    let _g = self.guard();
                    let f = self.read()?;
                    let p = self.find(f.as_ref(), id)?;
                    let key = self.key_for(&p)?;
                    (p.clone(), key)
                };
                let cfg = self.config_for(&p);
                let key = req
                    .api_key
                    .clone()
                    .filter(|k| !k.trim().is_empty())
                    .or(key)
                    .or_else(|| cfg.env_key());
                (p.protocol, p.base_url, key)
            }
            None => {
                let preset = req
                    .preset
                    .as_deref()
                    .and_then(catalog::preset)
                    .ok_or_else(|| AppError::Invalid("preset or provider_id required".into()))?;
                let base = req
                    .base_url
                    .clone()
                    .filter(|u| !u.trim().is_empty())
                    .unwrap_or_else(|| preset.base_url.to_owned());
                let protocol = match (preset.id, req.protocol) {
                    ("custom", Some(p)) => p,
                    _ => preset.protocol,
                };
                let env = req
                    .api_key_env
                    .clone()
                    .or_else(|| preset.api_key_env.map(str::to_owned));
                let key = req
                    .api_key
                    .clone()
                    .filter(|k| !k.trim().is_empty())
                    .or_else(|| env.and_then(|v| std::env::var(v).ok()));
                (protocol, base, key)
            }
        };
        est_ai::list_models(protocol, &base_url, key.as_deref().map(str::trim)).await
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelsQuery {
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub protocol: Option<Protocol>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
}
