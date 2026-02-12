use basic_human_duration::ChronoHumanDuration;
use egui::os::OperatingSystem;
use egui::{
    Align2, DragAndDrop, EventFilter, Galley, Id, Image, Key, LayerId, Modifiers, Order, Rangef,
    Rect, RichText, Sense, TextStyle, TextWrapMode, ViewportCommand, include_image, vec2,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::mem;
use tracing::instrument;
use web_time::{Duration, Instant};

use crate::output::Response;
use crate::tab::{ContentState, TabContent, TabStatus, core_get_by_relative_path, image_viewer};
use crate::theme::icons::Icon;
use crate::theme::palette_v2::ThemeExt;
use crate::widgets::IconButton;
use crate::workspace::Workspace;

impl Workspace {
    #[instrument(level = "trace", skip_all)]
    pub fn show(&mut self, ui: &mut egui::Ui) -> Response {
        if self.ctx.input(|inp| !inp.raw.events.is_empty()) {
            self.user_last_seen = Instant::now();
        }

        self.set_tooltip_visibility(ui);

        self.process_lb_updates();
        self.process_task_updates();
        self.process_keys();
        self.status.message = self.status_message();

        if self.is_empty() {
            if self.show_tabs {
                self.show_landing_page(ui);
            } else {
                self.show_mobile_landing_page(ui);
            }
        } else {
            ui.centered_and_justified(|ui| self.show_tabs(ui));
        }
        if self.out.tabs_changed || self.current_tab_changed {
            self.cfg.set_tabs(&self.tabs, self.current_tab);
        }

        mem::take(&mut self.out)
    }

    fn set_tooltip_visibility(&mut self, ui: &mut egui::Ui) {
        let has_touch = ui.input(|r| {
            r.events.iter().any(|e| {
                matches!(e, egui::Event::Touch { device_id: _, id: _, phase: _, pos: _, force: _ })
            })
        });
        if has_touch && self.last_touch_event.is_none() {
            self.last_touch_event = Some(Instant::now());
        }

        if let Some(last_touch_event) = self.last_touch_event {
            if Instant::now() - last_touch_event > Duration::from_secs(5) {
                self.ctx
                    .style_mut(|style| style.interaction.tooltip_delay = 0.0);
                self.last_touch_event = None;
            } else {
                self.ctx
                    .style_mut(|style| style.interaction.tooltip_delay = f32::MAX);
            }
        }
    }

    fn show_mobile_landing_page(&mut self, ui: &mut egui::Ui) {
        let punchout = if ui.visuals().dark_mode {
            include_image!("../punchout-dark.png")
        } else {
            include_image!("../punchout-light.png")
        };

        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                let image_size = egui::vec2(200.0, 200.0);
                ui.add(Image::new(punchout).fit_to_exact_size(image_size));
                ui.add_space(120.0);

                ui.label(
                    RichText::new("TOOLS")
                        .small()
                        .weak()
                        .text_style(egui::TextStyle::Button),
                );
                ui.add_space(24.0);

                let is_beta = self
                    .core
                    .get_account()
                    .map(|a| a.is_beta())
                    .unwrap_or_default();
                if is_beta
                    && ui
                        .add_sized(
                            [200.0, 44.0],
                            egui::Button::new(RichText::new("Mind Map").size(18.0)),
                        )
                        .clicked()
                {
                    self.upsert_mind_map(self.core.clone());
                }
                ui.add_space(12.0);

                if ui
                    .add_sized(
                        [200.0, 44.0],
                        egui::Button::new(RichText::new("Space Inspector").size(18.0)),
                    )
                    .clicked()
                {
                    self.start_space_inspector(self.core.clone(), None);
                }
            });
        });
    }

    fn show_tabs(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

        ui.vertical(|ui| {
            if self.current_tab().is_some() && self.show_tabs {
                self.show_tab_strip(ui);
            }

            ui.centered_and_justified(|ui| {
                let mut open_id = None;
                let mut new_tab = false;
                if let Some(tab) = self.current_tab_mut() {
                    let id = tab.id();
                    match &mut tab.content {
                        ContentState::Loading(_) => {
                            ui.spinner();
                        }
                        ContentState::Failed(fail) => {
                            ui.label(fail.msg());
                        }
                        ContentState::Open(content) => {
                            match content {
                                TabContent::Markdown(md) => {
                                    let initialized = md.initialized;
                                    let resp = md.show(ui);
                                    // The editor signals a text change when the buffer is initially
                                    // loaded. Since we use that signal to trigger saves, we need to
                                    // check that this change was not from the initial frame.
                                    if !tab.read_only && resp.text_updated && initialized {
                                        tab.last_changed = Instant::now();
                                    }

                                    self.out.open_camera = resp.open_camera;

                                    if resp.text_updated {
                                        self.out.markdown_editor_text_updated = true;
                                        self.out.markdown_editor_selection_updated = true;
                                    }
                                    if resp.selection_updated {
                                        self.out.markdown_editor_selection_updated = true;
                                    }
                                    if resp.scroll_updated {
                                        self.out.markdown_editor_scroll_updated = true;
                                    }
                                }
                                TabContent::Image(img) => {
                                    if let Err(err) = img.show(ui) {
                                        tab.content = ContentState::Failed(err.into());
                                    }
                                }
                                TabContent::Pdf(pdf) => pdf.show(ui),
                                TabContent::Svg(svg) => {
                                    let res = svg.show(ui);
                                    if res.request_save {
                                        tab.last_changed = Instant::now();
                                    }
                                }

                                #[cfg(not(target_family = "wasm"))]
                                TabContent::MindMap(mm) => {
                                    let response = mm.show(ui);
                                    if let Some(value) = response {
                                        self.open_file(value, true, false);
                                    }
                                }
                                TabContent::SpaceInspector(sv) => {
                                    sv.show(ui);
                                }
                            };
                        }
                    }

                    ui.ctx().output_mut(|w| {
                        if let Some(url) = &w.open_url {
                            // only intercept open urls for tabs representing files
                            let Some(id) = id else {
                                return;
                            };

                            // lookup this file so we can get the parent
                            let Ok(file) = self.core.get_file_by_id(id) else {
                                return;
                            };

                            // evaluate relative path based on parent location
                            let Ok(file) =
                                core_get_by_relative_path(&self.core, file.parent, &url.url)
                            else {
                                return;
                            };

                            // if all that found something then open within lockbook
                            open_id = Some(file.id);
                            new_tab = url.new_tab;

                            w.open_url = None;
                        }
                    });
                }
                if let Some(id) = open_id {
                    self.open_file(id, true, new_tab);
                }
            });
        });
    }

    fn show_tab_strip(&mut self, ui: &mut egui::Ui) {
        let active_tab_changed = self.current_tab_changed;
        self.current_tab_changed = false;

        let mut back = false;
        let mut forward = false;

        let cursor = ui
            .horizontal(|ui| {
                if IconButton::new(Icon::ARROW_LEFT)
                    .disabled(
                        self.current_tab()
                            .map(|tab| tab.back.is_empty())
                            .unwrap_or_default(),
                    )
                    .size(37.)
                    .tooltip("Go Back")
                    .show(ui)
                    .clicked()
                {
                    back = true;
                }
                if IconButton::new(Icon::ARROW_RIGHT)
                    .disabled(
                        self.current_tab()
                            .map(|tab| tab.forward.is_empty())
                            .unwrap_or_default(),
                    )
                    .size(37.)
                    .tooltip("Go Forward")
                    .show(ui)
                    .clicked()
                {
                    forward = true;
                }

                egui::ScrollArea::horizontal()
                    .max_width(ui.available_width())
                    .show(ui, |ui| {
                        let mut responses = HashMap::new();
                        for i in 0..self.tabs.len() {
                            if let Some(resp) =
                                self.tab_label(ui, i, self.current_tab == i, active_tab_changed)
                            {
                                responses.insert(i, resp);
                            }
                        }

                        // handle responses after showing all tabs because closing a tab invalidates tab indexes
                        for (i, resp) in responses {
                            match resp {
                                TabLabelResponse::Clicked => {
                                    if self.current_tab == i {
                                        // we should rename the file.

                                        self.out.tab_title_clicked = true;
                                        let active_name = self.tab_title(&self.tabs[i]);

                                        let mut rename_edit_state =
                                            egui::text_edit::TextEditState::default();
                                        rename_edit_state.cursor.set_char_range(Some(
                                            egui::text::CCursorRange {
                                                primary: egui::text::CCursor::new(
                                                    active_name
                                                        .rfind('.')
                                                        .unwrap_or(active_name.len()),
                                                ),
                                                secondary: egui::text::CCursor::new(0),
                                            },
                                        ));
                                        egui::TextEdit::store_state(
                                            ui.ctx(),
                                            egui::Id::new("rename_tab"),
                                            rename_edit_state,
                                        );
                                        self.tabs[i].rename = Some(active_name);
                                    } else {
                                        self.tabs[i].rename = None;
                                        self.make_current(i);
                                    }
                                }
                                TabLabelResponse::Closed => {
                                    self.close_tab(i);
                                }
                                TabLabelResponse::Renamed(name) => {
                                    self.tabs[i].rename = None;
                                    if let Some(id) = self.tabs[i].id() {
                                        self.rename_file((id, name.clone()), true);
                                    }
                                }
                                TabLabelResponse::Reordered { src, mut dst } => {
                                    let current = self.current_tab_id();

                                    let tab = self.tabs.remove(src);
                                    if src < dst {
                                        dst -= 1;
                                    }
                                    self.tabs.insert(dst, tab);

                                    if let Some(current) = current {
                                        self.make_current_by_id(current);
                                    }
                                }
                            }
                            ui.ctx().request_repaint();
                        }
                    });
                ui.cursor()
            })
            .inner;

        ui.style_mut().animation_time = 2.0;

        let end_of_tabs = cursor.min.x;
        let available_width = ui.available_width();
        let remaining_rect = Rect::from_x_y_ranges(
            Rangef { min: end_of_tabs, max: end_of_tabs + available_width },
            cursor.y_range(),
        );
        let sep_stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        let theme = self.ctx.get_lb_theme();

        let bg_color = theme.bg().grey;
        ui.painter().rect_filled(remaining_rect, 0.0, bg_color);

        ui.painter()
            .hline(remaining_rect.x_range(), cursor.max.y, sep_stroke);

        if back {
            self.back();
        }
        if forward {
            self.forward();
        }
    }

    fn process_keys(&mut self) {
        const APPLE: bool = cfg!(target_vendor = "apple");
        const COMMAND: Modifiers = Modifiers::COMMAND;
        const CTRL: Modifiers = Modifiers::CTRL;
        const SHIFT: Modifiers = Modifiers::SHIFT;
        const ALT: Modifiers = Modifiers::ALT;
        const NUM_KEYS: [Key; 10] = [
            Key::Num0,
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];

        // Ctrl-N pressed while new file modal is not open.
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND, egui::Key::N))
        {
            self.create_doc(false);
        }

        // Ctrl-S to save current tab.
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND, egui::Key::S))
        {
            self.save_tab(self.current_tab);
        }

        // Ctrl-M to open mind map
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND, egui::Key::M))
        {
            self.upsert_mind_map(self.core.clone());
        }

        // Ctrl-W to close current tab.
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND, egui::Key::W))
            && !self.is_empty()
        {
            self.close_tab(self.current_tab);
            self.ctx.send_viewport_cmd(ViewportCommand::Title(
                self.current_tab_title().unwrap_or("Lockbook".to_owned()),
            ));

            self.out.selected_file = self.current_tab_id();
        }

        // Ctrl-shift-W to close all tabs
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND | SHIFT, egui::Key::W))
            && !self.is_empty()
        {
            for i in 0..self.tabs.len() {
                self.close_tab(i);
            }

            self.out.selected_file = None;
            self.ctx
                .send_viewport_cmd(ViewportCommand::Title("Lockbook".into()));
        }

        // reorder tabs
        // non-apple: ctrl+shift+pg down / up
        // apple: command+control+shift [ ]
        let change: i32 = self.ctx.input_mut(|input| {
            if APPLE {
                if input.consume_key_exact(Modifiers::MAC_CMD | CTRL | SHIFT, Key::OpenBracket) {
                    -1
                } else if input
                    .consume_key_exact(Modifiers::MAC_CMD | CTRL | SHIFT, Key::CloseBracket)
                {
                    1
                } else {
                    0
                }
            } else if input.consume_key_exact(CTRL | SHIFT, Key::PageUp) {
                -1
            } else if input.consume_key_exact(CTRL | SHIFT, Key::PageDown) {
                1
            } else {
                0
            }
        });
        if change != 0 {
            let old = self.current_tab as i32;
            let new = old + change;
            if new >= 0 && new < self.tabs.len() as i32 {
                self.tabs.swap(old as usize, new as usize);
                self.make_current(new as usize);
            }
        }

        // tab navigation
        let mut goto_tab = None;
        self.ctx.input_mut(|input| {
            // Cmd+1 through Cmd+8 to select tab by cardinal index
            for (i, &key) in NUM_KEYS.iter().enumerate().skip(1).take(8) {
                if input.consume_key_exact(COMMAND, key)
                    || (!APPLE && input.consume_key_exact(Modifiers::ALT, key))
                {
                    goto_tab = Some(i.min(self.tabs.len()) - 1);
                }
            }

            // Cmd+9 to go to last tab
            if input.consume_key_exact(COMMAND, Key::Num9)
                || (!APPLE && input.consume_key_exact(Modifiers::ALT, Key::Num9))
            {
                goto_tab = Some(self.tabs.len() - 1);
            }

            // Cmd+Shift+[ or ctrl shift tab to go to previous tab
            if ((APPLE && input.consume_key_exact(COMMAND | SHIFT, Key::OpenBracket))
                || (!APPLE && input.consume_key_exact(CTRL | SHIFT, Key::Tab)))
                && self.current_tab != 0
            {
                goto_tab = Some(self.current_tab - 1);
            }

            // Cmd+Shift+] or ctrl tab to go to next tab
            if ((APPLE && input.consume_key_exact(COMMAND | SHIFT, Key::CloseBracket))
                || (!APPLE && input.consume_key_exact(CTRL, Key::Tab)))
                && self.current_tab != self.tabs.len() - 1
            {
                goto_tab = Some(self.current_tab + 1);
            }
        });

        if let Some(goto_tab) = goto_tab {
            self.make_current(goto_tab);
        }

        // forward/back
        // non-apple: alt + arrows
        // apple: command + brackets
        let mut back = false;
        let mut forward = false;
        self.ctx.input_mut(|input| {
            if APPLE {
                if input.consume_key_exact(COMMAND, Key::OpenBracket) {
                    back = true;
                }
                if input.consume_key_exact(COMMAND, Key::CloseBracket) {
                    forward = true;
                }
            } else {
                if input.consume_key_exact(ALT, Key::ArrowLeft) {
                    back = true;
                }
                if input.consume_key_exact(ALT, Key::ArrowRight) {
                    forward = true;
                }
            }
        });

        if back {
            self.back();
        }
        if forward {
            self.forward();
        }
    }

    fn tab_label(
        &mut self, ui: &mut egui::Ui, t: usize, is_active: bool, active_tab_changed: bool,
    ) -> Option<TabLabelResponse> {
        let mut result = None;
        let icon_size = 15.0;
        let x_icon = Icon::CLOSE.size(icon_size);
        let status = self.tab_status(t);

        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::new(14.0, egui::FontFamily::Proportional));

        let tab_bg = if is_active {
            ui.style().visuals.extreme_bg_color
        } else {
            self.ctx.get_lb_theme().bg().grey
        };

        let tab_padding = egui::Margin::symmetric(10.0, 10.0);

        let tab_label = egui::Frame::default()
            .fill(tab_bg)
            .inner_margin(tab_padding)
            .show(ui, |ui| {
                ui.add_visible_ui(self.tabs[t].rename.is_none(), |ui| {
                    let start = ui.available_rect_before_wrap().min;

                    // create galleys - text layout

                    // tab label - the actual file name
                    let text: egui::WidgetText = self.tab_title(&self.tabs[t]).into();
                    let text = text.into_galley(
                        ui,
                        Some(TextWrapMode::Truncate),
                        200.0,
                        egui::TextStyle::Body,
                    );

                    // tab marker - tab status / tab number
                    let tab_marker = if status == TabStatus::Clean {
                        (t + 1).to_string()
                    } else {
                        "*".to_string()
                    };
                    let tab_marker: egui::WidgetText = egui::RichText::new(tab_marker)
                        .font(egui::FontId::monospace(12.0))
                        .color(if status == TabStatus::Clean {
                            ui.style().visuals.weak_text_color()
                        } else {
                            ui.style().visuals.warn_fg_color
                        })
                        .into();
                    let tab_marker = tab_marker.into_galley(
                        ui,
                        Some(TextWrapMode::Extend),
                        f32::INFINITY,
                        egui::TextStyle::Body,
                    );

                    // close button - the x
                    let close_button: egui::WidgetText = egui::RichText::new(x_icon.icon)
                        .font(egui::FontId::monospace(10.))
                        .into();
                    let close_button = close_button.into_galley(
                        ui,
                        Some(TextWrapMode::Extend),
                        f32::INFINITY,
                        egui::TextStyle::Body,
                    );

                    // create rects - place these relative to one another
                    let marker_rect = centered_galley_rect(&tab_marker);
                    let marker_rect = Align2::LEFT_TOP.anchor_size(
                        start
                            + egui::vec2(
                                0.0,
                                text.rect.height() / 2.0 - marker_rect.height() / 2.0,
                            ),
                        marker_rect.size(),
                    );

                    let text_rect = egui::Align2::LEFT_TOP.anchor_size(
                        start + egui::vec2(tab_marker.rect.width() + 7.0, 0.0),
                        text.size(),
                    );

                    let close_button_rect = centered_galley_rect(&close_button);
                    let close_button_rect = egui::Align2::LEFT_TOP.anchor_size(
                        text_rect.right_top()
                            + vec2(5.0, (text.rect.height() - close_button_rect.height()) / 2.0),
                        close_button_rect.size(),
                    );

                    // tab label rect represents the whole tab label
                    let left_top = start - tab_padding.left_top();
                    let right_bottom =
                        close_button_rect.right_bottom() + tab_padding.right_bottom();
                    let tab_label_rect = Rect::from_min_max(left_top, right_bottom);

                    // uncomment to see geometry debug views
                    // let s = egui::Stroke::new(1., egui::Color32::RED);
                    // ui.painter().rect_stroke(marker_rect, 1., s);
                    // ui.painter().rect_stroke(text_rect, 1., s);
                    // ui.painter().rect_stroke(close_button_rect, 1., s);
                    // ui.painter().rect_stroke(tab_label_rect, 1., s);

                    // render & process input
                    let touch_mode =
                        matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);

                    ui.painter().galley(
                        marker_rect.left_top(),
                        tab_marker.clone(),
                        ui.visuals().text_color(),
                    );

                    let mut tab_label_resp = ui.interact(
                        tab_label_rect,
                        Id::new("tab label").with(t),
                        Sense { click: true, drag: true, focusable: false },
                    );

                    let pointer_pos = ui.input(|i| i.pointer.interact_pos().unwrap_or_default());
                    let close_button_interact_rect =
                        close_button_rect.expand(if touch_mode { 4. } else { 2. });
                    let close_button_pointed = close_button_interact_rect.contains(pointer_pos);
                    let close_button_hovered = tab_label_resp.hovered() && close_button_pointed;
                    let close_button_clicked = tab_label_resp.clicked() && close_button_pointed;

                    tab_label_resp.clicked &= !close_button_clicked;

                    let text_color = if is_active {
                        ui.visuals().text_color()
                    } else {
                        ui.visuals()
                            .widgets
                            .noninteractive
                            .fg_stroke
                            .color
                            .linear_multiply(0.8)
                    };

                    // draw the tab text
                    ui.painter().galley(text_rect.min, text, text_color);

                    if close_button_clicked || tab_label_resp.middle_clicked() {
                        result = Some(TabLabelResponse::Closed);
                    }
                    if close_button_hovered {
                        ui.painter().rect(
                            close_button_interact_rect,
                            2.0,
                            ui.visuals().code_bg_color,
                            egui::Stroke::NONE,
                        );
                    }

                    let show_close_button = touch_mode || tab_label_resp.hovered() || is_active;
                    if show_close_button {
                        ui.painter().galley(
                            close_button_rect.min,
                            close_button,
                            ui.visuals().text_color(),
                        );
                    }
                    if tab_label_resp.clicked() {
                        result = Some(TabLabelResponse::Clicked);
                    }
                    tab_label_resp.context_menu(|ui| {
                        if ui.button("Close tab").clicked() {
                            result = Some(TabLabelResponse::Closed);
                            ui.close_menu();
                        }
                    });

                    ui.advance_cursor_after_rect(text_rect.union(close_button_rect));

                    // drag 'n' drop
                    {
                        // when drag starts, dragged tab sets dnd payload
                        if tab_label_resp.dragged() && !DragAndDrop::has_any_payload(ui.ctx()) {
                            DragAndDrop::set_payload(ui.ctx(), t);
                        }

                        if let (Some(pointer), true) = (
                            ui.input(|i| i.pointer.interact_pos()),
                            DragAndDrop::has_any_payload(ui.ctx()),
                        ) {
                            let contains_pointer = tab_label_rect.contains(pointer);
                            if contains_pointer {
                                // during drag, drop target renders indicator
                                let drop_left_side = pointer.x < tab_label_rect.center().x;
                                let stroke = ui.style().visuals.widgets.active.fg_stroke;
                                let x = if drop_left_side {
                                    tab_label_rect.min.x
                                } else {
                                    tab_label_rect.max.x
                                };
                                let y_range = tab_label_rect.y_range();

                                ui.with_layer_id(
                                    LayerId::new(
                                        Order::Foreground,
                                        Id::from("tab_reorder_drop_indicator"),
                                    ),
                                    |ui| {
                                        ui.painter().vline(x, y_range, stroke);
                                    },
                                );

                                // when drag ends, dropped-on tab consumes dnd payload
                                if let Some(drag_index) =
                                    tab_label_resp.dnd_release_payload::<usize>()
                                {
                                    let drop_index = if drop_left_side { t } else { t + 1 };
                                    result = Some(TabLabelResponse::Reordered {
                                        src: *drag_index,
                                        dst: drop_index,
                                    });
                                }
                            }
                        }
                    }

                    tab_label_resp
                })
            });

        // renaming
        if let Some(ref mut str) = self.tabs[t].rename {
            let res = ui
                .allocate_ui_at_rect(tab_label.response.rect, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(str)
                            .font(TextStyle::Small)
                            .frame(false)
                            .id(egui::Id::new("rename_tab")),
                    )
                })
                .inner;

            if !res.has_focus() && !res.lost_focus() {
                // request focus on the first frame (todo: wrong but works)
                res.request_focus();
            }
            if res.has_focus() {
                // focus lock filter must be set every frame
                ui.memory_mut(|m| {
                    m.set_focus_lock_filter(
                        res.id,
                        EventFilter {
                            tab: true, // suppress 'tab' behavior
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            escape: false, // press 'esc' to release focus
                        },
                    )
                })
            }

            // submit
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                result = Some(TabLabelResponse::Renamed(str.to_owned()));
                // t.rename = None; is done by code processing this response
            }

            // release focus to cancel ('esc' or click elsewhere)
            if res.lost_focus() {
                self.tabs[t].rename = None;
            }
        }

        if is_active && active_tab_changed {
            tab_label.response.scroll_to_me(None);
        }

        if !is_active && tab_label.response.hovered() {
            // this logic probably needs to be brought to the icon forwad and back buttons
            ui.painter().rect_filled(
                tab_label.response.rect,
                0.0,
                egui::Color32::WHITE.linear_multiply(0.002),
            );
        }

        if is_active && active_tab_changed {
            tab_label.response.scroll_to_me(None);
        }

        // draw separators
        let sep_stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        if !is_active {
            ui.painter().hline(
                tab_label.response.rect.x_range(),
                tab_label.response.rect.max.y,
                sep_stroke,
            );
        }
        ui.painter().vline(
            tab_label.response.rect.max.x,
            tab_label.response.rect.y_range(),
            sep_stroke,
        );

        tab_label.response.on_hover_ui(|ui| {
            let text = self.tab_status(t).summary();
            let text: egui::WidgetText = RichText::from(text).size(15.0).into();
            let text = text.into_galley(ui, Some(TextWrapMode::Extend), 0., egui::TextStyle::Body);
            ui.add(egui::Label::new(text));

            let last_saved = self.tabs[t].last_saved.elapsed_human_string();
            let text: egui::WidgetText = RichText::from(format!("last saved {last_saved}"))
                .size(12.0)
                .into();
            let text = text.into_galley(ui, Some(TextWrapMode::Extend), 0., egui::TextStyle::Body);
            ui.add(egui::Label::new(text));

            ui.ctx().request_repaint_after_secs(1.0);
        });

        result
    }
}

