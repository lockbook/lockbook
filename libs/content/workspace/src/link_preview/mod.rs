//! Link previews: what a web page says about itself, fetched at most once per
//! run of the app and shared by every tab. Nothing is written to disk.

mod page;
mod providers;

pub use page::{LinkMeta, extract_link_meta, is_junk_meta};
pub use providers::label;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::Context;
use lb_rs::spawn;
use web_time::Instant;

use crate::egress::{FetchError, fetch_html};
use crate::tab::markdown_editor::HttpClient;
use crate::workspace::WsPersistentStore;

/// A preview this old is fetched again the next time it's shown; the old one
/// stays up until the new one lands.
const STALE_AFTER: Duration = Duration::from_secs(10 * 60);
/// Self-assigned retry-after for network-level failures; doubles per
/// consecutive failure up to the cap.
const TRANSIENT_RETRY_BASE: Duration = Duration::from_secs(60);
const TRANSIENT_RETRY_CAP: Duration = Duration::from_secs(15 * 60);
/// Bot walls outlive short retries — a site's posture changes slowly.
const BLOCKED_RETRY: Duration = Duration::from_secs(6 * 60 * 60);

enum State {
    Loading,
    Loaded(LinkMeta),
    /// `retry_at: Some(t)` — eligible for a refetch once `t` passes (the
    /// host's `Retry-After`, a transient-failure backoff, or a bot-wall
    /// cooldown). `None` — deterministic failure, no retry this run.
    /// `attempts` counts consecutive failures and drives the backoff.
    Failed {
        retry_at: Option<Instant>,
        attempts: u32,
    },
}

struct Entry {
    state: State,
    /// When `state` was last set or a refresh of it began.
    at: Instant,
}

/// What a link's preview is right now.
pub enum Preview {
    Loaded(LinkMeta),
    /// In flight — a card holds a skeleton until it resolves.
    Loading,
    /// None, and none coming for now (failed, fetching off) — show the link.
    Unavailable,
}

/// The previews every tab shows, made once by the workspace. A clone is a
/// handle to the same cache.
#[derive(Clone)]
pub struct LinkPreviews {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
    seq: Arc<AtomicU64>,
    ws_seq: Arc<AtomicU64>,
    ctx: Context,
    client: HttpClient,
    /// Where the reader's choice to contact linked sites is kept; with none,
    /// sites are never contacted.
    persistence: Option<WsPersistentStore>,
}

impl LinkPreviews {
    pub fn new(ctx: Context, client: HttpClient, persistence: WsPersistentStore) -> Self {
        Self { client, persistence: Some(persistence), ..Self::offline(ctx) }
    }

    /// A cache that never contacts a site, for markdown shown outside a note.
    pub fn offline(ctx: Context) -> Self {
        Self {
            entries: Default::default(),
            seq: Default::default(),
            ws_seq: crate::seq::ws_seq(&ctx),
            ctx,
            client: Default::default(),
            persistence: None,
        }
    }

    /// Whether the reader lets linked sites be contacted.
    fn contact(&self) -> bool {
        self.persistence
            .as_ref()
            .is_some_and(|p| p.get_contact_linked_sites())
    }

    /// Changes whenever a link may read differently: a fetch landed, or the
    /// files links resolve against changed.
    pub fn seq(&self) -> u64 {
        self.seq.load(Ordering::Relaxed)
    }

