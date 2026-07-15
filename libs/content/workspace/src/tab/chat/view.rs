//! Rendering and hit-testing for the chat tab: the transcript's two-pass
//! measure→paint layout, the composer and toolbar, the onboarding / connect /
//! empty-state canvas, and touch geometry. All pixel-pushing, no data logic —
//! the reviewable state lives in the parent module and `config`.

use super::*;
use crate::show::InputStateExt as _;

/// A vertically-stacked, horizontally-centered column painted as one block in
/// a rect — the shared skeleton of the onboarding chooser, connect step,
/// status line, and empty-state marker, which otherwise centered themselves by
/// hand. The total height is *derived* from the pushed items rather than summed
/// per site; getting that sum wrong (a stray or missing gap term) was a
/// recurring spacing bug.
#[derive(Default)]
struct CenteredColumn {
    /// (gap above the item, item), top to bottom.
    items: Vec<(f32, ColItem)>,
}

enum ColItem {
    /// A pre-colored galley. `halign_center` (a wrapped paragraph laid out with
    /// `Align::Center`) anchors at the column center; otherwise it centers by
    /// its own width.
    Galley {
        galley: Arc<Galley>,
        halign_center: bool,
    },
    Glyph {
        tex: egui::TextureId,
        size: f32,
        tint: egui::Color32,
    },
    /// Space for the caller to fill after layout (an interactive widget); its
    /// centered rect comes back from `show` in push order.
    Reserved {
        size: egui::Vec2,
    },
}

impl ColItem {
    fn height(&self) -> f32 {
        match self {
            ColItem::Galley { galley, .. } => galley.size().y,
            ColItem::Glyph { size, .. } => *size,
            ColItem::Reserved { size } => size.y,
        }
    }
}

impl CenteredColumn {
    fn galley(&mut self, gap: f32, galley: Arc<Galley>, halign_center: bool) {
        self.items
            .push((gap, ColItem::Galley { galley, halign_center }));
    }
    fn glyph(&mut self, gap: f32, tex: egui::TextureId, size: f32, tint: egui::Color32) {
        self.items.push((gap, ColItem::Glyph { tex, size, tint }));
    }
    fn reserve(&mut self, gap: f32, size: egui::Vec2) {
        self.items.push((gap, ColItem::Reserved { size }));
    }
    /// Center the column vertically in `area`, paint its galleys and glyphs,
    /// and return the reserved rects (push order) for the caller to place
    /// widgets into.
    fn show(self, ui: &Ui, area: Rect, center_x: f32) -> Vec<Rect> {
        let total: f32 = self.items.iter().map(|(gap, it)| gap + it.height()).sum();
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let mut y = area.min.y + ((area.height() - total) / 2.0).max(TOP_MARGIN);
        let mut reserved = Vec::new();
        for (gap, it) in self.items {
            y += gap;
            let h = it.height();
            match it {
                ColItem::Galley { galley, halign_center } => {
                    let x = if halign_center { center_x } else { center_x - galley.size().x / 2.0 };
                    ui.painter()
                        .galley(pos2(x, y), galley, egui::Color32::WHITE);
                }
                ColItem::Glyph { tex, size, tint } => {
                    let r = Rect::from_min_size(pos2(center_x - size / 2.0, y), vec2(size, size));
                    ui.painter().image(tex, r, uv, tint);
                }
                ColItem::Reserved { size } => {
                    reserved.push(Rect::from_min_size(pos2(center_x - size.x / 2.0, y), size));
                }
            }
            y += h;
        }
        reserved
    }
}

/// Tool-container header padding (the table-header-bar look).
const TOOL_PAD_X: f32 = 10.0;
const TOOL_PAD_Y: f32 = 6.0;

/// Inter-block spacing for rendered diffs. Band vertical padding is half of
/// this (the drag-card rule), so adjacent old/new bands sit flush — a
/// changed pair reads as one card with a color boundary.
const DIFF_BLOCK_SPACING: f32 = 8.0;

/// Lay out plain tool-result text as one dim mono galley. Notes and diffs
/// render as markdown via an `MdLabel` instead.
fn body_galley(ui: &Ui, text: &str, wrap_w: f32, dim: egui::Color32) -> Arc<Galley> {
    ui.fonts(|f| f.layout(text.into(), egui::FontId::monospace(TOOL_FONT), dim, wrap_w))
}

/// Total height of stacked diff segments at `width`.
fn segments_height(label: &mut MdLabel, segs: &[diff::Segment], width: f32) -> f32 {
    let mut h = 0.0;
    for (k, seg) in segs.iter().enumerate() {
        if k > 0 {
            h += DIFF_BLOCK_SPACING;
        }
        h += label.height(&seg.text, width);
    }
    h
}

/// Paint stacked diff segments through `label`; changed ones get a
/// full-width wash band (the drag-card shape: the segment's own rect padded
/// by half the stacking gap, so adjacent del/add bands sit flush). Returns
/// the glyphon areas for the caller's text callback.
#[allow(clippy::too_many_arguments)]
fn paint_segments(
    ui: &mut Ui, label: &mut MdLabel, salt: Id, segs: &[diff::Segment], pos: Pos2, width: f32,
    band_x: (f32, f32), add_wash: egui::Color32, del_wash: egui::Color32,
) -> Vec<crate::TextBufferArea> {
    let mut areas = Vec::new();
    let mut y = pos.y;
    for (k, seg) in segs.iter().enumerate() {
        if k > 0 {
            y += DIFF_BLOCK_SPACING;
        }
        // Wash first: egui-painted content (task checkboxes, rules, code
        // fills) then paints above it, and glyphon text rides a later layer
        // regardless — the tint reads as behind everything.
        let h = label.height(&seg.text, width);
        let wash = match seg.kind {
            diff::SegKind::Add => Some(add_wash),
            diff::SegKind::Del => Some(del_wash),
            diff::SegKind::Context => None,
        };
        if let Some(wash) = wash {
            let pad = DIFF_BLOCK_SPACING / 2.0;
            ui.painter().rect_filled(
                Rect::from_min_max(pos2(band_x.0, y - pad), pos2(band_x.1, y + h + pad)),
                CornerRadius::same(2),
                wash,
            );
        }
        // Distinct id scope per segment: the old and new snippets are
        // near-identical, so their widgets' `ui.id().with(node_range)` ids
        // would otherwise collide and cross-wire checkbox animations
        // (a checked and an unchecked box fighting over one lerp).
        let a = ui
            .push_id(salt.with(k), |ui| label.paint_at(ui, &seg.text, pos2(pos.x, y), width).0)
            .inner;
        areas.extend(a);
        y += h;
    }
    areas
}

impl Chat {
    /// True for transcript touches (which scroll), so Android doesn't treat a
    /// short transcript scroll as a keyboard-summoning tap. Composer touches
    /// fall through so they still raise the keyboard.
    pub fn will_consume_touch(&self, pos: egui::Pos2) -> bool {
        !self.composer_rect.contains(pos)
    }
    /// Renders the transcript + composer. Returns whether the transcript
    /// changed this frame (user sent or the agent replied — triggers a save),
    /// and the composer's text rect (egui points) used to position the native
    /// iOS text-interaction overlay.
    pub fn show(&mut self, ui: &mut Ui) -> (bool, Rect, bool) {
        self.kick_initial_config_load();
        self.pump_models();
        self.pump_config();
        self.poll_key_connection();
        let agent_changed = self.pump_agent();
        // Tool-row metrics/bodies derive from a fold of the visible records;
        // rebuilt only when the timeline fingerprint misses.
        self.ensure_tool_viz();

        // Live agent state for this frame's rendering.
        let (agent_busy, agent_streaming) = match &self.harness {
            Some(h) => (h.busy, h.streaming.clone()),
            None => (false, String::new()),
        };
        // A connect step or summoned chooser can't coexist; the step wins.
        if self.key_entry.is_some() {
            self.chooser_open = false;
        }
        // Esc backs out of a summoned chooser (picking or cancelling are the
        // other exits).
        if self.chooser_open && ui.input(|i| i.key_pressed(Key::Escape)) {
            self.chooser_open = false;
        }
        // The one timeline to display, resolved fresh each frame from the
        // flat log + branch choices. While the connect step or a summoned
        // provider chooser is open it takes over the canvas — rows step aside
        // (and with them their hover strips and menus) instead of being
        // painted under the form.
        let connect_open = self.key_entry.is_some();
        let takeover = connect_open || self.chooser_open;
        let mut visible = if takeover { Vec::new() } else { self.visible() };
        // Write-back receipts stay in the *data* timeline (they fold and
        // persist) but not the rendered one — the seed/fold paths call
        // `self.visible()` themselves and still see them.
        visible.retain(|row| !user_actioned(&self.entries[row.idx].msg));
        // First-run guidance, only in an empty chat and once config has
        // loaded (else it flashes before the background load lands). A
        // summoned chooser reuses the same surface over an existing chat.
        let onboard = if self.chooser_open {
            Some(Onboard::Choose)
        } else {
            (self.unshared && self.config_loaded && visible.is_empty())
                .then(|| self.onboard_stage())
                .flatten()
        };
        let show_agent_hint = onboard.is_some();

        // A broken key turns the composer's text field into an "add key"
        // button — the fix always sits where you'd type. Computed regardless
        // of chat emptiness (an established chat keeps its history above) and
        // suppressed while the masked field or the roster is up.
        let need_key = (self.unshared && self.config_loaded && !connect_open && !self.chooser_open)
            .then(|| match self.onboard_stage() {
                Some(Onboard::NeedKey { name, label }) => Some((name, label)),
                _ => None,
            })
            .flatten();

        // No usable key yet → hide the composer, so there's ever at most one
        // text field for the native iOS text bridge to own. `NeedKey` keeps
        // the composer surface (its text field becomes the "add key" button);
        // everything past a valid key (Connecting/Unreachable/PickModel) keeps
        // the composer too — PickModel picks its model from the toolbar.
        let hide_composer = connect_open || matches!(onboard, Some(Onboard::Choose));

        // Retry the last turn when it errored and the agent is idle.
        let can_retry = !agent_busy
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

        // The connect step closes itself (validation lands in the background),
        // so no click refocuses anything — hand focus back to the composer.
        // Harmless when the composer is hidden: an unrendered widget drops it.
        if self.connect_was_open && !connect_open {
            ui.memory_mut(|m| m.request_focus(composer_id));
        }
        self.connect_was_open = connect_open;

        // Esc cancels an in-progress edit and restores the stashed draft.
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
        // types a newline, which at least shows the key landed. Exact match:
        // `consume_key` ignores extra Shift, so it'd treat Shift+Enter as a send.
        let composer_focused = ui.memory(|m| m.has_focus(composer_id));
        let completions_open =
            self.composer.emoji_completions.active || self.composer.link_completions.active;
        let send_requested = composer_focused
            && !agent_busy
            && ui.ctx().input_mut(|i| {
                i.consume_key(Modifiers::COMMAND, Key::Enter)
                    || (!completions_open && i.consume_key_exact(Modifiers::NONE, Key::Enter))
            });

        // ⌘Enter approve / Esc deny while a call awaits a decision. A send
        // can't fire mid-turn, so the send stroke is free to mean "yes, go"
        // while the card is up. Consumed before the composer's input phase
        // so neither falls through to the editor; stood down while the
        // chooser owns the canvas (its own Esc closes it).
        let approve_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::Enter);
        let deny_shortcut = egui::KeyboardShortcut::new(Modifiers::NONE, Key::Escape);
        let mut approve_clicked = false;
        let mut deny_clicked = false;
        if self
            .harness
            .as_ref()
            .is_some_and(|h| h.pending_tool.is_some())
            && !self.chooser_open
        {
            ui.ctx().input_mut(|i| {
                approve_clicked = i.consume_shortcut(&approve_shortcut);
                deny_clicked = !approve_clicked && i.consume_shortcut(&deny_shortcut);
            });
        }
        let touch_os = matches!(
            ui.ctx().os(),
            egui::os::OperatingSystem::Android | egui::os::OperatingSystem::IOS
        );
        // Hint labels show where a hardware keyboard is plausible: desktop,
        // and tablet-width touch screens (iPad); not phones.
        let show_key_hints = !touch_os || available_width >= 600.0;
        let approve_hint = ui.ctx().format_shortcut(&approve_shortcut);
        // format_shortcut spells it "Escape" — every editor's label is "esc".
        let deny_hint = "esc".to_string();

