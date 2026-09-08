//! Chat tab — one append-only log, painted as a generic transcript.
//!
//! Replay of [`lb_rs::model::chat::Buffer`] is the only constructor of
//! on-screen state. Typed send and (later) voice append the same events.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use egui::{Context, Rect, Ui};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::account::Account;
use lb_rs::model::chat::{Buffer, Content, Event, EventBody, ItemKind, Status, Transcript};
use lb_rs::model::file_metadata::DocumentHmac;

use serde_json::json;
use tracing::{error, info, warn};

use crate::file_cache::FileCache;
use crate::resolvers::image_embed::ImageEmbedResolver;
use crate::tab::markdown_editor::{MdEdit, MdLabel};
use crate::widgets::image_cache::ImageCache;
use crate::workspace::WsPersistentStore;

mod auth;
mod call;
mod grok;
mod media;
mod show;
pub(crate) mod tools;

#[cfg(all(not(target_family = "wasm"), not(target_os = "android")))]
mod audio;
#[cfg(all(not(target_family = "wasm"), not(target_os = "android")))]
mod realtime;

use auth::{Poll, TokenSet};
use grok::{ToolCall, TurnEv};
use lb_rs::model::chat::ItemMeta;

#[derive(Default)]
pub struct TabBridge {
    pub snaps: Vec<tools::TabSnap>,
    pub cmds: Vec<tools::TabOp>,
}

const SYSTEM: &str = "You are Grok in Lockbook. You can search the web (including images) and X, run Python, and search, list, read, inspect, edit, create, rename, move, delete, pin, duplicate, and share the user's notes. `.` is this chat. Names starting with `.` are hidden. You can generate and edit images (imagine), inspect images (look), get or set image captions, transcribe audio, get or set stored audio transcripts, and record text-to-speech into an audio file (does not play on a call). Content search covers notes, chats, image captions, and audio transcripts. Embed a Lockbook image with markdown ![](path). You can open, focus, close, reorder, and go back or forward in workspace tabs. You can check recents, contacts, and account status, change voice and audio devices, and hang up a voice call. Be concise.";
const MAX_ROUNDS: u32 = 8;

enum LoginEv {
    Pending { user_code: String, url: String },
    Ready(TokenSet),
    Err(String),
}

pub(super) enum AuthUi {
    SignedOut,
    Pending { user_code: String, url: String },
    Ready,
    Error(String),
}

pub struct Chat {
    pub id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub seq: usize,
    pub initialized: bool,
    account: Account,
    ctx: Context,
    core: Lb,
    cfg: WsPersistentStore,
    tokens: Option<TokenSet>,
    pub(super) auth_ui: AuthUi,
    log: Buffer,
    transcript: Transcript,
    composer: MdEdit,
    composer_rect: Rect,
    expanded: HashSet<Uuid>,
    login_rx: Option<Receiver<LoginEv>>,
    turn_rx: Option<Receiver<TurnEv>>,
    /// Client tools run off the UI thread; results land here.
    tool_rx: Option<Receiver<Vec<(grok::ToolCall, Result<tools::ClientToolOut, String>)>>>,
    tool_parent_id: Option<Uuid>,
    pending_images: Vec<(String, Vec<u8>)>,
    assistant_id: Option<Uuid>,
    assistant_text: String,
    rounds: u32,
    pub(super) model: String,
    /// One automatic resume per tab open (interrupted in-flight turns).
    resume_attempted: bool,
    labels: HashMap<Uuid, MdLabel>,
    cancel: Arc<AtomicBool>,
    /// Last Responses `id`. Client-tool follow-up sends `previous_response_id`
    /// so server search from that turn stays in context. Realtime won't use this.
    prev_response: Option<String>,
    on_call: bool,
    call_tx: Option<std::sync::mpsc::Sender<call::CallCmd>>,
    call_rx: Option<Receiver<call::CallEv>>,
    tabs: Arc<Mutex<TabBridge>>,
    images: ImageCache,
}

