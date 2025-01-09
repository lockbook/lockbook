use basic_human_duration::ChronoHumanDuration;
use core::f32;
use egui::emath::easing;
use egui::os::OperatingSystem;
use egui::{EventFilter, Id, Key, Modifiers, Sense, TextWrapMode, ViewportCommand};
use std::collections::HashMap;
use std::mem;
use std::time::{Duration, Instant};

use crate::output::Response;
use crate::tab::{TabContent, TabFailure};
use crate::theme::icons::Icon;
use crate::widgets::Button;
use crate::workspace::Workspace;

impl Workspace {
    pub fn show(&mut self, ui: &mut egui::Ui) -> Response {
        if self.ctx.input(|inp| !inp.raw.events.is_empty()) {
            self.user_last_seen = Instant::now();
        }

        self.set_tooltip_visibility(ui);

        self.process_updates();
        self.process_keys();
        self.status.populate_message();

        if self.is_empty() {
            self.show_empty_workspace(ui);
        } else {
            ui.centered_and_justified(|ui| self.show_tabs(ui));
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

    fn show_empty_workspace(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(ui.clip_rect().height() / 3.0);

            ui.label(egui::RichText::new("Welcome to your Lockbook").size(40.0));
            ui.label(
                "Right click on your file tree to explore all that your lockbook has to offer",
            );

            ui.add_space(40.0);

            ui.visuals_mut().widgets.inactive.bg_fill = ui.visuals().widgets.active.bg_fill;
            ui.visuals_mut().widgets.hovered.bg_fill = ui.visuals().widgets.active.bg_fill;

            let text_stroke =
                egui::Stroke { color: ui.visuals().extreme_bg_color, ..Default::default() };
            ui.visuals_mut().widgets.inactive.fg_stroke = text_stroke;
            ui.visuals_mut().widgets.active.fg_stroke = text_stroke;
            ui.visuals_mut().widgets.hovered.fg_stroke = text_stroke;

            if Button::default()
                .text("New document")
                .rounding(egui::Rounding::same(3.0))
                .frame(true)
                .show(ui)
                .clicked()
            {
                self.create_file(false);
            }
            if Button::default()
                .text("New drawing")
                .rounding(egui::Rounding::same(3.0))
                .frame(true)
                .show(ui)
                .clicked()
            {
                self.create_file(true);
            }
            ui.visuals_mut().widgets.inactive.fg_stroke =
                egui::Stroke { color: ui.visuals().widgets.active.bg_fill, ..Default::default() };
            ui.visuals_mut().widgets.hovered.fg_stroke =
                egui::Stroke { color: ui.visuals().widgets.active.bg_fill, ..Default::default() };
            if Button::default().text("New folder").show(ui).clicked() {
                self.out.new_folder_clicked = true;
            }
        });
    }

