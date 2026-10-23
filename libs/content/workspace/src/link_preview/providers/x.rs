//! X (formerly Twitter): profiles and posts read as `@handle`; a post's card
//! comes from oEmbed, which carries its text and date.

use scraper::{Html, Selector};
use serde_json::Value;
use url::Url;

use super::{Provider, html_text, joined, segments, text, with_url};
use crate::link_preview::LinkMeta;

pub(super) const PROVIDER: Provider = Provider { label, endpoint, parse };

/// First path segments that are X's own pages, not a handle.
const NOT_HANDLES: &[&str] = &[
    "home",
    "explore",
    "search",
    "i",
    "settings",
    "notifications",
    "messages",
    "hashtag",
    "intent",
    "share",
    "login",
    "signup",
    "tos",
    "privacy",
];

fn path(url: &Url) -> Option<Vec<&str>> {
    let path = segments(url, "x.com").or_else(|| segments(url, "twitter.com"))?;
    Some(path).filter(|p| p.first().is_some_and(|h| !NOT_HANDLES.contains(h)))
}

fn label(url: &Url) -> Option<String> {
    Some(format!("@{}", path(url)?[0]))
}

fn endpoint(url: &Url) -> Option<String> {
    match path(url)?[..] {
        [_, "status", id, ..] if id.parse::<u64>().is_ok() => {
            Some(with_url("https://publish.x.com/oembed", url) + "&omit_script=1")
        }
        _ => None,
    }
}

fn parse(v: &Value, url: &Url) -> Option<LinkMeta> {
    // the embed is a blockquote: the post's text, then a link labeled with its date
    let embed = Html::parse_fragment(v["html"].as_str()?);
    let last = |css| {
        let sel = Selector::parse(css).ok()?;
        let el = embed.select(&sel).next_back()?;
        Some(html_text(&el.inner_html())).filter(|s| !s.is_empty())
    };
    Some(LinkMeta {
        title: text(&v["author_name"])?,
        description: last("p"),
        byline: joined([label(url), last("a")]),
        site_name: Some("X".into()),
        favicon_url: Some("https://abs.twimg.com/favicons/twitter.3.ico".into()),
        ..Default::default()
    })
}