impl Chat {
    pub fn new(
        bytes: &[u8], id: Uuid, hmac: Option<DocumentHmac>, account: Account, ctx: Context,
        files: Arc<RwLock<FileCache>>, core: &Lb, cfg: WsPersistentStore,
        tabs: Arc<Mutex<TabBridge>>, images: ImageCache,
    ) -> Self {
        let log = Buffer::new(bytes);
        let transcript = log.transcript();
        let mut composer = MdEdit::empty(ctx.clone());
        composer.file_id = id;
        composer.renderer.files = files;
        composer.renderer.layout.margin = 0.0;
        let tokens = auth::load_or_migrate(&cfg, &core.get_config().writeable_path);
        let auth_ui = if tokens.is_some() { AuthUi::Ready } else { AuthUi::SignedOut };
        let model = grok::canonical_model(&cfg.grok().model);
        Self {
            id,
            hmac,
            seq: 0,
            initialized: false,
            account,
            ctx,
            core: core.clone(),
            cfg,
            tokens,
            auth_ui,
            log,
            transcript,
            composer,
            composer_rect: Rect::NOTHING,
            expanded: HashSet::new(),
            login_rx: None,
            turn_rx: None,
            tool_rx: None,
            tool_parent_id: None,
            pending_images: Vec::new(),
            assistant_id: None,
            assistant_text: String::new(),
            rounds: 0,
            model,
            resume_attempted: false,
            labels: HashMap::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            prev_response: None,
            on_call: false,
            call_tx: None,
            call_rx: None,
            tabs,
            images,
        }
    }

    pub fn focused_field(&mut self) -> &mut MdEdit {
        &mut self.composer
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.log.serialize()
    }

    pub fn reload(&mut self, bytes: &[u8], hmac: Option<DocumentHmac>) {
        self.log = Buffer::new(bytes);
        self.transcript = self.log.transcript();
        self.hmac = hmac;
        self.seq += 1;
    }

    pub fn saved(&mut self, hmac: DocumentHmac, seq: usize, content: Vec<u8>) {
        self.hmac = Some(hmac);
        // A save cloned earlier in the stream must not wipe tokens that
        // arrived after the snapshot. Keep the live log; the hmac is enough
        // for the next write.
        if seq < self.seq {
            info!(saved_seq = seq, live_seq = self.seq, "chat save hmac only; live log is newer");
            return;
        }
        self.log = Buffer::new(&content);
        self.transcript = self.log.transcript();
    }

    pub fn kick_config_load(&mut self) {}

    pub fn will_consume_touch(&self, pos: egui::Pos2) -> bool {
        self.composer_rect.contains(pos)
    }

    pub(super) fn signed_in(&self) -> bool {
        matches!(self.auth_ui, AuthUi::Ready)
    }

    pub(super) fn busy(&self) -> bool {
        self.assistant_id.is_some() || self.turn_rx.is_some() || self.tool_rx.is_some()
    }

    pub fn on_call(&self) -> bool {
        self.on_call
    }

    pub(super) fn current_voice(&self) -> String {
        let v = self.cfg.grok().voice;
        if v.is_empty() { tools::DEFAULT_VOICE.to_string() } else { v }
    }

    pub(super) fn set_text_model(&mut self, id: &str) {
        self.model = grok::canonical_model(id);
        let mut g = self.cfg.grok();
        if g.model == self.model {
            return;
        }
        g.model = self.model.clone();
        self.cfg.set_grok(g);
    }

    pub(super) fn set_voice(&mut self, id: &str) {
        let id = id.trim();
        if id.is_empty() {
            return;
        }
        let mut g = self.cfg.grok();
        if g.voice != id {
            g.voice = id.to_string();
            self.cfg.set_grok(g);
        }
        if self.on_call {
            if let Some(tx) = &self.call_tx {
                let _ = tx.send(call::CallCmd::SetVoice(id.to_string()));
            }
        }
    }

    pub(super) fn voice_ok(&self) -> bool {
        call::supported() && self.signed_in()
    }

    fn next_ts(&self) -> i64 {
        let now = chrono::Utc::now().timestamp_millis();
        let last = self.log.events.last().map(|e| e.ts).unwrap_or(0);
        now.max(last + 1)
    }

    fn push(&mut self, ev: Event) {
        self.transcript.apply(&ev);
        self.log.events.push(ev);
        self.seq += 1;
    }

