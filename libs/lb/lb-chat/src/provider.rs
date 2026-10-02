//! Provider configuration lives in the vault as `/.agent/providers/<name>.json`
//! and `/.agent/default.json`, encrypted and synced like any note.

use std::net::IpAddr;

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
    /// The file's `display_name`, when it has one ("OpenRouter").
    pub display_name: Option<String>,
    pub kind: Kind,
    pub base_url: String,
    pub api_key: Option<String>,
    /// The file has a key field with nothing usable in it: blank, or the
    /// placeholder a template leaves. A file without the field needs none.
    pub needs_key: bool,
    pub model: String,
    /// How hard the model is asked to think: the chat's choice or the
    /// default's, and only a value `efforts` lists. Absent means the
    /// provider's own default.
    pub effort: Option<String>,
}

/// Where a real request has shown the effort setting to work beside tools:
/// host, model, and the values it took, least thinking first. Everything
/// else runs at its provider's default until it has been tried.
const EFFORTS: &[(&str, &str, &[&str])] = &[
    ("api.x.ai", "grok-4.7", &["low", "medium", "high", "xhigh"]),
    ("api.x.ai", "grok-4.3", &["none", "low", "medium", "high", "xhigh"]),
    ("generativelanguage.googleapis.com", "gemini-3.8-flash", &["none", "low", "medium", "high"]),
    ("generativelanguage.googleapis.com", "gemini-3.5-flash", &["none", "low", "medium", "high"]),
    ("api.cerebras.ai", "qwen-3.8-27b", &["none", "low", "medium", "high"]),
    ("api.cerebras.ai", "gpt-oss-120b", &["low", "medium", "high"]),
    ("api.groq.com", "openai/gpt-oss-120b", &["low", "medium", "high"]),
    ("api.groq.com", "openai/gpt-oss-20b", &["low", "medium", "high"]),
    ("api.groq.com", "qwen/qwen3.8-27b", &["none", "low", "medium"]),
    ("api.anthropic.com", "claude-sonnet-5-5", &["low", "medium", "high", "xhigh", "max"]),
    ("api.anthropic.com", "claude-opus-5-5", &["low", "medium", "high", "xhigh", "max"]),
    ("api.anthropic.com", "claude-fable-5-1", &["low", "medium", "high", "xhigh", "max"]),
    ("api.anthropic.com", "claude-opus-5", &["low", "medium", "high", "xhigh", "max"]),
    ("api.anthropic.com", "claude-sonnet-5", &["low", "medium", "high", "xhigh", "max"]),
    ("api.anthropic.com", "claude-fable-5", &["low", "medium", "high", "xhigh", "max"]),
    ("api.anthropic.com", "claude-opus-4-8", &["low", "medium", "high", "xhigh", "max"]),
    ("api.anthropic.com", "claude-opus-4-7", &["low", "medium", "high", "xhigh", "max"]),
    ("api.anthropic.com", "claude-sonnet-4-6", &["low", "medium", "high", "max"]),
    ("api.anthropic.com", "claude-opus-4-6", &["low", "medium", "high", "max"]),
    ("api.anthropic.com", "claude-opus-4-5-20251101", &["on"]),
    ("api.anthropic.com", "claude-haiku-4-5-20251001", &["on"]),
    ("api.anthropic.com", "claude-sonnet-4-5-20250929", &["on"]),
    ("openrouter.ai", "anthropic/claude-haiku-4.5", &["none", "low", "medium", "high"]),
    ("openrouter.ai", "anthropic/claude-sonnet-5.5", &["low", "medium", "high", "xhigh", "max"]),
    ("openrouter.ai", "anthropic/claude-opus-5.5", &["low", "medium", "high", "xhigh", "max"]),
    ("openrouter.ai", "openai/gpt-5.5", &["none", "low", "medium", "high", "xhigh"]),
];

#[derive(Deserialize)]
struct ProviderFile {
    #[serde(default)]
    display_name: Option<String>,
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
    #[serde(default)]
    effort: Option<String>,
}

const PLACEHOLDER_KEY: &str = "YOUR API KEY HERE";

