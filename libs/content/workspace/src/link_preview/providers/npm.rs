//! npm: a package's page reads as the package's name, scope included; its
//! card comes from the registry.

use serde_json::Value;
use url::Url;

use super::{Provider, segments, text};
use crate::link_preview::LinkMeta;

pub(super) const PROVIDER: Provider = Provider { label, endpoint, parse };

/// The icon npm's pages link; `/favicon.ico` turns away non-browser agents.
const FAVICON: &str = "https://static-production.npmjs.com/b0f1a8318363185cc2ea6a40ac23eeb2.png";

fn label(url: &Url) -> Option<String> {
    match segments(url, "npmjs.com")?[..] {
        ["package", scope, name, ..] if scope.starts_with('@') => Some(format!("{scope}/{name}")),
        ["package", name, ..] => Some(name.to_string()),
        _ => None,
    }
}

fn endpoint(url: &Url) -> Option<String> {
    Some(format!("https://registry.npmjs.org/{}/latest", label(url)?))
}

fn parse(v: &Value, _: &Url) -> Option<LinkMeta> {
    Some(LinkMeta {
        title: text(&v["name"])?,
        description: text(&v["description"]),
        byline: text(&v["version"]).map(|v| format!("v{v}")),
        site_name: Some("npm".into()),
        favicon_url: Some(FAVICON.into()),
        ..Default::default()
    })
}