    fn say(&mut self, text: String) {
        let text = text.trim_end_matches('\n').to_string();
        if text.trim().is_empty() {
            return;
        }
        info!(chars = text.len(), "chat send");
        if self.on_call {
            if let Some(tx) = &self.call_tx {
                let _ = tx.send(call::CallCmd::Say(text));
            }
            return;
        }
        let item = Uuid::new_v4();
        let parent = self.transcript.items.last().map(|i| i.id);
        let from = self.account.username.clone();
        let mut ts = self.next_ts();
        self.push(Event::new(
            from.clone(),
            ts,
            EventBody::Open { item, parent, kind: ItemKind::User },
        ));
        ts += 1;
        self.push(Event::new(
            from.clone(),
            ts,
            EventBody::Replace { item, blocks: vec![Content::Text { text }] },
        ));
        ts += 1;
        self.push(Event::new(from, ts, EventBody::SetStatus { item, status: Status::Done }));
        self.rounds = 0;
        self.prev_response = None;
        if self.signed_in() && !self.busy() {
            self.start_turn();
        }
    }

    pub(super) fn start_login(&mut self) {
        if self.login_rx.is_some() {
            return;
        }
        info!("chat oauth start");
        let (tx, rx) = mpsc::channel();
        self.login_rx = Some(rx);
        self.auth_ui = AuthUi::Pending { user_code: String::new(), url: String::new() };
        let cfg = self.cfg.clone();
        let ctx = self.ctx.clone();
        thread::spawn(move || {
            let start = match auth::start_device() {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(LoginEv::Err(e));
                    ctx.request_repaint();
                    return;
                }
            };
            let _ = tx.send(LoginEv::Pending {
                user_code: start.user_code.clone(),
                url: start.verification_url.clone(),
            });
            ctx.request_repaint();
            let mut interval = start.interval.max(1);
            loop {
                thread::sleep(Duration::from_secs(interval));
                match auth::poll_device(&start) {
                    Poll::Pending => {}
                    Poll::SlowDown => interval = (interval + 1).min(30),
                    Poll::Tokens(tokens) => {
                        auth::save_prefs(&cfg, &tokens);
                        let _ = tx.send(LoginEv::Ready(tokens));
                        ctx.request_repaint();
                        return;
                    }
                    Poll::Failed(e) => {
                        let _ = tx.send(LoginEv::Err(e));
                        ctx.request_repaint();
                        return;
                    }
                }
            }
        });
    }

    fn fail_stale_in_flight(&mut self) {
        if self.busy() {
            return;
        }
        let stale: Vec<Uuid> = self
            .transcript
            .items
            .iter()
            .filter(|i| i.status.in_flight())
            .map(|i| i.id)
            .collect();
        if stale.is_empty() {
            return;
        }
        info!(n = stale.len(), "chat salvage stale in-flight");
        let from = self.account.username.clone();
        for id in stale {
            self.push(Event::new(
                from.clone(),
                self.next_ts(),
                EventBody::SetStatus { item: id, status: Status::Failed },
            ));
        }
    }

    pub(super) fn needs_turn(&self) -> bool {
        self.transcript.should_create()
    }

    fn recover_if_needed(&mut self) {
        if self.busy() || self.on_call {
            return;
        }
        // Quit mid-stream leaves Running rows. Mark them failed, then retry
        // the unanswered user turn once — not every frame, and not when a
        // Done assistant already persisted. Live calls own in-flight rows;
        // salvaging them every frame was stamping Failed over speaking turns.
        self.fail_stale_in_flight();
        if !self.signed_in() || self.resume_attempted {
            return;
        }
        self.resume_attempted = true;
        if self.transcript.should_create() {
            info!("chat resume interrupted turn");
            self.rounds = 0;
            self.prev_response = None;
            self.start_turn();
        }
    }

    pub(super) fn md_label(&mut self, id: Uuid) -> &mut MdLabel {
        if !self.labels.contains_key(&id) {
            let mut label = MdLabel::new(self.ctx.clone());
            label.renderer.layout.margin = 0.0;
            label.renderer.files = self.composer.renderer.files.clone();
            label.renderer.embeds = Box::new(ImageEmbedResolver::new(self.images.clone(), self.id));
            self.labels.insert(id, label);
        }
        self.labels.get_mut(&id).expect("just inserted")
    }

    pub(super) fn retry(&mut self) {
        if self.busy() || !self.signed_in() {
            return;
        }
        info!("chat retry");
        self.rounds = 0;
        self.prev_response = None;
        self.start_turn();
    }

    pub(super) fn cancel_turn(&mut self) {
        if self.on_call {
            if let Some(tx) = &self.call_tx {
                let _ = tx.send(call::CallCmd::Barge);
            }
            return;
        }
        if !self.busy() {
            return;
        }
        info!("chat turn cancel");
        self.cancel.store(true, Ordering::SeqCst);
        let from = self.account.username.clone();
        let stale: Vec<Uuid> = self
            .transcript
            .items
            .iter()
            .filter(|i| i.status.in_flight())
            .map(|i| i.id)
            .collect();
        for id in stale {
            self.push(Event::new(
                from.clone(),
                self.next_ts(),
                EventBody::SetStatus { item: id, status: Status::Cancelled },
            ));
        }
        self.assistant_id = None;
        self.turn_rx = None;
        self.tool_rx = None;
        self.tool_parent_id = None;
        self.pending_images.clear();
        self.assistant_text.clear();
        self.prev_response = None;
    }

    pub(super) fn toggle_call(&mut self) {
        if self.on_call {
            self.hangup_call();
        } else {
            self.start_call();
        }
    }

    fn start_call(&mut self) {
        if self.on_call || !self.signed_in() || !call::supported() {
            return;
        }
        if self.busy() {
            self.cancel_turn();
        }
        self.fail_stale_in_flight();
        let Some(tokens) = self.tokens.clone() else {
            warn!("chat call skipped: no tokens");
            return;
        };
        info!("chat call start");
        let (tx, rx) = mpsc::channel();
        let grok = self.cfg.grok();
        let cmd = call::start(
            call::CallSpawn {
                transcript: self.transcript.clone(),
                from: self.account.username.clone(),
                tokens,
                cfg: self.cfg.clone(),
                core: self.core.clone(),
                chat_id: self.id,
                ctx: self.ctx.clone(),
                instructions: SYSTEM.to_string(),
                voice: if grok.voice.is_empty() {
                    tools::DEFAULT_VOICE.to_string()
                } else {
                    grok.voice
                },
                input: grok.input,
                output: grok.output,
                tabs: self.tabs.clone(),
            },
            tx,
        );
        self.call_tx = Some(cmd);
        self.call_rx = Some(rx);
        self.on_call = true;
        self.prev_response = None;
    }

    fn hangup_call(&mut self) {
        match self
            .call_tx
            .as_ref()
            .map(|tx| tx.send(call::CallCmd::Hangup))
        {
            Some(Ok(())) => {}
            _ => self.end_call(),
        }
    }

    /// Tear down a live call because the tab is closing (or dropping). The
    /// socket thread exits on a dead command channel even if HungUp never
    /// lands in `pump`.
    pub(crate) fn hangup_for_close(&mut self) {
        if !self.on_call && self.call_tx.is_none() {
            return;
        }
        if let Some(tx) = self.call_tx.take() {
            let _ = tx.send(call::CallCmd::Hangup);
        }
        self.call_rx = None;
        self.on_call = false;
        info!("chat call closed with tab");
    }

    fn end_call(&mut self) {
        self.on_call = false;
        self.call_tx = None;
        self.call_rx = None;
        info!("chat call ended");
        if self.transcript.should_create()
            && self
                .transcript
                .items
                .iter()
                .rev()
                .any(|i| i.kind == ItemKind::Tool && i.status.terminal())
        {
            // Pick up a tool follow-up in typed chat after hangup.
            self.rounds = 0;
            if self.signed_in() && !self.busy() {
                self.start_turn();
            }
        }
    }

    fn start_turn(&mut self) {
        if self.busy() || !self.signed_in() || self.on_call {
            return;
        }
        if self.rounds >= MAX_ROUNDS {
            warn!(rounds = self.rounds, "chat turn stopped: too many tool rounds");
            self.push_error(
                self.account.username.clone(),
                "stopped after too many tool rounds".into(),
            );
            return;
        }
        self.rounds += 1;
        let mut tokens = match self.tokens.clone() {
            Some(t) => t,
            None => {
                warn!("chat turn skipped: no tokens");
                return;
            }
        };
        let cfg = self.cfg.clone();
        let ctx = self.ctx.clone();
        let model = self.model.clone();
        let follow = self.rounds > 1 && self.prev_response.is_some();
        let mut input = if follow {
            grok::tool_outputs_from_transcript(&self.transcript)
        } else {
            grok::input_from_transcript(&self.transcript)
        };
        for (mime, bytes) in self.pending_images.drain(..) {
            input.push(json!({
                "role": "user",
                "content": [
                    {
                        "type": "input_image",
                        "image_url": format!("data:{mime};base64,{}", media::b64(&bytes)),
                        "detail": "high"
                    }
                ]
            }));
        }
        let instructions = if follow { None } else { Some(SYSTEM) };
        let prev_id = if follow { self.prev_response.clone() } else { None };
        info!(round = self.rounds, n_input = input.len(), follow, "chat turn start");

        let item = Uuid::new_v4();
        let parent = self.transcript.items.last().map(|i| i.id);
        let from = self.account.username.clone();
        let mut ts = self.next_ts();
        self.push(Event::new(
            from.clone(),
            ts,
            EventBody::Open { item, parent, kind: ItemKind::Assistant },
        ));
        ts += 1;
        self.push(Event::new(from, ts, EventBody::SetStatus { item, status: Status::Running }));
        self.assistant_id = Some(item);
        self.assistant_text.clear();
        self.cancel.store(false, Ordering::SeqCst);
        let cancel = self.cancel.clone();

        let (tx, rx) = mpsc::channel();
        self.turn_rx = Some(rx);
        thread::spawn(move || {
            let expires = tokens.expires_at;
            let bearer = match auth::resolve_bearer(&mut tokens) {
                Ok(b) => b,
                Err(e) => {
                    error!(error = %e, "chat bearer resolve failed");
                    let _ = tx.send(TurnEv::Err(e));
                    ctx.request_repaint();
                    return;
                }
            };
            if tokens.expires_at != expires {
                auth::save_prefs(&cfg, &tokens);
            }
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(TurnEv::Err(e.to_string()));
                    ctx.request_repaint();
                    return;
                }
            };
            rt.block_on(grok::stream(
                &bearer,
                &model,
                input,
                instructions,
                prev_id.as_deref(),
                &cancel,
                |ev| {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    let _ = tx.send(ev);
                    ctx.request_repaint();
                },
            ));
        });
    }

    pub(crate) fn pump(&mut self) {
        if let Some(rx) = self.login_rx.take() {
            let mut keep = true;
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    LoginEv::Pending { user_code, url } => {
                        self.ctx
                            .open_url(egui::OpenUrl { url: url.clone(), new_tab: true });
                        self.auth_ui = AuthUi::Pending { user_code, url };
                    }
                    LoginEv::Ready(tokens) => {
                        info!("chat oauth ready");
                        self.tokens = Some(tokens);
                        self.auth_ui = AuthUi::Ready;
                        self.initialized = false;
                        keep = false;
                    }
                    LoginEv::Err(e) => {
                        error!(error = %e, "chat oauth failed");
                        self.auth_ui = AuthUi::Error(e);
                        keep = false;
                    }
                }
            }
            if keep {
                self.login_rx = Some(rx);
            }
        }

        if let Some(rx) = self.call_rx.take() {
            let mut keep = true;
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    call::CallEv::Log(ev) => self.push(ev),
                    call::CallEv::Listening { mic, spk } => {
                        info!(mic, spk, "chat call listening");
                    }
                    call::CallEv::HungUp => {
                        keep = false;
                        self.end_call();
                    }
                    call::CallEv::Err(e) => {
                        error!(error = %e, "chat call error");
                        self.push_error(self.account.username.clone(), e);
                        keep = false;
                        self.end_call();
                    }
                }
            }
            if keep {
                self.call_rx = Some(rx);
            }
        }

        if let Some(rx) = self.turn_rx.take() {
            let mut keep = true;
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    TurnEv::Text(t) => {
                        if self.cancel.load(Ordering::SeqCst) {
                            continue;
                        }
                        self.assistant_text.push_str(&t);
                        if let Some(id) = self.assistant_id {
                            let from = self.account.username.clone();
                            self.push(Event::new(
                                from,
                                self.next_ts(),
                                EventBody::Replace {
                                    item: id,
                                    blocks: vec![Content::Text {
                                        text: self.assistant_text.clone(),
                                    }],
                                },
                            ));
                        }
                    }
                    TurnEv::Usage(usage) => {
                        if let Some(id) = self.assistant_id {
                            let from = self.account.username.clone();
                            self.push(Event::new(
                                from,
                                self.next_ts(),
                                EventBody::Usage { item: id, usage },
                            ));
                        }
                    }
                    TurnEv::ResponseId(id) => {
                        self.prev_response = Some(id);
                    }
                    TurnEv::ServerTool { id, name, args } => {
                        if self.cancel.load(Ordering::SeqCst) {
                            continue;
                        }
                        if let Some(parent) = self.tool_parent() {
                            let body = tools::persist_body(&name, &args);
                            self.record_tool(parent, &ToolCall { id, name, args }, Ok(body));
                        } else {
                            warn!(name, "chat server tool with no assistant parent");
                        }
                    }
                    TurnEv::ToolCalls(calls) => {
                        if self.cancel.load(Ordering::SeqCst) {
                            keep = false;
                            continue;
                        }
                        info!(n = calls.len(), "chat tool calls");
                        let parent = self.tool_parent();
                        self.finish_assistant(Status::Done);
                        self.assistant_text.clear();
                        keep = false;
                        if let Some(parent) = parent {
                            self.spawn_tools(parent, calls);
                        } else if self.signed_in() {
                            self.start_turn();
                        }
                    }
                    TurnEv::Done => {
                        if self.cancel.load(Ordering::SeqCst) {
                            keep = false;
                            continue;
                        }
                        info!(chars = self.assistant_text.len(), "chat turn done");
                        self.finish_assistant(Status::Done);
                        self.assistant_text.clear();
                        keep = false;
                    }
                    TurnEv::Err(e) => {
                        if self.cancel.load(Ordering::SeqCst) {
                            keep = false;
                            continue;
                        }
                        error!(error = %e, "chat turn failed");
                        if e.contains("401") || e.contains("403") || e.contains("entitled") {
                            auth::clear_prefs(&self.cfg);
                            self.tokens = None;
                            self.auth_ui = AuthUi::Error(e.clone());
                        }
                        if let Some(id) = self.assistant_id.take() {
                            let from = self.account.username.clone();
                            self.push(Event::new(
                                from.clone(),
                                self.next_ts(),
                                EventBody::SetStatus { item: id, status: Status::Failed },
                            ));
                            self.push_error(from, e);
                        } else {
                            self.push_error(self.account.username.clone(), e);
                        }
                        self.assistant_text.clear();
                        keep = false;
                    }
                }
            }
            if keep {
                self.turn_rx = Some(rx);
            }
        }

        if let Some(rx) = self.tool_rx.take() {
            match rx.try_recv() {
                Ok(results) => {
                    if !self.cancel.load(Ordering::SeqCst) {
                        if let Some(parent) = self.tool_parent_id.take() {
                            for (call, result) in results {
                                let result = match result {
                                    Ok(out) => {
                                        if let Some(img) = out.image {
                                            self.pending_images.push(img);
                                        }
                                        Ok(out.text)
                                    }
                                    Err(e) => Err(e),
                                };
                                self.record_tool(parent, &call, result);
                            }
                        }
                        if self.signed_in() {
                            self.start_turn();
                        }
                    }
                    self.tool_parent_id = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.tool_rx = Some(rx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.tool_parent_id = None;
                    warn!("chat tool worker disconnected");
                }
            }
        }
    }

    fn spawn_tools(&mut self, parent: Uuid, calls: Vec<grok::ToolCall>) {
        let core = self.core.clone();
        let chat_id = self.id;
        let cfg = self.cfg.clone();
        let tabs = self.tabs.clone();
        let cancel = self.cancel.clone();
        let ctx = self.ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.tool_rx = Some(rx);
        self.tool_parent_id = Some(parent);
        thread::spawn(move || {
            let mut out = Vec::with_capacity(calls.len());
            for c in calls {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                let result =
                    dispatch_client_tool(&core, chat_id, &c.name, &c.args, &tabs, &cfg, false);
                out.push((c, result));
            }
            let _ = tx.send(out);
            ctx.request_repaint();
        });
    }

    fn finish_assistant(&mut self, status: Status) {
        if let Some(id) = self.assistant_id.take() {
            let from = self.account.username.clone();
            self.push(Event::new(from, self.next_ts(), EventBody::SetStatus { item: id, status }));
        }
    }

    /// In-flight assistant, else the last one opened. Server tools can land
    /// before assistant text (voice); foundry parents them the same way.
    fn tool_parent(&self) -> Option<Uuid> {
        self.assistant_id
            .or_else(|| self.transcript.on_turn_assistant())
            .or_else(|| self.transcript.last_assistant())
    }

    fn record_tool(&mut self, parent: Uuid, call: &ToolCall, result: Result<String, String>) {
        let item = Uuid::new_v4();
        let from = self.account.username.clone();
        let mut ts = self.next_ts();
        self.push(Event::new(
            from.clone(),
            ts,
            EventBody::Open { item, parent: Some(parent), kind: ItemKind::Tool },
        ));
        ts += 1;
        self.push(Event::new(
            from.clone(),
            ts,
            EventBody::SetMeta {
                item,
                meta: ItemMeta {
                    title: Some(tools::summary(&call.name, &call.args)),
                    tool_kind: Some(call.name.clone()),
                    wire_id: Some(call.id.clone()),
                    args: Some(call.args.clone()),
                },
            },
        ));
        let (text, status) = match result {
            Ok(t) => (t, Status::Done),
            Err(e) => (e, Status::Failed),
        };
        ts += 1;
        self.push(Event::new(
            from.clone(),
            ts,
            EventBody::Replace { item, blocks: vec![Content::Text { text }] },
        ));
        ts += 1;
        self.push(Event::new(from, ts, EventBody::SetStatus { item, status }));
    }

    fn push_error(&mut self, from: String, msg: String) {
        error!(error = %msg, "chat error row");
        let item = Uuid::new_v4();
        let parent = self.transcript.items.last().map(|i| i.id);
        let mut ts = self.next_ts();
        self.push(Event::new(
            from.clone(),
            ts,
            EventBody::Open { item, parent, kind: ItemKind::Error },
        ));
        ts += 1;
        self.push(Event::new(
            from.clone(),
            ts,
            EventBody::Replace { item, blocks: vec![Content::Text { text: msg }] },
        ));
        ts += 1;
        self.push(Event::new(from, ts, EventBody::SetStatus { item, status: Status::Failed }));
    }

    pub fn show(&mut self, ui: &mut Ui) -> (bool, Rect, bool, bool) {
        self.pump();
        self.recover_if_needed();
        show::show(self, ui)
    }
}

