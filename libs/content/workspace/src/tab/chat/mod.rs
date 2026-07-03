//! Chat tab — markdown-rendered messages in a scrollable transcript, with a
//! multiline composer at the bottom. The document on disk is newline-delimited
//! JSON, merged across devices via `lb_rs::model::chat::Buffer::merge`
//! (symmetric union over timestamp).
//!
//! An unshared chat is a conversation with the user's own agent: replies
//! stream live onto the canvas and are appended to the transcript (as the
//! user, flagged `agent`) when the turn ends. The agent runs on the device
//! that composed the invoking message; shared chats get no agent.
//!
//! Split three ways so the review-worthy logic isn't buried in geometry:
//! this file owns the `Chat` state, transcript merge/persistence, and agent
//! folding; [`config`] the provider/model/key state machine; [`view`] all
//! rendering and hit-testing.

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

mod anthropic;
mod backend;
mod config;
mod glyphs;
mod harness;
mod openai;
mod settings;
mod view;

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

/// Templates for the "add provider" menu: UI affordances that pre-fill an
/// ordinary provider file. The file is the source of truth — the template
/// name only decides what gets written; the filename stays a plain label,
/// and `display_name` carries casing a filename can't ("OpenRouter").
///
/// Groups render separator-divided, unlabeled: model makers, then hosts
/// serving others' models, then local + the blank escape hatch. The brands
/// carry the distinction; anything not listed is one `custom` file away.
/// The `api_key` value in a fresh key-requiring template — an obvious
/// placeholder so "edit file" can drop the selection right on it, and so an
/// opened file reads as unconfigured rather than blank.
const KEY_PLACEHOLDER: &str = "YOUR API KEY HERE";

const TEMPLATES: &[&[(&str, &str)]] = &[
    &[
        (
            "openai",
            r#"{
  "display_name": "OpenAI",
  "base_url": "https://api.openai.com/v1",
  "model": "gpt-5.5",
  "api_key": "YOUR API KEY HERE"
}
"#,
        ),
        (
            "anthropic",
            r#"{
  "display_name": "Anthropic",
  "kind": "anthropic",
  "base_url": "https://api.anthropic.com/v1",
  "model": "claude-opus-4-8",
  "api_key": "YOUR API KEY HERE"
}
"#,
        ),
        (
            "google",
            r#"{
  "display_name": "Google",
  "base_url": "https://generativelanguage.googleapis.com/v1beta/openai",
  "model": "gemini-3.5-flash",
  "api_key": "YOUR API KEY HERE"
}
"#,
        ),
        (
            "xai",
            r#"{
  "display_name": "xAI",
  "base_url": "https://api.x.ai/v1",
  "model": "grok-4.3",
  "api_key": "YOUR API KEY HERE"
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
  "api_key": "YOUR API KEY HERE"
}
"#,
        ),
        (
            "groq",
            r#"{
  "display_name": "Groq",
  "base_url": "https://api.groq.com/openai/v1",
  "model": "openai/gpt-oss-120b",
  "api_key": "YOUR API KEY HERE"
}
"#,
        ),
        (
            "cerebras",
            r#"{
  "display_name": "Cerebras",
  "base_url": "https://api.cerebras.ai/v1",
  "model": "gemma-4-31b",
  "api_key": "YOUR API KEY HERE"
}
"#,
        ),
        (
            "together",
            r#"{
  "display_name": "Together",
  "base_url": "https://api.together.xyz/v1",
  "model": "deepseek-ai/DeepSeek-V4-Pro",
  "api_key": "YOUR API KEY HERE"
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
  "model": ""
}
"#,
        ),
        (
            "custom",
            r#"{
  "base_url": "https://api.example.com/v1",
  "model": "model-id",
  "api_key": "YOUR API KEY HERE"
}
"#,
        ),
    ],
];

