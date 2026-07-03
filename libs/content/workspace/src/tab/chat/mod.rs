//! Chat tab — markdown-rendered messages in a scrollable transcript, with a
//! multiline composer at the bottom. The document on disk is newline-delimited
//! JSON, merged across devices via `lb_rs::model::chat::Buffer::merge`
//! (symmetric union over timestamp).
//!
//! An unshared chat is a conversation with the user's own agent: replies
//! stream live onto the canvas and are appended to the transcript (as the
//! user, flagged `agent`) when the turn ends. The agent runs on the device
//! that composed the invoking message; shared chats get no agent.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Local, Utc};
use egui::{
    CornerRadius, Galley, Id, Key, Modifiers, Pos2, Rect, ScrollArea, Sense, Stroke, StrokeKind,
    Ui, pos2, vec2,
};
use lb_rs::Uuid;
use lb_rs::model::account::Account;
use lb_rs::model::chat::{Buffer, Message};
use lb_rs::model::file_metadata::DocumentHmac;

use crate::GlyphonRendererCallback;
use crate::file_cache::FileCache;
use crate::resolvers::FileCacheLinkResolver;
use crate::tab::markdown_editor::{MdEdit, MdLabel};
use crate::theme::icons::Icon;
use crate::theme::palette_v2::{Palette, ThemeExt, username_color};

#[cfg(not(target_family = "wasm"))]
mod backend;
#[cfg(not(target_family = "wasm"))]
mod harness;
#[cfg(not(target_family = "wasm"))]
mod openai;
#[cfg(not(target_family = "wasm"))]
mod settings;

const MAX_WIDTH: f32 = 800.0;
const H_PAD: f32 = 12.0;
const V_PAD: f32 = 10.0;
const H_MARGIN: f32 = 12.0;
const ROW_GAP: f32 = 4.0;
const CORNER: u8 = 10;
/// Bigger on Android to clear the status bar / safe area.
const TOP_MARGIN: f32 = if cfg!(target_os = "android") { 60.0 } else { 15.0 };
const BOTTOM_PAD: f32 = 15.0;
const COMPOSER_MAX_HEIGHT: f32 = 160.0;
const COMPOSER_BOTTOM_GAP: f32 = 16.0;
/// Android nav-bar clearance, used while the keyboard is down.
const COMPOSER_NAV_CLEARANCE: f32 = 60.0;
/// Horizontal inset on each side of the composer bubble within its column. The
/// send button lives in the right inset, outside the bubble.
const SIDE_INSET: f32 = 48.0;
/// Font size of note rows (errors, agent status).
const NOTE_FONT: f32 = 12.0;
/// Gap under a run's name header.
const NAME_GAP: f32 = 6.0;
/// The metadata strip under every message row: timestamp, branch arrows,
/// and (on hover) action icons — in reserved space below the message, never
/// overlaid on it.
const STRIP_H: f32 = 18.0;
/// The toolbar row inside the composer bubble (Zed-style): provider/model
/// dropdowns on the left, send/stop on the right.
const TOOLBAR_H: f32 = 32.0;
/// Horizontal gap between strip items.
const STRIP_GAP: f32 = 10.0;
/// Where click-to-set-up creates the first provider file. One suggested
/// provider is enough for the empty state — the folder is the real
/// interface, and any other file placed there works the same.
const SETUP_PATH: &str = "/.agent/providers/cerebras.json";

/// Templates for the "add provider" menu: UI affordances that pre-fill an
/// ordinary provider file. The file is the source of truth — the template
/// name only decides what gets written; the filename stays a plain label,
/// and `display_name` carries casing a filename can't ("OpenRouter").
///
/// Groups render separator-divided, unlabeled: model makers, then hosts
/// serving others' models, then local + the blank escape hatch. The brands
/// carry the distinction; anything not listed is one `custom` file away.
#[cfg(not(target_family = "wasm"))]
const TEMPLATES: &[&[(&str, &str)]] = &[
    &[
        (
            "openai",
            r#"{
  "display_name": "OpenAI",
  "base_url": "https://api.openai.com/v1",
  "model": "gpt-5.5",
  "api_key": ""
}
"#,
        ),
        (
            "anthropic",
            r#"{
  "display_name": "Anthropic",
  "base_url": "https://api.anthropic.com/v1",
  "model": "claude-opus-4-8",
  "api_key": ""
}
"#,
        ),
        (
            "google",
            r#"{
  "display_name": "Google",
  "base_url": "https://generativelanguage.googleapis.com/v1beta/openai",
  "model": "gemini-3.5-flash",
  "api_key": ""
}
"#,
        ),
        (
            "xai",
            r#"{
  "display_name": "xAI",
  "base_url": "https://api.x.ai/v1",
  "model": "grok-4.3",
  "api_key": ""
}
"#,
        ),
    ],
    &[
        (
            "openrouter",
            r#"{
  "display_name": "OpenRouter",
  "base_url": "https://openrouter.ai/api/v1",
  "model": "openrouter/auto",
  "api_key": ""
}
"#,
        ),
        (
            "groq",
            r#"{
  "display_name": "Groq",
  "base_url": "https://api.groq.com/openai/v1",
  "model": "openai/gpt-oss-120b",
  "api_key": ""
}
"#,
        ),
        (
            "cerebras",
            r#"{
  "display_name": "Cerebras",
  "base_url": "https://api.cerebras.ai/v1",
  "model": "gemma-4-31b",
  "api_key": ""
}
"#,
        ),
        (
            "together",
            r#"{
  "display_name": "Together",
  "base_url": "https://api.together.xyz/v1",
  "model": "deepseek-ai/DeepSeek-V4-Pro",
  "api_key": ""
}
"#,
        ),
    ],
    &[
        (
            "ollama",
            r#"{
  "display_name": "Local (Ollama)",
  "base_url": "http://localhost:11434/v1",
  "model": "gemma3"
}
"#,
        ),
        (
            "custom",
            r#"{
  "base_url": "https://api.example.com/v1",
  "model": "model-id",
  "api_key": ""
}
"#,
        ),
    ],
];

/// What the add-provider menu shows for a template: the JSON's
/// `display_name`, through the same fallback a hand-made file gets.
#[cfg(not(target_family = "wasm"))]
fn template_label(name: &str, json: &str) -> String {
    settings::parse(name, json.as_bytes())
        .map(|p| p.label())
        .unwrap_or_else(|| name.to_string())
}

/// A message paired with the label that renders its markdown body. Labels
/// are per-message so each one's `LayoutCache` can actually memoize across
/// frames — rotating one shared label through every message would invalidate
/// the cache on every rotation.
pub struct Entry {
    pub msg: Message,
    pub label: MdLabel,
}

/// Per-row layout plan produced by the first pass. All rects absolute; pass 2
/// paints with no egui layout involvement.
///
/// Bubbles are people; the canvas is the agent. Human messages get a
/// surface-fill bubble (the user's own right-aligned, everyone else's left);
/// agent replies are naked markdown at near-full column width — they're the
/// transcript's long-form reading content, and the asymmetry keeps a future
/// many-humans-plus-agent chat legible.
enum RowPlan {
    Bubble {
        bubble_rect: Rect,
        name_galley: Option<Arc<Galley>>,
        name_h: f32,
        content_h: f32,
    },
    /// An agent reply: unbubbled markdown on the transcript background.
    Agent {
        pos: Pos2,
        name_galley: Option<Arc<Galley>>,
        name_h: f32,
        content_h: f32,
    },
    /// A plain unbubbled line: harness errors (dim red) and agent status.
    /// Shaped like an agent row — headed, monospace body — so errors read
    /// as part of the conversation rather than floating chrome.
    Note {
        header: Option<Arc<Galley>>,
        galley: Arc<Galley>,
        pos: Pos2,
    },
}

/// The metadata strip under a row, laid out in pass 1 and drawn after the
/// rows: timestamp (run tail), ‹ 2/3 › arrows (forks), hover action icons.
struct StripPlan {
    vi: usize,
    rect: Rect,
    /// The message row above, for hover detection and the context menu.
    row_rect: Rect,
    /// Lay items from the right edge (own messages) or the left.
    right: bool,
    ts: Option<Arc<Galley>>,
}

impl Entry {
    fn new(
        msg: Message, ctx: &egui::Context, files: Arc<RwLock<FileCache>>, chat_id: Uuid,
    ) -> Self {
        let mut label = MdLabel::new(ctx.clone());
        label.renderer.files = Arc::clone(&files);
        label.renderer.link_resolver = Box::new(FileCacheLinkResolver::new(files, chat_id));
        Self { msg, label }
    }

    /// Carries no rendered row: a config entry or an otherwise empty message.
    fn hidden(&self) -> bool {
        self.msg.config.is_some() || self.msg.content.is_empty()
    }
}

pub struct Chat {
    pub id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub entries: Vec<Entry>,
    pub composer: MdEdit,
    pub account: Account,
    pub seq: usize,
    pub initialized: bool,
    /// Last server state our local edits sit on top of — the merge base. Held
    /// so `reload` does a real 3-way merge instead of an empty-base union.
    base: Vec<u8>,
    /// Composer region from the last frame, for `will_consume_touch`.
    composer_rect: Rect,
    /// Sending while scrolled up jumps back to the bottom (which also
    /// re-sticks the scroll). Set on submit, consumed next frame.
    scroll_to_bottom: bool,
    /// Chosen child per fork (parent id → child id), for the ‹ 2/3 › arrows.
    /// View state: per-device, unsynced, defaults to the newest sibling.
    branch_choice: HashMap<Uuid, Uuid>,
    /// After a branch switch: hold the clicked strip (by visible index) at
    /// its previous on-screen y, and suspend stick-to-bottom for the frame.
    branch_anchor: Option<(usize, f32)>,
    /// While regenerating: hide the superseded reply (children of this
    /// parent) so the transcript ends at the invoking message during the
    /// turn. Cleared when the new reply (or error) lands.
    hide_children_of: Option<Uuid>,
    ctx: egui::Context,

