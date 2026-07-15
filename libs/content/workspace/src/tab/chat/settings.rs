//! Agent provider config: one file per provider under `/.agent/providers/`,
//! e.g. `/.agent/providers/cerebras.json`:
//!
//! ```json
//! {
//!   "base_url": "https://api.cerebras.ai/v1",
//!   "model": "gemma-4-31b",
//!   "api_key": "csk-…"
//! }
//! ```
//!
//! Lockbook documents are encrypted and synced, so key management is file
//! management: a provider set up on one device follows the user everywhere,
//! and deleting the file revokes it. Per-provider files also make settings
//! edits conflict-free across devices (documents merge per file).
//!
//! Every file is self-contained — the filename is purely a label. `base_url`
//! and `model` are required; `kind` names the wire format (default `openai`,
//! covering every OpenAI-compatible endpoint — `anthropic` and an on-device
//! `local` slot in as new backends); `api_key` is optional, and absent means
//! authenticate with nothing (a local Ollama, say). There are no built-in
//! provider names, no env-var fallbacks, and no persisted model lists — a
//! future setup UI offers known providers as templates that pre-fill a file.

use lb_rs::blocking::Lb;
use serde::Deserialize;

const PROVIDERS_DIR: &str = "/.agent/providers";
pub const PROMPT_PATH: &str = "/.agent/prompt.md";
/// The sticky default provider/model for new chats: the most recent pick in
/// any chat, mirrored here so a fresh chat starts where the last one left off.
/// Synced like everything under `/.agent`; absent until a first pick, and a
/// new chat then falls back to the alphabetically-first provider.
const DEFAULT_PATH: &str = "/.agent/default.json";

/// Read the sticky default selection, if one has been recorded.
pub fn load_default(core: &Lb) -> Option<lb_rs::model::chat::ModelSelection> {
    let file = core.get_by_path(DEFAULT_PATH).ok()?;
    let bytes = core.read_document(file.id, false).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Record the sticky default selection. Best-effort — a convenience, not
/// load-bearing — so I/O failures are swallowed.
pub fn write_default(core: &Lb, selection: &lb_rs::model::chat::ModelSelection) {
    let Ok(bytes) = serde_json::to_vec_pretty(selection) else { return };
    let id = match core.get_by_path(DEFAULT_PATH) {
        Ok(f) => f.id,
        Err(_) => match core.create_at_path(DEFAULT_PATH) {
            Ok(f) => f.id,
            Err(_) => return,
        },
    };
    let _ = core.write_document(id, &bytes);
}

/// Custom system prompt: the whole of `/.agent/prompt.md` whenever the file
/// exists — an empty file means no system prompt, not the default; deleting
/// the file restores the built-in preamble. Read at send time like
/// everything else here — edit, then just send.
pub fn system_prompt(core: &Lb) -> Option<String> {
    let file = core.get_by_path(PROMPT_PATH).ok()?;
    let bytes = core.read_document(file.id, false).ok()?;
    Some(String::from_utf8_lossy(&bytes).trim().to_string())
}

/// One provider, fully resolved: everything a backend needs to run.
#[derive(Clone, PartialEq)]
pub struct Provider {
    /// The file name (sans `.json`) — a display label, nothing more.
    pub name: String,
    /// Optional prettier label from the file's `display_name` field, for
    /// casing a filename can't carry ("OpenRouter"). Never matched against —
    /// selections and resolution use `name`.
    pub display_name: Option<String>,
    /// Wire format / backend to drive this provider with.
    pub kind: String,
    pub base_url: String,
    /// The model to run chats with, until a per-chat selection exists.
    pub model: String,
    /// Absent: authenticate with nothing (e.g. a locally hosted server).
    pub api_key: Option<String>,
    /// Reasoning effort ("low"/"medium"/"high", plus provider-specific
    /// extras), sent only when set — where the model doesn't reason it's
    /// ignored or errors. Backends map it: OpenAI-compat `reasoning_effort`,
    /// Anthropic `output_config.effort` with adaptive thinking.
    pub effort: Option<String>,
}

/// The on-disk shape. `base_url` and `model` are required; a file missing
/// them doesn't parse and the provider doesn't exist.
#[derive(Deserialize)]
struct ProviderFile {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default = "default_kind")]
    kind: String,
    base_url: String,
    model: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    effort: Option<String>,
}

impl Provider {
    /// What the picker shows: the file's `display_name` when set, else the
    /// filename with its first letter uppercased.
    pub fn label(&self) -> String {
        match &self.display_name {
            Some(name) => name.clone(),
            None => {
                let mut chars = self.name.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                    None => String::new(),
                }
            }
        }
    }
}

pub const DEFAULT_KIND: &str = "openai";

fn default_kind() -> String {
    DEFAULT_KIND.to_string()
}