    pub fn bump(&self) {
        self.seq
            .store(self.ws_seq.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
    }

    /// `url`'s preview, starting a fetch when there is none yet, an earlier
    /// failure's retry time has passed, or the one shown has gone stale —
    /// and only when the reader lets sites be contacted.
    pub fn get(&self, url: &str) -> Preview {
        let contact = self.contact();
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        let fetch = match entries.get_mut(url) {
            None if contact => {
                entries.insert(url.into(), Entry { state: State::Loading, at: now });
                Some(0)
            }
            None => return Preview::Unavailable,
            Some(entry) if contact => match entry.state {
                State::Failed { retry_at: Some(t), attempts } if t <= now => {
                    *entry = Entry { state: State::Loading, at: now };
                    Some(attempts)
                }
                State::Loaded(_) if now - entry.at >= STALE_AFTER => {
                    entry.at = now;
                    Some(0)
                }
                _ => None,
            },
            Some(_) => None,
        };
        let preview = match &entries[url].state {
            State::Loaded(meta) => Preview::Loaded(meta.clone()),
            State::Loading => Preview::Loading,
            State::Failed { retry_at: Some(until), .. } if *until > now => {
                // Wake a frame when the cooldown lapses — an idle editor never
                // repaints, so without this the retry waits for user input.
                self.ctx.request_repaint_after(*until - now);
                Preview::Unavailable
            }
            State::Failed { .. } => Preview::Unavailable,
        };
        drop(entries);
        if let Some(attempts) = fetch {
            self.fetch(url.into(), attempts);
        }
        preview
    }

    /// `url`'s preview if one has loaded; never fetches.
    pub fn loaded(&self, url: &str) -> Option<LinkMeta> {
        match &self.entries.lock().unwrap().get(url)?.state {
            State::Loaded(meta) => Some(meta.clone()),
            _ => None,
        }
    }

    /// Drops `url`'s preview and fetches it fresh — "Refresh Preview".
    pub fn refresh(&self, url: &str) {
        if !self.contact() {
            return;
        }
        let entry = Entry { state: State::Loading, at: Instant::now() };
        self.entries.lock().unwrap().insert(url.into(), entry);
        self.fetch(url.into(), 0);
    }

    /// A preview known without fetching, as tests and films seed them.
    pub fn insert(&self, url: &str, meta: LinkMeta) {
        self.set(url, State::Loaded(meta));
    }

    /// A failed fetch, retried after `retry_at`.
    #[cfg(test)]
    pub fn fail(&self, url: &str, retry_at: Instant) {
        self.set(url, State::Failed { retry_at: Some(retry_at), attempts: 1 });
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.lock().unwrap().is_empty()
    }

    #[cfg(test)]
    pub fn is_loading(&self, url: &str) -> bool {
        matches!(self.entries.lock().unwrap()[url].state, State::Loading)
    }

    fn set(&self, url: &str, state: State) {
        let entry = Entry { state, at: Instant::now() };
        self.entries.lock().unwrap().insert(url.into(), entry);
        self.bump();
    }

    /// Fetches and scrapes `url` off-thread, then bumps [`Self::seq`] and
    /// repaints. `attempts` counts prior consecutive failures.
    fn fetch(&self, url: String, attempts: u32) {
        let this = self.clone();
        let client = self.client.clone();
        let ctx = self.ctx.clone();
        spawn!({
            #[cfg(not(target_arch = "wasm32"))]
            let html = fetch_html(&client, &url);
            #[cfg(target_arch = "wasm32")]
            let html = fetch_html(&client, &url).await;

            let now = Instant::now();
            let failed = |retry_at| State::Failed { retry_at, attempts: attempts + 1 };
            let state = match html {
                Ok(h) => match extract_link_meta(&h, &url) {
                    Some(meta) if !is_junk_meta(&h, &meta, &url) => State::Loaded(meta),
                    // junk describes the wall, not the page
                    Some(_) => failed(Some(now + BLOCKED_RETRY)),
                    None => failed(None),
                },
                Err(FetchError::RateLimited { until }) => failed(Some(until)),
                Err(FetchError::Blocked(_)) => failed(Some(now + BLOCKED_RETRY)),
                Err(FetchError::Transient(_)) => {
                    let backoff =
                        (TRANSIENT_RETRY_BASE * 2u32.pow(attempts.min(4))).min(TRANSIENT_RETRY_CAP);
                    failed(Some(now + backoff))
                }
                Err(FetchError::Permanent(_)) => failed(None),
            };
            let mut entries = this.entries.lock().unwrap();
            let entry = entries
                .entry(url)
                .or_insert(Entry { state: State::Loading, at: now });
            // a stale preview beats none when its refresh fails
            if matches!(state, State::Loaded(_)) || !matches!(entry.state, State::Loaded(_)) {
                *entry = Entry { state, at: now };
            }
            drop(entries);
            this.bump();
            ctx.request_repaint();
        });
    }
}