    /// Agent driver for 1-1 agent chats. `None` when the chat is shared.
    #[cfg(not(target_family = "wasm"))]
    harness: Option<harness::Harness>,
    /// Renders the in-flight streaming reply.
    #[cfg(not(target_family = "wasm"))]
    streaming_label: MdLabel,
    /// Unshared at open — eligible for an agent (setup guidance is shown if
    /// none could be built).
    #[cfg(not(target_family = "wasm"))]
    unshared: bool,
    #[cfg(not(target_family = "wasm"))]
    core: lb_rs::blocking::Lb,
    /// Cached provider, for the setup hint only — turns resolve their
    /// provider fresh at send time, so this is display state, not config.
    /// Refreshed on tab re-activation (a frame after a gap) and after
    /// click-to-set-up.
    #[cfg(not(target_family = "wasm"))]
    provider: Option<settings::Provider>,
    #[cfg(not(target_family = "wasm"))]
    last_frame: Option<std::time::Instant>,
    /// Message being edited via composer-prefill (its id). Sending appends a
    /// sibling — the original and its subtree stay reachable via arrows.
    #[cfg(not(target_family = "wasm"))]
    editing: Option<Uuid>,
    /// The composer draft stashed when editing began, restored on cancel or
    /// after the edit is sent.
    #[cfg(not(target_family = "wasm"))]
    draft_stash: Option<String>,
    /// Parent for the reply of the turn in flight — the message it answers.
    /// Stamped as `reply_to` on the pushed reply/error row.
    #[cfg(not(target_family = "wasm"))]
    pending_parent: Option<Uuid>,
    /// Live `/models` listing for the picker, keyed by (provider name,
    /// base_url) so editing the file's url invalidates it. Runtime data,
    /// never persisted. Only successes cache; failures set `models_err`
    /// and retry on a cooldown, so pasting a key in doesn't strand the
    /// picker on an old failure.
    #[cfg(not(target_family = "wasm"))]
    models: Option<(ModelsKey, ModelListing)>,
    #[cfg(not(target_family = "wasm"))]
    models_rx: Option<std::sync::mpsc::Receiver<(ModelsKey, Result<ModelListing, String>)>>,
    #[cfg(not(target_family = "wasm"))]
    models_attempt: Option<(ModelsKey, std::time::Instant)>,
    /// Keyed like the cache so provider A's failure never displays under
    /// provider B's picker.
    #[cfg(not(target_family = "wasm"))]
    models_err: Option<(ModelsKey, String)>,
}

#[cfg(not(target_family = "wasm"))]
type ModelListing = Vec<backend::ModelInfo>;

/// (provider name, base_url) — what a cached listing is valid for.
#[cfg(not(target_family = "wasm"))]
type ModelsKey = (String, String);

/// How long a failed `/models` fetch waits before retrying.
#[cfg(not(target_family = "wasm"))]
const MODELS_RETRY: std::time::Duration = std::time::Duration::from_secs(15);

/// This user's latest per-chat model selection (config entries are ts-sorted
/// with the rest of the log; last wins).
#[cfg(not(target_family = "wasm"))]
fn latest_selection(
    entries: &[Entry], username: &str,
) -> Option<lb_rs::model::chat::ModelSelection> {
    entries
        .iter()
        .filter(|e| e.msg.from == username)
        .filter_map(|e| e.msg.config.as_ref())
        .filter_map(|c| c.model.clone())
        .next_back()
}

/// The model this user last ran `provider` with, mined from the append-only
/// config history — what makes switching back to a provider land on the
/// model you left it on, not the file default.
#[cfg(not(target_family = "wasm"))]
fn last_model_for(entries: &[Entry], username: &str, provider: &str) -> Option<String> {
    entries
        .iter()
        .filter(|e| e.msg.from == username)
        .filter_map(|e| e.msg.config.as_ref())
        .filter_map(|c| c.model.as_ref())
        .filter(|sel| sel.provider == provider && !sel.model.trim().is_empty())
        .map(|sel| sel.model.clone())
        .next_back()
}

impl Chat {
    pub fn new(
        bytes: &[u8], id: Uuid, hmac: Option<DocumentHmac>, account: Account, ctx: egui::Context,
        files: Arc<RwLock<FileCache>>, core: &lb_rs::blocking::Lb,
    ) -> Self {
        let entries: Vec<Entry> = Buffer::new(bytes)
            .messages
            .into_iter()
            .map(|m| Entry::new(m, &ctx, Arc::clone(&files), id))
            .collect();
        let mut composer = MdEdit::empty(ctx.clone());
        composer.renderer.files = Arc::clone(&files);
        composer.renderer.link_resolver =
            Box::new(FileCacheLinkResolver::new(Arc::clone(&files), id));
        composer.file_id = id;

        #[cfg(target_family = "wasm")]
        let _ = core;
        #[cfg(not(target_family = "wasm"))]
        let unshared = !is_shared(&files.read().unwrap(), id);
        #[cfg(not(target_family = "wasm"))]
        let harness = unshared.then(|| {
            let visible = resolve_visible(&entries, &HashMap::new(), None);
            harness::Harness::new(ctx.clone(), seed_history(&entries, &visible))
        });
        #[cfg(not(target_family = "wasm"))]
        let provider = if unshared {
            let selection = latest_selection(&entries, &account.username);
            settings::resolve(settings::load(core), selection.as_ref())
        } else {
            None
        };

        Self {
            id,
            hmac,
            entries,
            composer,
            account,
            seq: 0,
            initialized: false,
            base: bytes.to_vec(),
            composer_rect: Rect::NOTHING,
            scroll_to_bottom: false,
            branch_choice: HashMap::new(),
            branch_anchor: None,
            hide_children_of: None,
            #[cfg(not(target_family = "wasm"))]
            harness,
            #[cfg(not(target_family = "wasm"))]
            streaming_label: {
                let mut label = MdLabel::new(ctx.clone());
                label.renderer.files = Arc::clone(&files);
                label.renderer.link_resolver = Box::new(FileCacheLinkResolver::new(files, id));
                label
            },
            #[cfg(not(target_family = "wasm"))]
            unshared,
            #[cfg(not(target_family = "wasm"))]
            core: core.clone(),
            #[cfg(not(target_family = "wasm"))]
            provider,
            #[cfg(not(target_family = "wasm"))]
            last_frame: None,
            #[cfg(not(target_family = "wasm"))]
            editing: None,
            #[cfg(not(target_family = "wasm"))]
            draft_stash: None,
            #[cfg(not(target_family = "wasm"))]
            pending_parent: None,
            #[cfg(not(target_family = "wasm"))]
            models: None,
            #[cfg(not(target_family = "wasm"))]
            models_rx: None,
            #[cfg(not(target_family = "wasm"))]
            models_attempt: None,
            #[cfg(not(target_family = "wasm"))]
            models_err: None,
            ctx,
        }
    }

    /// The visible timeline under the current branch choices.
    fn visible(&self) -> Vec<VisibleRow> {
        resolve_visible(&self.entries, &self.branch_choice, self.hide_children_of)
    }

    /// Model context for the visible timeline.
    #[cfg(not(target_family = "wasm"))]
    fn visible_seed(&self) -> Vec<backend::ChatMsg> {
        seed_history(&self.entries, &self.visible())
    }

    /// The provider the next turn runs with: this user's per-chat selection
    /// (latest config entry) resolved against the provider files.
    #[cfg(not(target_family = "wasm"))]
    fn resolve_provider(&self) -> Option<settings::Provider> {
        let selection = latest_selection(&self.entries, &self.account.username);
        settings::resolve(settings::load(&self.core), selection.as_ref())
    }

    /// Record a provider/model pick as a config entry in the transcript —
    /// selection syncs across this user's devices; credentials never do. An
    /// empty `model` means the provider's file default.
    #[cfg(not(target_family = "wasm"))]
    fn write_selection(&mut self, provider: String, model: String) {
        let config = lb_rs::model::chat::ChatConfig {
            model: Some(lb_rs::model::chat::ModelSelection { provider, model }),
        };
        let msg =
            Message::config_entry(self.account.username.clone(), Utc::now().timestamp(), config);
        self.entries.push(Entry::new(
            msg,
            &self.ctx,
            Arc::clone(&self.composer.renderer.files),
            self.id,
        ));
        self.seq += 1;
        self.provider = self.resolve_provider();
    }

    /// Kick a background `/models` fetch for the picker, unless a matching
    /// success is cached, a fetch is in flight, or a recent failure is
    /// still cooling down.
    #[cfg(not(target_family = "wasm"))]
    fn fetch_models(&mut self, provider: &settings::Provider) {
        let key: ModelsKey = (provider.name.clone(), provider.base_url.clone());
        let cached = self.models.as_ref().is_some_and(|(k, _)| *k == key);
        let cooling = self
            .models_attempt
            .as_ref()
            .is_some_and(|(k, at)| *k == key && at.elapsed() < MODELS_RETRY);
        if cached || cooling || self.models_rx.is_some() {
            return;
        }
        self.models_attempt = Some((key.clone(), std::time::Instant::now()));
        let (tx, rx) = std::sync::mpsc::channel();
        self.models_rx = Some(rx);
        let ctx = self.ctx.clone();
        let provider = provider.clone();
        std::thread::spawn(move || {
            let result = openai::list_models_blocking(&provider);
            let _ = tx.send((key, result));
            ctx.request_repaint();
        });
    }

    #[cfg(not(target_family = "wasm"))]
    fn pump_models(&mut self) {
        if let Some(rx) = &self.models_rx {
            if let Ok((key, result)) = rx.try_recv() {
                self.models_rx = None;
                match result {
                    Ok(list) => {
                        self.models = Some((key, list));
                        self.models_err = None;
                    }
                    Err(e) => self.models_err = Some((key, e)),
                }
            }
        }
    }

    /// Refresh the cached provider on tab re-activation: a frame after a
    /// gap means the user was away (editing the provider file, say). Turns
    /// don't depend on this — they resolve config at send — it only keeps
    /// the setup hint honest.
    #[cfg(not(target_family = "wasm"))]
    fn refresh_provider_on_return(&mut self) {
        let now = std::time::Instant::now();
        let away = self
            .last_frame
            .is_none_or(|t| now - t > std::time::Duration::from_millis(300));
        self.last_frame = Some(now);
        if away && self.unshared {
            self.provider = self.resolve_provider();
        }
    }

    /// Create (or reuse) a provider file from a template, select it for this
    /// chat, and open it in a tab — setup is "click, paste your key". An
    /// existing file is opened untouched, never clobbered.
    #[cfg(not(target_family = "wasm"))]
    fn create_provider_file(&mut self, name: &str) {
        use crate::tab::ExtendedOutput as _;
        let Some((_, template)) = TEMPLATES
            .iter()
            .flat_map(|group| group.iter())
            .find(|(n, _)| *n == name)
        else {
            return;
        };
        let path = format!("/.agent/providers/{name}.json");
        let file = match self.core.get_by_path(&path) {
            Ok(file) => file,
            Err(_) => match self.core.create_at_path(&path) {
                Ok(file) => file,
                Err(e) => {
                    tracing::warn!("chat: couldn't create {path}: {e:?}");
                    return;
                }
            },
        };
        // Fill the template only when there's nothing to clobber.
        if self
            .core
            .read_document(file.id, false)
            .is_ok_and(|b| b.is_empty())
        {
            let _ = self.core.write_document(file.id, template.as_bytes());
        }
        self.write_selection(name.to_string(), String::new());
        self.ctx.open_file(file.id, true);
    }

