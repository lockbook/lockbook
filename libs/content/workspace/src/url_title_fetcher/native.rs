//! Blocking HTTP fetch for URL titles. Kept out of the parent module so wasm builds never reference
//! `reqwest::blocking` (that API is not wired up for wasm in our dependency graph).

use std::thread;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::Client;
use url::Url;

use super::{channel, extract_title_from_html, format_markdown_link};

/// Crawler-style user agents: many sites (YouTube, X) serve minimal HTML to generic browsers but
/// include `og:title` / richer markup for link-preview bots. [`crate::mind_map::data`] uses a
/// desktop Chrome UA; we rotate bot UAs here because paste auto-title cares about those meta tags.
const CRAWLER_USER_AGENTS: &[&str] = &[
    "facebookexternalhit/1.1 (+http://www.facebook.com/externalhit_uatext.php)",
    "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
];

fn fetch_page_title_blocking(url: &Url) -> Option<String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
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

        if res.status() != StatusCode::OK {
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
/// completes, `(from_text, markdown_link)` is sent to the channel for the main thread to process.
pub fn spawn_fetch_and_replace(from_text: String, url: Url) {
    let tx = channel().0.clone();
    let url_str = url.to_string();
    thread::spawn(move || {
        let title = fetch_page_title_blocking(&url);
        let markdown_link = format_markdown_link(title.as_deref().unwrap_or(&url_str), &url_str);
        let _ = tx.send((from_text, markdown_link));
    });
}
