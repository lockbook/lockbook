//! The chat tab: a view over a `.chat` document plus a composer. The tab
//! owns the transcript: the driver appends settled lines into it and the
//! workspace saves it like any other document. Lines that reach the
//! document another way (sync, a collaborator, the CLI) are merged in by id.

mod rows;
mod setup;
#[cfg(test)]
mod tests;
mod view;

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, RwLock};

use egui::{Context, Rect};
use lb_chat::driver::Config;
use lb_chat::{Cmd, Driver, Event, Provider, SharedStore, VaultTools};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::account::Account;
use lb_rs::model::chat::{self, Body, Chat as Transcript, Entry, Settings};
use lb_rs::model::file_metadata::DocumentHmac;
use tracing::error;

use crate::file_cache::{FileCache, FilesExt};
use crate::resolvers::link::FileCacheLinkResolver;
use crate::tab::markdown_editor::{MdEdit, MdLabel};

pub use setup::Setup;

/// What `/.agent` currently says: the selected provider and the names on offer.
type ConfigLoad = (Result<Provider, String>, Vec<String>);

pub struct Chat {
    pub id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub initialized: bool,

    ctx: Context,
    core: Lb,
    account: Account,
    files: Arc<RwLock<FileCache>>,
    working_dir: String,

    /// The transcript the driver appends into.
    store: SharedStore,
    /// The bytes last read from or written to disk: the merge base when the
    /// document changes underneath this tab.
    base: Vec<u8>,
    /// Store seq already copied into `transcript`.
    view_seq: usize,
    /// Store seq already reported through `take_changed`.
    save_seq: usize,
    /// A copy of the store for drawing, refreshed when the seq moves.
    transcript: Transcript,
    labels: HashMap<Uuid, MdLabel>,
    expanded: HashSet<Uuid>,

    driver: Option<Driver>,
    pub busy: bool,
    streaming: String,
    streaming_label: MdLabel,
    /// The call the driver is executing right now, drawn as a live row.
    running_tool: Option<lb_chat::Call>,
    pending_ask: Option<(lb_chat::Call, String)>,

    composer: MdEdit,
    composer_rect: Rect,
    composer_seq: usize,
    composer_text_seq: usize,
    editing: Option<Uuid>,
    scroll_to_bottom: bool,
    adding_root: bool,
    root_draft: String,

    /// The provider this chat would send to and the providers on offer,
    /// both read off-thread from `/.agent`.
    provider: Option<Result<Provider, String>>,
    providers: Vec<String>,
    provider_rx: Option<Receiver<ConfigLoad>>,
    /// Top-level folders, as (name, path), for the folder picker.
    folders: Vec<(String, String)>,
    setup: Setup,
}

impl Chat {
    pub fn new(
        bytes: &[u8], id: Uuid, hmac: Option<DocumentHmac>, account: Account, ctx: Context,
        files: Arc<RwLock<FileCache>>, core: &Lb,
    ) -> Self {
        let mut composer = MdEdit::empty(ctx.clone());
        composer.renderer.files = Arc::clone(&files);
        composer.renderer.link_resolver =
            Box::new(FileCacheLinkResolver::new(Arc::clone(&files), id));
        composer.file_id = id;

        let mut streaming_label = MdLabel::new(ctx.clone());
        streaming_label.renderer.files = Arc::clone(&files);
        streaming_label.renderer.link_resolver =
            Box::new(FileCacheLinkResolver::new(Arc::clone(&files), id));

        let working_dir = {
            let files = files.read().unwrap();
            let path = files.path(id);
            path[..path.rfind('/').map_or(0, |i| i + 1)].to_string()
        };

        let transcript = Transcript::parse(bytes);
        let mut chat = Self {
            id,
            hmac,
            initialized: false,
            ctx,
            core: core.clone(),
            account,
            files,
            working_dir,
            store: SharedStore::new(transcript.clone()),
            base: bytes.to_vec(),
            view_seq: 0,
            save_seq: 0,
            transcript,
            labels: HashMap::new(),
            expanded: HashSet::new(),
            driver: None,
            busy: false,
            streaming: String::new(),
            streaming_label,
            running_tool: None,
            pending_ask: None,
            composer,
            composer_rect: Rect::NOTHING,
            composer_seq: 0,
            composer_text_seq: 0,
            editing: None,
            scroll_to_bottom: true,
            adding_root: false,
            root_draft: String::new(),
            provider: None,
            providers: Vec::new(),
            provider_rx: None,
            folders: Vec::new(),
            setup: Setup::default(),
        };
        chat.kick_config_load();
        chat
    }

    /// The document changed on disk. Three-way merge it with what this tab
    /// holds; lines held here but not on disk make the tab dirty again.
    pub fn reload(&mut self, bytes: &[u8], hmac: Option<DocumentHmac>) {
        let merged = {
            let mut held = self.store.chat.lock().unwrap();
            let merged = chat::merge(&self.base, &held.serialize(), bytes);
            *held = Transcript::parse(&merged);
            merged
        };
        if merged != bytes {
            self.store.bump();
        }
        self.base = bytes.to_vec();
        self.hmac = hmac;
        self.refresh_view();
        self.kick_config_load();
    }

    pub fn saved(&mut self, hmac: DocumentHmac, content: Vec<u8>) {
        self.hmac = Some(hmac);
        self.base = content;
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.store.chat.lock().unwrap().serialize()
    }

    pub fn seq(&self) -> usize {
        self.store.seq()
    }