    /// Create (or reuse) the instructions file and open it in a tab. Only a
    /// freshly created file gets the preamble template (so customizing
    /// starts from what's actually sent) — an existing file opens untouched
    /// even when empty, because an empty instructions file is a meaningful
    /// state: no system prompt. The date sentence stays out of the template
    /// — it would freeze today into the file.
    #[cfg(not(target_family = "wasm"))]
    fn create_instructions_file(&mut self) {
        use crate::tab::ExtendedOutput as _;
        let path = "/.agent/instructions.md";
        let file = match self.core.get_by_path(path) {
            Ok(file) => file,
            Err(_) => match self.core.create_at_path(path) {
                Ok(file) => {
                    let template = format!("{}\n", harness::PREAMBLE);
                    let _ = self.core.write_document(file.id, template.as_bytes());
                    file
                }
                Err(e) => {
                    tracing::warn!("chat: couldn't create {path}: {e:?}");
                    return;
                }
            },
        };
        self.ctx.open_file(file.id, true);
    }

    /// True for transcript touches (which scroll), so Android doesn't treat a
    /// short transcript scroll as a keyboard-summoning tap. Composer touches
    /// fall through so they still raise the keyboard.
    pub fn will_consume_touch(&self, pos: egui::Pos2) -> bool {
        !self.composer_rect.contains(pos)
    }

    /// Begin editing a message: stash the in-progress draft, prefill the
    /// composer with the target's content. Send commits it as a sibling;
    /// Esc cancels and restores the draft.
    #[cfg(not(target_family = "wasm"))]
    fn enter_edit(&mut self, idx: usize, ui: &Ui, composer_id: Id) {
        let Some(id) = self.entries[idx].msg.id else { return };
        let draft = self.composer.renderer.buffer.current.text.clone();
        if !draft.trim().is_empty() && self.editing.is_none() {
            self.draft_stash = Some(draft);
        }
        let content = self.entries[idx].msg.content.clone();
        self.composer.set_text(&content);
        self.editing = Some(id);
        ui.memory_mut(|m| m.request_focus(composer_id));
    }

    /// Re-run a user message as-is: append a sibling with the same content
    /// and run its turn. The old subtree stays behind the arrows.
    #[cfg(not(target_family = "wasm"))]
    fn resend_as_sibling(&mut self, idx: usize) {
        let Some(parent) = parent_for_sibling(&self.entries, idx) else { return };
        let content = self.entries[idx].msg.content.clone();
        let mut msg =
            Message::new(self.account.username.clone(), content.clone(), Utc::now().timestamp());
        msg.reply_to = Some(parent);
        let msg_id = msg.id;
        if let Some(id) = msg_id {
            self.branch_choice.insert(parent, id);
        }
        self.entries.push(Entry::new(
            msg,
            &self.ctx,
            Arc::clone(&self.composer.renderer.files),
            self.id,
        ));
        self.seq += 1;
        self.pending_parent = msg_id;
        self.provider = self.resolve_provider();
        let system = settings::instructions(&self.core);
        let mut seed = self.visible_seed();
        seed.pop(); // `say` pushes the new message itself
        if let (Some(harness), Some(provider)) = (&mut self.harness, self.provider.clone()) {
            harness.reseed(seed);
            harness.say(content, provider, system);
        }
        self.scroll_to_bottom = true;
    }

    /// Re-roll an agent reply: hide it for the duration of the turn and run
    /// a retry against the timeline ending at its invoking message. The new
    /// reply lands as the old one's sibling.
    #[cfg(not(target_family = "wasm"))]
    fn regenerate(&mut self, idx: usize) {
        let Some(parent) = parent_for_sibling(&self.entries, idx) else { return };
        self.hide_children_of = Some(parent);
        self.pending_parent = Some(parent);
        self.provider = self.resolve_provider();
        let system = settings::instructions(&self.core);
        let seed = self.visible_seed();
        if let (Some(harness), Some(provider)) = (&mut self.harness, self.provider.clone()) {
            harness.reseed(seed);
            harness.retry(provider, system);
        }
        self.scroll_to_bottom = true;
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        Buffer { messages: self.entries.iter().map(|e| e.msg.clone()).collect() }.serialize()
    }

    /// Merge in a freshly-synced version. `bytes` is the remote; the local
    /// state is `self`; `self.base` is the last server state our local edits
    /// sit on top of — a real 3-way merge, matching the sync engine.
    ///
    /// Unchanged entries keep their existing label (and therefore its layout
    /// cache); new entries get a fresh label.
    pub fn reload(&mut self, bytes: &[u8], hmac: Option<DocumentHmac>) {
        let local = self.to_bytes();
        let merged = Buffer::merge(&self.base, &local, bytes);
        let merged_msgs = Buffer::new(&merged).messages;
        self.base = bytes.to_vec();

        let mut old: Vec<Entry> = std::mem::take(&mut self.entries);
        self.entries = merged_msgs
            .into_iter()
            .map(|msg| match old.iter().position(|e| e.msg == msg) {
                Some(idx) => old.swap_remove(idx),
                None => {
                    Entry::new(msg, &self.ctx, Arc::clone(&self.composer.renderer.files), self.id)
                }
            })
            .collect();

        self.hmac = hmac;
        self.seq += 1;

        // The merge may have added rows the driver never saw (another
        // device's messages); refresh its context while idle. Mid-turn the
        // reseed contract forbids it — the send-time reseed in `submit`
        // repairs the divergence at the next turn instead.
        #[cfg(not(target_family = "wasm"))]
        {
            let seed = self.visible_seed();
            if let Some(harness) = self.harness.as_mut().filter(|h| !h.busy) {
                harness.reseed(seed);
            }
        }
    }

    /// Mark saved after a successful write — called from the workspace save
    /// completion path. Advances the hmac (the next save's compare-and-swap
    /// base) and the merge base (the server now holds exactly these bytes);
    /// without this every subsequent save fails ReReadRequired and pays a
    /// reload + merge round-trip.
    pub fn saved(&mut self, hmac: DocumentHmac, content: Vec<u8>) {
        self.hmac = Some(hmac);
        self.base = content;
    }

    /// Append an agent-authored row: a reply, or (with `error`) a dim red
    /// note excluded from agent context. Attached to the message the turn
    /// answered, and the branch choice follows it — a regenerated reply
    /// supersedes its sibling on screen without deleting it.
    #[cfg(not(target_family = "wasm"))]
    fn push_agent_message(
        &mut self, content: String, usage: Option<lb_rs::model::chat::Usage>, error: bool,
    ) {
        let mut msg = Message::new(self.account.username.clone(), content, Utc::now().timestamp());
        msg.agent = true;
        msg.usage = usage;
        msg.error = error;
        msg.reply_to = self.pending_parent;
        if let (Some(parent), Some(id)) = (self.pending_parent, msg.id) {
            self.branch_choice.insert(parent, id);
        }
        self.hide_children_of = None;
        self.entries.push(Entry::new(
            msg,
            &self.ctx,
            Arc::clone(&self.composer.renderer.files),
            self.id,
        ));
        self.seq += 1;
    }

    /// Fold agent events into the transcript. Returns whether anything was
    /// appended (and so needs saving).
    #[cfg(not(target_family = "wasm"))]
    fn pump_agent(&mut self) -> bool {
        let updates = self.harness.as_mut().map(|h| h.pump()).unwrap_or_default();
        let changed = !updates.is_empty();
        for update in updates {
            match update {
                harness::HarnessUpdate::Reply { text, usage } => {
                    self.push_agent_message(text, usage, false)
                }
                harness::HarnessUpdate::Error(e) => self.push_agent_message(e, None, true),
            }
        }
        // A regenerate stopped before its first token ends the turn without
        // any agent row landing (empty replies are dropped above the push),
        // so the push never clears the hide — unhide on any idle, or the
        // superseded reply and every later message stay invisible.
        if self.hide_children_of.is_some() && self.harness.as_ref().is_some_and(|h| !h.busy) {
            self.hide_children_of = None;
        }
        changed
    }

    /// Append the trimmed composer text as a message and clear the composer.
    /// While editing, the message lands as a *sibling* of the edited one
    /// (the old timeline stays behind the arrows) and the stashed draft is
    /// restored. Returns whether a (non-empty) message was sent.
    fn submit(&mut self, ui: &Ui, composer_id: Id) -> bool {
        let content = self
            .composer
            .renderer
            .buffer
            .current
            .text
            .trim()
            .to_string();
        if content.is_empty() {
            return false;
        }

        #[cfg(not(target_family = "wasm"))]
        let editing = self.editing.take();
        #[cfg(target_family = "wasm")]
        let editing: Option<Uuid> = None;
        let reply_to = match editing {
            Some(target) => {
                let idx = self.entries.iter().position(|e| e.msg.id == Some(target));
                match idx.and_then(|i| parent_for_sibling(&self.entries, i)) {
                    Some(parent) => Some(parent),
                    // Target vanished (sync?) — fall through to a plain send.
                    None => self.tail_reply_to(),
                }
            }
            None => self.tail_reply_to(),
        };

        let mut msg =
            Message::new(self.account.username.clone(), content.clone(), Utc::now().timestamp());
        msg.reply_to = reply_to;
        let msg_id = msg.id;
        if let (Some(parent), Some(id)) = (reply_to, msg_id) {
            self.branch_choice.insert(parent, id);
        }
        self.entries.push(Entry::new(
            msg,
            &self.ctx,
            Arc::clone(&self.composer.renderer.files),
            self.id,
        ));
        // Config is resolved here, at send — never watched. A provider-file
        // edit simply applies to the next turn.
        #[cfg(not(target_family = "wasm"))]
        {
            self.pending_parent = msg_id;
            self.provider = self.resolve_provider();
            // Every send reseeds from the visible timeline — the per-frame
            // truth — so the driver's private history can't diverge from
            // it. The divergence paths are real: messages typed before a
            // provider existed never reached the driver, and sync merges
            // add entries the driver never saw.
            let mut seed = self.visible_seed();
            seed.pop(); // `say` pushes the new message itself
            let system = settings::instructions(&self.core);
            if let (Some(harness), Some(provider)) = (&mut self.harness, self.provider.clone()) {
                harness.reseed(seed);
                harness.say(content, provider, system);
            }
        }
        self.composer.clear();
        #[cfg(not(target_family = "wasm"))]
        if let Some(stash) = self.draft_stash.take() {
            self.composer.set_text(&stash);
        }
        self.seq += 1;
        self.scroll_to_bottom = true;
        ui.memory_mut(|m| m.request_focus(composer_id));
        true
    }

