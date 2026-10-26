//! Sites a preview knows by name, one file each. A provider reads what a
//! link is from its URL and, where the site offers one, fetches a small
//! documented endpoint instead of the page.

mod crates;
mod github;
mod hacker_news;
mod npm;
mod stack_exchange;
mod wikipedia;
mod x;
mod youtube;

use serde_json::Value;
use url::Url;

use super::LinkMeta;

struct Provider {
    /// The chip's text when the URL alone says what the link is.
    label: fn(&Url) -> Option<String>,
    /// The endpoint that describes this link, when there is one.
    endpoint: fn(&Url) -> Option<String>,
    /// The preview, read from what [`Self::endpoint`] answered.
    parse: fn(&Value, &Url) -> Option<LinkMeta>,
}

const PROVIDERS: &[Provider] = &[
    github::PROVIDER,
    youtube::PROVIDER,
    wikipedia::PROVIDER,
    x::PROVIDER,
    hacker_news::PROVIDER,
    stack_exchange::PROVIDER,
    crates::PROVIDER,
    npm::PROVIDER,
];

/// The chip text a provider reads off `url` (`lockbook#5188`, `@jack`). It
/// needs no fetch, so it shows with previews off too.
pub fn label(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    PROVIDERS.iter().find_map(|p| (p.label)(&url))
}

/// Where a link's preview comes from when a provider knows a better place
/// than its page.
pub struct Source {
    pub endpoint: String,
    url: Url,
    parse: fn(&Value, &Url) -> Option<LinkMeta>,
}

impl Source {
    pub fn parse(&self, body: &str) -> Option<LinkMeta> {
        (self.parse)(&serde_json::from_str(body).ok()?, &self.url)
    }
}

pub fn source(url: &str) -> Option<Source> {
    let url = Url::parse(url).ok()?;
    PROVIDERS.iter().find_map(|p| {
        let endpoint = (p.endpoint)(&url)?;
        Some(Source { endpoint, url: url.clone(), parse: p.parse })
    })
}

/// `url`'s path segments when its host is `host` or a `www.` of it.
fn segments<'a>(url: &'a Url, host: &str) -> Option<Vec<&'a str>> {
    let h = url.host_str()?;
    if h != host && h.strip_prefix("www.") != Some(host) {
        return None;
    }
    Some(url.path_segments()?.filter(|s| !s.is_empty()).collect())
}

/// `url` with `link` as its one query value, for oEmbed endpoints.
fn with_url(endpoint: &str, link: &Url) -> String {
    format!("{endpoint}?url={}", urlencoding::encode(link.as_str()))
}

fn text(value: &Value) -> Option<String> {
    Some(value.as_str()?.trim().to_string()).filter(|s| !s.is_empty())
}

/// 27546 → "27.5k".
fn compact(n: u64) -> String {
    let short = |n: f64, unit| {
        let digits = if n < 100.0 { format!("{n:.1}") } else { format!("{n:.0}") };
        format!("{}{unit}", digits.trim_end_matches(".0"))
    };
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => short(n as f64 / 1e3, "k"),
        1_000_000..1_000_000_000 => short(n as f64 / 1e6, "M"),
        _ => short(n as f64 / 1e9, "B"),
    }
}

/// The text of an HTML fragment, tags dropped and entities decoded.
fn html_text(fragment: &str) -> String {
    let doc = scraper::Html::parse_fragment(fragment);
    doc.root_element()
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// "a · b · c" from the parts that are there.
fn joined<const N: usize>(parts: [Option<String>; N]) -> Option<String> {
    let parts: Vec<String> = parts.into_iter().flatten().collect();
    Some(parts.join(" · ")).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{compact, label};

    #[test]
    fn counts_read_short() {
        for (n, want) in [
            (7, "7"),
            (999, "999"),
            (1_000, "1k"),
            (27_546, "27.5k"),
            (104_000, "104k"),
            (1_578_778_352, "1.6B"),
        ] {
            assert_eq!(compact(n), want);
        }
    }

    #[test]
    fn labels_from_urls() {
        let cases = [
            ("https://github.com/lockbook/lockbook/issues/5188", Some("lockbook#5188")),
            ("https://github.com/lockbook/lockbook/pull/5229/files", Some("lockbook#5229")),
            (
                "https://github.com/lockbook/lockbook/pull/5229#issuecomment-1",
                Some("lockbook#5229"),
            ),
            ("https://github.com/lockbook/lockbook/discussions/12", Some("lockbook#12")),
            (
                "https://github.com/lockbook/lockbook/commit/dd32572ee0a1b2c3",
                Some("lockbook@dd32572"),
            ),
            ("https://github.com/lockbook/lockbook", Some("lockbook/lockbook")),
            ("https://github.com/lockbook/lockbook/", Some("lockbook/lockbook")),
            ("https://www.github.com/lockbook/lockbook/issues/1", Some("lockbook#1")),
            ("https://github.com/lockbook/lockbook/blob/master/README.md", None),
            ("https://github.com/lockbook/lockbook/issues", None),
            ("https://github.com/lockbook/lockbook/issues/new", None),
            ("https://github.com/orgs/lockbook/projects", None),
            ("https://github.com/lockbook", None),
            ("https://gist.github.com/lockbook/lockbook", None),
            ("https://notgithub.com/lockbook/lockbook/issues/1", None),
            (
                "https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type",
                Some("Conflict-free replicated data type"),
            ),
            (
                "https://en.wikipedia.org/wiki/Ramer%E2%80%93Douglas%E2%80%93Peucker_algorithm",
                Some("Ramer–Douglas–Peucker algorithm"),
            ),
            ("https://en.m.wikipedia.org/wiki/Markdown#History", Some("Markdown")),
            ("https://en.wikipedia.org/w/index.php?title=Markdown", None),
            ("https://x.com/jack/status/20", Some("@jack")),
            ("https://twitter.com/rustlang", Some("@rustlang")),
            ("https://x.com/home", None),
            ("https://x.com/i/status/20", None),
            ("https://crates.io/crates/serde", Some("serde")),
            ("https://crates.io/crates/serde/1.0.228", Some("serde")),
            ("https://crates.io/search?q=serde", None),
            ("https://www.npmjs.com/package/react", Some("react")),
            ("https://www.npmjs.com/package/@tanstack/query-core", Some("@tanstack/query-core")),
            ("https://example.com/lockbook/lockbook/issues/1", None),
        ];
        for (url, want) in cases {
            assert_eq!(label(url).as_deref(), want, "{url}");
        }
    }
}