/// Read every provider file, sorted by name. Unparseable files are logged
/// and skipped. A file still carrying the template key placeholder is a
/// half-created provider — skipped too, so an unconfigured provider is, in
/// every respect (picker, resolution, onboarding), as if its file didn't
/// exist; the roster's "add provider" flow is where it gets set up.
pub fn load(core: &Lb) -> Vec<Provider> {
    let Ok(dir) = core.get_by_path(PROVIDERS_DIR) else { return Vec::new() };
    let Ok(children) = core.get_children(&dir.id) else { return Vec::new() };
    let mut providers: Vec<Provider> = children
        .iter()
        .filter(|f| f.is_document())
        .filter_map(|f| {
            let name = f.name.trim_end_matches(".json").to_string();
            let bytes = core.read_document(f.id, false).ok()?;
            match parse(&name, &bytes) {
                Some(p) if p.api_key.as_deref() == Some(super::KEY_PLACEHOLDER) => None,
                Some(p) => Some(p),
                None => {
                    tracing::warn!("chat: invalid provider file {PROVIDERS_DIR}/{}", f.name);
                    None
                }
            }
        })
        .collect();
    providers.sort_by(|a, b| a.name.cmp(&b.name));
    providers
}

/// The provider a turn runs with: the user's per-chat selection resolved
/// against the provider files; with no selection ever recorded, the
/// alphabetically-first provider. Read at send time — config is resolved
/// when it's used, never watched.
///
/// A selection naming a missing provider (file deleted, not yet synced)
/// resolves to None rather than substituting another provider — silently
/// rerouting messages to a provider the user never picked is worse than
/// pausing until they pick again. An empty selection model means the named
/// provider's file default.
pub fn resolve(
    providers: Vec<Provider>, selection: Option<&lb_rs::model::chat::ModelSelection>,
) -> Option<Provider> {
    match selection {
        Some(sel) => {
            let i = providers.iter().position(|p| p.name == sel.provider)?;
            let mut p = providers.into_iter().nth(i).expect("position is in range");
            if !sel.model.trim().is_empty() {
                p.model = sel.model.clone();
            }
            Some(p)
        }
        None => providers.into_iter().next(),
    }
}

pub fn parse(name: &str, bytes: &[u8]) -> Option<Provider> {
    let file: ProviderFile = serde_json::from_slice(bytes).ok()?;
    Some(Provider {
        name: name.to_string(),
        display_name: file.display_name.filter(|n| !n.trim().is_empty()),
        kind: file.kind,
        base_url: file.base_url,
        model: file.model,
        // An empty key (the setup template before the paste) means keyless,
        // not "Bearer <nothing>".
        api_key: file.api_key.filter(|k| !k.trim().is_empty()),
        effort: file.effort.filter(|e| !e.trim().is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use lb_rs::model::chat::ModelSelection;

    use super::*;

    fn provider(name: &str, model: &str) -> Provider {
        Provider {
            name: name.into(),
            display_name: None,
            kind: DEFAULT_KIND.into(),
            base_url: "https://api.example.com".into(),
            model: model.into(),
            api_key: None,
            effort: None,
        }
    }

    fn sel(provider: &str, model: &str) -> ModelSelection {
        ModelSelection { provider: provider.into(), model: model.into() }
    }

    #[test]
    fn resolution_precedence() {
        let files = || vec![provider("anthropic", "claude"), provider("cerebras", "gemma")];

        // No selection → first provider, its file default.
        let p = resolve(files(), None).unwrap();
        assert_eq!((p.name.as_str(), p.model.as_str()), ("anthropic", "claude"));

        // Selection picks the provider and the model.
        let p = resolve(files(), Some(&sel("cerebras", "gemma-big"))).unwrap();
        assert_eq!((p.name.as_str(), p.model.as_str()), ("cerebras", "gemma-big"));

        // Empty selection model → the named provider's file default.
        let p = resolve(files(), Some(&sel("cerebras", ""))).unwrap();
        assert_eq!((p.name.as_str(), p.model.as_str()), ("cerebras", "gemma"));

        // Selection naming a deleted provider resolves to none — never a
        // silent substitution of a provider the user didn't pick.
        assert!(resolve(files(), Some(&sel("gone", "x"))).is_none());

        assert!(resolve(Vec::new(), None).is_none());
    }

    #[test]
    fn parses_a_minimal_file() {
        let p =
            parse("my-box", br#"{ "base_url": "http://192.168.1.5:11434/v1", "model": "llama3" }"#)
                .unwrap();
        assert_eq!(
            (p.name.as_str(), p.kind.as_str(), p.base_url.as_str(), p.model.as_str(), p.api_key),
            ("my-box", "openai", "http://192.168.1.5:11434/v1", "llama3", None)
        );
    }

    #[test]
    fn display_name_labels() {
        let file = br#"{ "display_name": "OpenRouter", "base_url": "u", "model": "m" }"#;
        assert_eq!(parse("openrouter", file).unwrap().label(), "OpenRouter");
        let file = br#"{ "base_url": "u", "model": "m" }"#;
        assert_eq!(parse("my-box", file).unwrap().label(), "My-box");
    }

    #[test]
    fn requires_base_url_and_model() {
        assert!(parse("x", br#"{ "api_key": "k", "model": "m" }"#).is_none());
        assert!(
            parse("x", br#"{ "api_key": "k", "base_url": "https://api.example.com" }"#).is_none()
        );
        assert!(parse("x", b"not json").is_none());
    }
}