    /// `reply_to` for a message appended to the end of the visible timeline:
    /// the tail's id, nil for the first message, or `None` when the tail is
    /// a legacy id-less row (implicit chaining still lands it right).
    fn tail_reply_to(&self) -> Option<Uuid> {
        match self.visible().last() {
            None => Some(Uuid::nil()),
            Some(row) => self.entries[row.idx].msg.id,
        }
    }

    /// Renders the transcript + composer. Returns whether the transcript
    /// changed this frame (user sent or the agent replied — triggers a save),
    /// and the composer's text rect (egui points) used to position the native
    /// iOS text-interaction overlay.
    pub fn show(&mut self, ui: &mut Ui) -> (bool, Rect) {
        #[cfg(not(target_family = "wasm"))]
        self.refresh_provider_on_return();
        #[cfg(not(target_family = "wasm"))]
        self.pump_models();
        #[cfg(not(target_family = "wasm"))]
        let agent_changed = self.pump_agent();
        #[cfg(target_family = "wasm")]
        let agent_changed = false;

        // Live agent state for this frame's rendering.
        #[cfg(not(target_family = "wasm"))]
        let (agent_busy, agent_streaming) = match &self.harness {
            Some(h) => (h.busy, h.streaming.clone()),
            None => (false, String::new()),
        };
        #[cfg(target_family = "wasm")]
        let (agent_busy, agent_streaming) = (false, String::new());
        #[cfg(not(target_family = "wasm"))]
        let show_agent_hint = self.unshared && self.provider.is_none();
        #[cfg(target_family = "wasm")]
        let show_agent_hint = false;
        // The one timeline to display, resolved fresh each frame from the
        // flat log + branch choices.
        let visible = self.visible();

        // Retry the last turn when it errored and the agent is idle.
        let can_retry = cfg!(not(target_family = "wasm"))
            && !agent_busy
            && !show_agent_hint
            && visible
                .last()
                .is_some_and(|row| self.entries[row.idx].msg.error);

        let theme = ui.ctx().get_lb_theme();
        let available_width = ui.available_width();
        let col_width = available_width.min(MAX_WIDTH);
        let max_bubble_content_w = (col_width * 0.72 - H_PAD * 2.0).max(120.0);
        let text_color = theme.neutral_fg();
        let secondary_color = theme
            .neutral_fg_secondary()
            .lerp_to_gamma(theme.neutral_fg(), 0.5); // fg secondary hard to read on colored bg
        let error_color = theme.fg().get_color(Palette::Red);

        // Surface for the composer and others' bubbles — a slight lift off
        // `neutral_bg`. Derived from `neutral_bg`/`neutral_fg` (the reliable
        // poles) rather than `neutral_bg_secondary`, which Android Material You
        // maps to a foreground tone (near-white in dark mode).
        let bubble_surface = theme.neutral_bg().lerp_to_gamma(theme.neutral_fg(), 0.06);

        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        let full_rect = ui.available_rect_before_wrap();

        // The panel is inset past the keyboard but not the nav bar, so on
        // Android clear the nav bar only while the keyboard is down.
        let keyboard_up = ui
            .memory(|m| m.data.get_temp::<f32>(Id::new("ws_keyboard_height")))
            .unwrap_or(0.0)
            > 0.0;
        let composer_bottom_inset = if cfg!(target_os = "android") && !keyboard_up {
            COMPOSER_NAV_CLEARANCE
        } else {
            COMPOSER_BOTTOM_GAP
        };

        let composer_id = Id::new("chat_composer");
        if !self.initialized {
            ui.memory_mut(|m| m.request_focus(composer_id));
            self.initialized = true;
        }

        // Esc cancels an in-progress edit and restores the stashed draft.
        #[cfg(not(target_family = "wasm"))]
        if self.editing.is_some()
            && ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.editing = None;
            let restore = self.draft_stash.take().unwrap_or_default();
            self.composer.set_text(&restore);
        }

        // Enter → send (Shift+Enter for a newline; Cmd/Ctrl+Enter kept for
        // muscle memory). Consumed before handle_input so the composer
        // doesn't translate the Enter into a Newline — but not while a
        // completion popup is up (which accepts with Enter itself), and not
        // while a turn is streaming: a send can't fire then, and consuming
        // the key would eat the stroke with no feedback. Left unconsumed it
        // types a newline, which at least shows the key landed.
        let composer_focused = ui.memory(|m| m.has_focus(composer_id));
        let completions_open =
            self.composer.emoji_completions.active || self.composer.link_completions.active;
        let send_requested = composer_focused
            && !agent_busy
            && ui.ctx().input_mut(|i| {
                i.consume_key(Modifiers::COMMAND, Key::Enter)
                    || (!completions_open && i.consume_key(Modifiers::NONE, Key::Enter))
            });

        // Composer input phase — drain workspace-origin events (native iOS
        // text input arrives this way: Newline / Indent / Replace pushed by the
        // FFI), then keyboard / completions / internal events.
        let workspace_events = self.composer.drain_workspace_events(ui.ctx());
        self.composer.event.internal_events.extend(workspace_events);
        let _ = self.composer.handle_input(ui.ctx(), composer_id);

        // Measure at the exact render width so the composer bubble grows
        // same-frame. The re-parse inside `show` below hits the layout cache.
        // `SIDE_INSET` and `H_PAD` mirror the h_inset / shrink geometry below.
        let composer_inner_w = (col_width - 2.0 * SIDE_INSET - 2.0 * H_PAD).max(0.0);
        let measured_h = self.composer.measure_height(composer_inner_w);

        // Autogrow with a max cap, no lower floor — a lower floor makes a
        // single-line composer bottom-heavy (content is top-anchored).
        let composer_height = (measured_h + V_PAD * 2.0).min(COMPOSER_MAX_HEIGHT);
        // The composer bubble carries a toolbar row at its bottom; the model
        // dropdowns appear there for agent chats.
        #[cfg(not(target_family = "wasm"))]
        let indicator = if self.unshared { self.provider.clone() } else { None };
        let transcript_rect = Rect::from_min_max(
            full_rect.min,
            pos2(
                full_rect.max.x,
                full_rect.max.y - composer_height - TOOLBAR_H - composer_bottom_inset,
            ),
        );
        let mut text_areas = Vec::new();
        let mut retry_clicked = false;
        let mut setup_clicked = false;
        // Row actions (hover pill + context menu). Copy always works;
        // everything that mutates the transcript or timeline is gated on no
        // turn being in flight, and the agent-rerunning actions additionally
        // on this being an agent chat.
        let mut action: Option<RowAction> = None;
        let can_mutate = !agent_busy;
        #[cfg(not(target_family = "wasm"))]
        let agent_actions = self.harness.is_some();
        #[cfg(target_family = "wasm")]
        let agent_actions = false;

