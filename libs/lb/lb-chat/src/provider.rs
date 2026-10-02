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
}

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
        let provider = Self::load(lb, &name, &model)?;
        if provider.model.is_empty() {
            return Err(format!("/.agent/providers/{name}.json: no model"));
        }
        Ok(provider)
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
}
