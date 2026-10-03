//! Connecting a provider. A hosted one is picked and given a key; a server
//! of the user's own is given an address. Provider files are ordinary
//! encrypted documents under `/.agent/providers/`. What a new chat starts
//! with lives in `/.agent/default.json` and is simply the last choice made
//! in any chat, so there is no separate control for it.

use std::sync::mpsc::{Receiver, channel};

use lb_chat::{Place, Provider, friendly_name, host, place};
use lb_rs::blocking::Lb;
use serde_json::{Value, json};

/// Whether a provider takes an API key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Required,
    /// A server of the user's own may or may not want one.
    Optional,
    /// The device's own model wants none.
    None,
}

pub struct Template {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: &'static str,
    pub base_url: &'static str,
    pub model: &'static str,
    pub key: Key,
}

const DEFAULT_PATH: &str = "/.agent/default.json";

/// Hosted providers: a company runs the model and issues a key. On an
/// Apple device, its own model comes first, wanting nothing.
pub const TEMPLATES: &[Template] = &[
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    Template {
        name: "apple",
        label: "Apple Intelligence",
        kind: "apple",
        base_url: "",
        model: "on-device",
        key: Key::None,
    },
    Template {
        name: "anthropic",
        label: "Anthropic",
        kind: "anthropic",
        base_url: "https://api.anthropic.com/v1",
        model: "claude-opus-5-5",
        key: Key::Required,
    },
    Template {
        name: "openai",
        label: "OpenAI",
        kind: "openai",
        base_url: "https://api.openai.com/v1",
        model: "gpt-5.5",
        key: Key::Required,
    },
    Template {
        name: "google",
        label: "Google",
        kind: "openai",
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        model: "gemini-3.5-flash",
        key: Key::Required,
    },
    Template {
        name: "xai",
        label: "xAI",
        kind: "openai",
        base_url: "https://api.x.ai/v1",
        model: "grok-4.7",
        key: Key::Required,
    },
    Template {
        name: "openrouter",
        label: "OpenRouter",
        kind: "openai",
        base_url: "https://openrouter.ai/api/v1",
        model: "openrouter/auto",
        key: Key::Required,
    },
    Template {
        name: "groq",
        label: "Groq",
        kind: "openai",
        base_url: "https://api.groq.com/openai/v1",
        model: "openai/gpt-oss-120b",
        key: Key::Required,
    },
    Template {
        name: "cerebras",
        label: "Cerebras",
        kind: "openai",
        base_url: "https://api.cerebras.ai/v1",
        model: "gpt-oss-120b",
        key: Key::Required,
    },
];

/// A server the user runs: any OpenAI-compatible endpoint, on this device
/// until the address says otherwise. Its file is named for its host, so
/// each machine keeps one.
pub static OWN: Template = Template {
    name: "",
    label: "Your own server",
    kind: "openai",
    base_url: "http://localhost:11434/v1",
    model: "",
    key: Key::Optional,
};

#[derive(Default)]
pub struct Setup {
    pub picked: Option<&'static Template>,
    /// The file the form rewrites, when it opened on an existing provider
    /// that has no template.
    pub name: Option<String>,
    pub key: String,
    pub model: String,
    pub base_url: String,
    pub error: Option<String>,
    /// A server of the user's own is being asked what it offers; its first
    /// model arrives here.
    pub asking: Option<Receiver<Result<String, String>>>,
}

impl Setup {
    pub fn pick(&mut self, template: &'static Template) {
        *self = Setup {
            picked: Some(template),
            model: template.model.to_string(),
            base_url: template.base_url.to_string(),
            ..Default::default()
        };
    }

    /// The form describes a server of the user's own.
    pub fn own(&self) -> bool {
        self.picked.is_some_and(|t| t.key == Key::Optional)
    }

    /// The provider the form describes: its file's name and contents.
    fn describe(&self) -> Result<(String, Value), String> {
        let Some(t) = self.picked else { return Err("pick a provider".into()) };
        let key = self.key.trim();
        if t.key == Key::Required && key.is_empty() {
            return Err("paste an API key".into());
        }
        let (name, label, base_url) = if self.own() {
            let base_url = address(&self.base_url);
            if base_url.is_empty() {
                return Err("give the server's address".into());
            }
            let name = self.name.clone().unwrap_or_else(|| own_name(&base_url));
            (name, own_label(&base_url), base_url)
        } else {
            (t.name.to_string(), t.label.to_string(), self.base_url.trim().to_string())
        };
        let mut file = json!({
            "display_name": label,
            "kind": t.kind,
            "base_url": base_url,
            "model": self.model.trim(),
        });
        if !key.is_empty() {
            file["api_key"] = json!(key);
        }
        Ok((name, file))
    }

