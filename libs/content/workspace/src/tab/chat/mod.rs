//! Chat tab — markdown-rendered messages in a scrollable transcript, with a
//! multiline composer at the bottom. The document on disk is
//! newline-delimited JSON (`{from, content, ts}`), merged across devices via
//! `lb_rs::model::chat::Buffer::merge` (symmetric union over timestamp).

#[cfg(not(target_family = "wasm"))]
pub mod backend;
#[cfg(not(target_family = "wasm"))]
pub mod harness;
#[cfg(not(target_family = "wasm"))]
pub mod rig_backend;
#[cfg(not(target_family = "wasm"))]
pub mod settings;
#[cfg(not(target_family = "wasm"))]
pub mod tools;

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Local, Utc};
use egui::{
    Color32, CornerRadius, Galley, Id, Key, Modifiers, Rect, ScrollArea, Sense, Ui, pos2, vec2,
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

const MAX_WIDTH: f32 = 800.0;
const H_PAD: f32 = 12.0;
const V_PAD: f32 = 10.0;
const H_MARGIN: f32 = 12.0;
const ROW_GAP: f32 = 4.0;
const CORNER: u8 = 10;
/// Top padding before the first message. Larger on Android to clear the system
/// status bar / safe area, mirroring the markdown editor's `leading_precise`.
const TOP_MARGIN: f32 = if cfg!(target_os = "android") { 60.0 } else { 15.0 };
const BOTTOM_PAD: f32 = 15.0;
const COMPOSER_MAX_HEIGHT: f32 = 160.0;
/// Gap between the composer and the bottom of the editable area.
const COMPOSER_BOTTOM_GAP: f32 = 16.0;
/// Extra bottom padding on Android while the keyboard is down, to clear the
/// system nav bar (the egui panel isn't inset past it in that state).
const COMPOSER_NAV_CLEARANCE: f32 = 60.0;
/// Horizontal inset on each side of the composer bubble within its column. The
/// send button lives in the right inset, outside the bubble.
const SIDE_INSET: f32 = 48.0;
/// Breathing room between the transcript cutoff and the composer bubble —
/// the composer bar region is this much taller than the bubble itself.
const COMPOSER_BAR_PAD: f32 = 10.0;

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
struct RowPlan {
    bubble_rect: Rect,
    bubble_color: Color32,
    name_galley: Option<Arc<Galley>>,
    name_h: f32,
    ts_galley: Option<Arc<Galley>>,
    content_h: f32,
    /// Tool-record row: just this dim galley, no bubble/name/ts/markdown.
    tool_galley: Option<Arc<Galley>>,
}

/// Per-frame output of the chat tab, surfaced to the workspace/FFI. `text_`/
/// `selection_updated` keep the native iOS `UITextInput` in sync with the
/// composer (without them, iOS holds stale ranges and crashes on send).
pub struct ChatResponse {
    pub sent: bool,
    pub interaction_rect: Rect,
    pub text_updated: bool,
    pub selection_updated: bool,
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
}

pub struct Chat {
    pub id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub entries: Vec<Entry>,
    pub composer: MdEdit,
    pub account: Account,
    pub seq: usize,
    pub initialized: bool,
    /// Regular size class (desktop or full-screen tablet), where a hardware
    /// keyboard is plausible — gates the approval keyboard shortcuts. False on
    /// phones (and compact-width tablets).
    tablet_or_desktop: bool,
    /// Last server state our local edits sit on top of — the merge base. Held
    /// so `reload` does a real 3-way merge instead of an empty-base union.
    base: Vec<u8>,
    /// Composer region from the last frame; touches outside it scroll the
    /// transcript. Queried by the native gesture layer via `will_consume_touch`.
    composer_rect: Rect,
    /// Agent driver for 1-1 agent chats. `None` when the chat is shared or no
    /// API key is configured.
    #[cfg(not(target_family = "wasm"))]
    harness: Option<harness::Harness>,
    #[cfg(not(target_family = "wasm"))]
    core: lb_rs::blocking::Lb,
    /// Gear-menu state. The picker edits this chat's per-user model selection,
    /// persisted as a config entry in the chat on close.
    #[cfg(not(target_family = "wasm"))]
    settings_open: bool,
    #[cfg(not(target_family = "wasm"))]
    settings_dirty: bool,
    /// Name of the provider being edited / made active.
    #[cfg(not(target_family = "wasm"))]
    provider_buf: String,
    #[cfg(not(target_family = "wasm"))]
    api_key_buf: String,
    #[cfg(not(target_family = "wasm"))]
    model_buf: String,
    /// Loaded `/chat.json` provider registry (keys, base URLs). Global and
    /// device-local; the per-chat selection references a provider by name.
    #[cfg(not(target_family = "wasm"))]
    settings: settings::ChatSettings,
    ctx: egui::Context,
}

/// `user`'s latest model selection in this chat — the most recent config entry
/// they authored. `None` when they haven't chosen one (use the global default).
#[cfg(not(target_family = "wasm"))]
fn chat_model_selection(
    entries: &[Entry], user: &str,
) -> Option<lb_rs::model::chat::ModelSelection> {
    entries
        .iter()
        .filter(|e| e.msg.from == user)
        .filter_map(|e| Some((e.msg.ts, e.msg.config.as_ref()?.model.clone()?)))
        .max_by_key(|(ts, _)| *ts)
        .map(|(_, m)| m)
}

impl Chat {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bytes: &[u8], id: Uuid, hmac: Option<DocumentHmac>, account: Account, ctx: egui::Context,
        files: Arc<RwLock<FileCache>>, core: &lb_rs::blocking::Lb, tablet_or_desktop: bool,
    ) -> Self {
        let entries: Vec<Entry> = Buffer::new(bytes)
            .messages
            .into_iter()
            .map(|m| Entry::new(m, &ctx, Arc::clone(&files), id))
            .collect();

        #[cfg(target_family = "wasm")]
        let _ = core;
        let mut composer = MdEdit::empty(ctx.clone());
        composer.renderer.files = Arc::clone(&files);
        composer.renderer.link_resolver =
            Box::new(FileCacheLinkResolver::new(Arc::clone(&files), id));
        composer.file_id = id;

        #[cfg(not(target_family = "wasm"))]
        let settings = settings::ChatSettings::load(core);
        // This user's per-chat selection (latest config entry they authored),
        // else the global default from `/chat.json`.
        #[cfg(not(target_family = "wasm"))]
        let selection = chat_model_selection(&entries, &account.username);
        #[cfg(not(target_family = "wasm"))]
        let provider = selection
            .as_ref()
            .map(|s| s.provider.clone())
            .unwrap_or_else(|| settings.provider());
        #[cfg(not(target_family = "wasm"))]
        let model = selection
            .as_ref()
            .map(|s| s.model.clone())
            .unwrap_or_else(|| settings.edit_model());
        #[allow(unused_mut)]
        let mut chat = Self {
            id,
            hmac,
            entries,
            composer,
            account,
            seq: 0,
            initialized: false,
            tablet_or_desktop,
            base: bytes.to_vec(),
            composer_rect: Rect::NOTHING,
            #[cfg(not(target_family = "wasm"))]
            harness: None,
            #[cfg(not(target_family = "wasm"))]
            core: core.clone(),
            #[cfg(not(target_family = "wasm"))]
            settings_open: false,
            #[cfg(not(target_family = "wasm"))]
            settings_dirty: false,
            #[cfg(not(target_family = "wasm"))]
            api_key_buf: settings.stored_api_key(&provider),
            #[cfg(not(target_family = "wasm"))]
            provider_buf: provider,
            #[cfg(not(target_family = "wasm"))]
            model_buf: model,
            #[cfg(not(target_family = "wasm"))]
            settings,
            ctx,
        };
        #[cfg(not(target_family = "wasm"))]
        chat.rebuild_harness();
        chat
    }

    /// (Re)spawn the agent driver from current settings and transcript. The
    /// agent only joins 1-1 chats: an unshared chat file is a conversation
    /// between the user and their own agent. Shared chats get no harness
    /// until group invocation (@mentions) lands.
    #[cfg(not(target_family = "wasm"))]
    fn rebuild_harness(&mut self) {
        let files = Arc::clone(&self.composer.renderer.files);
        self.harness = if is_shared(&files.read().unwrap(), self.id) {
            None
        } else {
            let history = self
                .entries
                .iter()
                .filter(|e| !e.msg.error && e.msg.config.is_none())
                .map(|e| {
                    if let Some(rec) = &e.msg.tool {
                        harness::SeedMsg::Tool {
                            id: e
                                .msg
                                .id
                                .map(|u| u.to_string())
                                .unwrap_or_else(|| e.msg.ts.to_string()),
                            name: rec.name.clone(),
                            args: rec.args.clone(),
                            result: rec.result.clone(),
                        }
                    } else if e.msg.agent {
                        harness::SeedMsg::Agent(e.msg.content.clone())
                    } else {
                        harness::SeedMsg::User(e.msg.content.clone())
                    }
                })
                .collect();
            harness::Harness::new(
                self.core.clone(),
                self.ctx.clone(),
                self.account.username.clone(),
                history,
                self.current_settings(),
            )
        };
    }

    /// Append an agent-authored message: a reply bubble, or (with `tool`) a
    /// dim tool-record row whose args + result reseed agent context on open.
    #[cfg(not(target_family = "wasm"))]
    fn push_agent_message(
        &mut self, content: String, tool: Option<lb_rs::model::chat::ToolRecord>,
        usage: Option<lb_rs::model::chat::Usage>,
    ) {
        let mut msg = Message::new(self.account.username.clone(), content, Utc::now().timestamp());
        msg.agent = true;
        msg.tool = tool;
        msg.usage = usage;
        self.entries.push(Entry::new(
            msg,
            &self.ctx,
            Arc::clone(&self.composer.renderer.files),
            self.id,
        ));
        self.seq += 1;
    }

    #[cfg(not(target_family = "wasm"))]
    fn push_error_message(&mut self, e: String) {
        let idx = self.entries.len();
        self.push_agent_message(format!("error: {e}"), None, None);
        self.entries[idx].msg.error = true;
    }

    #[cfg(not(target_family = "wasm"))]
    fn push_tool_record(&mut self, call: harness::ToolCall, result: String, denied: bool) {
        let summary = if denied {
            format!("{} {} (denied)", call.name, call.detail)
        } else {
            format!("{} {}", call.name, call.detail)
        };
        let record = lb_rs::model::chat::ToolRecord { name: call.name, args: call.args, result };
        self.push_agent_message(summary, Some(record), None);
    }

    /// Settings as currently edited: the loaded providers with the editor's
    /// key/model folded into the active one (empty buffers are unset fields).
    #[cfg(not(target_family = "wasm"))]
    fn current_settings(&self) -> settings::ChatSettings {
        let opt = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_string());
        self.settings.with_active_edits(
            &self.provider_buf,
            opt(&self.api_key_buf),
            opt(&self.model_buf),
        )
    }

    /// Reload `/chat.json` (the provider registry) and reseed the editor
    /// buffers from this chat's selection, falling back to the global default.
    #[cfg(not(target_family = "wasm"))]
    fn reload_settings(&mut self) {
        self.settings = settings::ChatSettings::load(&self.core);
        let selection = chat_model_selection(&self.entries, &self.account.username);
        self.provider_buf = selection
            .as_ref()
            .map(|s| s.provider.clone())
            .unwrap_or_else(|| self.settings.provider());
        self.model_buf = selection
            .map(|s| s.model)
            .unwrap_or_else(|| self.settings.edit_model());
        self.api_key_buf = self.settings.stored_api_key(&self.provider_buf);
    }

    /// Append this chat's current model selection as a config entry (this
    /// user's, per-chat) when it differs from what's already recorded.
    #[cfg(not(target_family = "wasm"))]
    fn persist_chat_selection(&mut self) {
        let provider = self.provider_buf.trim().to_string();
        if provider.is_empty() {
            return;
        }
        let sel = lb_rs::model::chat::ModelSelection {
            provider,
            model: self.model_buf.trim().to_string(),
        };
        if chat_model_selection(&self.entries, &self.account.username).as_ref() == Some(&sel) {
            return;
        }
        let msg = Message::config_entry(
            self.account.username.clone(),
            Utc::now().timestamp(),
            lb_rs::model::chat::ChatConfig { model: Some(sel) },
        );
        self.entries.push(Entry::new(
            msg,
            &self.ctx,
            Arc::clone(&self.composer.renderer.files),
            self.id,
        ));
        self.seq += 1;
    }

    /// Whether a touch at `pos` should scroll the transcript rather than reach
    /// the composer — true everywhere outside the composer region. Called by
    /// the native (Android/iOS) gesture layer.
    pub fn will_consume_touch(&self, pos: egui::Pos2) -> bool {
        !self.composer_rect.contains(pos)
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
    }

    /// Mark saved after a successful write — called from the workspace save
    /// completion path.
    pub fn saved(&mut self, hmac: DocumentHmac) {
        self.hmac = Some(hmac);
    }

    /// Append the trimmed composer text as a message and clear the composer.
    /// Returns whether a (non-empty) message was sent.
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
        let msg = Message::new(self.account.username.clone(), content, Utc::now().timestamp());
        #[cfg(not(target_family = "wasm"))]
        if let Some(harness) = &mut self.harness {
            harness.say(msg.content.clone());
        }
        self.entries.push(Entry::new(
            msg,
            &self.ctx,
            Arc::clone(&self.composer.renderer.files),
            self.id,
        ));
        self.composer.clear();
        self.seq += 1;
        ui.ctx().memory_mut(|m| m.request_focus(composer_id));
        true
    }

    /// Renders the transcript + composer, returning the per-frame [`ChatResponse`].
    pub fn show(&mut self, ui: &mut Ui) -> ChatResponse {
        let theme = ui.ctx().get_lb_theme();
        let available_width = ui.available_width();
        let col_width = available_width.min(MAX_WIDTH);
        let max_bubble_content_w = (col_width * 0.72 - H_PAD * 2.0).max(120.0);
        let text_color = theme.neutral_fg();
        let secondary_color = theme
            .neutral_fg_secondary()
            .lerp_to_gamma(theme.neutral_fg(), 0.5); // fg secondary hard to read on colored bg

        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        let full_rect = ui.available_rect_before_wrap();

        // Fold in agent replies before measuring so they render this frame.
        // Replies are persisted transcript messages like any other; the rest
        // of the harness state (pending approvals, busy, errors) is overlay-
        // only and painted above the composer below.
        let mut agent_sent = false;
        #[cfg(not(target_family = "wasm"))]
        {
            let updates = self.harness.as_mut().map(|h| h.pump()).unwrap_or_default();
            for update in updates {
                match update {
                    harness::HarnessUpdate::Reply { text, usage } => {
                        self.push_agent_message(text, None, Some(usage))
                    }
                    harness::HarnessUpdate::ToolDone { call, result } => {
                        self.push_tool_record(call, result, false)
                    }
                    harness::HarnessUpdate::Error(e) => self.push_error_message(e),
                }
                agent_sent = true;
            }
        }

        // The egui panel is already inset past the keyboard (when up); on
        // Android it is NOT inset past the nav bar (when the keyboard is down),
        // so add nav-bar clearance only in that state.
        let keyboard_up = ui
            .memory(|m| m.data.get_temp::<f32>(Id::new("ws_keyboard_height")))
            .unwrap_or(0.0)
            > 0.0;
        let composer_bottom_inset = if cfg!(target_os = "android") && !keyboard_up {
            COMPOSER_NAV_CLEARANCE
        } else {
            COMPOSER_BOTTOM_GAP
        };

        // Live activity ("thinking…", the running tool, a pending approval)
        // renders like an incoming message at the transcript's tail, not as
        // an overlay. `approval` adds Approve/Deny buttons to the row.
        #[cfg(not(target_family = "wasm"))]
        let (agent_status, approval): (Option<String>, bool) = self
            .harness
            .as_ref()
            .map(|h| {
                if let Some(call) = &h.pending {
                    (Some(format!("agent wants {} {}", call.name, call.detail)), true)
                } else if let Some(call) = &h.running {
                    (Some(format!("{} {}…", call.name, call.detail)), false)
                } else if h.busy {
                    (Some("thinking…".to_string()), false)
                } else {
                    (None, false)
                }
            })
            .unwrap_or((None, false));
        #[cfg(target_family = "wasm")]
        let (agent_status, approval): (Option<String>, bool) = (None, false);

        // Cmd+A / Cmd+D decide the pending call from the keyboard; consumed
        // before the composer handles input so it never sees the keystroke
        // (Cmd+A means select-all only while no approval is pending). Offered on
        // desktop and regular-size-class tablets (iPad), where a hardware
        // keyboard is plausible; phones have no modifier keys and use the
        // buttons below.
        let has_keyboard =
            !cfg!(any(target_os = "ios", target_os = "android")) || self.tablet_or_desktop;
        let approve_sc = egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::A);
        let deny_sc = egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::D);
        let mut approve =
            has_keyboard && approval && ui.input_mut(|i| i.consume_shortcut(&approve_sc));
        let mut deny = has_keyboard && approval && ui.input_mut(|i| i.consume_shortcut(&deny_sc));

        let composer_id = Id::new("chat_composer");
        // Focus the composer when first shown or whenever nothing else has
        // focus — the markdown editor's policy. Without the standing rule,
        // any tap that clears egui focus (iOS taps route through the native
        // overlay) leaves the composer deaf to typed text.
        if !self.initialized || ui.memory(|m| m.focused().is_none()) {
            ui.memory_mut(|m| m.request_focus(composer_id));
            self.initialized = true;
        }

        // Cmd/Ctrl+Enter → send. Consume before handle_input so the composer
        // doesn't translate the Enter into a Newline.
        let composer_focused = ui.memory(|m| m.has_focus(composer_id));
        if composer_focused {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    composer_id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: false,
                    },
                )
            });
        }
        let send_requested = composer_focused
            && ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));

        // Composer input phase — drain workspace-origin events (native iOS
        // text input arrives this way), then keyboard / completions / internal.
        let workspace_events = self.composer.drain_workspace_events(ui.ctx());
        self.composer.event.internal_events.extend(workspace_events);
        let prior_selection = self.composer.renderer.buffer.current.selection;
        let buf_resp = self.composer.handle_input(ui.ctx(), composer_id);
        let mut text_updated = buf_resp.text_updated;

        // Measure at the exact render width so the composer bubble grows
        // same-frame. The re-parse inside `show` below hits the layout cache.
        // `SIDE_INSET` and `H_PAD` mirror the h_inset / shrink geometry below.
        let composer_inner_w = (col_width - 2.0 * SIDE_INSET - 2.0 * H_PAD).max(0.0);
        let measured_h = self.composer.measure_height(composer_inner_w);

        // Autogrow with a max cap, no lower floor — a lower floor makes a
        // single-line composer bottom-heavy (content is top-anchored).
        let composer_height = (measured_h + V_PAD * 2.0).min(COMPOSER_MAX_HEIGHT);
        let transcript_rect = Rect::from_min_max(
            full_rect.min,
            pos2(
                full_rect.max.x,
                full_rect.max.y - composer_height - composer_bottom_inset - COMPOSER_BAR_PAD,
            ),
        );
        let mut text_areas = Vec::new();
        // Set by the row context menu (copy/delete); handled after the scroll
        // region so entry borrows are released.
        let mut delete_idx: Option<usize> = None;

        ui.scope_builder(egui::UiBuilder::new().max_rect(transcript_rect), |ui| {
            ui.set_clip_rect(transcript_rect.intersect(ui.clip_rect()));
            ScrollArea::vertical()
                .id_salt("chat_messages")
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let origin = ui.cursor().min;
                    let col_pad = (available_width - col_width) / 2.0;
                    let col_left = origin.x + col_pad;
                    let col_right = col_left + col_width;

                    // pass 1: measure each message and compute its bubble rect
                    // against a running y. This populates each label's layout
                    // cache so pass 2's paint is near-free.
                    let n = self.entries.len();
                    let mut plans: Vec<RowPlan> = Vec::with_capacity(n);
                    let mut y = origin.y + TOP_MARGIN;
                    let run_key = |e: &Entry| {
                        (e.msg.from.clone(), e.msg.agent, e.msg.tool.is_some() || e.msg.error)
                    };
                    for i in 0..n {
                        // Config entries carry settings, not chat content — no
                        // row. Keep `plans` aligned 1:1 with `entries`.
                        if self.entries[i].msg.config.is_some() {
                            plans.push(RowPlan {
                                bubble_rect: Rect::from_min_size(pos2(col_left, y), vec2(0.0, 0.0)),
                                bubble_color: Color32::TRANSPARENT,
                                name_galley: None,
                                name_h: 0.0,
                                ts_galley: None,
                                content_h: 0.0,
                                tool_galley: None,
                            });
                            continue;
                        }
                        let (from, agent, tool) = run_key(&self.entries[i]);
                        let ts = self.entries[i].msg.ts;

                        if tool {
                            let color = if self.entries[i].msg.error {
                                theme.fg().get_color(Palette::Red)
                            } else {
                                secondary_color
                            };
                            let font = egui::TextStyle::Monospace.resolve(ui.style());
                            let galley = ui.fonts(|f| {
                                f.layout(
                                    self.entries[i].msg.content.clone(),
                                    font,
                                    color,
                                    col_width - 2.0 * H_MARGIN,
                                )
                            });
                            let h = galley.rect.height();
                            plans.push(RowPlan {
                                bubble_rect: Rect::from_min_size(
                                    pos2(col_left + H_MARGIN, y),
                                    vec2(col_width - 2.0 * H_MARGIN, h),
                                ),
                                bubble_color: Color32::TRANSPARENT,
                                name_galley: None,
                                name_h: 0.0,
                                ts_galley: None,
                                content_h: h,
                                tool_galley: Some(galley),
                            });
                            y += h + ROW_GAP;
                            continue;
                        }

                        // Agent messages render like another participant's —
                        // left-aligned, named — even though `from` is mine.
                        let is_mine = from == self.account.username && !agent;
                        let first_in_run =
                            i == 0 || run_key(&self.entries[i - 1]) != (from.clone(), agent, tool);
                        let last_in_run = i + 1 >= n
                            || run_key(&self.entries[i + 1]) != (from.clone(), agent, tool);

                        let name_galley = if !is_mine && first_in_run {
                            let name_color = theme.fg().get_color(username_color(&from));
                            let name = if agent { format!("{from} (agent)") } else { from.clone() };
                            Some(ui.fonts(|f| {
                                f.layout_no_wrap(name, egui::FontId::proportional(11.0), name_color)
                            }))
                        } else {
                            None
                        };

                        let ts_galley = if last_in_run {
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
                        let content_h =
                            entry.label.height(&entry.msg.content, max_bubble_content_w);

                        let name_h = name_galley.as_ref().map_or(0.0, |g| g.rect.height());
                        let ts_h = ts_galley.as_ref().map_or(0.0, |g| g.rect.height());

                        let bubble_w = max_bubble_content_w + H_PAD * 2.0;
                        let bubble_h = name_h + content_h + ts_h + V_PAD * 2.0;
                        let bubble_x = if is_mine {
                            col_right - H_MARGIN - bubble_w
                        } else {
                            col_left + H_MARGIN
                        };
                        let bubble_rect =
                            Rect::from_min_size(pos2(bubble_x, y), vec2(bubble_w, bubble_h));

                        let bubble_color = if is_mine {
                            theme.bg().get_color(Palette::Blue)
                        } else {
                            theme.neutral_bg_secondary()
                        };

                        plans.push(RowPlan {
                            bubble_rect,
                            bubble_color,
                            name_galley,
                            name_h,
                            ts_galley,
                            content_h,
                            tool_galley: None,
                        });
                        y += bubble_h + ROW_GAP;
                    }

                    // Live agent activity as a trailing pseudo-message; an
                    // approval request gets Approve/Deny buttons below it.
                    let button = |ui: &Ui, label: String| {
                        let font = egui::TextStyle::Body.resolve(ui.style());
                        ui.fonts(|f| f.layout_no_wrap(label, font, text_color))
                    };
                    let status_row = agent_status.as_ref().map(|status| {
                        let font = egui::TextStyle::Body.resolve(ui.style());
                        let galley = ui.fonts(|f| {
                            f.layout(
                                status.clone(),
                                font,
                                secondary_color,
                                col_width - 2.0 * H_MARGIN,
                            )
                        });
                        let pos = pos2(col_left + H_MARGIN, y);
                        y += galley.rect.height() + ROW_GAP;

                        let buttons = approval.then(|| {
                            // Show the shortcut hint only where the shortcut exists.
                            let hint = |label: &str, sc| {
                                if has_keyboard {
                                    format!("{label} {}", ui.ctx().format_shortcut(sc))
                                } else {
                                    label.to_string()
                                }
                            };
                            let approve_galley = button(ui, hint("Approve", &approve_sc));
                            let deny_galley = button(ui, hint("Deny", &deny_sc));
                            let h = approve_galley.rect.height() + 10.0;
                            let mut x = col_left + H_MARGIN;
                            let approve_rect = Rect::from_min_size(
                                pos2(x, y),
                                vec2(approve_galley.rect.width() + 20.0, h),
                            );
                            x = approve_rect.max.x + 8.0;
                            let deny_rect = Rect::from_min_size(
                                pos2(x, y),
                                vec2(deny_galley.rect.width() + 20.0, h),
                            );
                            y += h + ROW_GAP;
                            (approve_rect, approve_galley, deny_rect, deny_galley)
                        });
                        (pos, galley, buttons)
                    });

                    // Allocate total footprint so ScrollArea sees the right
                    // height (stick-to-bottom depends on this).
                    let total_h = (y - origin.y) + BOTTOM_PAD;
                    let _ = ui.allocate_exact_size(vec2(available_width, total_h), Sense::hover());

                    // pass 2: paint absolute. No egui layout calls.
                    for (i, plan) in plans.into_iter().enumerate() {
                        if self.entries[i].msg.config.is_some() {
                            continue;
                        }
                        // Right-click / long-press menu. Allocated before the
                        // label's link fragments so links stay on top.
                        let row_resp =
                            ui.interact(plan.bubble_rect, Id::new(("chat_row", i)), Sense::click());
                        row_resp.context_menu(|ui| {
                            let msg = &self.entries[i].msg;
                            if ui.button("Copy").clicked() {
                                ui.ctx().copy_text(msg.content.clone());
                                ui.close();
                            }
                            if let Some(record) = &msg.tool {
                                // the full record (args + result the model
                                // saw), not just the summary line
                                if ui.button("Copy record").clicked() {
                                    if let Ok(json) = serde_json::to_string_pretty(record) {
                                        ui.ctx().copy_text(json);
                                    }
                                    ui.close();
                                }
                            }
                            if ui.button("Delete").clicked() {
                                delete_idx = Some(i);
                                ui.close();
                            }
                        });

                        if let Some(tg) = plan.tool_galley {
                            ui.painter()
                                .galley(plan.bubble_rect.min, tg, secondary_color);
                            continue;
                        }

                        ui.painter().rect_filled(
                            plan.bubble_rect,
                            CornerRadius::same(CORNER),
                            plan.bubble_color,
                        );

                        let mut text_y = plan.bubble_rect.min.y + V_PAD;
                        if let Some(ng) = plan.name_galley {
                            ui.painter().galley(
                                pos2(plan.bubble_rect.min.x + H_PAD, text_y),
                                ng,
                                text_color,
                            );
                            text_y += plan.name_h;
                        }

                        let content_top = pos2(plan.bubble_rect.min.x + H_PAD, text_y);
                        let entry = &mut self.entries[i];
                        let (areas, _) = entry.label.paint_at(
                            ui,
                            &entry.msg.content,
                            content_top,
                            max_bubble_content_w,
                        );
                        text_areas.extend(areas);
                        text_y += plan.content_h;

                        if let Some(tg) = plan.ts_galley {
                            let tg_w = tg.rect.width();
                            ui.painter().galley(
                                pos2(plan.bubble_rect.max.x - H_PAD - tg_w, text_y),
                                tg,
                                secondary_color,
                            );
                        }
                    }

                    if let Some((pos, galley, buttons)) = status_row {
                        ui.painter().galley(pos, galley, secondary_color);
                        if let Some((approve_rect, approve_galley, deny_rect, deny_galley)) =
                            buttons
                        {
                            let blue = theme.bg().get_color(Palette::Blue);
                            let painter = ui.painter();
                            painter.rect_filled(approve_rect, CornerRadius::same(CORNER), blue);
                            painter.galley(
                                approve_rect.min
                                    + (approve_rect.size() - approve_galley.size()) / 2.0,
                                approve_galley,
                                text_color,
                            );
                            painter.rect_filled(
                                deny_rect,
                                CornerRadius::same(CORNER),
                                theme.neutral_bg_secondary(),
                            );
                            painter.galley(
                                deny_rect.min + (deny_rect.size() - deny_galley.size()) / 2.0,
                                deny_galley,
                                text_color,
                            );
                            approve |= ui
                                .interact(approve_rect, Id::new("chat_approve"), Sense::click())
                                .clicked();
                            deny |= ui
                                .interact(deny_rect, Id::new("chat_deny"), Sense::click())
                                .clicked();
                        }
                    }
                });
        });

        // Deleting a message is context editing: the transcript is the
        // agent's memory, so the removal also reseeds the harness (when
        // idle — a rebuild mid-turn would kill the work in flight).
        if let Some(i) = delete_idx {
            self.entries.remove(i);
            self.seq += 1;
            agent_sent = true;
            #[cfg(not(target_family = "wasm"))]
            if self
                .harness
                .as_ref()
                .is_some_and(|h| !h.busy && h.pending.is_none() && h.running.is_none())
            {
                self.rebuild_harness();
            }
        }

        #[cfg(not(target_family = "wasm"))]
        if approve || deny {
            if let Some(harness) = &mut self.harness {
                let mut denied = None;
                if approve {
                    harness.approve();
                } else {
                    denied = harness.deny();
                }
                if let Some(call) = denied {
                    self.push_tool_record(call, harness::DENIED_RESULT.to_string(), true);
                    agent_sent = true;
                }
            }
        }
        #[cfg(target_family = "wasm")]
        let _ = (approve, deny);

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

        // Opaque composer bar painted after the transcript's text callback so
        // messages scroll cleanly under it, with a hairline top border.
        let bar_rect =
            Rect::from_min_max(pos2(full_rect.min.x, transcript_rect.max.y), full_rect.max);
        ui.painter()
            .rect_filled(bar_rect, CornerRadius::ZERO, ui.visuals().extreme_bg_color);
        ui.painter().hline(
            bar_rect.min.x..=bar_rect.max.x,
            bar_rect.min.y,
            egui::Stroke::new(1.0, theme.neutral_bg_secondary()),
        );

        // Composer bubble + body.
        let composer_rect = Rect::from_min_max(
            pos2(full_rect.min.x, full_rect.max.y - composer_height - composer_bottom_inset),
            full_rect.max,
        );
        self.composer_rect = composer_rect;
        let col_pad = (available_width - col_width) / 2.0;
        let h_inset = col_pad + SIDE_INSET;
        let bubble_rect = Rect::from_min_max(
            pos2(composer_rect.min.x + h_inset, composer_rect.min.y),
            pos2(composer_rect.max.x - h_inset, composer_rect.max.y - composer_bottom_inset),
        );
        ui.painter().rect_filled(
            bubble_rect,
            CornerRadius::same(CORNER),
            theme.neutral_bg_secondary(),
        );

        // Composer draw. Submits its own text callback internally.
        let inner_rect = bubble_rect.shrink2(vec2(H_PAD, V_PAD));
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

        // Send button — shown when there's text. It lives in the right inset
        // OUTSIDE the bubble, so `composer.show` can't occlude it and the iOS
        // overlay (which covers exactly `inner_rect`) can't swallow its taps.
        // Sized to the composer's single-line starting height, bottom-aligned.
        let non_empty = !self.composer.renderer.buffer.current.text.trim().is_empty();
        let mut send_clicked = false;
        if non_empty {
            let d = (self.composer.row_height() + 2.0 * V_PAD).min(SIDE_INSET);
            let center = pos2(bubble_rect.max.x + SIDE_INSET / 2.0, bubble_rect.max.y - d / 2.0);
            let send_rect = Rect::from_center_size(center, vec2(d, d));
            let resp = ui.interact(send_rect, Id::new("chat_send"), Sense::click());

            let painter = ui.painter();
            painter.circle_filled(center, d / 2.0, theme.bg().get_color(Palette::Blue));
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

        let sent = ((send_requested || send_clicked) && self.submit(ui, composer_id)) || agent_sent;
        text_updated |= sent;

        #[cfg(not(target_family = "wasm"))]
        self.show_settings_menu(ui, full_rect);

        // Popups land last so they composite over composer + transcript.
        self.composer.show_completions(ui);

        let selection_updated = prior_selection
            != self
                .composer
                .in_progress_selection
                .unwrap_or(self.composer.renderer.buffer.current.selection);

        ChatResponse { sent, interaction_rect: inner_rect, text_updated, selection_updated }
    }
}

