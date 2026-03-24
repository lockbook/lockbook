//! Auto-titling for pasted/inserted bare URLs: parses URLs, fetches page titles, extracts from HTML,
//! and replaces with [Title](url) markdown links. Works on all platforms (iOS, macOS, Linux, Windows, Android).
//!
//! Networked fetching lives in [`native`] and is disabled on wasm (see that module). HTML parsing uses
//! [`scraper`], consistent with the mind map’s URL title fetcher (`mind_map/data.rs`).

use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};

use scraper::{Html, Selector};
use url::Url;

type ReplacementPair = (String, String);
static REPLACEMENT_CHANNEL: OnceLock<(
    mpsc::Sender<ReplacementPair>,
    Mutex<mpsc::Receiver<ReplacementPair>>,
)> = OnceLock::new();

fn channel() -> &'static (mpsc::Sender<ReplacementPair>, Mutex<mpsc::Receiver<ReplacementPair>>) {
    REPLACEMENT_CHANNEL.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        (tx, Mutex::new(rx))
    })
}

/// Returns Some(url) if the text is a bare HTTP/HTTPS URL (only a URL, possibly with surrounding whitespace).
pub fn parse_bare_url(text: &str) -> Option<Url> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return None;
    }
    if trimmed.contains(' ') {
        return None;
    }
    let url = Url::parse(trimmed).ok()?;
    let scheme = url.scheme().to_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    Some(url)
}

/// Extracts the title from HTML: prefers `<title>`, falls back to `og:title` and `twitter:title` for SPAs.
fn extract_title_from_html(html: &str) -> Option<String> {
    const GENERIC_TITLES: &[&str] =
        &["youtube", "twitter", "x.com", "instagram", "facebook", "t.co"];

    let document = Html::parse_document(html);
    let title_sel = Selector::parse("title").expect("static selector");
    let title_text = document
        .select(&title_sel)
        .next()
        .map(|e| e.text().collect::<Vec<_>>().join("").trim().to_string());

    if let Some(ref title) = title_text {
        if !title.is_empty() {
            let lower = title.to_lowercase();
            let is_generic = GENERIC_TITLES
                .iter()
                .any(|g| lower.starts_with(g) || lower == *g);
            if !is_generic {
                return Some(trim_decoded_title(&decode_html_entities(title)));
            }
        }
    }

    let og_sel = Selector::parse(r#"meta[property="og:title"]"#).expect("static selector");
    if let Some(el) = document.select(&og_sel).next() {
        if let Some(c) = el.value().attr("content") {
            if !c.is_empty() {
                return Some(trim_decoded_title(&decode_html_entities(c)));
            }
        }
    }

    let tw_sel = Selector::parse(r#"meta[name="twitter:title"]"#).expect("static selector");
    if let Some(el) = document.select(&tw_sel).next() {
        if let Some(c) = el.value().attr("content") {
            if !c.is_empty() {
                return Some(trim_decoded_title(&decode_html_entities(c)));
            }
        }
    }

    if let Some(title) = title_text {
        if !title.is_empty() {
            return Some(trim_decoded_title(&decode_html_entities(&title)));
        }
    }
    None
}

/// Whitespace trim only after entity decode (no site-specific suffix stripping).
fn trim_decoded_title(title: &str) -> String {
    title.trim().to_string()
}

/// Decodes a small set of common named HTML entities in titles. This is not URL percent-decoding;
/// the `urlencoding` crate is for the latter (`%20`, etc.).
fn decode_html_entities(s: &str) -> String {
    let mut result = s.to_string();
    let replacements = [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&apos;", "'"),
        ("&nbsp;", "\u{00A0}"),
    ];
    for (entity, replacement) in replacements {
        result = result.replace(entity, replacement);
    }
    result
}

/// Formats title and URL as [title](url), escaping brackets in the title.
pub fn format_markdown_link(title: &str, url: &str) -> String {
    let mut s = String::with_capacity(title.len().saturating_add(url.len()).saturating_add(4));
    s.push('[');
    for ch in title.chars() {
        match ch {
            '\\' => s.push_str("\\\\"),
            ']' => s.push_str("\\]"),
            c => s.push(c),
        }
    }
    s.push_str("](");
    s.push_str(url);
    s.push(')');
    s
}

#[cfg(not(target_family = "wasm"))]
mod native;

#[cfg(not(target_family = "wasm"))]
pub use native::spawn_fetch_and_replace;

#[cfg(target_family = "wasm")]
pub fn spawn_fetch_and_replace(_from_text: String, _url: Url) {}

/// Drains pending URL→markdown replacements from the background fetcher. Call from the main thread
/// each frame. Returns (from, to) pairs to replace.
pub fn try_recv_pending_replacements() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let (_, rx_guard) = channel();
    if let Ok(rx) = rx_guard.lock() {
        while let Ok(pair) = rx.try_recv() {
            out.push(pair);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bare_url_valid_https() {
        let url = parse_bare_url("https://example.com");
        assert!(url.is_some());
        assert!(url.unwrap().as_str().starts_with("https://example.com"));
    }

    #[test]
    fn test_parse_bare_url_valid_http() {
        let url = parse_bare_url("http://example.com");
        assert!(url.is_some());
        assert!(url.unwrap().as_str().starts_with("http://example.com"));
    }

    #[test]
    fn test_parse_bare_url_with_whitespace() {
        let url = parse_bare_url("  https://example.com  ");
        assert!(url.is_some());
        assert!(url.unwrap().as_str().starts_with("https://example.com"));
    }

    #[test]
    fn test_parse_bare_url_rejects_space() {
        assert!(parse_bare_url("https://example.com more text").is_none());
    }

    #[test]
    fn test_parse_bare_url_rejects_empty() {
        assert!(parse_bare_url("").is_none());
        assert!(parse_bare_url("   ").is_none());
    }

    #[test]
    fn test_parse_bare_url_rejects_non_url() {
        assert!(parse_bare_url("not a url").is_none());
    }

    #[test]
    fn test_parse_bare_url_rejects_ftp() {
        assert!(parse_bare_url("ftp://example.com").is_none());
    }

    #[test]
    fn test_format_markdown_link_basic() {
        let result = format_markdown_link("Link", "https://example.com");
        assert_eq!(result, "[Link](https://example.com)");
    }

    #[test]
    fn test_format_markdown_link_escapes_brackets() {
        let result = format_markdown_link("See [here]", "https://example.com");
        assert_eq!(result, "[See [here\\]](https://example.com)");
    }

    #[test]
    fn test_format_markdown_link_escapes_backslash() {
        let result = format_markdown_link("C:\\path", "https://example.com");
        assert_eq!(result, "[C:\\\\path](https://example.com)");
    }

    #[test]
    fn test_extract_title_from_html_plain_title() {
        let html = "<html><head><title>Hello &amp; world</title></head></html>";
        assert_eq!(extract_title_from_html(html).as_deref(), Some("Hello & world"));
    }

    #[test]
    fn test_extract_title_from_html_prefers_og_when_title_generic() {
        let html = r#"<html><head>
            <title>YouTube</title>
            <meta property="og:title" content="Real Video Title" />
        </head></html>"#;
        assert_eq!(extract_title_from_html(html).as_deref(), Some("Real Video Title"));
    }

    #[test]
    fn test_decode_html_entities_nbsp() {
        let html = "<title>Foo&nbsp;Bar</title>";
        assert_eq!(extract_title_from_html(html).as_deref(), Some("Foo\u{00A0}Bar"));
    }
}