        // A branch switch suspends stick-to-bottom for the frame — the view
        // holds still while the timeline below the fork is swapped out.
        let branch_anchored = self.branch_anchor.is_some();
        ui.scope_builder(egui::UiBuilder::new().max_rect(transcript_rect), |ui| {
            ui.set_clip_rect(transcript_rect.intersect(ui.clip_rect()));
            ScrollArea::vertical()
                .id_salt("chat_messages")
                .stick_to_bottom(!branch_anchored)
                .show(ui, |ui| {
                    let origin = ui.cursor().min;
                    let col_pad = (available_width - col_width) / 2.0;
                    let col_left = origin.x + col_pad;
                    let col_right = col_left + col_width;
                    let note_x = col_left + H_MARGIN;
                    let note_wrap_w = col_width - 2.0 * H_MARGIN;

                    // Messages group into runs (consecutive rows that render
                    // as one visual block: name above, timestamp below) by
                    // author-and-kind. Notes never group.
                    let run_key = |e: &Entry| (e.msg.from.clone(), e.msg.agent, e.msg.error);

                    // pass 1: measure each visible message and compute its
                    // rect against a running y. This populates each label's
                    // layout cache so pass 2's paint is near-free.
                    let n = visible.len();
                    let mut plans: Vec<RowPlan> = Vec::with_capacity(n);
                    let mut strips: Vec<StripPlan> = Vec::with_capacity(n);
                    let mut y = origin.y + TOP_MARGIN;
                    for vi in 0..n {
                        let i = visible[vi].idx;
                        let is_mine_row = {
                            let m = &self.entries[i].msg;
                            m.from == self.account.username && !m.agent
                        };
                        // Reserved metadata strip below every row — actions
                        // live in dedicated space, never overlaid on text.
                        let mut strip = |y: &mut f32, row_rect: Rect, ts: Option<Arc<Galley>>| {
                            let rect =
                                Rect::from_min_size(pos2(note_x, *y), vec2(note_wrap_w, STRIP_H));
                            strips.push(StripPlan { vi, rect, row_rect, right: is_mine_row, ts });
                            *y += STRIP_H + ROW_GAP;
                        };
                        if self.entries[i].msg.error {
                            let header = ui.fonts(|f| {
                                f.layout_no_wrap(
                                    "error".into(),
                                    egui::FontId::proportional(11.0),
                                    error_color,
                                )
                            });
                            let galley = ui.fonts(|f| {
                                f.layout(
                                    self.entries[i].msg.content.clone(),
                                    egui::FontId::monospace(NOTE_FONT),
                                    error_color,
                                    note_wrap_w,
                                )
                            });
                            let h = header.size().y + NAME_GAP + galley.size().y;
                            let w = galley.size().x.max(header.size().x);
                            plans.push(RowPlan::Note {
                                header: Some(header),
                                galley: galley.clone(),
                                pos: pos2(note_x, y),
                            });
                            let row_rect = Rect::from_min_size(pos2(note_x, y), vec2(w, h));
                            y += h + ROW_GAP;
                            strip(&mut y, row_rect, None);
                            continue;
                        }

                        let from = self.entries[i].msg.from.clone();
                        let ts = self.entries[i].msg.ts;
                        let agent = self.entries[i].msg.agent;
                        let is_mine = is_mine_row;
                        let key = run_key(&self.entries[i]);
                        let first_in_run =
                            vi == 0 || run_key(&self.entries[visible[vi - 1].idx]) != key;
                        let last_in_run =
                            vi + 1 >= n || run_key(&self.entries[visible[vi + 1].idx]) != key;

                        // Every run is headed — usernames for people, "agent"
                        // for the agent — so attribution survives any mix of
                        // speakers in a shared chat.
                        let name_galley = if first_in_run {
                            let (name, color) = if agent {
                                ("agent".to_string(), secondary_color)
                            } else {
                                (from.clone(), theme.fg().get_color(username_color(&from)))
                            };
                            Some(ui.fonts(|f| {
                                f.layout_no_wrap(name, egui::FontId::proportional(11.0), color)
                            }))
                        } else {
                            None
                        };

                        // Timestamps live in the strip, on the run's tail
                        // row — and only for human runs: an agent reply lands
                        // within the minute of the request above it.
                        let ts_galley = if last_in_run && !agent {
                            Some(ui.fonts(|f| {
                                f.layout_no_wrap(
                                    format_ts(ts),
                                    egui::FontId::proportional(11.0),
                                    secondary_color,
                                )
                            }))
                        } else {
                            None
                        };

                        let entry = &mut self.entries[i];
                        // Height includes the gap separating it from the
                        // content; pass 2 paints with a matching offset.
                        let name_h = name_galley
                            .as_ref()
                            .map_or(0.0, |g| g.rect.height() + NAME_GAP);

                        if agent {
                            let content_h = entry.label.height(&entry.msg.content, note_wrap_w);
                            let row_rect = Rect::from_min_size(
                                pos2(note_x, y),
                                vec2(note_wrap_w, name_h + content_h),
                            );
                            plans.push(RowPlan::Agent {
                                pos: pos2(note_x, y),
                                name_galley,
                                name_h,
                                content_h,
                            });
                            y += name_h + content_h + ROW_GAP;
                            strip(&mut y, row_rect, ts_galley);
                        } else {
                            let content_h =
                                entry.label.height(&entry.msg.content, max_bubble_content_w);
                            let bubble_w = max_bubble_content_w + H_PAD * 2.0;
                            let bubble_h = name_h + content_h + V_PAD * 2.0;
                            let bubble_x = if is_mine {
                                col_right - H_MARGIN - bubble_w
                            } else {
                                col_left + H_MARGIN
                            };
                            let bubble_rect =
                                Rect::from_min_size(pos2(bubble_x, y), vec2(bubble_w, bubble_h));
                            plans.push(RowPlan::Bubble {
                                bubble_rect,
                                name_galley,
                                name_h,
                                content_h,
                            });
                            y += bubble_h + ROW_GAP;
                            strip(&mut y, bubble_rect, ts_galley);
                        }
                    }

                    // Trailing agent rows: the streaming reply live on the
                    // canvas under an "agent" header ("thinking…" until the
                    // first token), or setup guidance when the chat has no
                    // configured agent.
                    #[allow(unused_mut, unused_variables)]
                    let mut streaming_plan: Option<(Pos2, Arc<Galley>)> = None;
                    #[cfg(not(target_family = "wasm"))]
                    if agent_busy && !agent_streaming.is_empty() {
                        let name = ui.fonts(|f| {
                            f.layout_no_wrap(
                                "agent".into(),
                                egui::FontId::proportional(11.0),
                                secondary_color,
                            )
                        });
                        let name_h = name.rect.height() + NAME_GAP;
                        let content_h = self.streaming_label.height(&agent_streaming, note_wrap_w);
                        y += V_PAD;
                        streaming_plan = Some((pos2(note_x, y), name));
                        y += name_h + content_h + ROW_GAP + V_PAD;
                    }
                    // The setup hint doubles as the first-run flow: clicking
                    // it creates a template provider file and opens it —
                    // config is files, so setup is filling one in.
                    let mut note = None;
                    if agent_busy && agent_streaming.is_empty() {
                        note = Some(("thinking…".to_string(), secondary_color, false));
                    } else if show_agent_hint {
                        note = Some((
                            format!("agent is off — click to create {SETUP_PATH} and paste in an API key"),
                            theme.fg().get_color(Palette::Blue),
                            true,
                        ));
                    }
                    let note_plan = note.map(|(text, color, link)| {
                        let galley = ui.fonts(|f| {
                            f.layout(text, egui::FontId::proportional(NOTE_FONT), color, note_wrap_w)
                        });
                        let h = galley.rect.height();
                        let pos = pos2(note_x, y);
                        y += h + ROW_GAP;
                        (galley, pos, link)
                    });

                    // Keep the clicked arrows where they were: correct for
                    // any height change of the swapped fork row (content
                    // above the fork is identical, so that's the whole
                    // delta). Consuming a scroll also un-sticks the area.
                    if let Some((avi, old_y)) = self.branch_anchor.take() {
                        if let Some(s) = strips.iter().find(|s| s.vi == avi) {
                            let delta = old_y - s.rect.min.y;
                            if delta.abs() > 0.5 {
                                ui.scroll_with_delta(vec2(0.0, delta));
                            }
                        }
                    }

                    // Allocate total footprint so ScrollArea sees the right
                    // height (stick-to-bottom depends on this).
                    let total_h = (y - origin.y) + BOTTOM_PAD;
                    let _ = ui.allocate_exact_size(vec2(available_width, total_h), Sense::hover());

                    if std::mem::take(&mut self.scroll_to_bottom) {
                        ui.scroll_to_rect(
                            Rect::from_min_size(pos2(origin.x, origin.y + total_h), vec2(1.0, 1.0)),
                            Some(egui::Align::BOTTOM),
                        );
                    }

                    // pass 2: paint absolute. No egui layout calls.
                    #[cfg(not(target_family = "wasm"))]
                    let editing_id = self.editing;
                    #[cfg(target_family = "wasm")]
                    let editing_id: Option<Uuid> = None;
                    for (vi, plan) in plans.into_iter().enumerate() {
                        let i = visible[vi].idx;
                        match plan {
                            RowPlan::Bubble { bubble_rect, name_galley, name_h, content_h } => {
                                ui.painter().rect_filled(
                                    bubble_rect,
                                    CornerRadius::same(CORNER),
                                    bubble_surface,
                                );
                                // The message being edited is outlined in the
                                // accent — send commits a sibling of it.
                                if editing_id.is_some() && self.entries[i].msg.id == editing_id {
                                    ui.painter().rect_stroke(
                                        bubble_rect,
                                        CornerRadius::same(CORNER),
                                        Stroke::new(1.0, theme.fg().get_color(theme.prefs().primary)),
                                        StrokeKind::Inside,
                                    );
                                }

                                let mut text_y = bubble_rect.min.y + V_PAD;
                                if let Some(ng) = name_galley {
                                    ui.painter().galley(
                                        pos2(bubble_rect.min.x + H_PAD, text_y),
                                        ng,
                                        text_color,
                                    );
                                    text_y += name_h;
                                }

                                let content_top = pos2(bubble_rect.min.x + H_PAD, text_y);
                                let entry = &mut self.entries[i];
                                let (areas, _) = entry.label.paint_at(
                                    ui,
                                    &entry.msg.content,
                                    content_top,
                                    max_bubble_content_w,
                                );
                                text_areas.extend(areas);
                                let _ = content_h;
                            }
                            RowPlan::Agent { pos, name_galley, name_h, content_h } => {
                                let mut text_y = pos.y;
                                if let Some(ng) = name_galley {
                                    ui.painter()
                                        .galley(pos2(pos.x, text_y), ng, secondary_color);
                                    text_y += name_h;
                                }

                                let entry = &mut self.entries[i];
                                let (areas, _) = entry.label.paint_at(
                                    ui,
                                    &entry.msg.content,
                                    pos2(pos.x, text_y),
                                    note_wrap_w,
                                );
                                text_areas.extend(areas);
                                let _ = content_h;
                            }
                            RowPlan::Note { header, galley, pos } => {
                                let mut text_y = pos.y;
                                if let Some(header) = header {
                                    let h = header.size().y;
                                    ui.painter().galley(pos2(pos.x, text_y), header, error_color);
                                    text_y += h + NAME_GAP;
                                }
                                ui.painter().galley(pos2(pos.x, text_y), galley, error_color);
                            }
                        }
                    }

                    // Metadata strips: timestamp, ‹ 2/3 › arrows, and hover
                    // action icons, in the reserved space under each row.
                    // All row interaction (context menu included) lives here.
                    for strip in &strips {
                        let vi = strip.vi;
                        let i = visible[vi].idx;
                        let is_tail = vi + 1 == visible.len();
                        let kind = {
                            let m = &self.entries[i].msg;
                            if m.error {
                                RowKind::Other
                            } else if m.agent {
                                RowKind::AgentReply
                            } else if m.from == self.account.username {
                                RowKind::OwnUser
                            } else {
                                RowKind::Other
                            }
                        };
                        let editable = can_mutate
                            && agent_actions
                            && self.entries[i].msg.id.is_some()
                            && parent_for_sibling(&self.entries, i).is_some();
                        let regen = editable && kind == RowKind::AgentReply && is_tail;
                        // Retry lives where every other row action lives.
                        let retryable = is_tail && can_retry && self.entries[i].msg.error;
                        let hovered = ui.rect_contains_pointer(strip.row_rect)
                            || ui.rect_contains_pointer(strip.rect);
                        let show_icons =
                            hovered || (is_tail && kind == RowKind::AgentReply) || retryable;

                        // Items lay out from the row's aligned edge inward:
                        // action icons, then arrows, then the timestamp.
                        let dir: f32 = if strip.right { -1.0 } else { 1.0 };
                        let mut x = if strip.right { strip.rect.max.x } else { strip.rect.min.x };
                        let mut place = |w: f32| {
                            let min_x = if strip.right { x - w } else { x };
                            let r = Rect::from_min_size(
                                pos2(min_x, strip.rect.min.y),
                                vec2(w, STRIP_H),
                            );
                            x += dir * (w + STRIP_GAP);
                            r
                        };

                        if show_icons {
                            let mut icons: Vec<(&Icon, Option<RowAction>)> = Vec::new();
                            if editable && kind == RowKind::OwnUser {
                                icons.push((&Icon::PENCIL, Some(RowAction::Edit(i))));
                            }
                            icons.push((&Icon::CONTENT_COPY, None));
                            if regen {
                                icons.push((&Icon::SYNC, Some(RowAction::Regenerate(i))));
                            }
                            if retryable {
                                icons.push((&Icon::SYNC, Some(RowAction::RetryLast)));
                            }
                            for (bi, (icon, act)) in icons.iter().enumerate() {
                                let r = place(STRIP_H);
                                let resp = ui
                                    .interact(r, Id::new(("chat_strip", i, bi)), Sense::click())
                                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                                let color =
                                    if resp.hovered() { text_color } else { secondary_color };
                                let g = ui.fonts(|f| {
                                    f.layout_no_wrap(
                                        icon.icon.to_string(),
                                        egui::FontId::monospace(13.0),
                                        color,
                                    )
                                });
                                ui.painter().galley(r.center() - g.size() / 2.0, g, color);
                                if resp.clicked() {
                                    match act {
                                        None => {
                                            let content = self.entries[i].msg.content.clone();
                                            ui.ctx().copy_text(content);
                                        }
                                        Some(a) => action = Some(*a),
                                    }
                                }
                            }
                        }

                        if let Some(fork) = &visible[vi].fork {
                            let font = egui::FontId::proportional(NOTE_FONT);
                            let label = format!("{}/{}", fork.pos + 1, fork.siblings.len());
                            let lg =
                                ui.fonts(|f| f.layout_no_wrap(label, font.clone(), secondary_color));
                            // Near-edge-first placement; flip on right-laid
                            // strips so it always reads ‹ n/m › on screen.
                            let mut parts = [
                                ("‹", fork.pos.checked_sub(1), true),
                                ("", None, false), // label slot
                                ("›", Some(fork.pos + 1), true),
                            ];
                            if strip.right {
                                parts.reverse();
                            }
                            for (glyph, target_pos, is_arrow) in parts {
                                if !is_arrow {
                                    let r = place(lg.size().x);
                                    ui.painter().galley(
                                        pos2(r.min.x, r.center().y - lg.size().y / 2.0),
                                        lg.clone(),
                                        secondary_color,
                                    );
                                    continue;
                                }
                                let target =
                                    target_pos.and_then(|p| fork.siblings.get(p)).copied();
                                let active = target.is_some() && can_mutate;
                                let color = if active { text_color } else { secondary_color };
                                let r = place(STRIP_H * 0.7);
                                let g = ui
                                    .fonts(|f| f.layout_no_wrap(glyph.into(), font.clone(), color));
                                ui.painter().galley(r.center() - g.size() / 2.0, g, color);
                                if let Some(target) = target.filter(|_| can_mutate) {
                                    let resp = ui
                                        .interact(
                                            r,
                                            Id::new(("chat_arrow", vi, glyph)),
                                            Sense::click(),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                                    if resp.clicked() {
                                        action = Some(RowAction::Switch {
                                            parent: fork.parent,
                                            target,
                                            vi,
                                            anchor_y: strip.rect.min.y,
                                        });
                                    }
                                }
                            }
                        }

                        if let Some(ts) = &strip.ts {
                            let r = place(ts.size().x);
                            ui.painter().galley(
                                pos2(r.min.x, r.center().y - ts.size().y / 2.0),
                                ts.clone(),
                                secondary_color,
                            );
                        }

                        // Context menu over the message row (secondary path
                        // to the same actions, plus Retry-from-here/Delete).
                        let content = self.entries[i].msg.content.clone();
                        row_menu(
                            ui,
                            strip.row_rect,
                            i,
                            &content,
                            kind,
                            editable,
                            regen,
                            can_mutate,
                            &mut action,
                        );
                    }

                    // Trailing agent rows paint after the transcript.
                    #[cfg(not(target_family = "wasm"))]
                    if let Some((pos, name)) = streaming_plan {
                        let name_h = name.rect.height() + NAME_GAP;
                        ui.painter().galley(pos, name, secondary_color);
                        let (areas, _) = self.streaming_label.paint_at(
                            ui,
                            &agent_streaming,
                            pos2(pos.x, pos.y + name_h),
                            note_wrap_w,
                        );
                        text_areas.extend(areas);
                    }
                    if let Some((galley, pos, link)) = note_plan {
                        let rect = Rect::from_min_size(pos, galley.size());
                        ui.painter().galley(pos, galley, secondary_color);
                        if link {
                            let resp = ui
                                .interact(rect, Id::new("chat_setup_link"), Sense::click())
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            setup_clicked = resp.clicked();
                        }
                    }
                });
        });

        // Transcript text callback. Submit before composer so the composer's
        // own callback (inside show) lands on a later glyphon layer.
        // `clip_rect` not `max_rect`: egui_wgpu drops a zero-area callback rect.
        if !text_areas.is_empty() {
            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    ui.clip_rect(),
                    GlyphonRendererCallback::new(text_areas),
                ));
        }

        // Commit the frame's row action. Deletions propagate to other
        // devices via the merge; timeline changes reseed the driver.
        let mut changed = false;
        match action {
            Some(RowAction::Delete(i)) => {
                self.entries.remove(i);
                self.seq += 1;
                changed = true;
                #[cfg(not(target_family = "wasm"))]
                {
                    let seed = self.visible_seed();
                    if let Some(harness) = &mut self.harness {
                        harness.reseed(seed);
                    }
                }
            }
            Some(RowAction::Switch { parent, target, vi, anchor_y }) => {
                if let Some(id) = self.entries[target].msg.id {
                    self.branch_choice.insert(parent, id);
                    self.branch_anchor = Some((vi, anchor_y));
                    #[cfg(not(target_family = "wasm"))]
                    {
                        let seed = self.visible_seed();
                        if let Some(harness) = &mut self.harness {
                            harness.reseed(seed);
                        }
                    }
                }
            }
            #[cfg(not(target_family = "wasm"))]
            Some(RowAction::Edit(i)) => self.enter_edit(i, ui, composer_id),
            #[cfg(not(target_family = "wasm"))]
            Some(RowAction::ResendFrom(i)) => {
                self.resend_as_sibling(i);
                changed = true;
            }
            #[cfg(not(target_family = "wasm"))]
            Some(RowAction::Regenerate(i)) => self.regenerate(i),
            Some(RowAction::RetryLast) => retry_clicked = true,
            _ => {}
        }

        #[cfg(not(target_family = "wasm"))]
        {
            if retry_clicked {
                // The rerun's reply is a sibling of the error row: same parent.
                self.pending_parent = self
                    .visible()
                    .last()
                    .and_then(|row| parent_for_sibling(&self.entries, row.idx));
                self.provider = self.resolve_provider();
                let system = settings::instructions(&self.core);
                if let (Some(harness), Some(provider)) = (&mut self.harness, self.provider.clone())
                {
                    harness.retry(provider, system);
                }
            }
            if setup_clicked {
                self.create_provider_file("cerebras");
                // The selection config entry must reach a save, same as the
                // add-provider path.
                changed = true;
            }
        }
        #[cfg(target_family = "wasm")]
        let _ = (retry_clicked, setup_clicked);

        // Composer bubble: text on top, a Zed-style toolbar row at the
        // bottom (model dropdowns left, send/stop right).
        let composer_rect = Rect::from_min_max(
            pos2(
                full_rect.min.x,
                full_rect.max.y - composer_height - TOOLBAR_H - composer_bottom_inset,
            ),
            full_rect.max,
        );
        self.composer_rect = composer_rect;
        let col_pad = (available_width - col_width) / 2.0;
        let h_inset = col_pad + SIDE_INSET;
        let bubble_rect = Rect::from_min_max(
            pos2(composer_rect.min.x + h_inset, composer_rect.min.y),
            pos2(composer_rect.max.x - h_inset, composer_rect.max.y - composer_bottom_inset),
        );
        ui.painter()
            .rect_filled(bubble_rect, CornerRadius::same(CORNER), bubble_surface);

        // Composer draw (text area above the toolbar). Submits its own text
        // callback internally.
        let text_rect = Rect::from_min_max(
            bubble_rect.min,
            pos2(bubble_rect.max.x, bubble_rect.max.y - TOOLBAR_H),
        );
        let inner_rect = text_rect.shrink2(vec2(H_PAD, V_PAD));
        self.composer.show(ui, inner_rect, composer_id);

        // Ghosted placeholder over the empty composer.
        if self.composer.renderer.buffer.current.text.is_empty() {
            let row_h = self.composer.row_height();
            let hint = ui.fonts(|f| {
                f.layout_no_wrap(
                    "Type a message".into(),
                    egui::FontId::proportional(row_h * 0.85),
                    theme.neutral(),
                )
            });
            let y = inner_rect.min.y + (row_h - hint.size().y) / 2.0;
            ui.painter()
                .galley(pos2(inner_rect.min.x, y), hint, theme.neutral());
        }

        let toolbar_rect = Rect::from_min_max(
            pos2(bubble_rect.min.x + H_PAD, bubble_rect.max.y - TOOLBAR_H),
            pos2(bubble_rect.max.x - H_PAD, bubble_rect.max.y),
        );

        // Provider and model dropdowns, Zed-style: label ⌄ buttons opening
        // menus. Picking appends a config entry — selection syncs across
        // this user's devices; credentials never do.
        #[cfg(not(target_family = "wasm"))]
        if let Some(current) = &indicator {
            let mut cursor_x = toolbar_rect.min.x;

            // The listing feeds the ring, the model button's display name,
            // and the picker — fetch as soon as the toolbar shows, not on
            // first picker open. The cache makes this a per-frame no-op.
            self.fetch_models(current);

            // Context usage ring, Zed-style: the last turn's tokens against
            // the model's window, when the /models listing reports one — no
            // data means no ring, never a guess.
            let window = self.models.as_ref().and_then(|((name, url), list)| {
                (name == &current.name && url == &current.base_url)
                    .then(|| {
                        list.iter()
                            .find(|m| m.id == current.model)
                            .and_then(|m| m.window)
                    })
                    .flatten()
            });
            let last_usage = self.visible().iter().rev().find_map(|row| {
                let m = &self.entries[row.idx].msg;
                (m.agent && !m.error).then_some(m.usage).flatten()
            });
            if let (Some(window), Some(u)) = (window, last_usage) {
                let used = u.input + u.output + u.cache_read + u.cache_write;
                let ratio = (used as f32 / window as f32).min(1.0);
                let r = 6.0;
                let center = pos2(cursor_x + r + 1.0, toolbar_rect.center().y);
                let rect = Rect::from_center_size(center, vec2(2.0 * r + 6.0, TOOLBAR_H));
                cursor_x = rect.max.x + STRIP_GAP;

                let track = theme.neutral_bg().lerp_to_gamma(theme.neutral_fg(), 0.18);
                let fill = if ratio >= 0.85 {
                    error_color
                } else {
                    theme.fg().get_color(theme.prefs().primary)
                };
                ui.painter()
                    .circle_stroke(center, r, Stroke::new(2.0, track));
                if ratio > 0.0 {
                    let n = 32;
                    let points: Vec<Pos2> = (0..=n)
                        .map(|k| {
                            let a = -std::f32::consts::FRAC_PI_2
                                + ratio * std::f32::consts::TAU * k as f32 / n as f32;
                            pos2(center.x + r * a.cos(), center.y + r * a.sin())
                        })
                        .collect();
                    ui.painter()
                        .add(egui::Shape::line(points, Stroke::new(2.0, fill)));
                }
                ui.interact(rect, Id::new("chat_context_ring"), Sense::hover())
                    .on_hover_text(format!("{used} / {window} tokens ({:.0}%)", ratio * 100.0));
            }

            let mut dropdown = |ui: &mut Ui, id: &str, text: &str| -> egui::Response {
                let galley = ui.fonts(|f| {
                    f.layout_no_wrap(
                        text.to_string(),
                        egui::FontId::proportional(NOTE_FONT),
                        secondary_color,
                    )
                });
                let chevron = ui.fonts(|f| {
                    f.layout_no_wrap(
                        Icon::CHEVRON_DOWN.icon.to_string(),
                        egui::FontId::monospace(12.0),
                        secondary_color,
                    )
                });
                let w = galley.size().x + chevron.size().x + 6.0;
                let rect =
                    Rect::from_min_size(pos2(cursor_x, toolbar_rect.min.y), vec2(w, TOOLBAR_H));
                cursor_x = rect.max.x + STRIP_GAP;
                let resp = ui
                    .interact(rect, Id::new(id), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let text_top = rect.min.y + (TOOLBAR_H - galley.size().y) / 2.0;
                ui.painter()
                    .galley(pos2(rect.min.x, text_top), galley, secondary_color);
                ui.painter().galley(
                    pos2(rect.max.x - chevron.size().x, text_top),
                    chevron,
                    secondary_color,
                );
                resp
            };

            let mut pick: Option<(String, String)> = None;

            // The selected row is marked by foreground text color rather
            // than egui's selection fill.
            let row_text = |text: &str, selected: bool| {
                egui::RichText::new(text).color(if selected { text_color } else { secondary_color })
            };

            // The toolbar hugs the bottom of the window, so menus open
            // upward — downward they'd clamp to a couple of rows.
            let mut add: Option<&'static str> = None;
            let mut open_instructions = false;
            let provider_resp = dropdown(ui, "chat_provider_btn", &current.label());
            egui::Popup::menu(&provider_resp)
                .align(egui::RectAlign::TOP_START)
                .show(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);
                    ui.set_min_width(140.0);
                    // Same grouping and order as the add-provider menu;
                    // names it doesn't know trail in their own group.
                    let providers = settings::load(&self.core);
                    let mut groups: Vec<Vec<&settings::Provider>> = TEMPLATES
                        .iter()
                        .map(|group| {
                            group
                                .iter()
                                .filter_map(|(n, _)| providers.iter().find(|p| &p.name == n))
                                .collect()
                        })
                        .collect();
                    groups.push(
                        providers
                            .iter()
                            .filter(|p| {
                                !TEMPLATES
                                    .iter()
                                    .flat_map(|g| g.iter())
                                    .any(|(n, _)| *n == p.name)
                            })
                            .collect(),
                    );
                    let mut rendered_any = false;
                    for group in groups {
                        if group.is_empty() {
                            continue;
                        }
                        if rendered_any {
                            ui.separator();
                        }
                        rendered_any = true;
                        for p in group {
                            if ui
                                .button(row_text(&p.label(), p.name == current.name))
                                .clicked()
                            {
                                // Sticky per provider: switching back lands
                                // on the model it last ran with.
                                let model =
                                    last_model_for(&self.entries, &self.account.username, &p.name)
                                        .unwrap_or_default();
                                pick = Some((p.name.clone(), model));
                            }
                        }
                    }
                    ui.separator();
                    // Templates pre-fill an ordinary provider file, which
                    // opens for the key paste and becomes this chat's
                    // selection.
                    ui.menu_button(row_text("add provider", false), |ui| {
                        ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);
                        ui.set_min_width(140.0);
                        for (i, group) in TEMPLATES.iter().enumerate() {
                            if i > 0 {
                                ui.separator();
                            }
                            for (name, json) in group.iter() {
                                if ui
                                    .button(row_text(&template_label(name, json), false))
                                    .clicked()
                                {
                                    add = Some(name);
                                    ui.close();
                                }
                            }
                        }
                    });
                    // The system prompt, as a file: created pre-filled with
                    // the default so edits start from what's sent today.
                    if ui.button(row_text("instructions", false)).clicked() {
                        open_instructions = true;
                        ui.close();
                    }
                });

            // The button shows the selected model's display name when the
            // listing has landed; the id (what's actually configured) until
            // then, or when the endpoint doesn't offer names.
            let model_label = self
                .models
                .as_ref()
                .filter(|((name, url), _)| name == &current.name && url == &current.base_url)
                .and_then(|(_, list)| list.iter().find(|m| m.id == current.model))
                .map(|m| m.label().to_string())
                .unwrap_or_else(|| current.model.clone());
            let model_resp = dropdown(ui, "chat_model_btn", &model_label);
            egui::Popup::menu(&model_resp)
                .align(egui::RectAlign::TOP_START)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);
                    ui.set_min_width(180.0);
                    match &self.models {
                        Some(((name, url), list))
                            if *name == current.name && *url == current.base_url =>
                        {
                            if list.is_empty() {
                                ui.weak("no model listing — set \"model\" in the provider file");
                            }
                            egui::ScrollArea::vertical()
                                .id_salt("chat_model_list")
                                // The popup's Ui extends toward the screen
                                // bottom even when the menu opens upward, so
                                // available_height is a sliver; this floors
                                // the viewport regardless.
                                .min_scrolled_height(400.0)
                                .max_height(400.0)
                                .show(ui, |ui| {
                                    for m in list {
                                        let selected = m.id == current.model;
                                        if ui.button(row_text(m.label(), selected)).clicked() {
                                            pick = Some((current.name.clone(), m.id.clone()));
                                            ui.close();
                                        }
                                    }
                                });
                        }
                        _ => match &self.models_err {
                            Some(((name, url), err))
                                if *name == current.name && *url == current.base_url =>
                            {
                                ui.weak(format!("couldn't list models: {err}"));
                            }
                            _ => {
                                ui.weak("loading models…");
                            }
                        },
                    }
                });

            if let Some((provider, model)) = pick {
                self.write_selection(provider, model);
                changed = true;
            }
            if let Some(name) = add {
                self.create_provider_file(name);
                changed = true;
            }
            if open_instructions {
                self.create_instructions_file();
            }
        }

        // Send/stop at the toolbar's right end. While a turn streams the
        // button is a stop square (the reply keeps what streamed so far).
        let non_empty = !self.composer.renderer.buffer.current.text.trim().is_empty();
        let mut send_clicked = false;
        let mut stop_clicked = false;
        {
            let d = TOOLBAR_H - 8.0;
            let center = pos2(toolbar_rect.max.x - d / 2.0, toolbar_rect.center().y);
            let button_rect = Rect::from_center_size(center, vec2(d, d));
            let resp = ui.interact(button_rect, Id::new("chat_send"), Sense::click());

            let active = non_empty || agent_busy;
            let fill = if active {
                theme.bg().get_color(theme.prefs().primary)
            } else {
                theme.neutral_bg().lerp_to_gamma(theme.neutral_fg(), 0.12)
            };
            let painter = ui.painter();
            painter.circle_filled(center, d / 2.0, fill);
            if agent_busy {
                let side = d * 0.36;
                painter.rect_filled(
                    Rect::from_center_size(center, vec2(side, side)),
                    CornerRadius::same(2),
                    theme.neutral_fg(),
                );
                stop_clicked = resp.clicked();
            } else {
                let icon = ui.fonts(|f| {
                    f.layout_no_wrap(
                        Icon::SEND.icon.to_string(),
                        egui::FontId::monospace(d * 0.55),
                        theme.neutral_fg(),
                    )
                });
                painter.galley(center - icon.size() / 2.0, icon, theme.neutral_fg());
                send_clicked = resp.clicked();
            }
        }

        #[cfg(not(target_family = "wasm"))]
        if stop_clicked {
            if let Some(harness) = &mut self.harness {
                harness.stop();
            }
        }
        #[cfg(target_family = "wasm")]
        let _ = stop_clicked;

        let sent = (send_requested || send_clicked) && !agent_busy && self.submit(ui, composer_id);

        // Popups land last so they composite over composer + transcript.
        self.composer.show_completions(ui);

        (sent || agent_changed || changed, inner_rect)
    }
}