/// What the add-provider menu shows for a template: the JSON's
/// `display_name`, through the same fallback a hand-made file gets.
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
    /// The connect step's masked API-key input. A real `MdEdit` (not an egui
    /// `TextEdit`) so the native iOS keyboard/caret reach it through the same
    /// bridge the composer uses — `focused_mdedit_mut` points here while the
    /// connect step is open. Plaintext + `mask` so it renders as `*`.
    pub key_field: MdEdit,
    /// The key field's rect last frame, surfaced as the text-interaction rect
    /// so the native text view positions over it.
    key_field_rect: Rect,
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
    harness: Option<harness::Harness>,
    /// Renders the in-flight streaming reply.
    streaming_label: MdLabel,
    /// Unshared at open — eligible for an agent (setup guidance is shown if
    /// none could be built).
    unshared: bool,
    core: lb_rs::blocking::Lb,
    /// Config loaded off the UI thread — decrypting provider files (and the
    /// prompt) is too slow for the render frame in debug. `provider` is the
    /// resolved current one (setup hint + the turn's backend); `providers`
    /// backs the picker; `system_prompt` is the prompt file's contents.
    /// A background reload refreshes all three on tab re-activation and
    /// after set-up, so nothing reads files on the UI thread.
    provider: Option<settings::Provider>,
    providers: Vec<settings::Provider>,
    system_prompt: Option<String>,
    /// False until the first background load lands — distinguishes "no
    /// providers yet" (don't flash the setup hint) from "genuinely none".
    config_loaded: bool,
    config_rx: Option<std::sync::mpsc::Receiver<LoadedConfig>>,
    /// The API-key entry form, shown after picking a key-requiring provider
    /// or when a provider's key is missing/rejected.
    key_entry: Option<KeyEntry>,
    /// Whether the connect step was open last frame. The step closes *itself*
    /// (validation lands in the background), so the closing frame hands focus
    /// back to the composer — no click is involved to do it naturally.
    connect_was_open: bool,
    /// Provider whose auto key-prompt was dismissed, so it isn't re-shown
    /// every frame after Cancel. Cleared on a new key so a rejected one
    /// re-prompts.
    key_prompt_dismissed: Option<String>,
    /// The provider chooser opened over an existing transcript (toolbar →
    /// "add provider") — same canvas as first-run onboarding, but explicitly
    /// summoned, so it gets a cancel. Cleared by picking or cancelling.
    chooser_open: bool,
    last_frame: Option<std::time::Instant>,
    /// Message being edited via composer-prefill (its id). Sending appends a
    /// sibling — the original and its subtree stay reachable via arrows.
    editing: Option<Uuid>,
    /// The composer draft stashed when editing began, restored on cancel or
    /// after the edit is sent.
    draft_stash: Option<String>,
    /// Parent for the reply of the turn in flight — the message it answers.
    /// Stamped as `reply_to` on the pushed reply/error row.
    pending_parent: Option<Uuid>,
    /// Live `/models` listing for the picker, keyed by (provider name,
    /// base_url) so editing the file's url invalidates it. Runtime data,
    /// never persisted. Only successes cache; failures set `models_err`
    /// and retry on a cooldown, so pasting a key in doesn't strand the
    /// picker on an old failure.
    models: Option<(ModelsKey, ModelListing)>,
    models_rx: Option<std::sync::mpsc::Receiver<(ModelsKey, Result<ModelListing, String>)>>,
    models_attempt: Option<(ModelsKey, std::time::Instant)>,
    /// Keyed like the cache so provider A's failure never displays under
    /// provider B's picker.
    models_err: Option<(ModelsKey, String)>,
    /// Brand glyph textures for the picker, rasterized on first use.
    glyphs: glyphs::Glyphs,
}

type ModelListing = Vec<backend::ModelInfo>;

/// The result of a background config load — everything that lives in files
/// under `/.agent`, decrypted off the UI thread.
struct LoadedConfig {
    providers: Vec<settings::Provider>,
    system_prompt: Option<String>,
}

