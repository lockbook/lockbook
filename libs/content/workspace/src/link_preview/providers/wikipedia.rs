//! Wikipedia: an article link reads as its title; its card comes from the
//! REST summary Wikipedia's own page previews use.

use serde_json::Value;
use url::Url;

use super::{Provider, text};
use crate::link_preview::LinkMeta;

pub(super) const PROVIDER: Provider = Provider { label, endpoint, parse };

/// The article's title as it appears in the URL's path.
fn article(url: &Url) -> Option<&str> {
    if !url.host_str()?.ends_with(".wikipedia.org") {
        return None;
    }
    url.path().strip_prefix("/wiki/").filter(|t| !t.is_empty())
}

fn label(url: &Url) -> Option<String> {
    let title = urlencoding::decode(article(url)?).ok()?.replace('_', " ");
    Some(title)
}

fn endpoint(url: &Url) -> Option<String> {
    let host = url
        .host_str()?
        .replace(".m.wikipedia.org", ".wikipedia.org");
    Some(format!("https://{host}/api/rest_v1/page/summary/{}", article(url)?))
}

fn parse(v: &Value, _: &Url) -> Option<LinkMeta> {
    let thumb = &v["thumbnail"];
    Some(LinkMeta {
        title: text(&v["title"])?,
        byline: text(&v["description"]),
        description: text(&v["extract"]),
        site_name: Some("Wikipedia".into()),
        thumbnail_url: text(&thumb["source"]),
        thumbnail_width: thumb["width"].as_u64().map(|w| w as u32),
        thumbnail_height: thumb["height"].as_u64().map(|h| h as u32),
        favicon_url: Some("https://en.wikipedia.org/static/favicon/wikipedia.ico".into()),
        ..Default::default()
    })
}
