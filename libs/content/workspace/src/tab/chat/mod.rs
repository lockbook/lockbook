//! The chat tab: a view over a `.chat` document plus a composer. The tab
//! owns the transcript: the driver appends settled lines into it and the
//! workspace saves it like any other document. Lines that reach the
//! document another way (sync, a collaborator, the CLI) are merged in by id.

mod diff;
mod glyphs;
mod model_sheet;
mod rows;
mod setup;
#[cfg(test)]
mod tests;
mod view;

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use egui::{Context, Rect};
use lb_chat::driver::Config;
use lb_chat::{Cmd, Driver, Event, ModelInfo, Place, Provider, SharedStore, VaultTools};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::account::Account;
use lb_rs::model::chat::{self, Body, Chat as Transcript, Entry, Mention, Settings};
use lb_rs::model::file_metadata::DocumentHmac;
use tracing::error;

use crate::file_cache::{FileCache, FilesExt};
use crate::resolvers::image_embed::ImageEmbedResolver;
use crate::resolvers::link::{FileCacheLinkResolver, LinkResolver as _, ResolvedLink};
use crate::style::{Icon, phosphor};
use crate::tab::markdown_editor::{MdEdit, MdLabel};
use crate::widgets::image_cache::ImageCache;
use crate::workspace::WsPersistentStore;

pub use setup::Setup;

use glyphs::Glyphs;
use setup::Offered;

/// What a provider can run, or why that is not known yet.
pub enum ListingState {
    Loading,
    Ready(Vec<ModelInfo>),
    /// The provider's file has no key yet, so nothing was asked of it.
    NeedsKey,
    Failed(String),
}

/// Where pinned models live, as `provider/model` selections.
const FAVORITES_PATH: &str = "/.agent/favorites.json";
/// How long leaving a chat waits for its run to end.
const STOP_WAIT: Duration = Duration::from_millis(250);
/// The row of a thought still arriving, which has no line of its own yet.
const IN_FLIGHT: Uuid = Uuid::nil();