/// A row on the visible timeline: an index into `entries`, plus fork info
/// when the row has siblings (alternate timelines to arrow between).
struct VisibleRow {
    idx: usize,
    fork: Option<ForkInfo>,
}

/// A fork point: this row and its siblings under `parent` (`Uuid::nil()` =
/// root). Switching writes `branch_choice[parent] = sibling id`.
struct ForkInfo {
    parent: Uuid,
    /// Sibling entry indices in ts order, including the visible row itself.
    siblings: Vec<usize>,
    /// The visible row's position in `siblings`.
    pos: usize,
}

/// Walk the conversation tree from the root, taking the chosen child at each
/// fork (`branch_choice`, defaulting to newest). The stored transcript stays
/// a flat ts-sorted log; this derives the one timeline to display, fresh
/// each frame.
///
/// Parents: `reply_to` when present (nil = root); legacy rows without it are
/// implicitly parented to the preceding message, so an old transcript reads
/// as one linear chain. An unresolvable `reply_to` (parent deleted or not
/// yet synced) also falls back to the implicit parent rather than orphaning
/// the row. `hide_children_of` truncates the walk at that parent — used
/// while regenerating so the superseded reply disappears during the turn.
fn resolve_visible(
    entries: &[Entry], branch_choice: &HashMap<Uuid, Uuid>, hide_children_of: Option<Uuid>,
) -> Vec<VisibleRow> {
    let msgs: Vec<usize> = (0..entries.len())
        .filter(|&i| !entries[i].hidden())
        .collect();
    let id_to_idx: HashMap<Uuid, usize> = msgs
        .iter()
        .filter_map(|&i| entries[i].msg.id.map(|id| (id, i)))
        .collect();

    // children[parent entry index (None = root)] = child indices, ts order.
    let mut children: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
    for (k, &i) in msgs.iter().enumerate() {
        let implicit = (k > 0).then(|| msgs[k - 1]);
        let parent = match entries[i].msg.reply_to {
            Some(rt) if rt.is_nil() => None,
            Some(rt) => id_to_idx.get(&rt).copied().or(implicit),
            None => implicit,
        };
        children.entry(parent).or_default().push(i);
    }

    let mut visible = Vec::new();
    let mut parent: Option<usize> = None;
    // Cap at msgs.len(): a cyclic reply_to in a hand-mangled file must not
    // hang the frame.
    for _ in 0..msgs.len() {
        let parent_id = match parent {
            None => Uuid::nil(),
            Some(p) => match entries[p].msg.id {
                Some(id) => id,
                // Forks need an identity to record a choice against; an
                // id-less parent can only have its single implicit child.
                None => Uuid::nil(),
            },
        };
        if parent.is_some() && hide_children_of == Some(parent_id) {
            break;
        }
        let Some(kids) = children.get(&parent) else { break };
        let chosen = branch_choice
            .get(&parent_id)
            .and_then(|id| kids.iter().find(|&&k| entries[k].msg.id == Some(*id)))
            .copied()
            .unwrap_or(*kids.last().expect("children lists are non-empty"));
        let fork = (kids.len() > 1).then(|| ForkInfo {
            parent: parent_id,
            pos: kids.iter().position(|&k| k == chosen).unwrap_or(0),
            siblings: kids.clone(),
        });
        visible.push(VisibleRow { idx: chosen, fork });
        parent = Some(chosen);
    }
    visible
}

