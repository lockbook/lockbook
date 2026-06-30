//! Chat/agent settings, stored as `/chat.json` in the user's lockbook.
//! Lockbook documents are encrypted and synced, so the providers (and their
//! API keys) set up on one device follow the user to every other device.
//!
//! The file is a list of providers plus a pointer to the active
//! provider+model. Each provider has a wire-format `kind` (`openai` for any
//! OpenAI-compatible endpoint, `anthropic` for the Messages API):
//!
//! ```json
//! {
//!   "active": { "provider": "cerebras", "model": "gemma-4-31b" },
//!   "providers": [
//!     {
//!       "name": "cerebras",
//!       "kind": "openai",
//!       "base_url": "https://api.cerebras.ai/v1",
//!       "api_key": "csk-…",
//!       "models": ["gemma-4-31b"]
//!     },
//!     { "name": "anthropic", "kind": "anthropic", "base_url": "https://api.anthropic.com", "api_key": "sk-ant-…" }
//!   ]
//! }
//! ```
//!
//! An empty/absent file falls back to a built-in Cerebras/Gemma provider, so
//! just exporting `CEREBRAS_API_KEY` is enough to start. The settings panel
//! picks the active provider (from configured + known [`PRESETS`]) and its
//! model; per-provider keys/base URLs are edited in `/chat.json`.

use lb_rs::blocking::Lb;
use serde::{Deserialize, Serialize};

/// Built-in defaults for a fresh `/chat.json`: Gemma 4 31B on Cerebras over
/// its OpenAI-compatible endpoint.
pub const DEFAULT_PROVIDER: &str = "cerebras";
pub const DEFAULT_MODEL: &str = "gemma-4-31b";
pub const CEREBRAS_BASE_URL: &str = "https://api.cerebras.ai/v1";

/// Parsed `/chat.json`. `#[serde(default)]` keeps a partial file working and
/// lets new fields land without breaking older clients.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct ChatSettings {
    /// Which provider+model to talk to. Defaults to the first provider (or the
    /// built-in Cerebras/Gemma defaults) when absent.
    pub active: Option<Active>,
    /// Configured providers, each an OpenAI-compatible endpoint.
    pub providers: Vec<Provider>,
}

/// The selected provider+model.
#[derive(Serialize, Deserialize, Clone)]
pub struct Active {
    pub provider: String,
    pub model: String,
}

/// One provider — the standard client parameters: a wire-format `kind`, a base
/// URL, an API key, and the models it serves.
#[derive(Serialize, Deserialize, Clone)]
pub struct Provider {
    /// Identifier and display name (`cerebras`, `anthropic`, …). Also selects
    /// the `<NAME>_API_KEY` env-var fallback.
    pub name: String,
    /// Wire format / backend to drive this provider with. `openai` (the
    /// default) covers every OpenAI-compatible endpoint; other kinds
    /// (`anthropic`, a future on-device `local`) select a different backend.
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Provider base URL, e.g. `https://api.cerebras.ai/v1`.
    pub base_url: String,
    /// API key; falls back to `<NAME>_API_KEY` in the environment.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Models this provider serves, offered in the picker. Optional: when the
    /// provider answers `GET /models`, the live list is preferred.
    #[serde(default)]
    pub models: Vec<String>,
}

/// The default wire format when a provider omits `kind`.
pub const DEFAULT_KIND: &str = "openai";

fn default_kind() -> String {
    DEFAULT_KIND.to_string()
}

/// A built-in provider preset: a name mapped to its wire format and default
/// endpoint, so the picker can offer known providers and fill in `base_url`/
/// `kind` without the user memorizing URLs. All OpenAI-compatible except
/// Anthropic (its own Messages API).
pub struct Preset {
    pub name: &'static str,
    pub kind: &'static str,
    pub base_url: &'static str,
}

pub const PRESETS: &[Preset] = &[
    Preset { name: "cerebras", kind: "openai", base_url: CEREBRAS_BASE_URL },
    Preset { name: "anthropic", kind: "anthropic", base_url: "https://api.anthropic.com" },
    Preset { name: "openai", kind: "openai", base_url: "https://api.openai.com/v1" },
    Preset { name: "groq", kind: "openai", base_url: "https://api.groq.com/openai/v1" },
    Preset { name: "openrouter", kind: "openai", base_url: "https://openrouter.ai/api/v1" },
    Preset { name: "together", kind: "openai", base_url: "https://api.together.xyz/v1" },
    Preset { name: "ollama", kind: "openai", base_url: "http://localhost:11434/v1" },
];