        // Return submits the key — the connect step has no Connect button.
        let mut key_submit = false;
        // Text-input phase — drain workspace-origin events (native iOS text
        // arrives this way: Newline / Indent / Replace pushed by the FFI), then
        // keyboard / completions / internal events. Routes to *one* editor:
        // the connect step's key field while it's open, else the composer.
        if self.key_entry.is_some() {
            let key_field_id = Id::new("chat_key_field");
            // No per-frame request_focus: the render block's one-shot focuses
            // the field, and its interaction + focus lock hold it after that.
            // Consume Return before handle_input so it submits rather than
            // reaching the field as a newline (desktop).
            if ui.memory(|m| m.has_focus(key_field_id))
                && ui
                    .ctx()
                    .input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
            {
                key_submit = true;
            }
            let ws = self.key_field.drain_workspace_events(ui.ctx());
            self.key_field.event.internal_events.extend(ws);
            let _ = self.key_field.handle_input(ui.ctx(), key_field_id);
            // A single-line key never holds a newline; one that landed came
            // from the native keyboard's return (iOS) — submit and strip it.
            if self.key_field.renderer.buffer.current.text.contains('\n') {
                let cleaned = self
                    .key_field
                    .renderer
                    .buffer
                    .current
                    .text
                    .replace('\n', "");
                self.key_field.set_text(&cleaned);
                key_submit = true;
            }
        } else {
            let workspace_events = self.composer.drain_workspace_events(ui.ctx());
            self.composer.event.internal_events.extend(workspace_events);
            let _ = self.composer.handle_input(ui.ctx(), composer_id);
        }

        // Measure at the exact render width so the composer bubble grows
        // same-frame. The re-parse inside `show` below hits the layout cache.
        // `H_MARGIN` and `H_PAD` mirror the h_inset / shrink geometry below.
        let composer_inner_w = (col_width - 2.0 * H_MARGIN - 2.0 * H_PAD).max(0.0);
        let measured_h = self.composer.measure_height(composer_inner_w);

        // Autogrow with a max cap, no lower floor — a lower floor makes a
        // single-line composer bottom-heavy (content is top-anchored).
        let composer_height = (measured_h + V_PAD * 2.0).min(COMPOSER_MAX_HEIGHT);
        // The composer bubble carries a toolbar row at its bottom; the model
        // dropdowns appear there for agent chats.
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
        let mut onboard_pick: Option<&'static str> = None;
        let mut chooser_cancel = false;
        // Inline "connect a provider" step actions, applied after the scroll
        // closure releases its borrow of `self`.
        let mut key_cancel = false;
        // Row actions (hover pill + context menu). Copy always works;
        // everything that mutates the transcript or timeline is gated on no
        // turn being in flight, and the agent-rerunning actions additionally
        // on this being an agent chat.
        let mut action: Option<RowAction> = None;
        let can_mutate = !agent_busy;
        let agent_actions = self.harness.is_some();
        // The call awaiting a decision, rendered as a trailing approval card:
        // its adapted permission prose, the diff (edits), and Approve / Deny.
        // Buttons' clicks are applied after the closure.
        let provider_label = self
            .provider
            .as_ref()
            .map(|p| p.label())
            .unwrap_or_else(|| "the model".into());
        let pending_tool = self
            .harness
            .as_ref()
            .and_then(|h| h.pending_tool.as_ref())
            .map(|p| {
                (
                    tools::detail_for(&p.name, &p.args),
                    tools::permission_prose(&p.name, &p.args, &provider_label),
                    p.preview.clone(),
                )
            });
        // Soft washes behind rendered diffs' changed blocks.
        let add_wash = theme.bg().green.gamma_multiply(0.22);
        let del_wash = theme.bg().red.gamma_multiply(0.22);
        // Collapsed-row outcome metrics run dimmer than the summary text.
        let metric_color = theme.neutral_fg_secondary();
        // A clicked listing row to open, resolved after the closure.
        let mut open_list_path: Option<String> = None;

