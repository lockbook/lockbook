use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::{Arc, Mutex, RwLock};

use egui::Context;
use lb_rs::Uuid;
use lb_rs::spawn;

use crate::file_cache::{FileCache, FilesExt as _};
use crate::tab::markdown_editor::HttpClient;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Normal,
    Warning,
    Broken,
}

pub enum ResolvedLink {
    File(Uuid),
    External(String),
}

#[derive(Clone)]
pub enum LinkPreview {
    Loading,
    Ready(LinkPreviewData),
    Unavailable,
}

#[derive(Clone, Default)]
pub struct LinkPreviewData {
    pub title: Option<String>,
}

pub trait LinkResolver: Send + Clone {
    fn link_state(&self, url: &str) -> LinkState;
    fn wikilink_state(&self, url: &str) -> LinkState;
    fn resolve_link(&self, url: &str) -> Option<ResolvedLink>;
    fn resolve_wikilink(&self, url: &str) -> Option<Uuid>;
    fn link_preview(&self, url: &str) -> LinkPreview;
}

impl LinkResolver for () {
    fn link_state(&self, _url: &str) -> LinkState {
        LinkState::Normal
    }

    fn wikilink_state(&self, _url: &str) -> LinkState {
        LinkState::Normal
    }

    fn resolve_link(&self, _url: &str) -> Option<ResolvedLink> {
        None
    }

    fn resolve_wikilink(&self, _url: &str) -> Option<Uuid> {
        None
    }

    fn link_preview(&self, _url: &str) -> LinkPreview {
        LinkPreview::Unavailable
    }
}

#[derive(Clone)]
pub struct FileCacheLinkResolver {
    files: Arc<RwLock<FileCache>>,
    file_id: Uuid,
    client: HttpClient,
    ctx: Context,
    title_cache: Arc<Mutex<HashMap<String, Arc<Mutex<TitleCacheEntry>>>>>,
}

#[derive(Clone)]
enum TitleCacheEntry {
    Loading,
    Loaded(String),
    Failed,
}

impl FileCacheLinkResolver {
    pub fn new(
        files: Arc<RwLock<FileCache>>, file_id: Uuid, client: HttpClient, ctx: Context,
    ) -> Self {
        Self { files, file_id, client, ctx, title_cache: Default::default() }
    }
}

impl LinkResolver for FileCacheLinkResolver {
    fn link_state(&self, url: &str) -> LinkState {
        let guard = self.files.read().unwrap();
        let Some(from_id) = guard.get_by_id(self.file_id).map(|f| f.parent) else {
            return LinkState::Broken;
        };
        match guard.resolve_link(url, from_id) {
            None => LinkState::Broken,
            Some(crate::file_cache::ResolvedLink::External(_)) => LinkState::Normal,
            Some(crate::file_cache::ResolvedLink::File(target_id)) => {
                if guard.link_has_access_gap(self.file_id, target_id) {
                    LinkState::Warning
                } else {
                    LinkState::Normal
                }
            }
        }
    }

    fn wikilink_state(&self, url: &str) -> LinkState {
        let guard = self.files.read().unwrap();
        let Some(from_id) = guard.get_by_id(self.file_id).map(|f| f.parent) else {
            return LinkState::Broken;
        };
        match guard.resolve_wikilink(url, from_id) {
            None => LinkState::Broken,
            Some(target_id) => {
                if guard.link_has_access_gap(self.file_id, target_id) {
                    LinkState::Warning
                } else {
                    LinkState::Normal
                }
            }
        }
    }

    fn resolve_link(&self, url: &str) -> Option<ResolvedLink> {
        let guard = self.files.read().unwrap();
        let from_id = guard.get_by_id(self.file_id)?.parent;
        guard.resolve_link(url, from_id).map(|r| match r {
            crate::file_cache::ResolvedLink::File(id) => ResolvedLink::File(id),
            crate::file_cache::ResolvedLink::External(url) => ResolvedLink::External(url),
        })
    }

    fn resolve_wikilink(&self, url: &str) -> Option<Uuid> {
        let guard = self.files.read().unwrap();
        let from_id = guard.get_by_id(self.file_id)?.parent;
        guard.resolve_wikilink(url, from_id)
    }

    fn link_preview(&self, url: &str) -> LinkPreview {
        let resolved_url = match url {
            u if u.starts_with("http://") || u.starts_with("https://") => u.to_string(),
            _ => return LinkPreview::Unavailable,
        };

        let mut cache = self.title_cache.lock().unwrap();
        let entry = match cache.entry(resolved_url.clone()) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let arc = Arc::new(Mutex::new(TitleCacheEntry::Loading));
                e.insert(arc.clone());
                let client = self.client.clone();
                let ctx = self.ctx.clone();
                let title_state = arc.clone();
                spawn!({
                    const CHROME: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
                    const GOOGLEBOT: &str =
                        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";

                    #[cfg(not(target_arch = "wasm32"))]
                    let mut html = fetch_html(&client, &resolved_url, CHROME);
                    #[cfg(target_arch = "wasm32")]
                    let mut html = fetch_html(&client, &resolved_url, CHROME).await;

                    if html.as_deref().ok().and_then(extract_html_title).is_none() {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            html = fetch_html(&client, &resolved_url, GOOGLEBOT);
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            html = fetch_html(&client, &resolved_url, GOOGLEBOT).await;
                        }
                    }

                    let title = html
                        .as_deref()
                        .ok()
                        .and_then(extract_html_title)
                        .map(|t| t.to_string());

                    match title {
                        Some(title) => {
                            *title_state.lock().unwrap() = TitleCacheEntry::Loaded(title);
                        }
                        None => {
                            *title_state.lock().unwrap() = TitleCacheEntry::Failed;
                        }
                    }

                    ctx.request_repaint();
                });
                arc
            }
        };
        drop(cache);

        let result = match &*entry.lock().unwrap() {
            TitleCacheEntry::Loading => LinkPreview::Loading,
            TitleCacheEntry::Loaded(t) => {
                LinkPreview::Ready(LinkPreviewData { title: Some(t.clone()) })
            }
            TitleCacheEntry::Failed => LinkPreview::Unavailable,
        };
        result
    }
}

fn extract_html_title(html: &str) -> Option<&str> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")? + 6;
    let after_tag = lower[start..].find('>')? + start + 1;
    let end = lower[after_tag..].find("</title>")? + after_tag;
    let title = html[after_tag..end].trim();
    if title.is_empty() { None } else { Some(title) }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_html(client: &HttpClient, url: &str, user_agent: &str) -> Result<String, String> {
    client
        .get(url)
        .header("User-Agent", user_agent)
        .send()
        .and_then(|r| r.text())
        .map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
async fn fetch_html(client: &HttpClient, url: &str, user_agent: &str) -> Result<String, String> {
    client
        .get(url)
        .header("User-Agent", user_agent)
        .send()
        .await
        .and_then(|r| r.text())
        .await
        .map_err(|e| e.to_string())
}