impl Provider {
    /// The provider a chat's settings select, falling back to the vault
    /// default. `model` in settings is `provider/model`; a bare provider name
    /// means the file's own model. A chat that follows the default's model
    /// follows its effort too, unless it has chosen one.
    pub fn resolve(lb: &Lb, settings: &Settings) -> Result<Provider, String> {
        let (name, model, effort) = match &settings.model {
            Some(selection) => {
                let (name, model) = split(selection);
                (name, model, None)
            }
            None => {
                let bytes = read(lb, "/.agent/default.json")?
                    .ok_or("no provider selected and no /.agent/default.json")?;
                let default: DefaultFile = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("/.agent/default.json: {e}"))?;
                (default.provider, default.model, default.effort)
            }
        };
        let mut provider = Self::load(lb, &name, &model)?;
        if provider.model.is_empty() {
            return Err(format!("/.agent/providers/{name}.json: no model"));
        }
        let effort = settings.effort.clone().or(effort);
        provider.effort = effort.filter(|e| provider.efforts().contains(&e.as_str()));
        Ok(provider)
    }

    /// The values this model's effort may take: those it has been shown to
    /// take, and none for a model that has not been tried.
    pub fn efforts(&self) -> &'static [&'static str] {
        let host = host(&self.base_url);
        EFFORTS
            .iter()
            .find(|(at, model, _)| *at == host && *model == self.model)
            .map_or(&[], |(_, _, values)| values)
    }

    pub fn load(lb: &Lb, name: &str, model: &str) -> Result<Provider, String> {
        let path = format!("/.agent/providers/{name}.json");
        let bytes = read(lb, &path)?.ok_or_else(|| format!("no provider file at {path}"))?;
        Self::parse(name, model, &bytes)
    }

    /// The provider a file's bytes describe; `model`, when given, overrides
    /// the file's own.
    pub fn parse(name: &str, model: &str, bytes: &[u8]) -> Result<Provider, String> {
        let path = format!("/.agent/providers/{name}.json");
        let file: ProviderFile =
            serde_json::from_slice(bytes).map_err(|e| format!("{path}: {e}"))?;
        let kind = match file.kind.as_str() {
            "anthropic" => Kind::Anthropic,
            "openai" => Kind::OpenAi,
            other => return Err(format!("{path}: unknown kind {other:?}")),
        };
        let needs_key = file
            .api_key
            .as_deref()
            .is_some_and(|k| k.trim().is_empty() || k == PLACEHOLDER_KEY);
        let api_key = file.api_key.filter(|_| !needs_key);
        let model = if model.is_empty() { file.model } else { model.to_string() };
        Ok(Provider {
            name: name.to_string(),
            display_name: file.display_name.filter(|n| !n.trim().is_empty()),
            kind,
            base_url: file.base_url.trim_end_matches('/').to_string(),
            api_key,
            needs_key,
            model,
            effort: None,
        })
    }

    /// What the interface calls this provider: the file's `display_name`,
    /// else its name made readable.
    pub fn label(&self) -> String {
        self.display_name
            .clone()
            .unwrap_or_else(|| friendly_name(&self.name))
    }

    /// `provider/model`, the form `Settings::model` carries.
    pub fn selection(&self) -> String {
        format!("{}/{}", self.name, self.model)
    }

    pub fn place(&self) -> Place {
        place(&self.base_url)
    }
}

/// Where an endpoint's address puts what is sent to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Loopback: the model runs on this device.
    ThisDevice,
    /// A private or tailnet address, or a name only a local network
    /// resolves: a machine of the user's own.
    YourNetwork,
    Internet,
}

/// A base URL's host: lowercase, without scheme, credentials, port or
/// brackets.
pub fn host(base_url: &str) -> String {
    let rest = base_url
        .split_once("://")
        .map_or(base_url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(v6),
        None => authority.split(':').next().unwrap_or(authority),
    };
    host.to_ascii_lowercase()
}