        // A branch switch suspends stick-to-bottom for the frame — the view
        // holds still while the timeline below the fork is swapped out.
        let branch_anchored = self.branch_anchor.is_some();
        // Editor-style backdrop: a tap on empty transcript space dismisses
        // the keyboard. Registered before the scroll content so message
        // rows, links, and tool containers (higher z) win their own taps;
        // only bare-canvas taps reach it. Touch-only — desktop has no
        // on-screen keyboard to dismiss. Suppressed under a takeover (the
        // key field or chooser owns the surface): a tap there must not yank
        // focus off the field it's editing.
        let mut backdrop_tapped = false;
        ui.scope_builder(egui::UiBuilder::new().max_rect(transcript_rect), |ui| {
            ui.set_clip_rect(transcript_rect.intersect(ui.clip_rect()));
            if touch_os && !takeover {
                let backdrop = ui.interact(
                    transcript_rect,
                    Id::new("chat_transcript_backdrop"),
                    Sense::click(),
                );
                backdrop_tapped = backdrop.clicked();
            }
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
                        let mut strip = |y: &mut f32,
                                         row_rect: Rect,
                                         ts: Option<Arc<Galley>>,
                                         h: f32| {
                            let rect = Rect::from_min_size(pos2(note_x, *y), vec2(note_wrap_w, h));
                            strips.push(StripPlan { vi, rect, row_rect, right: is_mine_row, ts });
                            *y += h + ROW_GAP;
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
                            strip(&mut y, row_rect, None, STRIP_H);
                            continue;
                        }

                        // A tool round-trip: a chevron + one-line summary +
                        // right-aligned outcome metric, expanding to the call's
                        // body (bound note state, diff, or listing rows).
                        // Rendered dim, like a note, not a chat bubble.
                        if let Some(record) = self.entries[i].msg.tool.clone() {
                            // A run of consecutive tool rows groups: extra
                            // padding above its first row and below its last.
                            let tool_at =
                                |vj: usize| self.entries[visible[vj].idx].msg.tool.is_some();
                            if vi == 0 || !tool_at(vi - 1) {
                                y += TOOL_GROUP_PAD;
                            }
                            let last_in_group = vi + 1 >= n || !tool_at(vi + 1);
                            let id = self.entries[i].msg.id;
                            let viz = id.and_then(|id| self.tool_viz.get(&id));
                            let expanded = id.is_some_and(|id| self.expanded_tools.contains(&id));
                            let summary = ui.fonts(|f| {
                                f.layout_no_wrap(
                                    self.entries[i].msg.content.clone(),
                                    egui::FontId::monospace(TOOL_FONT),
                                    secondary_color,
                                )
                            });
                            let metric = viz.filter(|v| !v.metric.is_empty()).map(|v| {
                                let mono = egui::FontId::monospace(TOOL_FONT);
                                // An edit's "+a -d" reads in the diff colors;
                                // every other metric stays dim.
                                let two_tone = v
                                    .metric
                                    .split_once(' ')
                                    .filter(|(a, d)| a.starts_with('+') && d.starts_with('-'));
                                match two_tone {
                                    Some((add, del)) => {
                                        use egui::text::{LayoutJob, TextFormat};
                                        let mut job = LayoutJob::default();
                                        job.append(
                                            add,
                                            0.0,
                                            TextFormat {
                                                font_id: mono.clone(),
                                                color: theme.fg().green,
                                                ..Default::default()
                                            },
                                        );
                                        job.append(
                                            del,
                                            6.0,
                                            TextFormat {
                                                font_id: mono,
                                                color: theme.fg().red,
                                                ..Default::default()
                                            },
                                        );
                                        ui.fonts(|f| f.layout_job(job))
                                    }
                                    None => ui.fonts(|f| {
                                        f.layout_no_wrap(v.metric.clone(), mono, metric_color)
                                    }),
                                }
                            });
                            let header_h = summary.size().y + 2.0 * TOOL_PAD_Y;
                            let header_rect =
                                Rect::from_min_size(pos2(note_x, y), vec2(note_wrap_w, header_h));
                            let indent = TOOL_PAD_X;
                            let mut row_h = header_h;
                            let mut body = None;
                            let mut body_pos = Pos2::ZERO;
                            let mut rendered = None;
                            let mut list_rows = Vec::new();
                            let mut bottom_pad = TOOL_PAD_Y;
                            if expanded {
                                let by = y + header_h + TOOL_PAD_Y;
                                // Owned so the label (a `self` borrow) can lay
                                // out the rendered diff below.
                                let viz_body = viz.map(|v| v.body.clone());
                                match viz_body {
                                    Some(tools::Body::List { rows }) => {
                                        let mut ry = by;
                                        for row in rows {
                                            // PLACEHOLDER so paint can color
                                            // hovered rows.
                                            let galley = ui.fonts(|f| {
                                                f.layout_no_wrap(
                                                    row.clone(),
                                                    egui::FontId::monospace(TOOL_FONT),
                                                    egui::Color32::PLACEHOLDER,
                                                )
                                            });
                                            let rh = galley.size().y;
                                            let rect = Rect::from_min_size(
                                                pos2(note_x + indent, ry),
                                                vec2(note_wrap_w - 2.0 * indent, rh),
                                            );
                                            let open_path =
                                                (!row.ends_with('/')).then(|| row.clone());
                                            list_rows.push(ListRowPlan { galley, rect, open_path });
                                            ry += rh + 2.0;
                                        }
                                        row_h += TOOL_PAD_Y + (ry - by);
                                    }
                                    // An edit's diff: del/add segments,
                                    // nothing else. Separately parsed, so no
                                    // joining blank line renders between them
                                    // and lists can't merge.
                                    Some(tools::Body::Rendered { segments }) => {
                                        // Flush with the header: the body's
                                        // top/bottom padding is exactly the
                                        // band pad, so the first/last washes
                                        // meet the container edges.
                                        let pad = DIFF_BLOCK_SPACING / 2.0;
                                        let by = y + header_h + pad;
                                        let rw = note_wrap_w - 2.0 * indent;
                                        let label = &mut self.entries[i].label;
                                        label.renderer.layout.block_spacing = DIFF_BLOCK_SPACING;
                                        let rh = segments_height(label, &segments, rw);
                                        row_h += pad + rh;
                                        bottom_pad = pad;
                                        rendered = Some((segments, pos2(note_x + indent, by), rw));
                                    }
                                    // A read's note: nothing but the rendered
                                    // markdown (one unwashed segment).
                                    Some(tools::Body::Note { text }) => {
                                        let rw = note_wrap_w - 2.0 * indent;
                                        let segs = vec![diff::Segment {
                                            text,
                                            kind: diff::SegKind::Context,
                                        }];
                                        let rh =
                                            segments_height(&mut self.entries[i].label, &segs, rw);
                                        row_h += ROW_GAP + rh;
                                        rendered = Some((segs, pos2(note_x + indent, by), rw));
                                    }
                                    other => {
                                        // Result text, mono; a record with no
                                        // viz (id-less legacy row) falls back
                                        // to the raw result.
                                        let text = match &other {
                                            Some(tools::Body::Text(t)) => t.as_str(),
                                            _ => record.result.as_str(),
                                        };
                                        let g = body_galley(
                                            ui,
                                            text,
                                            note_wrap_w - 2.0 * indent,
                                            secondary_color,
                                        );
                                        body_pos = pos2(note_x + indent, by);
                                        row_h += ROW_GAP + g.size().y;
                                        body = Some(g);
                                    }
                                }
                            }
                            if expanded {
                                // Bottom padding inside the bordered body
                                // (band pad for flush diff bodies).
                                row_h += bottom_pad;
                            }
                            let row_rect =
                                Rect::from_min_size(pos2(note_x, y), vec2(note_wrap_w, row_h));
                            plans.push(RowPlan::Tool {
                                header_rect,
                                summary,
                                metric,
                                body,
                                body_pos,
                                rendered,
                                list_rows,
                                border: expanded.then_some(row_rect),
                            });
                            y += row_h + ROW_GAP;
                            // Tool rows show nothing in the strip (no
                            // timestamp, no icons) — reserve its height only
                            // when fork arrows need it.
                            let strip_h = if visible[vi].fork.is_some() { STRIP_H } else { 0.0 };
                            strip(&mut y, row_rect, None, strip_h);
                            if last_in_group {
                                y += TOOL_GROUP_PAD;
                            }
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
                            y += name_h + content_h + ROW_GAP + AGENT_STRIP_GAP;
                            strip(&mut y, row_rect, ts_galley, STRIP_H);
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
                            strip(&mut y, bubble_rect, ts_galley, STRIP_H);
                        }
                    }

                    // Trailing agent rows: the streaming reply live on the
                    // canvas under an "agent" header ("thinking…" until the
                    // first token), or setup guidance when the chat has no
                    // configured agent.
                    #[allow(unused_mut, unused_variables)]
                    let mut streaming_plan: Option<(Pos2, Arc<Galley>)> = None;
                    if agent_busy && !agent_streaming.is_empty() && !takeover {
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
                    // No "thinking…" while an edit awaits approval — the card
                    // is the active surface, not the model.
                    let note_plan = (agent_busy
                        && agent_streaming.is_empty()
                        && pending_tool.is_none()
                        && !takeover)
                        .then(|| {
                            let galley = ui.fonts(|f| {
                                f.layout(
                                    "thinking…".into(),
                                    egui::FontId::proportional(NOTE_FONT),
                                    secondary_color,
                                    note_wrap_w,
                                )
                            });
                            let h = galley.rect.height();
                            let pos = pos2(note_x, y);
                            y += h + ROW_GAP;
                            (galley, pos)
                        });

                    // The trailing approval card, shaped like a tool container
                    // pinned open: a header bar with the command summary, over
                    // a bordered body holding the permission prose, the
                    // proposed change, and Approve / Deny.
                    let review_plan =
                        pending_tool.as_ref().map(|(summary_text, prose, preview)| {
                            y += TOOL_GROUP_PAD;
                            let indent = TOOL_PAD_X;

                            // Header bar: the command summary, left-aligned. No
                            // right metric — the call hasn't been allowed to run.
                            let summary = ui.fonts(|f| {
                                f.layout_no_wrap(
                                    summary_text.clone(),
                                    egui::FontId::monospace(TOOL_FONT),
                                    secondary_color,
                                )
                            });
                            let header_h = summary.size().y + 2.0 * TOOL_PAD_Y;
                            let header_rect =
                                Rect::from_min_size(pos2(note_x, y), vec2(note_wrap_w, header_h));
                            let mut cy = y + header_h + TOOL_PAD_Y;

                            // Body: the permission request, wrapping. Dimmed mono,
                            // like the tool summary and result text.
                            let body_w = note_wrap_w - 2.0 * indent;
                            let prose = ui.fonts(|f| {
                                f.layout(
                                    prose.clone(),
                                    egui::FontId::monospace(TOOL_FONT),
                                    secondary_color,
                                    body_w,
                                )
                            });
                            let prose_pos = pos2(note_x + indent, cy);
                            cy += prose.size().y;

                            // The proposed change, under the prose.
                            let body = match preview {
                                // An edit's diff, rendered with changed blocks
                                // washed in place.
                                Some(Ok(segs)) => {
                                    self.review_label.renderer.layout.block_spacing =
                                        DIFF_BLOCK_SPACING;
                                    let h = segments_height(&mut self.review_label, segs, body_w);
                                    let pos = pos2(note_x + indent, cy + ROW_GAP);
                                    cy += ROW_GAP + h;
                                    ReviewBody::Rendered {
                                        segments: segs.clone(),
                                        pos,
                                        width: body_w,
                                    }
                                }
                                // The steering error approval would hit.
                                Some(Err(e)) => {
                                    let galley = ui.fonts(|f| {
                                        f.layout(
                                            e.clone(),
                                            egui::FontId::monospace(TOOL_FONT),
                                            secondary_color,
                                            body_w,
                                        )
                                    });
                                    let pos = pos2(note_x + indent, cy + ROW_GAP);
                                    cy += ROW_GAP + galley.size().y;
                                    ReviewBody::Galley { galley, pos }
                                }
                                None => ReviewBody::None,
                            };

                            // Button row, right-aligned. Hints show where a
                            // hardware keyboard is plausible (desktop, iPad).
                            cy += V_PAD;
                            let btn = |ui: &Ui, label: String, accent: bool| {
                                ui.fonts(|f| {
                                    f.layout_no_wrap(
                                        label,
                                        egui::FontId::proportional(13.0),
                                        if accent { text_color } else { secondary_color },
                                    )
                                })
                            };
                            let approve_label = if show_key_hints {
                                format!("Approve {approve_hint}")
                            } else {
                                "Approve".into()
                            };
                            let deny_label = if show_key_hints {
                                format!("Deny {deny_hint}")
                            } else {
                                "Deny".into()
                            };
                            let approve = btn(ui, approve_label, true);
                            let deny = btn(ui, deny_label, false);
                            let btn_h = approve.size().y.max(deny.size().y) + 8.0;
                            let bpad = 12.0;
                            let approve_w = approve.size().x + bpad * 2.0;
                            let deny_w = deny.size().x + bpad * 2.0;
                            let right = note_x + note_wrap_w - indent;
                            let deny_rect =
                                Rect::from_min_size(pos2(right - deny_w, cy), vec2(deny_w, btn_h));
                            let approve_rect = Rect::from_min_size(
                                pos2(deny_rect.min.x - STRIP_GAP - approve_w, cy),
                                vec2(approve_w, btn_h),
                            );
                            cy += btn_h + TOOL_PAD_Y;

                            let border =
                                Rect::from_min_size(pos2(note_x, y), vec2(note_wrap_w, cy - y));
                            y = cy + ROW_GAP + TOOL_GROUP_PAD;
                            ReviewPlan {
                                header_rect,
                                summary,
                                prose,
                                prose_pos,
                                body,
                                approve,
                                approve_rect,
                                deny,
                                deny_rect,
                                border,
                            }
                        });

                    // Keep the clicked arrows where they were: correct for
                    // any height change of the swapped fork row (content
                    // above the fork is identical, so that's the whole
                    // delta). Consuming a scroll also un-sticks the area.
                    if let Some((avi, old_y, row_top)) = self.branch_anchor.take() {
                        if let Some(s) = strips.iter().find(|s| s.vi == avi) {
                            let new_y = if row_top { s.row_rect.min.y } else { s.rect.min.y };
                            let delta = old_y - new_y;
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

                    // Row-wide widgets register BEFORE content paints, so
                    // content-level widgets — markdown links, which open on
                    // cmd-click — sit above them and win the pointer.
                    let row_resps: Vec<egui::Response> = strips
                        .iter()
                        .map(|s| {
                            ui.interact(
                                s.row_rect,
                                Id::new(("chat_row", visible[s.vi].idx)),
                                Sense::click(),
                            )
                        })
                        .collect();

                    // pass 2: paint absolute. No egui layout calls.
                    let editing_id = self.editing;
                    // Clickable listing rows per tool row (entry index →
                    // rects+paths), point-tested by the strips loop's row
                    // widget.
                    let mut tool_list_hits: HashMap<usize, Vec<(Rect, String)>> = HashMap::new();
                    for (vi, plan) in plans.into_iter().enumerate() {
                        let i = visible[vi].idx;
                        match plan {
                            RowPlan::Bubble { bubble_rect, name_galley, name_h, content_h } => {
                                // Rounding matches the tool containers.
                                ui.painter().rect_filled(
                                    bubble_rect,
                                    CornerRadius::same(2),
                                    bubble_surface,
                                );
                                // The message being edited is outlined in the
                                // accent — send commits a sibling of it.
                                if editing_id.is_some() && self.entries[i].msg.id == editing_id {
                                    ui.painter().rect_stroke(
                                        bubble_rect,
                                        CornerRadius::same(2),
                                        Stroke::new(
                                            1.0,
                                            theme.fg().get_color(theme.prefs().primary),
                                        ),
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
                                    ui.painter()
                                        .galley(pos2(pos.x, text_y), header, error_color);
                                    text_y += h + NAME_GAP;
                                }
                                ui.painter()
                                    .galley(pos2(pos.x, text_y), galley, error_color);
                            }
                            RowPlan::Tool {
                                header_rect,
                                summary,
                                metric,
                                body,
                                body_pos,
                                rendered,
                                list_rows,
                                border,
                            } => {
                                // Paint only — clicks are handled by the
                                // strips loop's whole-row widget (registered
                                // after, so it owns the row; a widget here
                                // would be occluded). Listing hits are
                                // point-tested against `tool_list_hits`.
                                //
                                // The container borrows the markdown table's
                                // grammar: filled header bar; expanded content
                                // on the transcript background inside a
                                // border, corners matching the table's.
                                let header_rounding = if border.is_some() {
                                    CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }
                                } else {
                                    CornerRadius::same(2)
                                };
                                ui.painter().rect_filled(
                                    header_rect,
                                    header_rounding,
                                    theme.neutral_bg_secondary(),
                                );
                                ui.painter().galley(
                                    pos2(
                                        header_rect.min.x + TOOL_PAD_X,
                                        header_rect.center().y - summary.size().y / 2.0,
                                    ),
                                    summary,
                                    secondary_color,
                                );
                                if let Some(metric) = metric {
                                    ui.painter().galley(
                                        pos2(
                                            header_rect.max.x - TOOL_PAD_X - metric.size().x,
                                            header_rect.center().y - metric.size().y / 2.0,
                                        ),
                                        metric,
                                        metric_color,
                                    );
                                }
                                if let Some(body) = body {
                                    ui.painter().galley(body_pos, body, secondary_color);
                                }
                                if let Some((segs, rpos, rwidth)) = rendered {
                                    // Bands bleed to the container edges
                                    // (inset for its 1px border).
                                    let areas = paint_segments(
                                        ui,
                                        &mut self.entries[i].label,
                                        Id::new(("tool_body", i)),
                                        &segs,
                                        rpos,
                                        rwidth,
                                        (note_x + 1.0, note_x + note_wrap_w - 1.0),
                                        add_wash,
                                        del_wash,
                                    );
                                    text_areas.extend(areas);
                                }
                                for row in list_rows {
                                    let openable = row.open_path.is_some();
                                    let color = if openable && ui.rect_contains_pointer(row.rect) {
                                        text_color
                                    } else {
                                        secondary_color
                                    };
                                    ui.painter().galley(row.rect.min, row.galley, color);
                                    if let Some(path) = row.open_path {
                                        tool_list_hits.entry(i).or_default().push((row.rect, path));
                                    }
                                }
                                if let Some(container) = border {
                                    ui.painter().rect_stroke(
                                        container,
                                        2.0,
                                        Stroke { width: 1.0, color: theme.neutral_bg_tertiary() },
                                        StrokeKind::Inside,
                                    );
                                }
                            }
                        }
                    }

                    // Metadata strips: timestamp, ‹ 2/3 › arrows, and hover
                    // action icons, in the reserved space under each row.
                    // All row interaction (context menu included) lives here.
                    for (si, strip) in strips.iter().enumerate() {
                        let vi = strip.vi;
                        let i = visible[vi].idx;
                        let is_tail = vi + 1 == visible.len();
                        let kind = {
                            let m = &self.entries[i].msg;
                            if m.error || m.tool.is_some() {
                                // Tool rows carry their own affordance (expand);
                                // no edit/copy/regen strip.
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
                        // Retry lives where every other row action lives.
                        let retryable = is_tail && can_retry && self.entries[i].msg.error;
                        // Union bridges the gap between the message and its
                        // strip, so the actions don't blink out when the
                        // pointer crosses the space between them.
                        let hovered = ui.rect_contains_pointer(strip.row_rect.union(strip.rect));
                        // Touch has no hover — reveal actions outright.
                        let show_icons = touch_os || hovered || retryable;

                        // Items lay out from the row's aligned edge inward:
                        // arrows, then timestamp, then action icons. The
                        // always-visible arrows and timestamp anchor the
                        // outer edge; hover-revealed icons sit innermost, so
                        // revealing them never shifts an element the cursor
                        // is reaching for.
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

                        if let Some(fork) = &visible[vi].fork {
                            let font = egui::FontId::proportional(NOTE_FONT);
                            let label = format!("{}/{}", fork.pos + 1, fork.siblings.len());
                            let lg = ui
                                .fonts(|f| f.layout_no_wrap(label, font.clone(), secondary_color));
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
                                let target = target_pos.and_then(|p| fork.siblings.get(p)).copied();
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

                        // Action icons, innermost so a hover reveal shifts
                        // nothing beyond them. Only a pointer over the row
                        // can reach them, so hover-gated visibility never
                        // hides a reachable target.
                        if show_icons {
                            let mut icons: Vec<(&Icon, Option<RowAction>)> = Vec::new();
                            // Rerun lives on the *user's* message: resend it
                            // as a sibling and the turn re-runs.
                            if editable && kind == RowKind::OwnUser {
                                icons.push((&Icon::PENCIL, Some(RowAction::Edit(i))));
                                icons.push((&Icon::SYNC, Some(RowAction::ResendFrom(i))));
                            }
                            // Tool rows expand instead — nothing worth
                            // copying in a summary line.
                            if self.entries[i].msg.tool.is_none() {
                                icons.push((&Icon::CONTENT_COPY, None));
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

                        // Context menu over the message row (secondary path
                        // to the same actions, plus Retry-from-here/Delete).
                        let content = self.entries[i].msg.content.clone();
                        let row_resp = row_resps[si].clone();
                        row_menu(&row_resp, i, &content, kind, editable, can_mutate, &mut action);

                        // A tool row toggles on a click anywhere on it —
                        // except its clickable listing rows, which open the
                        // note instead (point-tested: this row widget owns the
                        // whole rect, so they can't be their own widgets).
                        if self.entries[i].msg.tool.is_some() {
                            let row_resp = row_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                            if row_resp.clicked() {
                                let hit = row_resp.interact_pointer_pos().and_then(|p| {
                                    tool_list_hits.get(&i).and_then(|rows| {
                                        rows.iter()
                                            .find(|(rect, _)| rect.contains(p))
                                            .map(|(_, path)| path.clone())
                                    })
                                });
                                match (hit, self.entries[i].msg.id) {
                                    (Some(path), _) => open_list_path = Some(path),
                                    (None, Some(id)) => {
                                        action = Some(RowAction::ToggleTool {
                                            id,
                                            vi,
                                            anchor_y: strip.row_rect.min.y,
                                        });
                                    }
                                    (None, None) => {}
                                }
                            }
                        }
                    }

                    // Trailing agent rows paint after the transcript.
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
                    if let Some((galley, pos)) = note_plan {
                        ui.painter().galley(pos, galley, secondary_color);
                    }

                    // The approval card, painted like a tool container pinned
                    // open: a filled header bar (command summary), a bordered
                    // body with the permission prose + proposed change, and
                    // Approve / Deny.
                    if let Some(r) = review_plan {
                        // Fill with the bg-family accent (the send button's
                        // pattern) — the fg accent is for text and strokes.
                        let accent = theme.bg().get_color(theme.prefs().primary);
                        // Header bar with top corners rounded, like an expanded
                        // tool row's.
                        ui.painter().rect_filled(
                            r.header_rect,
                            CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 },
                            theme.neutral_bg_secondary(),
                        );
                        ui.painter().galley(
                            pos2(
                                r.header_rect.min.x + TOOL_PAD_X,
                                r.header_rect.center().y - r.summary.size().y / 2.0,
                            ),
                            r.summary,
                            secondary_color,
                        );
                        // The permission request.
                        ui.painter().galley(r.prose_pos, r.prose, secondary_color);
                        match r.body {
                            ReviewBody::None => {}
                            ReviewBody::Galley { galley, pos } => {
                                ui.painter().galley(pos, galley, secondary_color);
                            }
                            // The proposed change, rendered; changed blocks
                            // washed in place. Washes paint after the layout
                            // but under the glyphs — text rides the later
                            // glyphon callback layer. Bands bleed to the
                            // container edges (inset for its 1px border).
                            ReviewBody::Rendered { segments, pos, width } => {
                                let areas = paint_segments(
                                    ui,
                                    &mut self.review_label,
                                    Id::new("review_body"),
                                    &segments,
                                    pos,
                                    width,
                                    (note_x + 1.0, note_x + note_wrap_w - 1.0),
                                    add_wash,
                                    del_wash,
                                );
                                text_areas.extend(areas);
                            }
                        }

                        // Deny: bordered. Approve: accent-filled.
                        let deny = ui
                            .interact(r.deny_rect, Id::new("chat_review_deny"), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        ui.painter().rect_stroke(
                            r.deny_rect,
                            CornerRadius::same(6),
                            Stroke::new(1.0, secondary_color),
                            StrokeKind::Inside,
                        );
                        ui.painter().galley(
                            r.deny_rect.center() - r.deny.size() / 2.0,
                            r.deny,
                            secondary_color,
                        );
                        let approve = ui
                            .interact(
                                r.approve_rect,
                                Id::new("chat_review_approve"),
                                Sense::click(),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        ui.painter()
                            .rect_filled(r.approve_rect, CornerRadius::same(6), accent);
                        ui.painter().galley(
                            r.approve_rect.center() - r.approve.size() / 2.0,
                            r.approve,
                            text_color,
                        );
                        if approve.clicked() {
                            approve_clicked = true;
                        }
                        if deny.clicked() {
                            deny_clicked = true;
                        }
                        // Container border, like an expanded tool row's.
                        ui.painter().rect_stroke(
                            r.border,
                            2.0,
                            Stroke { width: 1.0, color: theme.neutral_bg_tertiary() },
                            StrokeKind::Inside,
                        );
                    }

                    // First-run: a minimal centered card. The Choose stage
                    // offers the provider roster as a two-column icon grid;
                    // the later stages track validation with one centered
                    // line. Fonts run larger than the transcript's note size,
                    // which is hard to read for standalone UI.
                    // The inline "connect a provider" step: a masked key field
                    // in the same centered canvas, in place of the status card,
                    // when a key-requiring provider was just picked or a saved
                    // one is missing its key. Modeled on a bank-connect flow —
                    // focused and in-context, with connecting/rejected feedback,
                    // rather than a floating dialog.
                    if self.key_entry.is_some() {
                        let ctx = ui.ctx().clone();
                        let center_x = note_x + note_wrap_w / 2.0;
                        let card_w = note_wrap_w.min(340.0);
                        let field_w = card_w.min(300.0);
                        let body_font = egui::FontId::proportional(13.5);

                        let entry_ref = self.key_entry.as_ref().unwrap();
                        let (label, connecting, attempted) =
                            (entry_ref.label.clone(), entry_ref.connecting, entry_ref.attempted);
                        let provider_glyph = entry_ref.name.clone();
                        // Validation results count only when the resolved
                        // provider is the one this entry is connecting — a
                        // reloading provider list can briefly resolve elsewhere.
                        let provider_key = self
                            .provider
                            .as_ref()
                            .filter(|p| p.name == entry_ref.name)
                            .map(|p| (p.name.clone(), p.base_url.clone()));
                        // After a submit: Some(true) = auth error (bad key),
                        // Some(false) = couldn't reach the server, None = no
                        // error yet. Distinguishes "check your key" from
                        // "check your connection".
                        let err_auth = provider_key.as_ref().and_then(|key| {
                            self.models_err
                                .as_ref()
                                .filter(|(k, _)| k == key)
                                .map(|(_, e)| is_auth_error(e))
                        });

                        let head = ui.fonts(|f| {
                            f.layout_no_wrap(
                                format!("Connect {label}"),
                                egui::FontId::proportional(19.0),
                                text_color,
                            )
                        });
                        let status = if connecting {
                            Some((format!("Connecting to {label}…"), secondary_color))
                        } else if attempted {
                            match err_auth {
                                Some(true) => Some((
                                    "That key didn't work. Check it and try again.".to_string(),
                                    error_color,
                                )),
                                Some(false) => Some((
                                    format!("Can't reach {label}. Check your connection."),
                                    error_color,
                                )),
                                None => None,
                            }
                        } else {
                            None
                        };
                        let status_galley = status.as_ref().map(|(t, c)| {
                            let mut job = egui::text::LayoutJob::simple(
                                t.clone(),
                                body_font.clone(),
                                *c,
                                card_w,
                            );
                            job.halign = egui::Align::Center;
                            ui.fonts(|f| f.layout_job(job))
                        });
                        // Minimal: the field submits on return, so the only
                        // control is a centered text Cancel (like the picker).
                        let cancel = ui.fonts(|f| {
                            f.layout_no_wrap("Cancel".into(), body_font.clone(), secondary_color)
                        });

                        let (glyph_sz, field_h) = (24.0, 30.0);

                        // Lay the whole column in one place — the vertical
                        // spacing follows from the pushed gaps, no hand-summed
                        // total to drift.
                        let tex = self.glyphs.get(&ctx, &provider_glyph, glyph_sz);
                        let mut col = CenteredColumn::default();
                        col.glyph(0.0, tex.id, glyph_sz, text_color);
                        col.galley(14.0, head, false);
                        col.reserve(18.0, vec2(field_w, field_h));
                        let cancel_gap = match status_galley {
                            Some(g) => {
                                col.galley(10.0, g, true);
                                14.0
                            }
                            None => 16.0,
                        };
                        col.reserve(cancel_gap, cancel.size());
                        let rects = col.show(ui, transcript_rect, center_x);
                        let (field_rect, cancel_rect) = (rects[0], rects[1]);

                        // Masked field on its own raised surface so the input
                        // is visible before it's focused or hovered.
                        ui.painter()
                            .rect_filled(field_rect, CornerRadius::same(6), bubble_surface);
                        ui.painter().rect_stroke(
                            field_rect,
                            CornerRadius::same(6),
                            Stroke::new(1.0, theme.neutral_bg().lerp_to_gamma(text_color, 0.16)),
                            StrokeKind::Inside,
                        );
                        // The MdEdit renders in the top-level ui (after the
                        // scroll closure) — the native iOS text bridge only
                        // binds to a top-level editor, never one nested in a
                        // scroll area. Recorded here: the text's inner rect
                        // (one row vertically centered, h-padded like the
                        // placeholder); the box itself paints at `field_rect`.
                        let row_h = self.key_field.row_height();
                        self.key_field_rect = Rect::from_min_max(
                            pos2(field_rect.min.x + 8.0, field_rect.center().y - row_h / 2.0),
                            pos2(field_rect.max.x - 8.0, field_rect.center().y + row_h / 2.0),
                        );
                        // The whole visible box is the tap/gesture target,
                        // reported to the native text view (the one-row rect
                        // above is only where the masked text lays out).
                        self.key_field_hit_rect = field_rect;
                        if self.key_field.renderer.buffer.current.text.is_empty() {
                            let hint = ui.fonts(|f| {
                                f.layout_no_wrap(
                                    "Paste your API key".into(),
                                    egui::FontId::proportional(13.5),
                                    secondary_color,
                                )
                            });
                            let y = field_rect.center().y - hint.size().y / 2.0;
                            ui.painter().galley(
                                pos2(field_rect.min.x + 8.0, y),
                                hint,
                                secondary_color,
                            );
                        }
                        // Centered text-only Cancel, filling its reserved rect.
                        let cancel_resp = ui
                            .interact(cancel_rect, Id::new("chat_key_cancel"), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        let cancel_color =
                            if cancel_resp.hovered() { text_color } else { secondary_color };
                        ui.painter().galley(cancel_rect.min, cancel, cancel_color);
                        if cancel_resp.clicked() {
                            key_cancel = true;
                        }
                    } else if let Some(stage) = &onboard {
                        let ctx = ui.ctx().clone();
                        let center_x = note_x + note_wrap_w / 2.0;
                        let card_w = note_wrap_w.min(420.0);
                        let body_font = egui::FontId::proportional(13.5);
                        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));

                        // A wrapped, centered paragraph.
                        let para = |ui: &Ui, text: String, color| {
                            let mut job = egui::text::LayoutJob::simple(
                                text,
                                body_font.clone(),
                                color,
                                card_w,
                            );
                            job.halign = egui::Align::Center;
                            ui.fonts(|f| f.layout_job(job))
                        };

                        match stage {
                            Onboard::Choose => {
                                // The same surface serves first-run and the
                                // toolbar's "add provider"; only the headline
                                // and the cancel escape differ.
                                let summoned = self.chooser_open;
                                let headline = if summoned {
                                    "Add a provider"
                                } else {
                                    "Chat with an AI agent"
                                };
                                let head = ui.fonts(|f| {
                                    f.layout_no_wrap(
                                        headline.into(),
                                        egui::FontId::proportional(19.0),
                                        text_color,
                                    )
                                });
                                // Two meaningful columns: the household-name
                                // model makers (+ `custom`, the hand-rolled
                                // escape hatch) on the left, third-party hosts
                                // serving others' models (+ local `ollama`) on
                                // the right. `TEMPLATES` already groups makers
                                // [0] and hosts [1]; the local group [2] holds
                                // ollama + custom.
                                let by_name = |n: &str| -> (&'static str, &'static str) {
                                    TEMPLATES
                                        .iter()
                                        .flat_map(|g| g.iter())
                                        .copied()
                                        .find(|(name, _)| *name == n)
                                        .unwrap()
                                };
                                let mut left: Vec<(&'static str, &'static str)> =
                                    TEMPLATES[0].to_vec();
                                left.push(by_name("custom"));
                                let mut right: Vec<(&'static str, &'static str)> =
                                    TEMPLATES[1].to_vec();
                                // Ollama's template points at localhost, which on
                                // a phone is the phone — nothing runs there, so
                                // it's a guaranteed dead end. Reaching an Ollama
                                // box on the LAN is a `custom` file with that
                                // machine's address, not localhost.
                                let mobile = cfg!(target_os = "ios") || cfg!(target_os = "android");
                                if !mobile {
                                    right.push(by_name("ollama"));
                                }
                                let columns = [left, right];

                                // Size cells to their widest label so the grid
                                // is a tight, centered block — wide fixed cells
                                // left the glyph+label hugging the left edge.
                                // Labels are flattened column-major (all of the
                                // left column, then the right), matching the
                                // render loop's iteration so indices line up.
                                let labels: Vec<Arc<Galley>> = columns
                                    .iter()
                                    .flatten()
                                    .map(|&(name, json)| {
                                        ui.fonts(|f| {
                                            f.layout_no_wrap(
                                                template_label(name, json),
                                                body_font.clone(),
                                                text_color,
                                            )
                                        })
                                    })
                                    .collect();
                                let (row_gap, button_h, glyph_sz, pad, glyph_gap, col_gap) =
                                    (8.0, 34.0, 16.0, 10.0, 10.0, 20.0);
                                // A configured provider gets a right-aligned
                                // check; reserve its column so the grid width
                                // doesn't depend on what's set up.
                                let check_sz = 14.0;
                                let added =
                                    |name: &str| self.providers.iter().any(|p| p.name == name);
                                let max_label =
                                    labels.iter().map(|g| g.size().x).fold(0.0, f32::max);
                                let cell_w = pad * 2.0
                                    + glyph_sz
                                    + glyph_gap
                                    + max_label
                                    + glyph_gap
                                    + check_sz;
                                let grid_w = cell_w * 2.0 + col_gap;
                                let rows = columns.iter().map(|c| c.len()).max().unwrap_or(0);
                                let grid_h = rows as f32 * button_h
                                    + rows.saturating_sub(1) as f32 * row_gap;

                                let cancel = summoned.then(|| {
                                    ui.fonts(|f| {
                                        f.layout_no_wrap(
                                            "Cancel".into(),
                                            body_font.clone(),
                                            secondary_color,
                                        )
                                    })
                                });

                                // headline, the grid, and (when summoned) a
                                // cancel — as one column.
                                let mut col = CenteredColumn::default();
                                col.galley(0.0, head, false);
                                col.reserve(22.0, vec2(grid_w, grid_h));
                                if let Some(c) = &cancel {
                                    col.reserve(20.0, c.size());
                                }
                                let rects = col.show(ui, transcript_rect, center_x);
                                let grid_rect = rects[0];

                                let mut li = 0;
                                for (c, column) in columns.iter().enumerate() {
                                    for (r, &(name, _json)) in column.iter().enumerate() {
                                        let x = grid_rect.min.x + c as f32 * (cell_w + col_gap);
                                        let yb = grid_rect.min.y + r as f32 * (button_h + row_gap);
                                        let cell = Rect::from_min_size(
                                            pos2(x, yb),
                                            vec2(cell_w, button_h),
                                        );
                                        let resp = ui
                                            .interact(
                                                cell,
                                                Id::new(("chat_onboard", name)),
                                                Sense::click(),
                                            )
                                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                                        if resp.hovered() {
                                            ui.painter().rect_filled(
                                                cell,
                                                CornerRadius::same(6),
                                                bubble_surface,
                                            );
                                        }
                                        let grect = Rect::from_min_size(
                                            pos2(x + pad, yb + (button_h - glyph_sz) / 2.0),
                                            vec2(glyph_sz, glyph_sz),
                                        );
                                        let tex = self.glyphs.get(&ctx, name, glyph_sz);
                                        ui.painter().image(tex.id, grect, uv, text_color);
                                        let label = labels[li].clone();
                                        ui.painter().galley(
                                            pos2(
                                                grect.max.x + glyph_gap,
                                                yb + (button_h - label.size().y) / 2.0,
                                            ),
                                            label,
                                            text_color,
                                        );
                                        // Right-aligned check for an already-
                                        // configured provider.
                                        if added(name) {
                                            let check = ui.fonts(|f| {
                                                f.layout_no_wrap(
                                                    Icon::DONE.icon.to_string(),
                                                    egui::FontId::monospace(check_sz),
                                                    secondary_color,
                                                )
                                            });
                                            ui.painter().galley(
                                                pos2(
                                                    cell.max.x - pad - check.size().x,
                                                    yb + (button_h - check.size().y) / 2.0,
                                                ),
                                                check,
                                                secondary_color,
                                            );
                                        }
                                        if resp.clicked() {
                                            onboard_pick = Some(name);
                                        }
                                        li += 1;
                                    }
                                }

                                if let Some(cancel) = cancel {
                                    let rect = rects[1];
                                    let resp = ui
                                        .interact(
                                            rect,
                                            Id::new("chat_chooser_cancel"),
                                            Sense::click(),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                                    let color =
                                        if resp.hovered() { text_color } else { secondary_color };
                                    ui.painter().galley(rect.min, cancel, color);
                                    if resp.clicked() {
                                        chooser_cancel = true;
                                    }
                                }
                            }
                            Onboard::Connecting(l)
                            | Onboard::Unreachable { label: l, .. }
                            | Onboard::PickModel(l) => {
                                let text = match stage {
                                    Onboard::Unreachable { local: true, .. } => {
                                        format!("Can't reach {l}. Is the server running?")
                                    }
                                    Onboard::Unreachable { .. } => format!(
                                        "Can't reach {l}. Check your internet connection, then \
                                         come back."
                                    ),
                                    Onboard::PickModel(_) => {
                                        format!("Connected to {l}. Pick a model below to start.")
                                    }
                                    _ => format!("Connecting to {l}…"),
                                };
                                let mut col = CenteredColumn::default();
                                col.galley(0.0, para(ui, text, secondary_color), true);
                                col.show(ui, transcript_rect, center_x);
                            }
                            // A missing key needs no centered card — the
                            // composer's "add key" button is the whole story.
                            Onboard::NeedKey { .. } => {}
                        }
                    } else if visible.is_empty()
                        && self.unshared
                        && self.config_loaded
                        && self.provider.is_some()
                    {
                        // Empty chat, provider ready: an ambient marker of who
                        // you're about to talk to — the provider's mark, the
                        // model, and where messages go — instead of a blank
                        // canvas.
                        let ctx = ui.ctx().clone();
                        let center_x = note_x + note_wrap_w / 2.0;
                        let card_w = note_wrap_w.min(340.0);

                        let p = self.provider.as_ref().unwrap();
                        let (name, label) = (p.name.clone(), p.label());
                        let local =
                            p.base_url.contains("localhost") || p.base_url.contains("127.0.0.1");
                        // The model's display name once the listing lands;
                        // its id until then — same fallback as the toolbar.
                        let model = self
                            .models
                            .as_ref()
                            .filter(|((n, u), _)| *n == p.name && *u == p.base_url)
                            .and_then(|(_, list)| list.iter().find(|m| m.id == p.model))
                            .map(|m| m.label().to_string())
                            .unwrap_or_else(|| p.model.clone());

                        let head = ui.fonts(|f| {
                            f.layout_no_wrap(model, egui::FontId::proportional(16.0), text_color)
                        });
                        let sub_text = if local {
                            "Messages stay on this device.".to_string()
                        } else {
                            format!("Messages you send go to {label}.")
                        };
                        let sub = {
                            let mut job = egui::text::LayoutJob::simple(
                                sub_text,
                                egui::FontId::proportional(13.5),
                                secondary_color,
                                card_w,
                            );
                            job.halign = egui::Align::Center;
                            ui.fonts(|f| f.layout_job(job))
                        };

                        let glyph_sz = 28.0;
                        let tex = self.glyphs.get(&ctx, &name, glyph_sz);
                        let mut col = CenteredColumn::default();
                        col.glyph(0.0, tex.id, glyph_sz, text_color);
                        col.galley(14.0, head, false);
                        col.galley(8.0, sub, true);
                        col.show(ui, transcript_rect, center_x);
                    }
                });
        });

        // A bare-canvas tap dismisses the keyboard (composer surrenders
        // focus; iOS hides the on-screen keyboard).
        if backdrop_tapped {
            ui.memory_mut(|m| m.surrender_focus(composer_id));
            ui.ctx().set_virtual_keyboard_shown(false);
        }

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
                self.delete_cascade(i);
                changed = true;
                {
                    let (seed, buffers) = self.agent_seed();
                    if let Some(harness) = &mut self.harness {
                        harness.reseed(seed, buffers);
                    }
                }
            }
            Some(RowAction::Switch { parent, target, vi, anchor_y }) => {
                if let Some(id) = self.entries[target].msg.id {
                    self.branch_choice.insert(parent, id);
                    self.branch_anchor = Some((vi, anchor_y, false));
                    {
                        let (seed, buffers) = self.agent_seed();
                        if let Some(harness) = &mut self.harness {
                            harness.reseed(seed, buffers);
                        }
                    }
                }
            }
            Some(RowAction::Edit(i)) => self.enter_edit(i, ui, composer_id),
            Some(RowAction::ResendFrom(i)) => {
                self.resend_as_sibling(i);
                changed = true;
            }
            Some(RowAction::RetryLast) => retry_clicked = true,
            Some(RowAction::ToggleTool { id, vi, anchor_y }) => {
                if !self.expanded_tools.remove(&id) {
                    self.expanded_tools.insert(id);
                }
                self.branch_anchor = Some((vi, anchor_y, true));
            }
            _ => {}
        }

        // Resolve the pending edit.
        if approve_clicked || deny_clicked {
            if let Some(harness) = &mut self.harness {
                if approve_clicked {
                    harness.approve();
                } else {
                    harness.deny();
                }
            }
        }

        // A clicked listing row opens the note in the workspace, resolved
        // through the same file cache markdown links use.
        if let Some(path) = open_list_path {
            use crate::file_cache::FilesExt as _;
            use crate::tab::ExtendedOutput as _;
            let target = self
                .composer
                .renderer
                .files
                .read()
                .unwrap()
                .by_path(&path)
                .filter(|f| f.is_document())
                .map(|f| f.id);
            if let Some(id) = target {
                ui.ctx().open_file(id, true);
            }
        }

        {
            if retry_clicked {
                // Re-run the turn behind the tail error row. Routed through
                // regenerate so a turn that failed mid-tool-chain reseeds
                // from its invoking message, not mid-turn.
                if let Some(idx) = self.visible().last().map(|row| row.idx) {
                    self.regenerate(idx);
                }
            }
            if let Some(name) = onboard_pick {
                self.chooser_open = false;
                self.begin_add_provider(name);
                // The selection config entry must reach a save, same as the
                // add-provider path.
                changed = true;
            }
            if chooser_cancel {
                self.chooser_open = false;
            }
            if key_submit {
                self.connect_provider_key();
            } else if key_cancel {
                // Cancel returns to the provider selector — pick a different
                // provider, or the same one to try again.
                self.key_entry = None;
                self.chooser_open = true;
            }
        }

        // The connect step's key field, rendered here in the top-level ui (not
        // in the transcript scroll area where its card is painted) so the
        // native iOS text bridge binds a caret and delivers keystrokes — a
        // scroll-nested editor gets a positioned keyboard but no working text.
        // `focused_mdedit_mut` + the text-interaction rect already point here.
        use crate::tab::ExtendedOutput as _;
        if self.key_entry.is_some() && self.key_field_rect.is_finite() {
            let key_field_id = Id::new("chat_key_field");
            // Focus + keyboard modeled on the find bar (the other text field
            // that appears without a tap): one-shot on open, edge-triggered
            // re-summon — never per-frame, which fights UIKit's own
            // first-responder state. The block waits for a laid-out (finite)
            // rect: on the step's first frame the native text view isn't
            // positioned on the field yet.
            if self.key_entry.as_ref().is_some_and(|e| e.needs_focus) {
                ui.memory_mut(|m| m.request_focus(key_field_id));
                ui.ctx().set_virtual_keyboard_shown(true);
                if let Some(e) = self.key_entry.as_mut() {
                    e.needs_focus = false;
                }
            }
            // The focus lock that keeps Tab / arrows (and so focus) in the
            // field is `post_render`'s, set every focused frame.
            self.key_field.show(ui, self.key_field_rect, key_field_id);
            let focused = ui.memory(|m| m.has_focus(key_field_id));
            // Re-summon the keyboard when focus returns after a dismissal
            // (swipe-down, then a tap back into the field) — edge-triggered on
            // the focused-with-keyboard-up latch, exactly like find.
            if focused && !self.key_entry.as_ref().is_some_and(|e| e.was_focused) {
                ui.ctx().set_virtual_keyboard_shown(true);
            }
            if let Some(e) = self.key_entry.as_mut() {
                e.was_focused = focused && keyboard_up;
            }
            // Newline (desktop Enter / the iOS return key, inserted as "\n")
            // submits rather than splitting the single-line field.
            if self.key_field.renderer.buffer.current.text.contains('\n') {
                let cleaned = self
                    .key_field
                    .renderer
                    .buffer
                    .current
                    .text
                    .replace('\n', "");
                self.key_field.set_text(&cleaned);
                self.connect_provider_key();
            }
            // Pasting a key is the whole gesture — submit on paste so there's
            // nothing left to click. Desktop-shaped: iOS paste arrives via the
            // FFI as a Replace, and the return key submits there.
            let pasted = ui
                .ctx()
                .input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))));
            let connecting = self.key_entry.as_ref().is_some_and(|e| e.connecting);
            if pasted
                && !connecting
                && !self
                    .key_field
                    .renderer
                    .buffer
                    .current
                    .text
                    .trim()
                    .is_empty()
            {
                self.connect_provider_key();
            }
        } else if hide_composer && self.key_entry.is_none() {
            // The chooser has no text input — keyboard away (also the connect
            // step's dismissal path). Not while the step awaits first layout:
            // that would race the one-shot summon right behind it.
            ui.ctx().set_virtual_keyboard_shown(false);
        }

