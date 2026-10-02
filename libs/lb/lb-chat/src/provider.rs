//! Provider configuration lives in the vault as `/.agent/providers/<name>.json`
//! and `/.agent/default.json`, encrypted and synced like any note.

use lb_rs::blocking::Lb;
use lb_rs::model::chat::Settings;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    OpenAi,
    Anthropic,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Provider {
    /// The provider file's name, as it appears in `Settings::model`.
    pub name: String,
    pub kind: Kind,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
}

#[derive(Deserialize)]
struct ProviderFile {
    #[serde(default = "default_kind")]
    kind: String,
    base_url: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    api_key: Option<String>,
}

fn default_kind() -> String {
    "openai".into()
}

#[derive(Deserialize)]
struct DefaultFile {
    provider: String,
    #[serde(default)]
    model: String,
}

const PLACEHOLDER_KEY: &str = "YOUR API KEY HERE";

impl Provider {
    /// The provider a chat's settings select, falling back to the vault
    /// default. `model` in settings is `provider/model`; a bare provider name
    /// means the file's own model.
    pub fn resolve(lb: &Lb, settings: &Settings) -> Result<Provider, String> {
        let (name, model) = match &settings.model {
            Some(selection) => split(selection),
            None => {
                let bytes = read(lb, "/.agent/default.json")?
                    .ok_or("no provider selected and no /.agent/default.json")?;
                let default: DefaultFile = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("/.agent/default.json: {e}"))?;
                (default.provider, default.model)
            }
        };
        Self::load(lb, &name, &model)
    }

    pub fn load(lb: &Lb, name: &str, model: &str) -> Result<Provider, String> {
        let path = format!("/.agent/providers/{name}.json");
        let bytes = read(lb, &path)?.ok_or_else(|| format!("no provider file at {path}"))?;
        let file: ProviderFile =
            serde_json::from_slice(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let kind = match file.kind.as_str() {
            "anthropic" => Kind::Anthropic,
            "openai" => Kind::OpenAi,
            other => return Err(format!("{path}: unknown kind {other:?}")),
        };
        let api_key = file
            .api_key
            .filter(|k| !k.trim().is_empty() && k != PLACEHOLDER_KEY);
        let model = if model.is_empty() { file.model } else { model.to_string() };
        if model.is_empty() {
            return Err(format!("{path}: no model"));
        }
        Ok(Provider {
            name: name.to_string(),
            kind,
            base_url: file.base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
        })
    }

    /// `provider/model`, the form `Settings::model` carries.
    pub fn selection(&self) -> String {
        format!("{}/{}", self.name, self.model)
    }
}

/// How a provider reads in the interface: "OpenAI", "xAI", "Anthropic".
pub fn friendly_name(provider: &str) -> String {
    match provider {
        "openai" => "OpenAI".into(),
        "xai" => "xAI".into(),
        "openrouter" => "OpenRouter".into(),
        other => capitalize(other),
    }
}

/// How a model id reads: "claude-opus-5-5-20260401" is "Opus 5.5",
/// "gpt-4o-mini" is "GPT 4o Mini", "llama-3.3-70b" is "Llama 3.3 70B".
pub fn friendly_model(id: &str) -> String {
    let id = id.rsplit('/').next().unwrap_or(id);
    let tokens = id
        .split(['-', '_', ':', ' '])
        .filter(|t| !t.is_empty())
        .filter(|t| !matches!(*t, "claude" | "latest"))
        .filter(|t| !(t.len() == 8 && t.chars().all(|c| c.is_ascii_digit())));
    let mut words: Vec<String> = Vec::new();
    for token in tokens {
        let numeric = token.chars().all(|c| c.is_ascii_digit() || c == '.');
        match words.last_mut() {
            Some(last) if numeric && last.chars().all(|c| c.is_ascii_digit() || c == '.') => {
                last.push('.');
                last.push_str(token);
            }
            _ => words.push(friendly_token(token)),
        }
    }
    words.join(" ")
}

fn friendly_token(token: &str) -> String {
    let letters = token.chars().filter(|c| c.is_ascii_alphabetic()).count();
    let digits = token.chars().filter(|c| c.is_ascii_digit()).count();
    let acronym = token.eq_ignore_ascii_case("gpt")
        || (digits > 0 && letters <= 2 && !token.ends_with('o'))
        || (digits > 0 && token.ends_with('b'));
    if acronym { token.to_ascii_uppercase() } else { capitalize(token) }
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn split(selection: &str) -> (String, String) {
    match selection.split_once('/') {
        Some((name, model)) => (name.to_string(), model.to_string()),
        None => (selection.to_string(), String::new()),
    }
}

fn read(lb: &Lb, path: &str) -> Result<Option<Vec<u8>>, String> {
    let Ok(file) = lb.get_by_path(path) else { return Ok(None) };
    lb.read_document(file.id, false)
        .map(Some)
        .map_err(|e| format!("{path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_and_providers_read_like_their_names() {
        for (id, name) in [
            ("claude-opus-5-5-20260401", "Opus 5.5"),
            ("claude-sonnet-4-5", "Sonnet 4.5"),
            ("gpt-4o-mini", "GPT 4o Mini"),
            ("gpt-5.5", "GPT 5.5"),
            ("o3-pro", "O3 Pro"),
            ("gemini-2.5-pro", "Gemini 2.5 Pro"),
            ("grok-4.6", "Grok 4.6"),
            ("llama-3.3-70b", "Llama 3.3 70B"),
            ("anthropic/claude-opus-5-5", "Opus 5.5"),
            ("qwen3:8b", "Qwen3 8B"),
            ("deepseek-r1", "Deepseek R1"),
        ] {
            assert_eq!(friendly_model(id), name, "{id}");
        }
        assert_eq!(friendly_name("openai"), "OpenAI");
        assert_eq!(friendly_name("xai"), "xAI");
        assert_eq!(friendly_name("anthropic"), "Anthropic");
        assert_eq!(friendly_name("my-box"), "My-box");
    }
}