/// egui, when rendering a single monospace symbol character doesn't seem to be able to center a character vertically
/// this fn takes into account where the text was positioned within the galley and computes a size using mesh_bounds
/// and retruns a rect with uniform padding.
fn centered_galley_rect(galley: &Galley) -> Rect {
    let min = galley.rect.min;
    let offset = galley.rect.min - galley.mesh_bounds.min;
    let max = galley.mesh_bounds.max - offset;

    Rect { min, max }
}

enum TabLabelResponse {
    Clicked,
    Closed,
    Renamed(String),
    Reordered { src: usize, dst: usize },
}

// The only difference from count_and_consume_key is that here we use matches_exact instead of matches_logical,
// preserving the behavior before egui 0.25.0. The documentation for the 0.25.0 count_and_consume_key says
// "you should match most specific shortcuts first", but this doesn't go well with egui's usual pattern where widgets
// process input in the order in which they're drawn, with parent widgets (e.g. workspace) drawn before children
// (e.g. editor). Using this older way of doing things affects matching keyboard shortcuts with shift included e.g. '+'
pub trait InputStateExt {
    fn count_and_consume_key_exact(
        &mut self, modifiers: egui::Modifiers, logical_key: egui::Key,
    ) -> usize;
    fn consume_key_exact(&mut self, modifiers: egui::Modifiers, logical_key: egui::Key) -> bool;
}

