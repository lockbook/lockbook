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
