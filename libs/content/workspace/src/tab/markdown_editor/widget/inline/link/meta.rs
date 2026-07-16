//! Link-preview metadata: the data a card is built from, plus the pure HTML
//! scrape that produces it (no fetch / cache / render coupling, so it's unit-
//! testable in isolation). Field precedence follows the cross-platform
//! convention (Open Graph → Twitter Card → plain HTML), resolving relative
//! `og:image`/favicon paths against the page URL.

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

/// Preview metadata scraped from an external link's page — the data a card is
/// built from. `Serialize`/`Deserialize` ready it for the synced sidecar.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkMeta {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    /// Declared `og:image` dimensions, when present — lets a card pick its
    /// hero-vs-horizontal form before the texture decodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_alt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favicon_url: Option<String>,
}

/// Async fetch state for a URL's `LinkMeta`, held in the layout cache.
pub enum LinkMetaState {
    Loading,
    Loaded(LinkMeta),
    /// `retry_at: Some(t)` — eligible for a refetch once `t` passes (the
    /// host's `Retry-After`, a transient-failure backoff, or a bot-wall
    /// cooldown). `None` — deterministic failure, no retry this session.
    /// `attempts` counts consecutive failures and drives the backoff.
    Failed {
        retry_at: Option<web_time::Instant>,
        attempts: u32,
    },
}

/// Scrape `LinkMeta` from a page's HTML. `base_url` is the page's own URL,
/// used to resolve relative image/favicon paths and to derive a fallback site
/// name/favicon. Returns `None` only when no usable title can be found.
pub fn extract_link_meta(html: &str, base_url: &str) -> Option<LinkMeta> {
    let doc = Html::parse_document(html);
    let title = extract_title(&doc)?;
    Some(LinkMeta {
        title,
        description: extract_description(&doc),
        site_name: meta_content(&doc, "meta[property='og:site_name']")
            .or_else(|| host_of(base_url)),
        thumbnail_url: meta_content(&doc, "meta[property='og:image'], meta[name='twitter:image']")
            .and_then(|u| resolve_url(base_url, &u)),
        thumbnail_width: meta_u32(&doc, "meta[property='og:image:width']"),
        thumbnail_height: meta_u32(&doc, "meta[property='og:image:height']"),
        thumbnail_alt: meta_content(
            &doc,
            "meta[property='og:image:alt'], meta[name='twitter:image:alt']",
        ),
        favicon_url: extract_favicon_url(&doc, base_url),
    })
}

/// og:title → twitter:title → `<title>`.
fn extract_title(doc: &Html) -> Option<String> {
    meta_content(doc, "meta[property='og:title'], meta[name='twitter:title']").or_else(|| {
        let sel = Selector::parse("title").ok()?;
        doc.select(&sel)
            .next()
            .map(|e| e.text().collect::<String>())
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
    })
}

/// og:description → twitter:description → `meta[name=description]`.
fn extract_description(doc: &Html) -> Option<String> {
    meta_content(
        doc,
        "meta[property='og:description'], meta[name='twitter:description'], meta[name='description']",
    )
}

/// First `<link rel~="icon">` href resolved against `base_url`, falling back to
/// the origin's `/favicon.ico` (the request may 404; the image cache treats
/// that as a miss and the favicon is simply dropped).
fn extract_favicon_url(doc: &Html, base_url: &str) -> Option<String> {
    if let Ok(sel) = Selector::parse("link[rel~='icon']") {
        if let Some(href) = doc
            .select(&sel)
            .find_map(|e| e.value().attr("href"))
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty())
            .and_then(|h| resolve_url(base_url, &h))
        {
            return Some(href);
        }
    }
    let origin = url::Url::parse(base_url).ok()?;
    Some(origin.join("/favicon.ico").ok()?.to_string())
}