impl InputStateExt for egui::InputState {
    fn count_and_consume_key_exact(
        &mut self, modifiers: egui::Modifiers, logical_key: egui::Key,
    ) -> usize {
        let mut count = 0usize;

        self.events.retain(|event| {
            let is_match = matches!(
                event,
                egui::Event::Key {
                    key: ev_key,
                    modifiers: ev_mods,
                    pressed: true,
                    ..
                } if *ev_key == logical_key && ev_mods.matches_exact(modifiers)
            );

            count += is_match as usize;

            !is_match
        });

        count
    }

    fn consume_key_exact(&mut self, modifiers: egui::Modifiers, logical_key: egui::Key) -> bool {
        self.count_and_consume_key_exact(modifiers, logical_key) > 0
    }
}

pub trait ElapsedHumanString {
    fn elapsed_human_string(&self) -> String;
}

impl ElapsedHumanString for time::Duration {
    fn elapsed_human_string(&self) -> String {
        let minutes = self.whole_minutes();
        let seconds = self.whole_seconds();
        if seconds > 0 && minutes == 0 {
            if seconds <= 1 { "1 second ago".to_string() } else { format!("{seconds} seconds ago") }
        } else {
            self.format_human().to_string()
        }
    }
}

impl ElapsedHumanString for std::time::Duration {
    fn elapsed_human_string(&self) -> String {
        time::Duration::milliseconds(self.as_millis() as _).elapsed_human_string()
    }
}