    /// Whether the transcript changed since the last call.
    pub fn take_changed(&mut self) -> bool {
        let seq = self.store.seq();
        std::mem::replace(&mut self.save_seq, seq) != seq
    }

    fn refresh_view(&mut self) {
        self.view_seq = self.store.seq();
        self.transcript = self.store.chat.lock().unwrap().clone();
        let ids: HashSet<Uuid> = self.transcript.entries.iter().map(|e| e.id).collect();
        self.labels.retain(|id, _| ids.contains(id));
    }

    pub fn focused_field(&mut self) -> Option<&mut MdEdit> {
        Some(&mut self.composer)
    }

    pub fn will_consume_touch(&self, pos: egui::Pos2) -> bool {
        !self.composer_rect.contains(pos)
    }

    /// Re-read the provider and the provider list off-thread, and the folder
    /// list from the cache; called when `/.agent` or the tree changes.
    pub fn kick_config_load(&mut self) {
        self.folders = {
            let files = self.files.read().unwrap();
            let root = files.root().id;
            let mut folders: Vec<(String, String)> = files
                .children(root)
                .into_iter()
                .filter(|f| f.is_folder() && !f.name.starts_with('.'))
                .map(|f| (f.name.clone(), files.path(f.id)))
                .collect();
            folders.sort_by_key(|f| f.0.to_lowercase());
            folders
        };
        let (tx, rx) = channel();
        let lb = self.core.clone();
        let settings = self.settings();
        std::thread::spawn(move || {
            let _ = tx.send((Provider::resolve(&lb, &settings), setup::providers(&lb)));
        });
        self.provider_rx = Some(rx);
        self.ctx.request_repaint();
    }

    /// The folder the agent works in: the one chosen for this chat, else the
    /// chat's own.
    pub fn scope(&self) -> String {
        self.settings()
            .include
            .first()
            .cloned()
            .unwrap_or_else(|| self.working_dir.clone())
    }

    pub fn settings(&self) -> Settings {
        self.store
            .chat
            .lock()
            .unwrap()
            .settings_for(&self.account.username)
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.provider, Some(Ok(_)))
    }

    pub fn composer_text(&self) -> String {
        self.composer.renderer.buffer.current.text.clone()
    }

    pub fn entry_count(&self) -> usize {
        self.visible_entries().len()
    }

    fn driver(&mut self) -> &Driver {
        if self.driver.is_none() {
            let lb = self.core.clone();
            let resolver_lb = lb.clone();
            let store = self.store.clone();
            let user = self.account.username.clone();
            let config = Config {
                user: user.clone(),
                working_dir: self.working_dir.clone(),
                provider: Box::new(move || {
                    let settings = store.chat.lock().unwrap().settings_for(&user);
                    Provider::resolve(&resolver_lb, &settings)
                }),
            };
            let ctx = self.ctx.clone();
            let driver =
                Driver::spawn(self.store.clone(), VaultTools::new(lb), config, move || {
                    ctx.request_repaint()
                });
            self.driver = Some(driver);
        }
        self.driver.as_ref().expect("spawned")
    }

    fn send_cmd(&mut self, cmd: Cmd) {
        self.driver().send(cmd);
    }

    pub fn set_settings(&mut self, settings: Settings) {
        self.send_cmd(Cmd::SetSettings(settings));
    }

    /// Ends a live run; closing the tab calls this.
    pub fn stop(&mut self) {
        if let Some(driver) = &self.driver {
            driver.send(Cmd::Stop);
        }
    }

    fn pump(&mut self) {
        if let Some(rx) = &self.provider_rx {
            if let Ok((resolved, providers)) = rx.try_recv() {
                self.provider = Some(resolved);
                self.providers = providers;
                self.provider_rx = None;
            }
        }
        if let Some(driver) = &self.driver {
            for event in driver.poll() {
                match event {
                    Event::RunStarted => {
                        self.busy = true;
                        self.scroll_to_bottom = true;
                    }
                    Event::Delta(text) => self.streaming.push_str(&text),
                    Event::ToolStarted(call) => {
                        self.streaming.clear();
                        self.running_tool = Some(call);
                    }
                    Event::Ask { call, prompt } => self.pending_ask = Some((call, prompt)),
                    Event::Written(entry) => {
                        if matches!(entry.body, Body::Assistant { .. }) {
                            self.streaming.clear();
                        }
                        if matches!(entry.body, Body::Tool { .. }) {
                            self.running_tool = None;
                            self.pending_ask = None;
                        }
                    }
                    Event::Lost { error, .. } => error!("chat line lost: {error}"),
                    Event::RunEnded => {
                        self.busy = false;
                        self.streaming.clear();
                        self.running_tool = None;
                        self.pending_ask = None;
                    }
                }
            }
        }
        if self.store.seq() != self.view_seq {
            self.refresh_view();
        }
    }

    fn label(&mut self, id: Uuid) -> &mut MdLabel {
        let (ctx, files, chat_id) = (&self.ctx, &self.files, self.id);
        self.labels.entry(id).or_insert_with(|| {
            let mut label = MdLabel::new(ctx.clone());
            label.renderer.files = Arc::clone(files);
            label.renderer.link_resolver =
                Box::new(FileCacheLinkResolver::new(Arc::clone(files), chat_id));
            label
        })
    }

    fn visible_entries(&self) -> Vec<Entry> {
        self.transcript
            .entries
            .iter()
            .filter(|e| !matches!(e.body, Body::Other(_)))
            .cloned()
            .collect()
    }
}