fn preset(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

impl Provider {
    /// A provider entry pre-filled from a preset (name, kind, base URL), with
    /// no key and no pinned models.
    fn from_preset(p: &Preset) -> Self {
        Self {
            name: p.name.to_string(),
            kind: p.kind.to_string(),
            base_url: p.base_url.to_string(),
            api_key: None,
            models: Vec::new(),
        }
    }

    fn cerebras_default() -> Self {
        let mut p = Self::from_preset(preset(DEFAULT_PROVIDER).expect("cerebras preset exists"));
        p.models = vec![DEFAULT_MODEL.to_string()];
        p
    }
}

impl ChatSettings {
    /// Read and parse `/chat.json`; defaults when missing or malformed.
    pub fn load(core: &Lb) -> Self {
        let Ok(file) = core.get_by_path("/chat.json") else { return Self::default() };
        let Ok(bytes) = core.read_document(file.id, false) else { return Self::default() };
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    /// The active provider's config: the one named by `active`, else the first
    /// configured, else the built-in Cerebras default.
    fn active_provider(&self) -> Provider {
        let named = self
            .active
            .as_ref()
            .and_then(|a| self.providers.iter().find(|p| p.name == a.provider));
        named
            .or_else(|| self.providers.first())
            .cloned()
            .unwrap_or_else(Provider::cerebras_default)
    }

    pub fn provider(&self) -> String {
        self.active_provider().name
    }

    /// Wire format of the active provider; selects the backend.
    pub fn kind(&self) -> String {
        self.active_provider().kind
    }

    pub fn base_url(&self) -> String {
        self.active_provider().base_url
    }

    /// The active provider's key: `/chat.json`, falling back to its
    /// `<NAME>_API_KEY` env var.
    pub fn api_key(&self) -> Option<String> {
        let p = self.active_provider();
        p.api_key
            .clone()
            .or_else(|| std::env::var(env_key_var(&p.name)).ok())
    }

    /// The active model id: `active.model`, else the active provider's first
    /// model, else the built-in default.
    pub fn model(&self) -> String {
        self.active
            .as_ref()
            .map(|a| a.model.clone())
            .filter(|m| !m.trim().is_empty())
            .or_else(|| self.active_provider().models.first().cloned())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string())
    }

    /// Stored key for the active provider (no env fallback), for seeding the
    /// settings editor.
    pub fn edit_api_key(&self) -> String {
        self.active_provider().api_key.unwrap_or_default()
    }

    /// Stored active model id (empty when unset), for seeding the editor so an
    /// empty field shows the default as hint text.
    pub fn edit_model(&self) -> String {
        self.active
            .as_ref()
            .map(|a| a.model.clone())
            .unwrap_or_default()
    }

    /// Names of configured providers, for the picker. Presets are a registry
    /// for resolving `base_url`/`kind`, not auto-listed — providers are added
    /// in `/chat.json`.
    pub fn provider_names(&self) -> Vec<String> {
        self.providers.iter().map(|p| p.name.clone()).collect()
    }

    /// Configured model ids for a named provider — the picker's fallback before
    /// a live `/models` fetch lands (or when listing isn't available).
    pub fn models_for(&self, name: &str) -> Vec<String> {
        self.providers
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.models.clone())
            .unwrap_or_default()
    }

    /// Stored key for a named provider (no env fallback), for reseeding the
    /// editor when the active provider changes.
    pub fn stored_api_key(&self, name: &str) -> String {
        self.providers
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.api_key.clone())
            .unwrap_or_default()
    }

    /// Fold the editor's values into `provider` (the chosen active provider),
    /// preserving every other configured provider. A provider not yet in the
    /// file is created — from a preset when the name is known, else a bare
    /// OpenAI-compatible entry the user finishes (e.g. `base_url`) in
    /// `/chat.json`.
    pub fn with_active_edits(
        &self, provider: &str, api_key: Option<String>, model: Option<String>,
    ) -> ChatSettings {
        let mut out = self.clone();
        let name =
            if provider.trim().is_empty() { out.provider() } else { provider.trim().to_string() };
        if !out.providers.iter().any(|p| p.name == name) {
            let entry = preset(&name)
                .map(Provider::from_preset)
                .unwrap_or_else(|| Provider {
                    name: name.clone(),
                    kind: DEFAULT_KIND.to_string(),
                    base_url: String::new(),
                    api_key: None,
                    models: Vec::new(),
                });
            out.providers.push(entry);
        }
        if let Some(p) = out.providers.iter_mut().find(|p| p.name == name) {
            p.api_key = api_key;
        }
        out.active = Some(Active { provider: name, model: model.unwrap_or_default() });
        out
    }
}

/// Env var holding the API key for a provider: `<NAME>_API_KEY`.
fn env_key_var(provider: &str) -> String {
    format!("{}_API_KEY", provider.to_uppercase())
}