/// What `/.agent` currently says: the selected provider, the providers on
/// offer, and the pinned models.
type ConfigLoad = (Result<Provider, String>, Vec<Offered>, Vec<String>);

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
    /// Each settled message's text, read-only: it selects and copies.
    readers: HashMap<Uuid, MdEdit>,
    expanded: HashSet<Uuid>,
    /// What each opened tool card holds, worked out when it opens.
    bodies: HashMap<Uuid, Vec<rows::Part>>,

    driver: Option<Driver>,
    pub busy: bool,
    streaming: String,
    streaming_label: MdLabel,
    /// Draws the pictures that messages and quoted notes embed.
    images: Option<ImageCache>,
    /// The message a finger last tapped: its actions show, as a pointer
    /// over it shows them.
    tapped: Option<Uuid>,
    /// Where this device keeps the place each chat was scrolled to.
    pub persistence: Option<WsPersistentStore>,
    /// Whether the kept place has been gone back to.
    placed: bool,
    /// The view's offset last frame, until the kept place is gone back to.
    held: Option<f32>,
    /// An offset the view is to take on the next frame.
    place_to: Option<f32>,
    /// The place the view was at last frame; one it rests at is kept.
    rested: Option<(Uuid, f32)>,
    /// Each drawn entry's top and bottom on screen, this frame.
    spans: Vec<(Uuid, f32, f32)>,
    /// What the model has shown of its thinking for the reply in flight.
    thinking: String,
    /// The call the driver is executing right now, drawn as a live row.
    running_tool: Option<lb_chat::Call>,

    composer: MdEdit,
    composer_rect: Rect,
    composer_seq: usize,
    composer_text_seq: usize,
    editing: Option<Uuid>,
    scroll_to_bottom: bool,
    /// Heading for the newest line since this time, or since the wheel last
    /// moved. See `view::WHEEL_REST_SECS`.
    to_latest: Option<f64>,
    /// The folder sheet: open, the draft choice, and which rows are unfolded.
    pub scope_open: bool,
    scope_dest: Option<Uuid>,
    scope_expanded: HashSet<Uuid>,

    /// The provider this chat would send to and the providers on offer,
    /// both read off-thread from `/.agent`.
    provider: Option<Result<Provider, String>>,
    providers: Vec<Offered>,
    provider_rx: Option<Receiver<ConfigLoad>>,
    /// Each provider's `/models` listing, by provider name.
    listings: HashMap<String, ListingState>,
    listing_tx: Sender<(String, ListingState)>,
    listing_rx: Receiver<(String, ListingState)>,
    /// Pinned `provider/model` selections, shown in the model menu.
    favorites: Vec<String>,
    /// The model sheet: open, the draft choice, and the filter text.
    pub models_open: bool,
    model_dest: Option<String>,
    model_filter: String,
    /// Providers folded shut in the model sheet.
    model_folded: HashSet<String>,
    model_reveal: Option<model_sheet::Reveal>,
    glyphs: Glyphs,
    setup: Setup,
    /// The setup form is up by request, over a provider that already works.
    adding_provider: bool,
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
        let (listing_tx, listing_rx) = channel();
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
            readers: HashMap::new(),
            expanded: HashSet::new(),
            bodies: HashMap::new(),
            driver: None,
            busy: false,
            streaming: String::new(),
            streaming_label,
            images: None,
            tapped: None,
            persistence: None,
            placed: false,
            held: None,
            place_to: None,
            rested: None,
            spans: Vec::new(),
            thinking: String::new(),
            running_tool: None,
            composer,
            composer_rect: Rect::NOTHING,
            composer_seq: 0,
            composer_text_seq: 0,
            editing: None,
            scroll_to_bottom: true,
            to_latest: None,
            scope_open: false,
            scope_dest: None,
            scope_expanded: HashSet::new(),
            provider: None,
            providers: Vec::new(),
            provider_rx: None,
            listings: HashMap::new(),
            listing_tx,
            listing_rx,
            favorites: Vec::new(),
            models_open: false,
            model_dest: None,
            model_filter: String::new(),
            model_folded: HashSet::new(),
            model_reveal: None,
            glyphs: Glyphs::default(),
            setup: Setup::default(),
            adding_provider: false,
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
        self.readers.retain(|id, _| ids.contains(id));
    }

    /// The composer is the field in use, so Return sends.
    pub fn composing(&self) -> bool {
        self.is_ready()
    }

    pub fn focused_field(&mut self) -> Option<&mut MdEdit> {
        Some(&mut self.composer)
    }

    pub fn will_consume_touch(&self, pos: egui::Pos2) -> bool {
        !self.composer_rect.contains(pos)
    }

    /// Re-read the provider and the provider list off-thread; called when
    /// `/.agent` changes.
    pub fn kick_config_load(&mut self) {
        // Whatever kept a listing from landing may be what just changed.
        self.listings
            .retain(|_, state| matches!(state, ListingState::Ready(_) | ListingState::Loading));
        let (tx, rx) = channel();
        let lb = self.core.clone();
        let settings = self.settings();
        std::thread::spawn(move || {
            let favorites = lb
                .get_by_path(FAVORITES_PATH)
                .ok()
                .and_then(|f| lb.read_document(f.id, false).ok())
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_default();
            let _ = tx.send((Provider::resolve(&lb, &settings), setup::providers(&lb), favorites));
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

    /// Lists a provider's models off-thread, once per tab.
    fn fetch_listing(&mut self, name: &str) {
        if self.listings.contains_key(name) {
            return;
        }
        self.listings
            .insert(name.to_string(), ListingState::Loading);
        let tx = self.listing_tx.clone();
        let lb = self.core.clone();
        let ctx = self.ctx.clone();
        let name = name.to_string();
        std::thread::spawn(move || {
            let state = match Provider::load(&lb, &name, "") {
                Ok(p) if p.needs_key => ListingState::NeedsKey,
                Ok(p) => match lb_chat::list_models_blocking(&p) {
                    Ok(models) => ListingState::Ready(models),
                    Err(err) => ListingState::Failed(err),
                },
                Err(err) => ListingState::Failed(err),
            };
            let _ = tx.send((name, state));
            ctx.request_repaint();
        });
    }

    /// What a provider is called: what its file says, else its name.
    fn provider_label(&self, name: &str) -> String {
        match self.providers.iter().find(|o| o.name == name) {
            Some(offered) => offered.label(),
            None => lb_chat::friendly_name(name),
        }
    }

    /// What stands for a provider: where its server is when that is a
    /// machine of the user's own, else its brand.
    fn mark(&mut self, ctx: &Context, name: &str, px: f32) -> Icon {
        match self
            .providers
            .iter()
            .find(|o| o.name == name)
            .map(Offered::place)
        {
            Some(Place::ThisDevice) => Icon::Glyph(phosphor::LAPTOP),
            Some(Place::YourNetwork) => Icon::Glyph(phosphor::HARD_DRIVES),
            Some(Place::Internet) | None => Icon::Mark(self.glyphs.get(ctx, name, px)),
        }
    }

    /// The current model's listing entry, once the listing has landed.
    fn listed_model(&self) -> Option<&ModelInfo> {
        let p = self.provider.as_ref()?.as_ref().ok()?;
        match self.listings.get(&p.name)? {
            ListingState::Ready(list) => list.iter().find(|m| m.id == p.model),
            _ => None,
        }
    }

    /// How a `provider/model` selection reads: the listing's name once it
    /// has landed, else the id made readable.
    fn selection_label(&self, selection: &str) -> String {
        let (name, model) = selection.split_once('/').unwrap_or((selection, ""));
        if let Some(ListingState::Ready(list)) = self.listings.get(name) {
            if let Some(m) = list.iter().find(|m| m.id == model) {
                return m.label();
            }
        }
        lb_chat::prettify(model)
    }

    /// The current `provider/model` selection.
    fn selection(&self) -> Option<String> {
        match &self.provider {
            Some(Ok(p)) => Some(p.selection()),
            _ => None,
        }
    }

    /// Pins or unpins a selection and writes the list to the vault.
    fn toggle_favorite(&mut self, selection: &str) {
        match self.favorites.iter().position(|f| f == selection) {
            Some(i) => {
                self.favorites.remove(i);
            }
            None => self.favorites.push(selection.to_string()),
        }
        let bytes = serde_json::to_vec_pretty(&self.favorites).expect("strings serialize");
        if let Err(err) = setup::write(&self.core, FAVORITES_PATH, &bytes) {
            error!("could not save pinned models: {err}");
        }
    }

    pub fn settings(&self) -> Settings {
        self.store
            .chat
            .lock()
            .unwrap()
            .settings_for(&self.account.username)
    }

    /// A provider is resolved and has what it needs to be called.
    fn usable(&self) -> bool {
        matches!(&self.provider, Some(Ok(p)) if !p.needs_key)
    }

    pub fn is_ready(&self) -> bool {
        self.usable() && !self.adding_provider
    }

    /// Brings up the setup form: for `name`, with what its file already
    /// says, or with nothing picked.
    fn begin_connect(&mut self, name: Option<&str>) {
        self.adding_provider = true;
        self.setup = Setup::default();
        if let Some(name) = name {
            if let Ok(provider) = Provider::load(&self.core, name, "") {
                self.prefill_setup(&provider);
            }
        }
    }

    /// Picks `provider`'s template, keeping the model and endpoint its file
    /// names. A file with no template is a server of the user's own, and is
    /// rewritten in place.
    fn prefill_setup(&mut self, provider: &Provider) {
        match setup::TEMPLATES.iter().find(|t| t.name == provider.name) {
            Some(template) => self.setup.pick(template),
            None => {
                self.setup.pick(&setup::OWN);
                self.setup.name = Some(provider.name.clone());
            }
        }
        if !provider.model.is_empty() {
            self.setup.model = provider.model.clone();
        }
        self.setup.base_url = provider.base_url.clone();
    }

    /// Writes the provider the setup form describes and chooses it. A server
    /// of the user's own with no model named is first asked what it offers.
    fn connect(&mut self) {
        if self.setup.ask(&self.ctx) {
            return;
        }
        match self.setup.connect(&self.core) {
            Ok(selection) => {
                self.adding_provider = false;
                self.setup = Setup::default();
                self.select(selection);
            }
            Err(err) => self.setup.error = Some(err),
        }
    }

    /// Chooses `selection` (`provider/model`) for this chat and makes it
    /// what the next new chat starts with. The last pick anywhere is the
    /// default, so there is nothing else to set.
    fn select(&mut self, selection: String) {
        if let Err(err) = setup::write_default(&self.core, &selection, None) {
            error!("could not save the default model: {err}");
        }
        let mut settings = self.settings();
        settings.model = Some(selection);
        // An effort belongs to the model it was chosen for.
        settings.effort = None;
        self.set_settings(settings);
    }

    /// The values this chat's model may be asked to think at.
    fn efforts(&self) -> &'static [&'static str] {
        match &self.provider {
            Some(Ok(provider)) => provider.efforts(),
            _ => &[],
        }
    }

    /// The effort this chat runs at; none is the provider's own default.
    fn effort(&self) -> Option<&str> {
        self.provider.as_ref()?.as_ref().ok()?.effort.as_deref()
    }

    /// Picks an effort for this chat. Like a model, the last one picked
    /// anywhere is what a new chat starts with.
    fn set_effort(&mut self, effort: Option<String>) {
        let Some(Ok(provider)) = &self.provider else { return };
        let written = setup::write_default(&self.core, &provider.selection(), effort.as_deref());
        if let Err(err) = written {
            error!("could not save the default effort: {err}");
        }
        let mut settings = self.settings();
        settings.effort = effort;
        self.set_settings(settings);
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
                window: lb_chat::window,
            };
            let ctx = self.ctx.clone();
            let driver = Driver::spawn(
                self.store.clone(),
                VaultTools::new(lb, self.id),
                config,
                move || ctx.request_repaint(),
            );
            self.driver = Some(driver);
        }
        self.driver.as_ref().expect("spawned")
    }

    fn send_cmd(&mut self, cmd: Cmd) {
        self.driver().send(cmd);
    }

    /// Writes this user's settings into the transcript and re-reads the
    /// provider they select.
    pub fn set_settings(&mut self, settings: Settings) {
        self.store
            .chat
            .lock()
            .unwrap()
            .set_settings(&self.account.username, settings);
        self.store.bump();
        self.kick_config_load();
    }

    /// Ends a live run and waits a moment for what it had said so far to
    /// settle, so the save that follows has it. Closing the tab and
    /// navigating it elsewhere call this.
    pub fn stop(&mut self) {
        let Some(driver) = &self.driver else { return };
        if !driver.busy() {
            return;
        }
        driver.send(Cmd::Stop);
        let asked = Instant::now();
        while driver.busy() && asked.elapsed() < STOP_WAIT {
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn pump(&mut self) {
        if let Some(rx) = &self.provider_rx {
            if let Ok((resolved, providers, favorites)) = rx.try_recv() {
                let name = resolved.as_ref().ok().map(|p| p.name.clone());
                if let Ok(provider) = &resolved {
                    if provider.needs_key && self.setup.picked.is_none() {
                        self.prefill_setup(provider);
                    }
                }
                self.provider = Some(resolved);
                self.providers = providers;
                self.favorites = favorites;
                self.provider_rx = None;
                if let Some(name) = name {
                    self.fetch_listing(&name);
                }
            }
        }
        while let Ok((name, state)) = self.listing_rx.try_recv() {
            self.listings.insert(name, state);
        }
        if let Some(Ok(first)) = self.setup.asking.as_ref().map(|rx| rx.try_recv()) {
            self.setup.asking = None;
            match first {
                Ok(model) => {
                    self.setup.model = model;
                    self.connect();
                }
                Err(err) => self.setup.error = Some(err),
            }
        }
        for event in self
            .driver
            .iter()
            .flat_map(Driver::poll)
            .collect::<Vec<_>>()
        {
            self.hear(event);
        }
        if self.store.seq() != self.view_seq {
            self.refresh_view();
        }
    }

    /// Gives the chat what draws embedded pictures, as a note has.
    pub fn show_pictures(&mut self, images: ImageCache) {
        let embeds = || Box::new(ImageEmbedResolver::new(images.clone(), self.id));
        self.composer.renderer.embeds = embeds();
        self.streaming_label.renderer.embeds = embeds();
        self.images = Some(images);
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    /// The label drawing a note quoted in card `id`. Its links and pictures
    /// resolve from `from`, the note they were written in.
    fn label(&mut self, id: Uuid, from: Uuid) -> &mut MdLabel {
        let (ctx, files, images) = (&self.ctx, &self.files, &self.images);
        self.labels.entry(id).or_insert_with(|| {
            let mut label = MdLabel::new(ctx.clone());
            label.renderer.files = Arc::clone(files);
            label.renderer.link_resolver =
                Box::new(FileCacheLinkResolver::new(Arc::clone(files), from));
            if let Some(images) = images {
                label.renderer.embeds = Box::new(ImageEmbedResolver::new(images.clone(), from));
            }
            label
        })
    }

    fn reader(&mut self, id: Uuid, text: &str) -> &mut MdEdit {
        let (ctx, files, chat_id, images) = (&self.ctx, &self.files, self.id, &self.images);
        let reader = self.readers.entry(id).or_insert_with(|| {
            let mut reader = MdEdit::empty(ctx.clone());
            reader.renderer.readonly = true;
            reader.renderer.files = Arc::clone(files);
            reader.renderer.link_resolver =
                Box::new(FileCacheLinkResolver::new(Arc::clone(files), chat_id));
            if let Some(images) = images {
                reader.renderer.embeds = Box::new(ImageEmbedResolver::new(images.clone(), chat_id));
            }
            reader.file_id = chat_id;
            reader
        });
        if reader.renderer.buffer.current.text != text {
            reader.set_text(text);
        }
        reader
    }

    /// The notes a message links to, which are what it attaches: each
    /// once, in the order linked. Folders and links out of the vault are not.
    fn linked(&self, text: &str) -> Vec<Mention> {
        let resolver = FileCacheLinkResolver::new(Arc::clone(&self.files), self.id);
        let files = self.files.read().unwrap();
        let mut found: Vec<Mention> = Vec::new();
        for (i, _) in text.match_indices("](") {
            let Some(url) = text[i + 2..].split(')').next() else { continue };
            let Some(ResolvedLink::File(id)) = resolver.resolve_link(url.trim()) else { continue };
            let is_note = files.get_by_id(id).is_some_and(|f| f.is_document());
            if is_note && found.iter().all(|m| m.id != Some(id)) {
                found.push(Mention { path: files.path(id), id: Some(id) });
            }
        }
        found
    }

    /// Goes back to the kept place once the rows are laid out, and from
    /// then on keeps the place the view is at. `offset` is how far the view
    /// is scrolled, `top` the view's top on screen. Returns an offset to
    /// scroll to.
    fn keep_place(&mut self, offset: f32, top: f32, at_bottom: bool) -> Option<f32> {
        let store = self.persistence.clone()?;
        let within = |(_, from, to): &(Uuid, f32, f32)| (from - top + offset, to - top + offset);
        if !self.placed {
            // Rows are laid out at the offset the frame began with; they
            // are where `offset` says once it has held for a frame.
            let held = self.held.replace(offset) == Some(offset);
            if self.spans.is_empty() || !held {
                return None;
            }
            self.placed = true;
            let (entry, into) = store.data.read().unwrap().chat.get(&self.id).copied()?;
            let (from, to) = within(self.spans.iter().find(|(id, ..)| *id == entry)?);
            // The kept place stands in for opening at the end.
            (self.scroll_to_bottom, self.to_latest) = (false, None);
            return Some(from + into * (to - from));
        }
        let at = self
            .spans
            .iter()
            .map(|span| (span.0, within(span)))
            .find(|(_, (_, to))| *to > offset)
            .filter(|_| !at_bottom)
            .map(|(id, (from, to))| (id, ((offset - from) / (to - from).max(1.0)).max(0.0)));
        let kept = store.data.read().unwrap().chat.get(&self.id).copied();
        let resting = std::mem::replace(&mut self.rested, at) == at;
        if resting && at != kept {
            let mut data = store.data.write().unwrap();
            match at {
                Some(at) => data.chat.insert(self.id, at),
                None => data.chat.remove(&self.id),
            };
            drop(data);
            store.write_to_file();
        }
        None
    }

    /// Takes in one thing the driver reports.
    fn hear(&mut self, event: Event) {
        match event {
            Event::RunStarted => {
                self.busy = true;
                self.scroll_to_bottom = true;
            }
            Event::Delta(text) => self.streaming.push_str(&text),
            Event::Thinking(text) => self.thinking.push_str(&text),
            Event::ToolStarted(call) => {
                self.streaming.clear();
                self.running_tool = Some(call);
            }
            Event::Written(entry) => {
                if matches!(entry.body, Body::Assistant { .. }) {
                    self.streaming.clear();
                    self.thinking.clear();
                    // A thought opened while it arrived stays open on its line.
                    if self.expanded.remove(&IN_FLIGHT) {
                        self.expanded.insert(entry.id);
                    }
                }
                if matches!(entry.body, Body::Tool { .. }) {
                    self.running_tool = None;
                }
            }
            Event::Lost { error, .. } => error!("chat line lost: {error}"),
            Event::RunEnded => {
                self.busy = false;
                self.streaming.clear();
                self.thinking.clear();
                self.expanded.remove(&IN_FLIGHT);
                self.running_tool = None;
            }
        }
    }

    /// The entries that draw a row. A reply with neither text nor thinking
    /// (a round that was only tool calls) draws nothing, so it takes no gap.
    fn visible_entries(&self) -> Vec<Entry> {
        self.transcript
            .entries
            .iter()
            .filter(|e| match &e.body {
                Body::Other(_) => false,
                Body::Assistant { text, thinking, .. } => !text.is_empty() || !thinking.is_empty(),
                _ => true,
            })
            .cloned()
            .collect()
    }
}