    fn show_tabs(&mut self, ui: &mut egui::Ui) {
        if self.active_tab_changed {
            self.cfg.set_tabs(&self.tabs, self.active_tab);
        }

        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

        ui.vertical(|ui| {
            if !self.tabs.is_empty() {
                if self.show_tabs {
                    self.show_tab_strip(ui);
                } else {
                    self.show_mobile_title(ui);
                }
            }

            ui.centered_and_justified(|ui| {
                let mut rename_req = None;
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    if let Some(fail) = &tab.failure {
                        match fail {
                            TabFailure::DeletedFromSync => {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(50.0);
                                    ui.label(format!(
                                        "This file ({}) was deleted after syncing.",
                                        tab.path
                                    ));
                                });
                            }
                            TabFailure::SimpleMisc(msg) => {
                                ui.label(msg);
                            }
                            TabFailure::Unexpected(msg) => {
                                ui.label(msg);
                            }
                        };
                    } else if let Some(content) = &mut tab.content {
                        match content {
                            TabContent::Markdown(md) => {
                                let resp = md.show(ui);
                                // The editor signals a text change when the buffer is initially
                                // loaded. Since we use that signal to trigger saves, we need to
                                // check that this change was not from the initial frame.
                                if resp.text_updated && md.past_first_frame() {
                                    tab.last_changed = Instant::now();
                                }

                                if let Some(new_name) = resp.suggest_rename {
                                    rename_req = Some((tab.id, new_name))
                                }

                                if resp.text_updated {
                                    self.out.markdown_editor_text_updated = true;
                                }
                                if resp.cursor_screen_postition_updated {
                                    // markdown_editor_selection_updated represents a change to the screen position of
                                    // the cursor, which is also updated when scrolling
                                    self.out.markdown_editor_selection_updated = true;
                                }
                                if resp.scroll_updated {
                                    self.out.markdown_editor_scroll_updated = true;
                                }
                            }
                            TabContent::Image(img) => img.show(ui),
                            TabContent::Pdf(pdf) => pdf.show(ui),
                            TabContent::Svg(svg) => {
                                let res = svg.show(ui);
                                if res.request_save {
                                    tab.last_changed = Instant::now();
                                }
                            }
                        };
                    } else {
                        ui.spinner();
                    }
                }
                if let Some(req) = rename_req {
                    self.rename_file(req);
                }
            });
        });
    }

    fn show_mobile_title(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let selectable_label =
                egui::widgets::Button::new(egui::RichText::new(self.tabs[0].name.clone()))
                    .frame(false)
                    .wrap_mode(TextWrapMode::Truncate)
                    .fill(if ui.visuals().dark_mode {
                        egui::Color32::BLACK
                    } else {
                        egui::Color32::WHITE
                    }); // matches iOS native toolbar

            ui.allocate_ui(ui.available_size(), |ui| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    if ui.add(selectable_label).clicked() {
                        self.out.tab_title_clicked = true
                    }
                });
            })
        });
    }

    fn show_tab_strip(&mut self, parent_ui: &mut egui::Ui) {
        let active_tab_changed = self.active_tab_changed;
        self.active_tab_changed = false;

        let mut ui =
            parent_ui.child_ui(parent_ui.painter().clip_rect(), egui::Layout::default(), None);

        let is_tab_strip_visible = self.tabs.len() > 1;
        let cursor = ui
            .horizontal(|ui| {
                egui::ScrollArea::horizontal()
                    .max_width(ui.available_width())
                    .show(ui, |ui| {
                        let mut responses = HashMap::new();
                        for i in 0..self.tabs.len() {
                            if let (true, Some(resp)) = (
                                is_tab_strip_visible,
                                self.tab_label(ui, i, self.active_tab == i, active_tab_changed),
                            ) {
                                responses.insert(i, resp);
                            }
                        }

                        // handle responses after showing all tabs because closing a tab invalidates tab indexes
                        for (i, resp) in responses {
                            match resp {
                                TabLabelResponse::Clicked => {
                                    if self.active_tab == i {
                                        // we should rename the file.

                                        self.out.tab_title_clicked = true;
                                        let active_name = self.tabs[i].name.clone();

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
                                        self.active_tab = i;
                                        self.active_tab_changed = true;
                                        self.ctx.send_viewport_cmd(ViewportCommand::Title(
                                            self.tabs[i].name.clone(),
                                        ));
                                        self.out.selected_file = Some(self.tabs[i].id);
                                    }
                                }
                                TabLabelResponse::Closed => {
                                    self.close_tab(i);

                                    let title = match self.current_tab() {
                                        Some(tab) => tab.name.clone(),
                                        None => "Lockbook".to_owned(),
                                    };
                                    self.ctx.send_viewport_cmd(ViewportCommand::Title(title));

                                    self.out.selected_file = self.current_tab().map(|tab| tab.id);
                                }
                                TabLabelResponse::Renamed(name) => {
                                    self.tabs[i].rename = None;
                                    let id = self.current_tab().unwrap().id;
                                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                                        if let Some(TabContent::Markdown(md)) = &mut tab.content {
                                            md.needs_name = false;
                                        }
                                    }
                                    self.rename_file((id, name.clone()));
                                }
                            }
                            ui.ctx().request_repaint();
                        }
                    });
                ui.cursor()
            })
            .inner;

        ui.style_mut().animation_time = 2.0;

        let how_on = ui.ctx().animate_bool_with_easing(
            "toolbar_height".into(),
            is_tab_strip_visible,
            easing::cubic_in_out,
        );
        parent_ui.add_space(cursor.height() * how_on);
        ui.set_opacity(how_on);

        if is_tab_strip_visible {
            let end_of_tabs = cursor.min.x;
            let available_width = ui.available_width();
            let sep_stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            ui.painter().hline(
                egui::Rangef { min: end_of_tabs, max: end_of_tabs + available_width },
                cursor.max.y,
                sep_stroke,
            );
        }
    }

    fn process_keys(&mut self) {
        const COMMAND: Modifiers = Modifiers::COMMAND;
        const SHIFT: Modifiers = Modifiers::SHIFT;
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
        if self.ctx.input_mut(|i| i.consume_key(COMMAND, egui::Key::N)) {
            self.create_file(false);
        }

        // Ctrl-S to save current tab.
        if self.ctx.input_mut(|i| i.consume_key(COMMAND, egui::Key::S)) {
            self.save_tab(self.active_tab);
        }

        // Ctrl-W to close current tab.
        if self.ctx.input_mut(|i| i.consume_key(COMMAND, egui::Key::W)) && !self.is_empty() {
            self.close_tab(self.active_tab);
            self.ctx.send_viewport_cmd(ViewportCommand::Title(
                self.current_tab()
                    .map(|tab| tab.name.as_str())
                    .unwrap_or("Lockbook")
                    .to_owned(),
            ));

            self.out.selected_file = self.current_tab().map(|tab| tab.id);
        }

        // tab navigation
        let mut goto_tab = None;
        self.ctx.input_mut(|input| {
            // Cmd+1 through Cmd+8 to select tab by cardinal index
            for (i, &key) in NUM_KEYS.iter().enumerate().skip(1).take(8) {
                if input.consume_key_exact(COMMAND, key) {
                    goto_tab = Some(i.min(self.tabs.len()) - 1);
                }
            }

            // Cmd+9 to go to last tab
            if input.consume_key_exact(COMMAND, Key::Num9) {
                goto_tab = Some(self.tabs.len() - 1);
            }

            // Cmd+Shift+[ to go to previous tab
            if input.consume_key_exact(COMMAND | SHIFT, Key::OpenBracket) && self.active_tab != 0 {
                goto_tab = Some(self.active_tab - 1);
            }

            // Cmd+Shift+] to go to next tab
            if input.consume_key_exact(COMMAND | SHIFT, Key::CloseBracket)
                && self.active_tab != self.tabs.len() - 1
            {
                goto_tab = Some(self.active_tab + 1);
            }
        });
        if let Some(goto_tab) = goto_tab {
            if self.active_tab != goto_tab {
                self.active_tab_changed = true;
            }

            self.active_tab = goto_tab;

            if let Some((name, id)) = self.current_tab().map(|tab| (tab.name.clone(), tab.id)) {
                self.ctx.send_viewport_cmd(ViewportCommand::Title(name));
                self.out.selected_file = Some(id);
            };
        }
    }

    fn tab_label(
        &mut self, ui: &mut egui::Ui, t: usize, is_active: bool, active_tab_changed: bool,
    ) -> Option<TabLabelResponse> {
        let t = &mut self.tabs[t];
        let mut result = None;

        let icon_size = 16.0;
        let x_icon = Icon::CLOSE.size(icon_size);
        let status_icon = if self.tasks.load_or_save_queued(t.id) {
            Icon::SCHEDULE.size(icon_size)
        } else if self.tasks.load_or_save_in_progress(t.id) {
            Icon::SAVE.size(icon_size)
        } else if t.is_dirty() {
            Icon::CIRCLE.size(icon_size)
        } else {
            Icon::CHECK_CIRCLE.size(icon_size)
        };

        let padding_x = 10.;
        let w = 160.;
        let h = 40.;

        let (tab_label_rect, tab_label_resp) = ui.allocate_exact_size(
            (w, h).into(),
            Sense { click: true, drag: false, focusable: false },
        );

        if is_active {
            ui.painter().rect(
                tab_label_rect,
                0.,
                ui.style().visuals.extreme_bg_color,
                egui::Stroke::NONE,
            );
        };

        if is_active && active_tab_changed {
            tab_label_resp.scroll_to_me(None);
        }

        // renaming
        if let Some(ref mut str) = t.rename {
            let res = ui
                .allocate_ui_at_rect(tab_label_rect, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(str)
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
                t.rename = None;
            }
        } else {
            // interact with button rect whether it's shown or not
            let close_button_pos = egui::pos2(
                tab_label_rect.max.x - padding_x - x_icon.size,
                tab_label_rect.center().y - x_icon.size / 2.0,
            );
            let close_button_rect =
                egui::Rect::from_min_size(close_button_pos, egui::vec2(x_icon.size, x_icon.size))
                    .expand(2.0);
            let close_button_resp = ui.interact(
                close_button_rect,
                Id::new("tab label close button").with(t.id),
                Sense { click: true, drag: false, focusable: false },
            );

            let status_icon_pos = egui::pos2(
                tab_label_rect.min.x + padding_x,
                tab_label_rect.center().y - status_icon.size / 2.0,
            );
            let status_icon_rect = egui::Rect::from_min_size(
                status_icon_pos,
                egui::vec2(status_icon.size, status_icon.size),
            )
            .expand(2.0);

            // touch mode: always show close button
            let touch_mode =
                matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);
            let show_close_button =
                touch_mode || tab_label_resp.hovered() || close_button_resp.hovered();

            // draw backgrounds and set cursor icon
            if close_button_resp.hovered() {
                ui.painter().rect(
                    close_button_rect,
                    2.0,
                    ui.visuals().code_bg_color,
                    egui::Stroke::NONE,
                );
                ui.output_mut(|o: &mut egui::PlatformOutput| {
                    o.cursor_icon = egui::CursorIcon::PointingHand
                });
            } else if tab_label_resp.hovered() {
                ui.output_mut(|o: &mut egui::PlatformOutput| {
                    o.cursor_icon = egui::CursorIcon::PointingHand
                });
            }

            // draw status icon
            {
                let icon_draw_pos = egui::pos2(
                    tab_label_rect.min.x + padding_x,
                    tab_label_rect.center().y - status_icon.size / 2.0,
                );

                let icon: egui::WidgetText = (&status_icon).into();
                let icon = icon.into_galley(
                    ui,
                    Some(TextWrapMode::Extend),
                    status_icon.size,
                    egui::TextStyle::Body,
                );
                ui.painter()
                    .galley(icon_draw_pos, icon, ui.visuals().text_color());
            }

            // status icon tooltip explains situation
            ui.ctx()
                .style_mut(|s| s.visuals.menu_rounding = (2.).into());
            ui.interact(
                status_icon_rect,
                Id::new("tab label status icon").with(t.id),
                Sense { click: false, drag: false, focusable: false },
            )
            .on_hover_ui(|ui| {
                let text = if self.tasks.load_or_save_queued(t.id) {
                    "save queued"
                } else if self.tasks.load_or_save_in_progress(t.id) {
                    "save in progress"
                } else if t.is_dirty() {
                    "unsaved changes"
                } else {
                    "all changes saved"
                };
                let text: egui::WidgetText = text.into();
                let text =
                    text.into_galley(ui, Some(TextWrapMode::Extend), 0., egui::TextStyle::Small);
                ui.add(egui::Label::new(text));

                let last_saved = {
                    let d = time::Duration::milliseconds(t.last_saved.elapsed().as_millis() as _);
                    let minutes = d.whole_minutes();
                    let seconds = d.whole_seconds();
                    if seconds > 0 && minutes == 0 {
                        if seconds <= 1 {
                            "1 second ago".to_string()
                        } else {
                            format!("{seconds} seconds ago")
                        }
                    } else {
                        d.format_human().to_string()
                    }
                };
                let text: egui::WidgetText = format!("last saved {last_saved}").into();
                let text =
                    text.into_galley(ui, Some(TextWrapMode::Extend), 0., egui::TextStyle::Small);
                ui.add(egui::Label::new(text));

                ui.ctx().request_repaint_after_secs(1.0);
            });

            // draw text
            let text: egui::WidgetText = (&t.name).into();
            let wrap_width = if show_close_button {
                w - (padding_x + status_icon.size + padding_x + padding_x + x_icon.size + padding_x)
            } else {
                w - (padding_x + status_icon.size + padding_x + padding_x)
            };

            // tooltip contains unelided text
            let mut text_rect = tab_label_resp.rect;
            text_rect.min.x = status_icon_rect.max.x;
            text_rect.max.x = close_button_rect.min.x;
            ui.interact(
                text_rect,
                Id::new("tab label text").with(t.id),
                Sense { click: false, drag: false, focusable: false },
            )
            .on_hover_ui(|ui| {
                let text = text.clone().into_galley(
                    ui,
                    Some(TextWrapMode::Extend),
                    wrap_width,
                    egui::TextStyle::Small,
                );
                ui.add(egui::Label::new(text));
            });

            let text = text.into_galley(
                ui,
                Some(TextWrapMode::Truncate),
                wrap_width,
                egui::TextStyle::Small,
            );
            let text_color = ui.style().interact(&tab_label_resp).text_color();
            let text_pos = egui::pos2(
                tab_label_rect.min.x + padding_x + status_icon.size + padding_x,
                tab_label_rect.center().y - 0.5 * text.size().y,
            );
            ui.painter().galley(text_pos, text, text_color);

            // draw close button icon
            if show_close_button {
                let icon_draw_pos = egui::pos2(
                    close_button_rect.center().x - x_icon.size / 2.,
                    close_button_rect.center().y - x_icon.size / 2.2,
                );
                let icon: egui::WidgetText = (&x_icon).into();
                let icon_color = if close_button_resp.is_pointer_button_down_on() {
                    ui.visuals().widgets.active.bg_fill
                } else {
                    ui.visuals().text_color()
                };
                let icon = icon.into_galley(
                    ui,
                    Some(TextWrapMode::Extend),
                    x_icon.size,
                    egui::TextStyle::Body,
                );
                ui.painter().galley(icon_draw_pos, icon, icon_color);
            }

            // respond to input
            if close_button_resp.clicked() || tab_label_resp.middle_clicked() {
                result = Some(TabLabelResponse::Closed);
            } else if tab_label_resp.clicked() {
                result = Some(TabLabelResponse::Clicked);
            }
        }

        // draw separators
        let sep_stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        if !is_active {
            ui.painter()
                .hline(tab_label_rect.x_range(), tab_label_rect.max.y, sep_stroke);
        }
        ui.painter()
            .vline(tab_label_rect.max.x, tab_label_rect.y_range(), sep_stroke);

        result
    }
}

enum TabLabelResponse {
    Clicked,
    Closed,
    Renamed(String),
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