/// The parent id a sibling of `entries[idx]` should carry: the row's own
/// resolved parent (`Uuid::nil()` = root), or `None` when the parent is a
/// legacy id-less row that can't be referenced.
fn parent_for_sibling(entries: &[Entry], idx: usize) -> Option<Uuid> {
    match entries[idx].msg.reply_to {
        Some(rt) => Some(rt),
        None => {
            let prev = (0..idx).rev().find(|&i| !entries[i].hidden());
            match prev {
                None => Some(Uuid::nil()),
                Some(p) => entries[p].msg.id,
            }
        }
    }
}

/// A row action requested via the hover pill, context menu, or arrows,
/// applied after the frame's paint.
#[derive(Clone, Copy)]
enum RowAction {
    Delete(usize),
    Edit(usize),
    ResendFrom(usize),
    Regenerate(usize),
    /// Re-run the turn behind the tail error row.
    RetryLast,
    /// Switch a fork to the sibling at entry index `target`. `vi`/`anchor_y`
    /// pin the clicked arrows' strip to its on-screen position across the
    /// content change, instead of letting stick-to-bottom yank the view.
    Switch {
        parent: Uuid,
        target: usize,
        vi: usize,
        anchor_y: f32,
    },
}

#[derive(Clone, Copy, PartialEq)]
enum RowKind {
    OwnUser,
    AgentReply,
    /// Other users' messages and error notes.
    Other,
}

