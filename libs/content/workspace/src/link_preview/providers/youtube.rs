//! YouTube: videos come from oEmbed, since a watch page puts its title and
//! tags past the first 256 KB a page fetch reads.

use serde_json::Value;
use url::Url;

use super::{Provider, joined, segments, text, with_url};
use crate::link_preview::LinkMeta;

pub(super) const PROVIDER: Provider = Provider { label: |_| None, endpoint, parse };

fn video_id(url: &Url) -> Option<String> {
    let id = match segments(url, "youtu.be") {
        Some(path) => path.first().map(|s| s.to_string()),
        None => {
            let path = segments(url, "youtube.com").or_else(|| segments(url, "m.youtube.com"))?;
            match path[..] {
                ["watch"] => url
                    .query_pairs()
                    .find(|(k, _)| k == "v")
                    .map(|(_, v)| v.into()),
                ["shorts" | "live" | "embed", id, ..] => Some(id.to_string()),
                _ => None,
            }
        }
    }?;
    Some(id).filter(|id| {
        id.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    })
}

fn endpoint(url: &Url) -> Option<String> {
    let watch = Url::parse(&format!("https://www.youtube.com/watch?v={}", video_id(url)?)).ok()?;
    Some(with_url("https://www.youtube.com/oembed", &watch) + "&format=json")
}

/// `t=95`, `t=95s`, or `t=1m35s`, in seconds.
fn start(url: &Url) -> Option<u64> {
    let t = url.query_pairs().find(|(k, _)| k == "t")?.1;
    let mut total = 0;
    let mut n = 0;
    for c in t.chars() {
        match c {
            '0'..='9' => n = n * 10 + c.to_digit(10)? as u64,
            'h' => (total, n) = (total + n * 3600, 0),
            'm' => (total, n) = (total + n * 60, 0),
            's' => (total, n) = (total + n, 0),
            _ => return None,
        }
    }
    Some(total + n).filter(|&s| s > 0)
}

fn clock(seconds: u64) -> String {
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

fn parse(v: &Value, url: &Url) -> Option<LinkMeta> {
    let id = video_id(url)?;
    Some(LinkMeta {
        title: text(&v["title"])?,
        site_name: Some("YouTube".into()),
        // 16:9 at every size, unlike oEmbed's letterboxed 4:3 thumbnail
        thumbnail_url: Some(format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")),
        thumbnail_width: Some(320),
        thumbnail_height: Some(180),
        favicon_url: Some("https://www.youtube.com/favicon.ico".into()),
        byline: joined([
            text(&v["author_name"]),
            start(url).map(|s| format!("starts at {}", clock(s))),
        ]),
        ..Default::default()
    })
}
