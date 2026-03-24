//! Auto-titling for pasted/inserted bare URLs: parses URLs, fetches page titles, extracts from HTML,
//! and replaces with [Title](url) markdown links. Works on all platforms (iOS, macOS, Linux, Windows, Android).

use regex::Regex;
use std::sync::mpsc;
use std::thread;
use url::Url;

const CRAWLER_USER_AGENTS: &[&str] = &[
    "facebookexternalhit/1.1 (+http://www.facebook.com/externalhit_uatext.php)",
    "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
];

static REPLACEMENT_CHANNEL: std::sync::OnceLock<(mpsc::Sender<(String, String)>, std::sync::Mutex<mpsc::Receiver<(String, String)>>)> =
    std::sync::OnceLock::new();

fn channel() -> &'static (mpsc::Sender<(String, String)>, std::sync::Mutex<mpsc::Receiver<(String, String)>>) {
    REPLACEMENT_CHANNEL.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        (tx, std::sync::Mutex::new(rx))
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

/// Extracts the title from HTML: prefers <title>, falls back to og:title and twitter:title for SPAs.
fn extract_title_from_html(html: &str) -> Option<String> {
    const GENERIC_TITLES: &[&str] = &[
        "youtube", "twitter", "x.com", "instagram", "facebook", "t.co",
    ];

    if let Some(title) = extract_tag(html, r"<title[^>]*>(.*?)</title>") {
        if !title.is_empty() {
            let lower = title.to_lowercase();
            let is_generic = GENERIC_TITLES
                .iter()
                .any(|g| lower.starts_with(g) || lower == *g);
            if !is_generic {
                return Some(clean_title(&decode_html_entities(&title)));
            }
        }
    }
    if let Some(og) = extract_meta_content(html, Some("og:title"), None) {
        if !og.is_empty() {
            return Some(clean_title(&decode_html_entities(&og)));
        }
    }
    if let Some(tw) = extract_meta_content(html, None, Some("twitter:title")) {
        if !tw.is_empty() {
            return Some(clean_title(&decode_html_entities(&tw)));
        }
    }
    if let Some(title) = extract_tag(html, r"<title[^>]*>(.*?)</title>") {
        if !title.is_empty() {
            return Some(clean_title(&decode_html_entities(&title)));
        }
    }
    None
}

fn extract_tag(html: &str, pattern: &str) -> Option<String> {
    let re = Regex::new(pattern).ok()?;
    let caps = re.captures(html)?;
    let m = caps.get(1)?;
    Some(m.as_str().trim().to_string())
}

fn extract_meta_content(
    html: &str,
    property: Option<&str>,
    name: Option<&str>,
) -> Option<String> {
    let (attr, attr_value) = if let Some(p) = property {
        ("property", p)
    } else if let Some(n) = name {
        ("name", n)
    } else {
        return None;
    };
    let escaped = regex::escape(attr_value);
    let patterns = [
        format!(r#"<meta[^>]+{attr}=["']{escaped}["'][^>]+content=["']([^"']*)["']"#),
        format!(r#"<meta[^>]+content=["']([^"']*)["'][^>]+{attr}=["']{escaped}["']"#),
    ];
    for pat in patterns {
        if let Some(s) = extract_tag(html, &pat) {
            return Some(s);
        }
    }
    None
}

/// Strips common site suffixes like " - YouTube", " | X" from titles.
fn clean_title(title: &str) -> String {
    const SUFFIXES: &[&str] = &[
        " - youtube", " | youtube", " – youtube",
        " - twitter", " | twitter", " - x", " | x", " – x",
    ];
    let mut t = title.trim().to_string();
    for suffix in SUFFIXES {
        if t.to_lowercase().ends_with(suffix) {
            let len = t.len().saturating_sub(suffix.len());
            t = t[..len].trim_end().to_string();
            break;
        }
    }
    t
}

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
    let escaped = title
        .replace('\\', "\\\\")
        .replace(']', "\\]");
    format!("[{escaped}]({url})")
}

#[cfg(not(target_family = "wasm"))]
fn fetch_page_title_blocking(url: &Url) -> Option<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    for user_agent in CRAWLER_USER_AGENTS {
        let res = client
            .get(url.as_str())
            .header("User-Agent", *user_agent)
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept", "text/html,application/xhtml+xml")
            .send()
            .ok()?;

        if res.status() != reqwest::StatusCode::OK {
            continue;
        }
        let html = res.text().ok()?;
        if let Some(title) = extract_title_from_html(&html) {
            if !title.is_empty() {
                return Some(title);
            }
        }
    }
    None
}

/// Spawns a background thread to fetch the page title and enqueue a replacement. When the fetch
/// completes, (from_text, markdown_link) is sent to the channel for the main thread to process.
/// Only works on non-wasm targets.
#[cfg(not(target_family = "wasm"))]
pub fn spawn_fetch_and_replace(from_text: String, url: Url) {
    let tx = channel().0.clone();
    let url_str = url.to_string();
    thread::spawn(move || {
        let title = fetch_page_title_blocking(&url);
        let markdown_link = format_markdown_link(
            title.as_deref().unwrap_or(&url_str),
            &url_str,
        );
        let _ = tx.send((from_text, markdown_link));
    });
}

#[cfg(target_family = "wasm")]
pub fn spawn_fetch_and_replace(_from_text: String, _url: Url) {
    // No-op on wasm: no blocking HTTP
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
}

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