impl Chat {
    /// Gear button in the top-right corner toggling a panel that edits
    /// `/chat.json`. Buffers write through on every change; the harness is
    /// rebuilt with the new settings when the panel closes.
    #[cfg(not(target_family = "wasm"))]
    fn show_settings_menu(&mut self, ui: &mut Ui, full_rect: Rect) {
        let theme = ui.ctx().get_lb_theme();

        let d = 28.0;
        let center = pos2(full_rect.max.x - 8.0 - d / 2.0, full_rect.min.y + 8.0 + d / 2.0);
        let gear_rect = Rect::from_center_size(center, vec2(d, d));
        let resp = ui.interact(gear_rect, Id::new("chat_settings_gear"), Sense::click());
        let gear_color = if self.settings_open || resp.hovered() {
            ui.visuals().text_color()
        } else {
            ui.visuals().weak_text_color()
        };
        let icon = ui.fonts(|f| {
            f.layout_no_wrap(
                Icon::SETTINGS.icon.to_string(),
                egui::FontId::monospace(d * 0.65),
                gear_color,
            )
        });
        ui.painter()
            .galley(center - icon.size() / 2.0, icon, gear_color);
        if resp.clicked() {
            self.settings_open = !self.settings_open;
            // Reload on open so edits merge onto current on-disk state (e.g.
            // synced from another device) rather than a stale snapshot.
            if self.settings_open {
                self.reload_settings();
            }
        }

        if !self.settings_open {
            if self.settings_dirty {
                self.settings_dirty = false;
                self.persist_chat_selection();
                self.rebuild_harness();
            }
            return;
        }

        let panel_w = 300.0;
        let mut changed = false;
        let mut provider_names = self.settings.provider_names();
        // The active provider is always selectable, even mid-add.
        if !provider_names.contains(&self.provider_buf) {
            provider_names.push(self.provider_buf.clone());
        }
        let area_resp = egui::Area::new(Id::new("chat_settings_panel"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2(full_rect.max.x - panel_w - 8.0, gear_rect.max.y + 4.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(panel_w);

                    // Provider picker over the configured providers. Switching
                    // reseeds the key/model fields and respawns the driver, so
                    // the new provider's model list is fetched right away.
                    let mut picked: Option<String> = None;
                    egui::ComboBox::from_id_salt("chat_settings_provider")
                        .width(panel_w - 16.0)
                        .selected_text(self.provider_buf.clone())
                        .show_ui(ui, |ui| {
                            for name in &provider_names {
                                if ui
                                    .selectable_label(&self.provider_buf == name, name)
                                    .clicked()
                                {
                                    picked = Some(name.clone());
                                }
                            }
                        });
                    if let Some(name) = picked {
                        if name != self.provider_buf {
                            self.api_key_buf = self.settings.stored_api_key(&name);
                            self.model_buf.clear();
                            self.provider_buf = name;
                            self.rebuild_harness();
                            changed = true;
                        }
                    }
                    ui.add_space(8.0);

                    // Model picker from the driver's live `/models` fetch; until
                    // that lands (or if listing isn't available), fall back to
                    // the provider's configured models, then a free-text id.
                    // `/chat.json` always stores the canonical id.
                    let live = self
                        .harness
                        .as_ref()
                        .map(|h| h.models.clone())
                        .unwrap_or_default();
                    let models = if live.is_empty() {
                        self.settings
                            .models_for(&self.provider_buf)
                            .into_iter()
                            .map(|id| harness::ModelChoice { label: id.clone(), id })
                            .collect::<Vec<_>>()
                    } else {
                        live
                    };
                    if models.is_empty() {
                        changed |= ui
                            .add(
                                egui::TextEdit::singleline(&mut self.model_buf)
                                    .hint_text(harness::DEFAULT_MODEL)
                                    .desired_width(f32::INFINITY),
                            )
                            .changed();
                    } else {
                        let label_for = |id: &str| {
                            models
                                .iter()
                                .find(|m| m.id == id)
                                .map(|m| m.label.clone())
                                .unwrap_or_else(|| id.to_string())
                        };
                        let default_label =
                            format!("Default ({})", label_for(harness::DEFAULT_MODEL));
                        let selected = if self.model_buf.trim().is_empty() {
                            default_label.clone()
                        } else {
                            label_for(self.model_buf.trim())
                        };
                        egui::ComboBox::from_id_salt("chat_settings_model")
                            .width(panel_w - 16.0)
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(
                                        self.model_buf.trim().is_empty(),
                                        default_label,
                                    )
                                    .clicked()
                                {
                                    self.model_buf.clear();
                                    changed = true;
                                }
                                for m in &models {
                                    if ui
                                        .selectable_label(self.model_buf.trim() == m.id, &m.label)
                                        .clicked()
                                    {
                                        self.model_buf = m.id.clone();
                                        changed = true;
                                    }
                                }
                            });
                    }
                    ui.add_space(8.0);

                    // Chat-lifetime usage: fold of the per-reply stamps in
                    // the transcript, so it survives restarts and syncs.
                    let mut turns = 0u64;
                    let mut total = lb_rs::model::chat::Usage::default();
                    for e in &self.entries {
                        if let Some(u) = &e.msg.usage {
                            turns += 1;
                            total.input += u.input;
                            total.output += u.output;
                            total.cache_read += u.cache_read;
                            total.cache_write += u.cache_write;
                        }
                    }
                    ui.colored_label(
                        theme.neutral_fg_secondary(),
                        format!("chat ({turns} turns): {}", fmt_usage(&total)),
                    );
                });
            });

        if changed {
            // The selection is persisted to the chat (per-user, per-chat) on
            // close; `/chat.json` (the provider registry) is edited out-of-band.
            self.settings_dirty = true;
        }

        // Dismiss on a click anywhere outside the panel and gear — unless a
        // popup (the model combo) is open, since its options render outside
        // the panel rect and selecting one shouldn't close settings.
        let popup_open = egui::Popup::is_any_open(ui.ctx());
        let clicked_outside = ui.input(|i| {
            i.pointer.any_pressed()
                && i.pointer.interact_pos().is_some_and(|pos| {
                    !area_resp.response.rect.contains(pos) && !gear_rect.contains(pos)
                })
        });
        if clicked_outside && !popup_open {
            self.settings_open = false;
        }
    }
}

/// One line of raw token counts. `in` is the uncached portion only; a warm
/// cache shows as `cached` carrying the bulk of the prompt.
#[cfg(not(target_family = "wasm"))]
fn fmt_usage(u: &lb_rs::model::chat::Usage) -> String {
    format!(
        "{} in · {} out · {} cache read · {} cache write",
        u.input, u.output, u.cache_read, u.cache_write
    )
}

/// Whether the file or any ancestor is shared — i.e. other lockbook users
/// can see this chat.
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
