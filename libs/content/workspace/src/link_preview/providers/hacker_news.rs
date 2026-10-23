//! Hacker News: its pages carry no preview tags, so stories come from the
//! official Firebase API.

use serde_json::Value;
use url::Url;

use super::{Provider, html_text, joined, segments, text};
use crate::link_preview::LinkMeta;

pub(super) const PROVIDER: Provider = Provider { label: |_| None, endpoint, parse };

fn endpoint(url: &Url) -> Option<String> {
    if segments(url, "news.ycombinator.com")? != ["item"] {
        return None;
    }
    let id: u64 = url.query_pairs().find(|(k, _)| k == "id")?.1.parse().ok()?;
    Some(format!("https://hacker-news.firebaseio.com/v0/item/{id}.json"))
}

fn parse(v: &Value, _: &Url) -> Option<LinkMeta> {
    let count = |n: &Value, one: &str, many: &str| {
        n.as_u64()
            .map(|n| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") })
    };
    // a link's site, or an Ask HN's own words
    let linked = text(&v["url"]).and_then(|u| {
        Url::parse(&u)
            .ok()?
            .host_str()
            .map(|h| h.trim_start_matches("www.").into())
    });
    Some(LinkMeta {
        title: text(&v["title"])?,
        description: linked.or_else(|| v["text"].as_str().map(html_text)),
        byline: joined([
            count(&v["score"], "point", "points"),
            count(&v["descendants"], "comment", "comments"),
            text(&v["by"]),
        ]),
        site_name: Some("Hacker News".into()),
        favicon_url: Some("https://news.ycombinator.com/favicon.ico".into()),
        ..Default::default()
    })
}