/// First non-empty `content` attribute matching `selectors` (a CSS selector
/// list, tried in order via the comma group).
fn meta_content(doc: &Html, selectors: &str) -> Option<String> {
    let sel = Selector::parse(selectors).ok()?;
    doc.select(&sel)
        .find_map(|e| e.value().attr("content"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn meta_u32(doc: &Html, selectors: &str) -> Option<u32> {
    meta_content(doc, selectors)?.parse().ok()
}

fn host_of(base_url: &str) -> Option<String> {
    url::Url::parse(base_url)
        .ok()?
        .host_str()
        .map(|h| h.trim_start_matches("www.").to_string())
}

fn resolve_url(base: &str, href: &str) -> Option<String> {
    Some(url::Url::parse(base).ok()?.join(href).ok()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://example.com/article";

    #[test]
    fn prefers_open_graph_over_html_title() {
        let html = r#"<html><head>
            <title>Raw Page Title | Example</title>
            <meta property="og:title" content="OG Title">
        </head></html>"#;
        assert_eq!(extract_link_meta(html, BASE).unwrap().title, "OG Title");
    }

    #[test]
    fn falls_back_to_html_title() {
        let html = "<html><head><title>  Just A Title  </title></head></html>";
        assert_eq!(extract_link_meta(html, BASE).unwrap().title, "Just A Title");
    }

    #[test]
    fn full_card_metadata() {
        let html = r#"<html><head>
            <title>t</title>
            <meta property="og:title" content="Title">
            <meta property="og:description" content="A description.">
            <meta property="og:site_name" content="Example Site">
            <meta property="og:image" content="/img/hero.png">
            <meta property="og:image:width" content="1200">
            <meta property="og:image:height" content="630">
            <meta property="og:image:alt" content="alt text">
            <link rel="icon" href="/favicon.png">
        </head></html>"#;
        let m = extract_link_meta(html, BASE).unwrap();
        assert_eq!(m.title, "Title");
        assert_eq!(m.description.as_deref(), Some("A description."));
        assert_eq!(m.site_name.as_deref(), Some("Example Site"));
        assert_eq!(m.thumbnail_url.as_deref(), Some("https://example.com/img/hero.png"));
        assert_eq!(m.thumbnail_width, Some(1200));
        assert_eq!(m.thumbnail_height, Some(630));
        assert_eq!(m.thumbnail_alt.as_deref(), Some("alt text"));
        assert_eq!(m.favicon_url.as_deref(), Some("https://example.com/favicon.png"));
    }

    #[test]
    fn twitter_card_fallbacks() {
        let html = r#"<html><head><title>t</title>
            <meta name="twitter:title" content="TW Title">
            <meta name="twitter:description" content="tw desc">
            <meta name="twitter:image" content="https://cdn.example.com/i.jpg">
        </head></html>"#;
        let m = extract_link_meta(html, BASE).unwrap();
        assert_eq!(m.title, "TW Title");
        assert_eq!(m.description.as_deref(), Some("tw desc"));
        assert_eq!(m.thumbnail_url.as_deref(), Some("https://cdn.example.com/i.jpg"));
    }

    #[test]
    fn favicon_falls_back_to_origin_root() {
        let html = "<html><head><title>t</title></head></html>";
        let m = extract_link_meta(html, BASE).unwrap();
        assert_eq!(m.favicon_url.as_deref(), Some("https://example.com/favicon.ico"));
    }

    #[test]
    fn site_name_falls_back_to_host_without_www() {
        let html = "<html><head><title>t</title></head></html>";
        let m = extract_link_meta(html, "https://www.example.com/x").unwrap();
        assert_eq!(m.site_name.as_deref(), Some("example.com"));
    }

    #[test]
    fn no_title_yields_none() {
        let html = "<html><head></head><body>no metadata</body></html>";
        assert!(extract_link_meta(html, BASE).is_none());
    }

    #[test]
    fn description_absent_is_none() {
        let html = "<html><head><title>t</title></head></html>";
        assert_eq!(extract_link_meta(html, BASE).unwrap().description, None);
    }
}

/// Whether scraped metadata describes the wall between us and the page — a
/// bot challenge or block/error page served as HTML — or carries nothing the
/// URL didn't (empty or bare-domain title). The plain URL reads better than
/// any of these, so callers treat junk as a fetch failure.
pub fn is_junk_meta(html: &str, meta: &LinkMeta, base_url: &str) -> bool {
    let title = meta.title.trim();
    if title.is_empty() {
        return true;
    }
    if let Some(host) = url::Url::parse(base_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
    {
        if title.eq_ignore_ascii_case(&host)
            || title.eq_ignore_ascii_case(host.trim_start_matches("www."))
        {
            return true;
        }
    }
    // challenge / block page titles (Cloudflare, Akamai, bare server errors)
    const WALL_TITLES: &[&str] = &[
        "just a moment",
        "attention required",
        "access denied",
        "verifying you are human",
        "checking your browser",
        "robot or human",
        "are you a robot",
        "403 forbidden",
        "404 not found",
    ];
    let lower = title.to_lowercase();
    if WALL_TITLES.iter().any(|w| lower.starts_with(w)) {
        return true;
    }
    // challenge machinery in the body
    const WALL_MARKERS: &[&str] =
        &["cf_chl_", "challenges.cloudflare.com", "hcaptcha.com", "google.com/recaptcha"];
    WALL_MARKERS.iter().any(|m| html.contains(m))
}

#[cfg(test)]
mod junk_tests {
    use super::*;

    fn meta(title: &str) -> LinkMeta {
        LinkMeta { title: title.into(), ..Default::default() }
    }

    #[test]
    fn wall_titles_and_markers_are_junk() {
        let base = "https://example.com/article";
        assert!(is_junk_meta("<html></html>", &meta("Just a moment..."), base));
        assert!(is_junk_meta("<html></html>", &meta("Attention Required! | Cloudflare"), base));
        assert!(is_junk_meta("<html></html>", &meta("Access Denied"), base));
        assert!(is_junk_meta("<html></html>", &meta("403 Forbidden"), base));
        assert!(is_junk_meta(
            r#"<script src="https://challenges.cloudflare.com/x.js"></script>"#,
            &meta("Anything"),
            base
        ));
        assert!(is_junk_meta("", &meta("  "), base));
        assert!(is_junk_meta("", &meta("example.com"), base));
        assert!(is_junk_meta("", &meta("www.example.com"), &format!("https://www.example.com")));
    }

    #[test]
    fn real_titles_are_not_junk() {
        let base = "https://example.com/article";
        assert!(!is_junk_meta("<html></html>", &meta("A Real Article Title"), base));
        assert!(!is_junk_meta("<html></html>", &meta("Home"), base)); // low-quality but real
        assert!(!is_junk_meta("<html></html>", &meta("Accessibility at Example"), base));
    }
}