    /// Writes the provider file and returns the `provider/model` it offers.
    pub fn connect(&mut self, lb: &Lb) -> Result<String, String> {
        let (name, file) = self.describe()?;
        let model = self.model.trim().to_string();
        if model.is_empty() {
            return Err("name a model".into());
        }
        let bytes = serde_json::to_vec_pretty(&file).unwrap();
        write(lb, &format!("/.agent/providers/{name}.json"), &bytes)?;
        self.key.clear();
        Ok(format!("{name}/{model}"))
    }

    /// Asks a server of the user's own what it offers, when the form names
    /// no model. Returns whether it asked; the answer arrives on `asking`.
    pub fn ask(&mut self, ctx: &egui::Context) -> bool {
        if !self.own() || !self.model.trim().is_empty() {
            return false;
        }
        let provider = self.describe().and_then(|(name, file)| {
            Provider::parse(&name, "", &serde_json::to_vec(&file).unwrap())
        });
        match provider {
            Ok(provider) => {
                let (tx, rx) = channel();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let first = lb_chat::list_models_blocking(&provider).and_then(|models| {
                        let first = models.into_iter().next();
                        first
                            .map(|m| m.id)
                            .ok_or_else(|| "the server lists no models; name one".to_string())
                    });
                    let _ = tx.send(first);
                    ctx.request_repaint();
                });
                self.asking = Some(rx);
                self.error = None;
            }
            Err(err) => self.error = Some(err),
        }
        true
    }
}

/// A typed address as the provider file holds it: `linux-box:11434` reads
/// as `http://linux-box:11434/v1`, the path OpenAI-compatible servers
/// answer on.
pub fn address(typed: &str) -> String {
    let typed = typed.trim().trim_end_matches('/');
    if typed.is_empty() {
        return String::new();
    }
    let (scheme, rest) = typed.split_once("://").unwrap_or(("http", typed));
    let path = if rest.contains('/') { "" } else { "/v1" };
    format!("{scheme}://{rest}{path}")
}

/// The file name for a server of the user's own: its host.
fn own_name(base_url: &str) -> String {
    host(base_url)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '-' })
        .collect()
}

/// What a server of the user's own is called: where it is.
fn own_label(base_url: &str) -> String {
    match place(base_url) {
        Place::ThisDevice => "This device".into(),
        Place::YourNetwork | Place::Internet => host(base_url),
    }
}

/// Makes `selection` (`provider/model`) and its effort, if it has one, what
/// a new chat starts with.
pub(super) fn write_default(lb: &Lb, selection: &str, effort: Option<&str>) -> Result<(), String> {
    let (provider, model) = selection.split_once('/').unwrap_or((selection, ""));
    let mut default = json!({ "provider": provider, "model": model });
    if let Some(effort) = effort {
        default["effort"] = json!(effort);
    }
    write(lb, DEFAULT_PATH, &serde_json::to_vec_pretty(&default).unwrap())
}

/// One provider file: its name and, when it parses, what it says.
pub struct Offered {
    pub name: String,
    pub file: Option<Provider>,
}

impl Offered {
    pub fn label(&self) -> String {
        match &self.file {
            Some(provider) => provider.label(),
            None => friendly_name(&self.name),
        }
    }

    pub fn place(&self) -> Place {
        self.file.as_ref().map_or(Place::Internet, Provider::place)
    }
}

/// The provider files that exist, in the order they are offered.
pub fn providers(lb: &Lb) -> Vec<Offered> {
    let Ok(folder) = lb.get_by_path("/.agent/providers/") else { return Vec::new() };
    let mut offered: Vec<Offered> = lb
        .get_children(&folder.id)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|f| f.name.strip_suffix(".json").map(str::to_string))
        .map(|name| Offered { file: Provider::load(lb, &name, "").ok(), name })
        .filter(|o| o.file.as_ref().is_none_or(Provider::runs_here))
        .collect();
    order(&mut offered);
    offered
}