/// Right-click menu on a message row — the secondary path to the row's
/// actions, plus the ones that don't earn an icon (Retry from here, Delete).
#[allow(clippy::too_many_arguments)]
fn row_menu(
    ui: &mut Ui, rect: Rect, i: usize, content: &str, kind: RowKind, editable: bool, regen: bool,
    can_delete: bool, action: &mut Option<RowAction>,
) {
    let resp = ui.interact(rect, Id::new(("chat_row", i)), Sense::click());
    resp.context_menu(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(content.to_string());
            ui.close();
        }
        if editable && kind == RowKind::OwnUser {
            if ui.button("Edit").clicked() {
                *action = Some(RowAction::Edit(i));
                ui.close();
            }
            if ui.button("Retry from here").clicked() {
                *action = Some(RowAction::ResendFrom(i));
                ui.close();
            }
        }
        if regen && ui.button("Regenerate").clicked() {
            *action = Some(RowAction::Regenerate(i));
            ui.close();
        }
        if can_delete && ui.button("Delete").clicked() {
            *action = Some(RowAction::Delete(i));
            ui.close();
        }
    });
}

/// The visible timeline as model context: agent replies as assistant turns,
/// everything else as user turns; errors carry nothing the model saw.
#[cfg(not(target_family = "wasm"))]
fn seed_history(entries: &[Entry], visible: &[VisibleRow]) -> Vec<backend::ChatMsg> {
    visible
        .iter()
        .map(|row| &entries[row.idx])
        .filter(|e| !e.msg.error)
        .map(|e| {
            if e.msg.agent {
                backend::ChatMsg::Assistant(e.msg.content.clone())
            } else {
                backend::ChatMsg::User(e.msg.content.clone())
            }
        })
        .collect()
}

/// Whether a file (or any ancestor) is shared — a shared chat is a group
/// conversation, which gets no agent.
#[cfg(not(target_family = "wasm"))]
fn is_shared(files: &FileCache, mut id: Uuid) -> bool {
    while let Some(f) = files.files.get(&id) {
        if !f.shares.is_empty() {
            return true;
        }
        if f.parent == f.id {
            break;
        }
        id = f.parent;
    }
    false
}

fn format_ts(ts: i64) -> String {
    let Some(utc) = DateTime::from_timestamp(ts, 0) else { return String::new() };
    let dt: DateTime<Local> = utc.with_timezone(&Local);
    let now = Local::now();
    let days_ago = (now.date_naive() - dt.date_naive()).num_days();
    if days_ago == 0 {
        dt.format("%H:%M").to_string()
    } else if days_ago == 1 {
        format!("Yesterday {}", dt.format("%H:%M"))
    } else {
        dt.format("%b %-d %H:%M").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(msg: Message) -> Entry {
        let files = Arc::new(RwLock::new(FileCache::empty()));
        Entry::new(msg, &egui::Context::default(), files, Uuid::new_v4())
    }

    fn msg(content: &str, reply_to: Option<Uuid>) -> Entry {
        let mut m = Message::new("me".into(), content.into(), 0);
        m.reply_to = reply_to;
        entry(m)
    }

    fn contents(entries: &[Entry], visible: &[VisibleRow]) -> Vec<String> {
        visible
            .iter()
            .map(|r| entries[r.idx].msg.content.clone())
            .collect()
    }

    /// Legacy rows (no ids, no reply_to) resolve as one linear chain.
    #[test]
    fn legacy_transcript_is_linear() {
        let entries: Vec<Entry> = ["a", "b", "c"]
            .iter()
            .map(|c| {
                let mut m = Message::new("me".into(), (*c).into(), 0);
                m.id = None;
                entry(m)
            })
            .collect();
        let visible = resolve_visible(&entries, &HashMap::new(), None);
        assert_eq!(contents(&entries, &visible), ["a", "b", "c"]);
        assert!(visible.iter().all(|r| r.fork.is_none()));
    }

    /// A sibling forks the timeline: newest wins by default, an explicit
    /// choice wins over that, and both siblings see the fork.
    #[test]
    fn fork_defaults_to_newest_and_honors_choice() {
        let root = msg("q1", Some(Uuid::nil()));
        let root_id = root.msg.id.unwrap();
        let reply = msg("a1", Some(root_id));
        let edit = msg("q1 (edited)", Some(Uuid::nil()));
        let edit_id = edit.msg.id.unwrap();
        let entries = vec![root, reply, edit];

        // Default: newest root sibling (the edit), which has no children.
        let visible = resolve_visible(&entries, &HashMap::new(), None);
        assert_eq!(contents(&entries, &visible), ["q1 (edited)"]);
        let fork = visible[0].fork.as_ref().unwrap();
        assert_eq!((fork.parent, fork.pos, fork.siblings.len()), (Uuid::nil(), 1, 2));

        // Choose the original: its subtree comes back.
        let mut choice = HashMap::new();
        choice.insert(Uuid::nil(), root_id);
        let visible = resolve_visible(&entries, &choice, None);
        assert_eq!(contents(&entries, &visible), ["q1", "a1"]);
        assert_eq!(visible[0].fork.as_ref().unwrap().pos, 0);

        // A stale choice (id not among the siblings) falls back to newest.
        let mut stale = HashMap::new();
        stale.insert(Uuid::nil(), Uuid::new_v4());
        let visible = resolve_visible(&entries, &stale, None);
        assert_eq!(contents(&entries, &visible), ["q1 (edited)"]);
        let _ = edit_id;
    }

    /// Regeneration hides the superseded reply for the duration of the turn:
    /// the walk stops at the invoking message.
    #[test]
    fn hide_children_truncates_at_parent() {
        let root = msg("q", Some(Uuid::nil()));
        let root_id = root.msg.id.unwrap();
        let reply = msg("a", Some(root_id));
        let entries = vec![root, reply];

        let visible = resolve_visible(&entries, &HashMap::new(), Some(root_id));
        assert_eq!(contents(&entries, &visible), ["q"]);
    }

    /// An unresolvable reply_to (parent not synced yet) falls back to the
    /// implicit chain instead of orphaning the row.
    #[test]
    fn dangling_reply_to_falls_back_to_implicit() {
        let a = msg("a", Some(Uuid::nil()));
        let b = msg("b", Some(Uuid::new_v4())); // parent unknown
        let entries = vec![a, b];
        let visible = resolve_visible(&entries, &HashMap::new(), None);
        assert_eq!(contents(&entries, &visible), ["a", "b"]);
    }

    /// Config rows never enter the tree: chains skip them.
    #[test]
    fn config_rows_are_invisible_to_the_chain() {
        let a = msg("a", None);
        let cfg =
            entry(Message::config_entry("me".into(), 0, lb_rs::model::chat::ChatConfig::default()));
        let b = msg("b", None);
        let entries = vec![a, cfg, b];
        let visible = resolve_visible(&entries, &HashMap::new(), None);
        assert_eq!(contents(&entries, &visible), ["a", "b"]);
    }

    /// Switching back to a provider restores the model it last ran with,
    /// mined from the selection history; empty models (provider-switch
    /// defaults) don't count, and other users' picks don't leak in.
    #[test]
    fn model_is_sticky_per_provider() {
        use lb_rs::model::chat::{ChatConfig, ModelSelection};
        let sel = |from: &str, ts, provider: &str, model: &str| {
            entry(Message::config_entry(
                from.into(),
                ts,
                ChatConfig {
                    model: Some(ModelSelection { provider: provider.into(), model: model.into() }),
                },
            ))
        };
        let entries = vec![
            sel("me", 0, "openai", "gpt-a"),
            sel("me", 1, "anthropic", "claude-x"),
            sel("me", 2, "openai", "gpt-b"),
            sel("me", 3, "openai", ""),
            sel("them", 4, "openai", "gpt-c"),
        ];
        assert_eq!(last_model_for(&entries, "me", "openai").as_deref(), Some("gpt-b"));
        assert_eq!(last_model_for(&entries, "me", "anthropic").as_deref(), Some("claude-x"));
        assert_eq!(last_model_for(&entries, "me", "groq"), None);
    }
}