/// The inline "connect a provider" step — a masked key field in the
/// onboarding canvas, with connecting/rejected feedback, in place of
/// hand-editing the JSON.
struct KeyEntry {
    label: String,
    /// The provider file's name — what the glyph and validation results match
    /// against. Not read from `self.provider`, which can briefly resolve
    /// elsewhere (or to nothing) while the provider list reloads.
    name: String,
    file_id: Uuid,
    /// One-shot: request egui focus + summon the keyboard on the next render.
    /// Set on open and on a rejected key. After firing, the field's own
    /// interaction + focus lock retain focus — never re-requested per frame.
    needs_focus: bool,
    /// Focused-with-keyboard-up last frame — the keyboard re-summons only on
    /// a focus *regain* (edge-triggered), never per frame.
    was_focused: bool,
    /// The provider-file key this form session accounts for: the key at open,
    /// replaced by each submitted key. A reload landing a *different* real key
    /// (synced or hand-edited elsewhere) means the form's job was done —
    /// [`Chat::poll_key_connection`] closes it.
    seen_key: Option<String>,
    /// The key was submitted and we're awaiting `/models` validation.
    connecting: bool,
    /// A key has been submitted at least once from this entry — gates the
    /// "rejected" message so a fresh, never-submitted form shows no error even
    /// when the provider's saved key is what triggered the auto-prompt.
    attempted: bool,
}

/// (provider name, base_url) — what a cached listing is valid for.
type ModelsKey = (String, String);

/// How long a failed `/models` fetch waits before retrying.
const MODELS_RETRY: std::time::Duration = std::time::Duration::from_secs(15);

/// First-run guidance shown in an empty, agent-eligible chat. Each stage
/// advances on its own as the config validates in the background — no
/// confirmation step: pick a provider, paste a key, and the chat notices.
enum Onboard {
    /// No provider file yet — offer the roster to pick from.
    Choose,
    /// A provider is set; its `/models` listing hasn't come back yet.
    Connecting(String),
    /// The listing failed. `auth` is whether it looked like an auth error
    /// (bad/missing key) versus a connectivity failure, and `local` whether
    /// the endpoint is this machine — each needs different advice ("check
    /// your key" / "check your connection" / "is the server running?").
    Unreachable { label: String, auth: bool, local: bool },
    /// Reachable, but no model chosen (a local server we can't guess for).
    PickModel(String),
}

/// This user's latest per-chat model selection (config entries are ts-sorted
/// with the rest of the log; last wins).
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

/// The toolbar effort control's "let the model decide" value — clears any
/// per-chat override back to the provider's own default (it does not force
/// reasoning off; a true off is model-specific).
const EFFORT_AUTO: &str = "auto";

/// The effort levels the toolbar offers, portable across OpenAI-compat and
/// Anthropic. Exotic values (xhigh, max, none) stay a provider-file affair.
const EFFORT_LEVELS: &[&str] = &["low", "medium", "high"];

/// This user's latest per-chat reasoning-effort pick (config entries are
/// ts-sorted with the log; last wins). `EFFORT_AUTO` clears the override.
fn latest_effort(entries: &[Entry], username: &str) -> Option<String> {
    entries
        .iter()
        .filter(|e| e.msg.from == username)
        .filter_map(|e| e.msg.config.as_ref())
        .filter_map(|c| c.effort.clone())
        .next_back()
}

/// The toolbar effort control shows only where the model reasons: Anthropic
/// drives adaptive thinking natively, and OpenAI-compat reasoning models
/// take `reasoning_effort`. The compat case is a best-effort id heuristic
/// (like `is_chat_id`) — a miss only hides the control, since the provider
/// file's `effort` field still applies.
fn effort_available(provider: &settings::Provider) -> bool {
    provider.kind == "anthropic" || is_reasoning_model(&provider.model)
}