impl ElapsedHumanString for Instant {
    fn elapsed_human_string(&self) -> String {
        time::Duration::milliseconds(self.elapsed().as_millis() as _).elapsed_human_string()
    }
}

impl ElapsedHumanString for u64 {
    fn elapsed_human_string(&self) -> String {
        time::Duration::milliseconds(lb_rs::model::clock::get_time().0 - *self as i64)
            .elapsed_human_string()
    }
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum DocType {
    PlainText,
    Markdown,
    SVG,
    Image,
    ImageUnsupported,
    Code,
    PDF,
    Unknown,
}

impl Display for DocType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocType::PlainText => write!(f, "Plain Text"),
            DocType::Markdown => write!(f, "Markdown"),
            DocType::SVG => write!(f, "SVG"),
            DocType::Image => write!(f, "Image"),
            DocType::ImageUnsupported => write!(f, "Image (Unsupported)"),
            DocType::Code => write!(f, "Code"),
            DocType::PDF => write!(f, "PDF"),
            DocType::Unknown => write!(f, "Unknown"),
        }
    }
}

impl DocType {
    pub fn from_name(name: &str) -> Self {
        let ext = name.split('.').next_back().unwrap_or_default();
        match ext {
            "draw" | "svg" => Self::SVG,
            "md" => Self::Markdown,
            "txt" => Self::PlainText,
            "cr2" => Self::ImageUnsupported,
            "go" => Self::Code,
            "pdf" => Self::PDF,
            _ if image_viewer::is_supported_image_fmt(ext) => Self::Image,
            _ => Self::Unknown,
        }
    }

    pub fn to_icon(&self) -> Icon {
        match self {
            DocType::Markdown => Icon::DOC_MD,
            DocType::PlainText => Icon::DOC_TEXT,
            DocType::SVG => Icon::DRAW,
            DocType::Image => Icon::IMAGE,
            DocType::Code => Icon::CODE,
            DocType::PDF => Icon::DOC_PDF,
            _ => Icon::DOC_UNKNOWN,
        }
    }

    pub fn hide_ext(&self) -> bool {
        match self {
            DocType::PlainText => false,
            DocType::Markdown => true,
            DocType::SVG => true,
            DocType::Image => false,
            DocType::ImageUnsupported => false,
            DocType::Code => false,
            DocType::PDF => true,
            DocType::Unknown => false,
        }
    }
}
