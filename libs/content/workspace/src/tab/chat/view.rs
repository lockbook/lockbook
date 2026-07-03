//! Rendering and hit-testing for the chat tab: the transcript's two-pass
//! measure→paint layout, the composer and toolbar, first-run guidance, and
//! touch geometry. All pixel-pushing, no data logic — the reviewable state
//! lives in the parent module and `config`.

use super::*;

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
    pub fn show(&mut self, ui: &mut Ui) -> (bool, Rect) {
        #[cfg(not(target_family = "wasm"))]
        self.refresh_provider_on_return();
        #[cfg(not(target_family = "wasm"))]
        self.pump_models();
        #[cfg(not(target_family = "wasm"))]
        self.pump_config();
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
        // Only once config has actually loaded — otherwise the hint flashes
        // for the frames before the background load lands.
        let show_agent_hint = self.unshared && self.config_loaded && self.provider.is_none();
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

                        // Action icons, innermost so a hover reveal shifts
                        // nothing beyond them. Only a pointer over the row
                        // can reach them, so hover-gated visibility never
                        // hides a reachable target.
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
                let system = self.system_prompt.clone();
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
            let mut add: Option<&'static str> = None;
            let mut open_prompt = false;
            let provider_resp = dropdown(ui, "chat_provider_btn", &current.label());
            let glyphs = &mut self.glyphs;
            // A row with the provider's brand mark, tinted like its text —
            // similar names in this space (Groq vs Grok) make a wordlist
            // menu genuinely confusable.
            let mut glyph_row = |ui: &mut egui::Ui, name: &str, label: &str, selected: bool| {
                let image = egui::Image::from_texture(glyphs.get(ui.ctx(), name))
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
                            if glyph_row(ui, &p.name, &p.label(), p.name == current.name).clicked()
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
                                if glyph_row(ui, name, &template_label(name, json), false).clicked()
                                {
                                    add = Some(name);
                                    ui.close();
                                }
                            }
                        }
                    });
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
                });

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
                    .filter(|((name, url), _)| name == &current.name && url == &current.base_url)
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

            // Reasoning-effort dropdown, only where the model reasons. The
            // button shows the effective level (file default or per-chat
            // pick), "effort" when none.
            let mut effort_pick: Option<String> = None;
            if effort_available(current) {
                // Self-labeling ("effort: high"): the level alone wouldn't
                // read as an effort control next to provider/model names.
                let label = format!("effort: {}", current.effort.as_deref().unwrap_or(EFFORT_AUTO));
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

            if let Some((provider, model)) = pick {
                self.write_selection(provider, model);
                changed = true;
            }
            if let Some(effort) = effort_pick {
                self.write_effort(effort);
                changed = true;
            }
            if let Some(name) = add {
                self.create_provider_file(name);
                changed = true;
            }
            if open_prompt {
                self.create_prompt_file();
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