/// Hosted providers by name, then the user's own servers, this device
/// first.
fn order(offered: &mut [Offered]) {
    offered.sort_by_key(|o| {
        let group = match o.place() {
            Place::Internet => 0,
            Place::ThisDevice => 1,
            Place::YourNetwork => 2,
        };
        (group, o.name.clone())
    });
}

pub(super) fn write(lb: &Lb, path: &str, bytes: &[u8]) -> Result<(), String> {
    let file = match lb.get_by_path(path) {
        Ok(f) => f,
        Err(_) => lb.create_at_path(path).map_err(|e| e.to_string())?,
    };
    lb.write_document(file.id, bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own(base_url: &str) -> Setup {
        let mut setup = Setup::default();
        setup.pick(&OWN);
        setup.base_url = base_url.into();
        setup.model = "m".into();
        setup
    }

    /// What people type for an address, and what the file ends up holding.
    #[test]
    fn a_typed_address_becomes_a_base_url() {
        for (typed, base_url) in [
            ("http://localhost:11434/v1", "http://localhost:11434/v1"),
            (" http://localhost:11434/v1/ ", "http://localhost:11434/v1"),
            ("linux-box:11434", "http://linux-box:11434/v1"),
            ("http://linux-box:11434", "http://linux-box:11434/v1"),
            ("100.64.0.1:8080/", "http://100.64.0.1:8080/v1"),
            ("https://llm.example.com/api", "https://llm.example.com/api"),
            ("", ""),
        ] {
            assert_eq!(address(typed), base_url, "{typed:?}");
        }
    }

    /// A server of the user's own is filed under its host and called where
    /// it is; an existing file keeps its name.
    #[test]
    fn a_server_of_your_own_is_named_for_its_address() {
        let (name, file) = own("localhost:11434").describe().unwrap();
        assert_eq!(
            (name.as_str(), file["display_name"].as_str()),
            ("localhost", Some("This device"))
        );
        assert_eq!(file["base_url"], "http://localhost:11434/v1");
        assert!(file.get("api_key").is_none(), "no key given, none written");

        let (name, file) = own("http://Linux-Box.tail1234.ts.net:11434/v1")
            .describe()
            .unwrap();
        assert_eq!(name, "linux-box.tail1234.ts.net");
        assert_eq!(file["display_name"], "linux-box.tail1234.ts.net");

        let mut legacy = own("http://100.64.0.1:8080/v1");
        legacy.name = Some("custom".into());
        legacy.key = " secret ".into();
        let (name, file) = legacy.describe().unwrap();
        assert_eq!((name.as_str(), file["display_name"].as_str()), ("custom", Some("100.64.0.1")));
        assert_eq!(file["api_key"], "secret");

        assert_eq!(own("  ").describe().unwrap_err(), "give the server's address");
    }

    /// A hosted provider keeps its template's name and needs its key.
    #[test]
    fn a_hosted_provider_needs_its_key() {
        let mut setup = Setup::default();
        let anthropic = TEMPLATES.iter().find(|t| t.name == "anthropic").unwrap();
        setup.pick(anthropic);
        assert_eq!(setup.describe().unwrap_err(), "paste an API key");
        setup.key = "k".into();
        let (name, file) = setup.describe().unwrap();
        assert_eq!(
            (name.as_str(), file["display_name"].as_str()),
            ("anthropic", Some("Anthropic"))
        );
        assert_eq!(file["base_url"], "https://api.anthropic.com/v1");
    }

    #[test]
    fn hosted_providers_are_offered_before_your_own() {
        let offer = |name: &str, base_url: &str| Offered {
            name: name.into(),
            file: Provider::parse(name, "", json!({ "base_url": base_url }).to_string().as_bytes())
                .ok(),
        };
        let mut offered = vec![
            offer("linux-box", "http://linux-box:11434/v1"),
            offer("openai", "https://api.openai.com/v1"),
            offer("localhost", "http://localhost:11434/v1"),
            Offered { name: "broken".into(), file: None },
            offer("anthropic", "https://api.anthropic.com/v1"),
        ];
        order(&mut offered);
        let names: Vec<&str> = offered.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["anthropic", "broken", "openai", "localhost", "linux-box"]);
    }
}