pub fn place(base_url: &str) -> Place {
    /// Names the public internet does not resolve.
    const OWN: &[&str] = &[".local", ".lan", ".internal", ".home.arpa", ".ts.net"];
    let host = host(base_url);
    let (this_device, own) = match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            // 100.64/10 is carrier-grade NAT space, where Tailscale lives.
            let [a, b, ..] = ip.octets();
            let tailnet = a == 100 && (64..128).contains(&b);
            (
                ip.is_loopback() || ip.is_unspecified(),
                ip.is_private() || ip.is_link_local() || tailnet,
            )
        }
        Ok(IpAddr::V6(ip)) => {
            // Unique local fc00::/7 and link-local fe80::/10.
            let head = ip.segments()[0];
            (
                ip.is_loopback() || ip.is_unspecified(),
                head & 0xfe00 == 0xfc00 || head & 0xffc0 == 0xfe80,
            )
        }
        Err(_) => (
            host == "localhost" || host.ends_with(".localhost"),
            !host.contains('.') || OWN.iter().any(|suffix| host.ends_with(suffix)),
        ),
    };
    if this_device {
        Place::ThisDevice
    } else if own {
        Place::YourNetwork
    } else {
        Place::Internet
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

    /// An address says where messages go: nowhere, to a machine of yours,
    /// or out to the internet.
    #[test]
    fn an_address_says_where_messages_go() {
        for url in [
            "http://localhost:11434/v1",
            "http://127.0.0.1:1234/v1",
            "http://[::1]:8080/v1",
            "http://0.0.0.0:8000",
        ] {
            assert_eq!(place(url), Place::ThisDevice, "{url}");
        }
        for url in [
            "http://100.64.0.1:11434/v1",
            "http://100.101.102.103:11434/v1",
            "http://192.168.1.20:11434/v1",
            "http://10.0.0.5/v1",
            "http://172.20.3.4:8000/v1",
            "http://linux-box:11434/v1",
            "https://linux-box.tail1234.ts.net/v1",
            "http://nas.local:8080/v1",
            "http://[fd7a:115c:a1e0::1]:11434/v1",
        ] {
            assert_eq!(place(url), Place::YourNetwork, "{url}");
        }
        for url in [
            "https://api.openai.com/v1",
            "https://api.groq.com/openai/v1",
            "http://172.32.0.1/v1",
            "http://100.128.0.1/v1",
            "https://8.8.8.8/v1",
        ] {
            assert_eq!(place(url), Place::Internet, "{url}");
        }
        assert_eq!(
            host("http://user:pw@Linux-Box.tail1234.ts.net:11434/v1"),
            "linux-box.tail1234.ts.net"
        );
        assert_eq!(host("http://[::1]:8080/v1"), "::1");
        assert_eq!(host("localhost:11434"), "localhost");
    }

    /// A key field with nothing usable in it means the provider is not set
    /// up; no key field at all means it needs none.
    #[test]
    fn a_blank_or_placeholder_key_needs_a_key() {
        let file = |key: &str| format!("{{\"base_url\":\"https://x/v1/\",\"model\":\"m\"{key}}}");
        let parse = |key: &str| Provider::parse("p", "", file(key).as_bytes()).unwrap();
        for key in
            [",\"api_key\":\"YOUR API KEY HERE\"", ",\"api_key\":\"\"", ",\"api_key\":\"  \""]
        {
            let p = parse(key);
            assert!(p.needs_key && p.api_key.is_none(), "{key}");
        }
        let keyed = parse(",\"api_key\":\"sk-real\"");
        assert!(!keyed.needs_key && keyed.api_key.as_deref() == Some("sk-real"));
        let keyless = parse("");
        assert!(!keyless.needs_key && keyless.api_key.is_none());
        assert_eq!((keyless.base_url.as_str(), keyless.model.as_str()), ("https://x/v1", "m"));
        assert_eq!(
            Provider::parse("p", "other", file("").as_bytes())
                .unwrap()
                .model,
            "other"
        );
    }

    #[test]
    fn providers_read_like_their_names() {
        assert_eq!(friendly_name("openai"), "OpenAI");
        assert_eq!(friendly_name("xai"), "xAI");
        assert_eq!(friendly_name("anthropic"), "Anthropic");
        assert_eq!(friendly_name("my-box"), "My-box");
    }
    /// The effort setting exists only where it has been shown to work: a
    /// model that was tried lists what it took, and nothing else lists
    /// anything, whatever its listing or its documentation says.
    #[test]
    fn only_a_tried_model_offers_an_effort() {
        let at = |base_url: &str, model: &str| {
            let file = serde_json::json!({ "base_url": base_url }).to_string();
            Provider::parse("p", model, file.as_bytes()).unwrap()
        };
        let grok = at("https://api.x.ai/v1/", "grok-4.7");
        assert_eq!(grok.efforts(), ["low", "medium", "high", "xhigh"]);
        assert!(at("https://api.x.ai/v1", "grok-9").efforts().is_empty());
        // The same model by another road is another row.
        let haiku = at("https://openrouter.ai/api/v1", "anthropic/claude-haiku-4.5");
        assert_eq!(haiku.efforts(), ["none", "low", "medium", "high"]);
        let haiku = at("https://api.anthropic.com/v1", "claude-haiku-4-5-20251001");
        assert_eq!(haiku.efforts(), ["on"]);
        assert!(
            at("https://api.openai.com/v1", "gpt-5.5")
                .efforts()
                .is_empty()
        );
        assert!(
            at("http://localhost:11434/v1", "grok-4.7")
                .efforts()
                .is_empty()
        );
    }
}