        // Composer bubble: text on top, a Zed-style toolbar row at the
        // bottom (model dropdowns left, send/stop right).
        let composer_rect = Rect::from_min_max(
            pos2(
                full_rect.min.x,
                full_rect.max.y - composer_height - TOOLBAR_H - composer_bottom_inset,
            ),
            full_rect.max,
        );
        self.composer_rect = if hide_composer { Rect::NOTHING } else { composer_rect };
        let col_pad = (available_width - col_width) / 2.0;
        // The bubble spans the content column, like the message rows.
        let h_inset = col_pad + H_MARGIN;
        let bubble_rect = Rect::from_min_max(
            pos2(composer_rect.min.x + h_inset, composer_rect.min.y),
            pos2(composer_rect.max.x - h_inset, composer_rect.max.y - composer_bottom_inset),
        );
        if !hide_composer {
            ui.painter()
                .rect_filled(bubble_rect, CornerRadius::same(CORNER), bubble_surface);
        }

        // Composer draw (text area above the toolbar). Submits its own text
        // callback internally. Skipped with no usable key so the connect
        // step's field is the sole text input the iOS bridge sees.
        let text_rect = Rect::from_min_max(
            bubble_rect.min,
            pos2(bubble_rect.max.x, bubble_rect.max.y - TOOLBAR_H),
        );
        // Inset the sides and top, but run the bottom to the toolbar rather
        // than insetting it. The composer is top-anchored, so this rect's
        // bottom is purely the glyphon clip — insetting it by `V_PAD` pinned
        // the clip onto the last line's box and shaved descenders (g/y tails,
        // worst on hi-DPI). The empty space above the toolbar is the padding.
        let inner_rect = Rect::from_min_max(
            text_rect.min + vec2(H_PAD, V_PAD),
            pos2(text_rect.max.x - H_PAD, text_rect.max.y),
        );
        // When the key is broken the text field is replaced by an accent
        // "add key" button — a control, never a place to type a secret.
        // Clicking it opens the dedicated masked field.
        let mut add_key_clicked = false;
        if let Some((_, label)) = &need_key {
            let btn_rect = Rect::from_min_max(
                text_rect.min + vec2(H_PAD, V_PAD),
                pos2(text_rect.max.x - H_PAD, text_rect.max.y - V_PAD),
            );
            let resp = ui
                .interact(btn_rect, Id::new("chat_add_key"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let accent = theme.bg().get_color(theme.prefs().primary);
            ui.painter()
                .rect_filled(btn_rect, CornerRadius::same(8), accent);
            let btn_label = ui.fonts(|f| {
                f.layout_no_wrap(
                    format!("Fix your {label} key"),
                    egui::FontId::proportional(15.0),
                    text_color,
                )
            });
            ui.painter()
                .galley(btn_rect.center() - btn_label.size() / 2.0, btn_label, text_color);
            add_key_clicked = resp.clicked();
        } else if !hide_composer {
            self.composer.show(ui, inner_rect, composer_id);
        }

        // A send that can't produce a reply is blocked rather than silently
        // swallowed: no provider resolves (deleted file, dangling selection),
        // the provider was never given a key, or no model is picked. The
        // reason doubles as the composer hint. Transient states (listing
        // loading, server unreachable) don't block — those sends fail loudly
        // as error rows with a retry.
        let send_block: Option<String> =
            if self.unshared && self.harness.is_some() && self.config_loaded {
                match &self.provider {
                    None => Some("pick an AI provider to start".into()),
                    Some(p) if p.model.trim().is_empty() => Some("pick a model to start".into()),
                    _ => None,
                }
            } else {
                None
            };

        // Ghosted placeholder over the empty composer (not while the field is
        // the "add key" button).
        if !hide_composer
            && need_key.is_none()
            && self.composer.renderer.buffer.current.text.is_empty()
        {
            let row_h = self.composer.row_height();
            let hint_text = send_block.as_deref().unwrap_or("Type a message");
            let hint = ui.fonts(|f| {
                f.layout_no_wrap(
                    hint_text.into(),
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
        // this user's devices; credentials never do. The provider dropdown
        // renders even with nothing resolved ("select provider") — a deleted
        // provider must leave a recovery path in a chat that has messages,
        // where the onboarding chooser can't show.
        if self.unshared && self.config_loaded && !hide_composer {
            let current = indicator.as_ref();
            let mut cursor_x = toolbar_rect.min.x;

            // The listing feeds the ring, the model button's display name,
            // and the picker — fetch as soon as the toolbar shows, not on
            // first picker open. The cache makes this a per-frame no-op.
            // Fetch off the *live* provider, not the frame-start `indicator`
            // clone: a key the connect step applied this frame must reach the
            // request now, or validation runs against the stale key.
            if let Some(live) = self.provider.clone() {
                self.fetch_models(&live);
            }

            // Context usage ring, Zed-style: the last turn's tokens against
            // the model's window, when the /models listing reports one — no
            // data means no ring, never a guess.
            let window = current.and_then(|current| {
                self.models.as_ref().and_then(|((name, url), list)| {
                    (name == &current.name && url == &current.base_url)
                        .then(|| {
                            list.iter()
                                .find(|m| m.id == current.model)
                                .and_then(|m| m.window)
                        })
                        .flatten()
                })
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
                        text_color,
                    )
                });
                let chevron = ui.fonts(|f| {
                    f.layout_no_wrap(
                        Icon::CHEVRON_DOWN.icon.to_string(),
                        egui::FontId::monospace(12.0),
                        text_color,
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
                    .galley(pos2(rect.min.x, text_top), galley, text_color);
                ui.painter().galley(
                    pos2(rect.max.x - chevron.size().x, text_top),
                    chevron,
                    text_color,
                );
                resp
            };

            let mut pick: Option<(String, String)> = None;

            // Menu rows in the main foreground color; the current row in the
            // accent so selection still reads without dimming the rest.
            let accent = theme.fg().get_color(theme.prefs().primary);
            let row_text = |text: &str, selected: bool| {
                egui::RichText::new(text).color(if selected { accent } else { text_color })
            };

            // The toolbar hugs the bottom of the window, so menus open
            // upward — downward they'd clamp to a couple of rows.
            let mut open_chooser = false;
            let mut open_prompt = false;
            let mut clear_chat = false;
            let provider_label = current
                .map(|c| c.label())
                .unwrap_or_else(|| "select provider".to_string());
            let provider_resp = dropdown(ui, "chat_provider_btn", &provider_label);
            let glyphs = &mut self.glyphs;
            // A row with the provider's brand mark, tinted like its text —
            // similar names in this space (Groq vs Grok) make a wordlist
            // menu genuinely confusable.
            let mut glyph_row = |ui: &mut egui::Ui, name: &str, label: &str, selected: bool| {
                let image = egui::Image::from_texture(glyphs.get(ui.ctx(), name, 14.0))
                    .fit_to_exact_size(egui::vec2(14.0, 14.0))
                    .tint(if selected { text_color } else { secondary_color });
                ui.add(egui::Button::image_and_text(image, row_text(label, selected)))
            };
            egui::Popup::menu(&provider_resp)
                .align(egui::RectAlign::TOP_START)
                .show(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);
                    ui.set_min_width(140.0);
                    // Same grouping and order as the add-provider menu;
                    // names it doesn't know trail in their own group.
                    let providers = &self.providers;
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
                            let selected = current.is_some_and(|c| c.name == p.name);
                            if glyph_row(ui, &p.name, &p.label(), selected).clicked() {
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
                    // Opens the onboarding chooser over the transcript — one
                    // canonical add-provider surface (glyph grid) instead of
                    // a cramped nested wordlist.
                    if ui.button(row_text("add provider", false)).clicked() {
                        open_chooser = true;
                        ui.close();
                    }
                });

            // Model and effort need a resolved provider; the picker above is
            // the whole toolbar until one resolves.
            let mut effort_pick: Option<String> = None;
            if let Some(current) = current {
                // The button shows the selected model's display name when the
                // listing has landed; the id (what's actually configured) until
                // then, or when the endpoint doesn't offer names. An empty model
                // is an unselected provider (a local server whose installed
                // models we can't guess) — prompt a pick from the listing.
                let model_label = if current.model.is_empty() {
                    "select model".to_string()
                } else {
                    self.models
                        .as_ref()
                        .filter(|((name, url), _)| {
                            name == &current.name && url == &current.base_url
                        })
                        .and_then(|(_, list)| list.iter().find(|m| m.id == current.model))
                        .map(|m| m.label().to_string())
                        .unwrap_or_else(|| current.model.clone())
                };
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
                                    ui.weak(
                                        "no model listing — set \"model\" in the provider file",
                                    );
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

                // Reasoning-effort dropdown, only where the model reasons. The
                // button shows the effective level (file default or per-chat
                // pick), "effort" when none.
                if effort_available(current) {
                    // Self-labeling ("effort: high"): the level alone wouldn't
                    // read as an effort control next to provider/model names.
                    let label =
                        format!("effort: {}", current.effort.as_deref().unwrap_or(EFFORT_AUTO));
                    let effort_resp = dropdown(ui, "chat_effort_btn", &label).on_hover_text(
                        "how hard the model reasons before replying — auto uses its own default",
                    );
                    egui::Popup::menu(&effort_resp)
                        .align(egui::RectAlign::TOP_START)
                        .show(|ui| {
                            ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);
                            ui.set_min_width(100.0);
                            if ui
                                .button(row_text(EFFORT_AUTO, current.effort.is_none()))
                                .clicked()
                            {
                                effort_pick = Some(EFFORT_AUTO.into());
                                ui.close();
                            }
                            for level in EFFORT_LEVELS {
                                let selected = current.effort.as_deref() == Some(*level);
                                if ui.button(row_text(level, selected)).clicked() {
                                    effort_pick = Some((*level).into());
                                    ui.close();
                                }
                            }
                        });
                }
            }

            // Conversation-scoped actions live in a ⋯ overflow at the
            // toolbar's right, beside send — not in the provider picker,
            // which stays a pure provider list. Right-aligned so the rare
            // actions sit away from the per-message controls.
            {
                let d = TOOLBAR_H - 8.0;
                let center =
                    pos2(toolbar_rect.max.x - d - STRIP_GAP - d / 2.0, toolbar_rect.center().y);
                let more_rect = Rect::from_center_size(center, vec2(d, d));
                let more_resp = ui
                    .interact(more_rect, Id::new("chat_more_btn"), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let color = if more_resp.hovered() { text_color } else { secondary_color };
                let glyph = ui.fonts(|f| {
                    f.layout_no_wrap(
                        Icon::DOTS_HORIZONTAL.icon.to_string(),
                        egui::FontId::monospace(15.0),
                        color,
                    )
                });
                ui.painter()
                    .galley(more_rect.center() - glyph.size() / 2.0, glyph, color);
                egui::Popup::menu(&more_resp)
                    .align(egui::RectAlign::TOP_END)
                    .show(|ui| {
                        ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);
                        ui.set_min_width(140.0);
                        // The system prompt, as an editable file. "system
                        // prompt" is the real term and reads unambiguously to
                        // anyone who's used an agent; the tooltip covers the
                        // rest and names the file it opens.
                        let resp = ui.button(row_text("system prompt", false)).on_hover_text(
                            "what the agent is told before every chat — opens prompt.md to edit",
                        );
                        if resp.clicked() {
                            open_prompt = true;
                            ui.close();
                        }
                        // Reset-in-place: the file is the deliberate artifact,
                        // the conversation just its current contents. The
                        // confirm is a submenu so one stray click can't clear
                        // anything. Hidden mid-turn (like every other
                        // transcript mutation) and when there's nothing to
                        // clear.
                        if !self.entries.is_empty() && !agent_busy {
                            ui.menu_button(row_text("clear chat", false), |ui| {
                                ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);
                                let confirm =
                                    egui::RichText::new("delete all messages").color(error_color);
                                if ui.button(confirm).clicked() {
                                    clear_chat = true;
                                    ui.close();
                                }
                            });
                        }
                    });
            }

            if let Some((provider, model)) = pick {
                // The dropdown only lists configured providers (unconfigured
                // ones are filtered at load), so a pick always has a key.
                self.write_selection(provider, model);
                changed = true;
            }
            if let Some(effort) = effort_pick {
                self.write_effort(effort);
                changed = true;
            }
            if open_chooser {
                self.chooser_open = true;
            }
            // The composer's "add key" button opens the dedicated masked
            // field for the broken provider — the only secret-entry surface.
            if add_key_clicked {
                if let Some((name, label)) = &need_key {
                    if let Ok(file) = self
                        .core
                        .get_by_path(&format!("/.agent/providers/{name}.json"))
                    {
                        self.open_key_entry(name, label.clone(), file.id);
                    }
                }
            }
            if open_prompt {
                self.create_prompt_file();
            }
            if clear_chat {
                self.clear_chat();
                changed = true;
            }
        }

        // Send/stop at the toolbar's right end. While a turn streams the
        // button is a stop square (the reply keeps what streamed so far).
        let non_empty = !self.composer.renderer.buffer.current.text.trim().is_empty();
        let mut send_clicked = false;
        let mut stop_clicked = false;
        // No send button while the field is the "add key" button.
        if !hide_composer && need_key.is_none() {
            let d = if touch_os { TOOLBAR_H - 2.0 } else { TOOLBAR_H - 8.0 };
            let center = pos2(toolbar_rect.max.x - d / 2.0, toolbar_rect.center().y);
            let button_rect = Rect::from_center_size(center, vec2(d, d));
            // Touch targets stay ~44pt even where the visual is smaller.
            let hit_rect = if touch_os { button_rect.expand(7.0) } else { button_rect };
            let active = (non_empty && send_block.is_none()) || agent_busy;
            let resp = ui.interact(hit_rect, Id::new("chat_send"), Sense::click());
            // A pointer cursor when it'll do something (send, or stop a turn).
            let resp =
                if active { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp };

            let painter = ui.painter();
            // The markdown-toolbar icon idiom — no filled disc, just the mark
            // itself: accent when actionable, foreground when idle.
            let mark = if active {
                theme.fg().get_color(theme.prefs().primary)
            } else {
                theme.neutral_fg()
            };
            if agent_busy {
                let side = d * 0.36;
                painter.rect_filled(
                    Rect::from_center_size(center, vec2(side, side)),
                    CornerRadius::same(2),
                    mark,
                );
                stop_clicked = resp.clicked();
            } else {
                let icon = ui.fonts(|f| {
                    f.layout_no_wrap(
                        Icon::SEND.icon.to_string(),
                        egui::FontId::monospace(d * 0.55),
                        mark,
                    )
                });
                painter.galley(center - icon.size() / 2.0, icon, mark);
                send_clicked = resp.clicked();
            }
        }

        if stop_clicked {
            if let Some(harness) = &mut self.harness {
                harness.stop();
            }
        }

        let sent = (send_requested || send_clicked)
            && !agent_busy
            && send_block.is_none()
            && self.submit(ui, composer_id);

        // Popups land last so they composite over composer + transcript.
        self.composer.show_completions(ui);

        // The one text field's rect, for the native text overlay: the connect
        // step's key field, else the composer, else nothing (the chooser has
        // no text input, so the keyboard should stay down).
        //
        // For the composer, report the whole bubble text region (`text_rect`),
        // not the padded layout rect — this frame is the iOS text view's, so
        // it doubles as the tap-to-focus / double-tap-to-select gesture
        // target, and the padding margins should be tappable too (touch→buffer
        // mapping is hit-tested separately, so a larger frame is safe). It
        // stops at the toolbar so the dropdowns and send button keep their own
        // egui taps.
        let interaction_rect = if self.key_entry.is_some() {
            self.key_field_hit_rect
        } else if hide_composer {
            Rect::NOTHING
        } else {
            text_rect
        };
        // The composer's `seq` bumps on any text/selection change. When it
        // moved but no native keystroke drove it (a send-clear, edit-prefill,
        // stash-restore), the native text view still holds the old caret —
        // report so the bridge re-syncs.
        let composer_seq = self.composer.renderer.buffer.current.seq;
        let composer_updated = composer_seq != self.composer_seq;
        self.composer_seq = composer_seq;

        (sent || agent_changed || changed, interaction_rect, composer_updated)
    }
}
