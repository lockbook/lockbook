//! crates.io: a crate's page reads as the crate's name. The site's pages
//! are drawn by script with nothing to preview, so cards come from its API.

use serde_json::Value;
use url::Url;

use super::{Provider, compact, joined, segments, text};
use crate::link_preview::LinkMeta;

pub(super) const PROVIDER: Provider = Provider { label, endpoint, parse };

fn label(url: &Url) -> Option<String> {
    match segments(url, "crates.io")?[..] {
        ["crates", name, ..] => Some(name.to_string()),
        _ => None,
    }
}

fn endpoint(url: &Url) -> Option<String> {
    // without `include`, the answer lists every version and runs past 256 KB
    Some(format!("https://crates.io/api/v1/crates/{}?include=default_version", label(url)?))
}

fn parse(v: &Value, _: &Url) -> Option<LinkMeta> {
    let c = &v["crate"];
    Some(LinkMeta {
        title: text(&c["name"])?,
        description: text(&c["description"]),
        byline: joined([
            text(&c["default_version"]).map(|v| format!("v{v}")),
            c["downloads"]
                .as_u64()
                .map(|n| format!("{} downloads", compact(n))),
        ]),
        site_name: Some("crates.io".into()),
        favicon_url: Some("https://crates.io/favicon.ico".into()),
        ..Default::default()
    })
}