impl Drop for Chat {
    fn drop(&mut self) {
        self.hangup_for_close();
    }
}

pub(super) fn dispatch_client_tool(
    core: &Lb, chat_id: Uuid, name: &str, args: &serde_json::Value, tabs: &Arc<Mutex<TabBridge>>,
    cfg: &WsPersistentStore, on_call: bool,
) -> Result<tools::ClientToolOut, String> {
    if name == "tabs" {
        let snaps = tabs.lock().unwrap().snaps.clone();
        let (msg, ops) = tools::tabs(core, chat_id, args, &snaps)?;
        if !ops.is_empty() {
            tabs.lock().unwrap().cmds.extend(ops);
        }
        return Ok(tools::ClientToolOut::text(msg));
    }
    if name == "settings" {
        let grok = cfg.grok();
        let mut state = tools::SettingsState {
            voice: if grok.voice.is_empty() {
                tools::DEFAULT_VOICE.to_string()
            } else {
                grok.voice
            },
            input: grok.input,
            output: grok.output,
        };
        let msg = tools::settings(args, &mut state)?;
        let mut grok = cfg.grok();
        grok.voice = state.voice;
        grok.input = state.input;
        grok.output = state.output;
        cfg.set_grok(grok);
        return Ok(tools::ClientToolOut::text(msg));
    }
    if name == "hangup" {
        return Ok(tools::ClientToolOut::text(if on_call {
            "hanging up"
        } else {
            "not on a call"
        }));
    }
    match name {
        "imagine" => media::imagine(core, chat_id, args, cfg),
        "look" => media::look(core, chat_id, args, cfg, on_call),
        "transcribe" => media::transcribe(core, chat_id, args, cfg),
        "record" => media::record(core, chat_id, args, cfg),
        _ => tools::run(core, chat_id, name, args).map(tools::ClientToolOut::text),
    }
}