fn is_reasoning_model(id: &str) -> bool {
    let id = id.rsplit('/').next().unwrap_or(id).to_ascii_lowercase();
    id.starts_with("gpt-5")
        || id.starts_with("o1")
        || id.starts_with("o3")
        || id.starts_with("o4")
        || id.starts_with("gemini-2.5")
        || id.starts_with("gemini-3")
        // Dotted Grok (4.1, 4.3) takes reasoning_effort; bare/dated grok-4
        // errors on it, so it's deliberately excluded.
        || id.starts_with("grok-4.")
        || id.starts_with("grok-3-mini")
        || id.contains("reasoner")
        || id.contains("reasoning")
        || id.contains("thinking")
        || id.contains("qwq")
}

/// Whether a `/models` error reads like an auth failure (a missing or wrong
/// key) rather than an unreachable server — the case the key form fixes.
fn is_auth_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("401")
        || m.contains("403")
        || m.contains("unauthor")
        || m.contains("api key")
        || m.contains("api-key")
        || m.contains("api_key")
}

/// The model this user last ran `provider` with, mined from the append-only
/// config history — what makes switching back to a provider land on the
/// model you left it on, not the file default.
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

        // The connect step's key input, set up like the composer (files, link
        // resolver, file id) so the native iOS text bridge reaches it the same
        // way. `plaintext + mask` renders every byte as `*` — byte-length
        // preserving, so cursor/selection and the iOS text rects stay aligned
        // with the real (unmasked) buffer.
        let mut key_field = MdEdit::empty(ctx.clone());
        key_field.renderer.files = Arc::clone(&files);
        key_field.renderer.link_resolver =
            Box::new(FileCacheLinkResolver::new(Arc::clone(&files), id));
        key_field.renderer.plaintext = true;
        key_field.renderer.mask = true;
        // API keys are long; a wrapped key doesn't fit the fixed-height field.
        key_field.renderer.single_line = true;
        key_field.file_id = id;

        let unshared = !is_shared(&files.read().unwrap(), id);
        let harness = unshared.then(|| {
            let visible = resolve_visible(&entries, &HashMap::new(), None);
            harness::Harness::new(ctx.clone(), seed_history(&entries, &visible))
        });
        Self {
            id,
            hmac,
            entries,
            composer,
            account,
            seq: 0,
            initialized: false,
            base: bytes.to_vec(),
            key_field,
            key_field_rect: Rect::NOTHING,
            composer_rect: Rect::NOTHING,
            scroll_to_bottom: false,
            branch_choice: HashMap::new(),
            branch_anchor: None,
            hide_children_of: None,
            harness,
            streaming_label: {
                let mut label = MdLabel::new(ctx.clone());
                label.renderer.files = Arc::clone(&files);
                label.renderer.link_resolver = Box::new(FileCacheLinkResolver::new(files, id));
                label
            },
            unshared,
            core: core.clone(),
            // Providers/prompt load on a background thread; the first show()
            // kicks it. Files are never read on the UI thread.
            provider: None,
            providers: Vec::new(),
            system_prompt: None,
            config_loaded: false,
            config_rx: None,
            key_entry: None,
            connect_was_open: false,
            key_prompt_dismissed: None,
            chooser_open: false,
            // Left None so the first frame kicks the initial background load
            // (a cheap thread spawn, not the file read that blew the budget).
            last_frame: None,
            editing: None,
            draft_stash: None,
            pending_parent: None,
            models: None,
            glyphs: glyphs::Glyphs::default(),
            models_rx: None,
            models_attempt: None,
            models_err: None,
            ctx,
        }
    }

    /// The visible timeline under the current branch choices.
    fn visible(&self) -> Vec<VisibleRow> {
        resolve_visible(&self.entries, &self.branch_choice, self.hide_children_of)
    }

    /// Model context for the visible timeline.
    fn visible_seed(&self) -> Vec<backend::ChatMsg> {
        seed_history(&self.entries, &self.visible())
    }

    /// Begin editing a message: stash the in-progress draft, prefill the
    /// composer with the target's content. Send commits it as a sibling;
    /// Esc cancels and restores the draft.
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
        let system = self.system_prompt.clone();
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
    fn regenerate(&mut self, idx: usize) {
        let Some(parent) = parent_for_sibling(&self.entries, idx) else { return };
        self.hide_children_of = Some(parent);
        self.pending_parent = Some(parent);
        self.provider = self.resolve_provider();
        let system = self.system_prompt.clone();
        let seed = self.visible_seed();
        if let (Some(harness), Some(provider)) = (&mut self.harness, self.provider.clone()) {
            harness.reseed(seed);
            harness.retry(provider, system);
        }
        self.scroll_to_bottom = true;
    }

    /// The editor the native (iOS) text bridge should target: the masked key
    /// field while the connect step is open, otherwise the composer.
    pub fn focused_field(&mut self) -> &mut MdEdit {
        if self.key_entry.is_some() { &mut self.key_field } else { &mut self.composer }
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

    /// Reset the chat to its at-creation state: every entry goes — messages,
    /// forks, and error rows. The deletion propagates to this user's other
    /// devices through the merge (cleared entries are all in `base`, so
    /// nothing re-adds them). Unshared chats only — a multi-user clear wants
    /// a clear *event* in the log, not a mass deletion.
    ///
    /// The provider/model/effort picks live in the same log as config
    /// entries; clearing re-records the latest ones, so a clear doesn't
    /// silently reset the chat to the first provider file (alphabetical).
    fn clear_chat(&mut self) {
        let selection = latest_selection(&self.entries, &self.account.username);
        let effort = latest_effort(&self.entries, &self.account.username);
        if let Some(h) = &mut self.harness {
            h.reseed(Vec::new());
        }
        self.entries.clear();
        self.branch_choice.clear();
        self.branch_anchor = None;
        self.hide_children_of = None;
        self.editing = None;
        self.draft_stash = None;
        self.pending_parent = None;
        if let Some(sel) = selection {
            self.write_selection(sel.provider, sel.model);
        }
        if let Some(eff) = effort {
            self.write_effort(eff);
        }
        // With no re-recorded selection this falls back to the default
        // provider (or none) — same resolution as a brand-new chat.
        self.provider = self.resolve_provider();
        self.key_prompt_dismissed = None;
        self.seq += 1;
    }

    /// Append an agent-authored row: a reply, or (with `error`) a dim red
    /// note excluded from agent context. Attached to the message the turn
    /// answered, and the branch choice follows it — a regenerated reply
    /// supersedes its sibling on screen without deleting it.
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

        let editing = self.editing.take();
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
            let system = self.system_prompt.clone();
            if let (Some(harness), Some(provider)) = (&mut self.harness, self.provider.clone()) {
                harness.reseed(seed);
                harness.say(content, provider, system);
            }
        }
        self.composer.clear();
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
                    ..Default::default()
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

    /// The effort control shows only for reasoning models. Bare/dated grok-4
    /// (rejects reasoning_effort) and plain chat models stay hidden.
    #[test]
    fn reasoning_models_detected() {
        for id in [
            "gpt-5.5",
            "gpt-5.1",
            "o3-mini",
            "o4-mini",
            "gemini-2.5-pro",
            "gemini-3.5-flash",
            "grok-4.3",
            "grok-3-mini",
            "deepseek-reasoner",
            "qwen3-max-thinking",
            "openai/gpt-5.1",
        ] {
            assert!(is_reasoning_model(id), "{id} should be a reasoning model");
        }
        for id in [
            "gpt-4o",
            "grok-4",
            "grok-4-0709",
            "gemini-2.0-flash",
            "llama-3.3-70b-versatile",
            "gemma-4-31b",
        ] {
            assert!(!is_reasoning_model(id), "{id} should not be a reasoning model");
        }
    }
}
