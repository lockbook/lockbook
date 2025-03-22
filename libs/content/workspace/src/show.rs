use basic_human_duration::ChronoHumanDuration;
use core::f32;
use egui::os::OperatingSystem;
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    include_image, Align, CursorIcon, EventFilter, FontSelection, Id, Image, Key, Label, Modifiers,
    Rangef, Rect, RichText, ScrollArea, Sense, TextStyle, TextWrapMode, Vec2, ViewportCommand,
    Widget as _, WidgetText,
};
use egui_extras::{Size, StripBuilder};
use std::collections::HashMap;
use std::mem;
use std::time::{Duration, Instant};
use tracing::instrument;

use crate::output::Response;
use crate::tab::{image_viewer, ContentState, Tab, TabContent};
use crate::theme::icons::Icon;
use crate::widgets::Button;
use crate::workspace::Workspace;

impl Workspace {
    #[instrument(level="trace", skip_all, fields(frame = self.ctx.frame_nr()))]
    pub fn show(&mut self, ui: &mut egui::Ui) -> Response {
        if self.ctx.input(|inp| !inp.raw.events.is_empty()) {
            self.user_last_seen = Instant::now();
        }

        self.set_tooltip_visibility(ui);

        self.process_updates();
        self.process_keys();
        self.status.message = self.status_message();

        if self.is_empty() {
            self.show_landing_page(ui);
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

    fn show_landing_page(&mut self, ui: &mut egui::Ui) {
        let blue = ui.visuals().widgets.active.bg_fill;
        let weak_blue = blue.gamma_multiply(0.9);
        let weaker_blue = blue.gamma_multiply(0.2);
        let weakest_blue = blue.gamma_multiply(0.15);
        let extreme_bg = ui.visuals().extreme_bg_color;

        // StripBuilder has no way to configure unequal remainders after exact allocations so we must do our own math
        // We must be careful to use layout wrapping when necessary, otherwise cells will expand and math will be wrong
        let padding = if ui.available_height() > 800. { 100. } else { 50. };
        let spacing = 50.;
        let total_content_height = ui.available_height() - 2. * padding - 1. * spacing;
        StripBuilder::new(ui)
            .size(Size::exact(padding)) // padding
            .size(Size::exact(total_content_height * 1. / 3.)) // logo
            .size(Size::exact(spacing)) // spacing
            .size(Size::exact(total_content_height * 2. / 3.)) // nested content
            .size(Size::exact(padding)) // padding
            .vertical(|mut strip| {
                strip.cell(|_| {});
                strip.cell(|ui| {
                    ui.vertical_centered(|ui| {
                        let punchout = if ui.visuals().dark_mode {
                            include_image!("../punchout-dark.png")
                        } else {
                            include_image!("../punchout-light.png")
                        };
                        ui.add(Image::new(punchout).max_size(ui.max_rect().size()));
                    });
                });
                strip.cell(|_| {});
                strip.cell(|ui| {
                    let padding = 100.;
                    let spacing = 50.;
                    let total_content_width = ui.available_width() - 2. * padding - 1. * spacing;
                    let actions_and_tips_width = total_content_width * 1. / 3.;
                    let suggestions_and_activity_width =
                        total_content_width - actions_and_tips_width;

                    StripBuilder::new(ui)
                        .size(Size::exact(padding)) // padding
                        .size(Size::exact(actions_and_tips_width)) // actions and tips
                        .size(Size::exact(spacing)) // spacing
                        .size(Size::exact(suggestions_and_activity_width)) // suggestions and activity
                        .size(Size::exact(padding)) // padding
                        .horizontal(|mut strip| {
                            strip.cell(|_| {});
                            strip.cell(|ui| {
                                ui.label(WidgetText::from(RichText::from("CREATE").weak().small()));
                                ui.horizontal_wrapped(|ui| {
                                    ui.visuals_mut().widgets.inactive.bg_fill = blue;
                                    ui.visuals_mut().widgets.inactive.fg_stroke.color = extreme_bg;

                                    ui.visuals_mut().widgets.hovered.bg_fill = weak_blue;
                                    ui.visuals_mut().widgets.hovered.fg_stroke.color = extreme_bg;

                                    ui.visuals_mut().widgets.active.bg_fill = weak_blue;
                                    ui.visuals_mut().widgets.active.fg_stroke.color = extreme_bg;

                                    if Button::default()
                                        .icon(&Icon::DOC_TEXT)
                                        .text("New Document")
                                        .frame(true)
                                        .rounding(3.)
                                        .show(ui)
                                        .clicked()
                                    {
                                        self.create_file(false);
                                    }

                                    ui.visuals_mut().widgets.inactive.bg_fill = weaker_blue;
                                    ui.visuals_mut().widgets.inactive.fg_stroke.color = blue;

                                    ui.visuals_mut().widgets.hovered.bg_fill = weakest_blue;
                                    ui.visuals_mut().widgets.hovered.fg_stroke.color = blue;

                                    ui.visuals_mut().widgets.active.bg_fill = weakest_blue;
                                    ui.visuals_mut().widgets.active.fg_stroke.color = blue;

                                    if Button::default()
                                        .icon(&Icon::DRAW)
                                        .text("New Drawing")
                                        .frame(true)
                                        .rounding(3.)
                                        .show(ui)
                                        .clicked()
                                    {
                                        self.create_file(true);
                                    }
                                });

                                ui.add_space(50.);

                                ui.label(WidgetText::from(RichText::from("TIPS").weak().small()));
                                for tip in TIPS {
                                    let mut layout_job = LayoutJob::default();
                                    RichText::new("- ").color(weak_blue).append_to(
                                        &mut layout_job,
                                        ui.style(),
                                        FontSelection::Default,
                                        Align::Center,
                                    );
                                    RichText::from(tip)
                                        .color(ui.style().visuals.text_color())
                                        .append_to(
                                            &mut layout_job,
                                            ui.style(),
                                            FontSelection::Default,
                                            Align::Center,
                                        );

                                    ui.label(layout_job);
                                }

                                let is_beta = self
                                    .core
                                    .get_account()
                                    .map(|a| a.is_beta())
                                    .unwrap_or_default();
                                if is_beta {
                                    ui.add_space(50.);

                                    ui.label(WidgetText::from(
                                        RichText::from("TOOLS").weak().small(),
                                    ));
                                    ui.visuals_mut().widgets.inactive.fg_stroke.color = weak_blue;
                                    ui.visuals_mut().widgets.hovered.fg_stroke.color = blue;
                                    ui.visuals_mut().widgets.active.fg_stroke.color = blue;

                                    if Button::default()
                                        .icon(&Icon::LANGUAGE)
                                        .text("Mind Map")
                                        .frame(false)
                                        .rounding(3.)
                                        .show(ui)
                                        .clicked()
                                    {
                                        self.upsert_mind_map(self.core.clone());
                                    }

                                    ui.visuals_mut().widgets.inactive.fg_stroke.color = weak_blue;
                                    ui.visuals_mut().widgets.hovered.fg_stroke.color = blue;
                                    ui.visuals_mut().widgets.active.fg_stroke.color = blue;

                                    if Button::default()
                                        .icon(&Icon::LANGUAGE)
                                        .text("Storage Viewer")
                                        .frame(false)
                                        .rounding(3.)
                                        .show(ui)
                                        .clicked()
                                    {
                                        self.start_storage_viewer(self.core.clone(), None);
                                    }
                                }
                            });
                            strip.cell(|_| {});
                            strip.cell(|ui| {
                                ui.label(WidgetText::from(
                                    RichText::from("SUGGESTED").weak().small(),
                                ));

                                let mut open_file = None;
                                if let Some(files) = &mut self.files {
                                    // this is a hacky way to quickly get the most recently modified files
                                    // if someplace else we use the same technique but a different sort order, we will end up sorting every frame
                                    if !files.suggested.is_sorted() {
                                        files.suggested.sort();
                                    }

                                    if files.suggested.is_empty() {
                                        ui.label("Suggestions are based on your activity on this device. Suggestions will appear after some use.");
                                    }

                                    ScrollArea::horizontal().show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            for &suggested_id in &files.suggested {
                                                let Some(file) = files
                                                    .files
                                                    .iter()
                                                    .find(|f| f.id == suggested_id)
                                                else {
                                                    continue;
                                                };

                                                let (id, rect) =
                                                    ui.allocate_space(Vec2 { x: 120., y: 100. });
                                                let resp = ui
                                                    .interact(rect, id, Sense::click())
                                                    .on_hover_text(&file.name);
                                                if resp.hovered() {
                                                    ui.output_mut(|o| {
                                                        o.cursor_icon = CursorIcon::PointingHand
                                                    });
                                                }
                                                if resp.clicked() {
                                                    open_file = Some(file.id);
                                                }

                                                ui.painter().rect_filled(
                                                    rect,
                                                    3.,
                                                    if resp.hovered() || resp.clicked() {
                                                        weakest_blue
                                                    } else {
                                                        weaker_blue
                                                    },
                                                );

                                                ui.allocate_ui_at_rect(rect, |ui| {
                                                    ui.vertical_centered(|ui| {
                                                        ui.add_space(15.);

                                                        Label::new(&DocType::from_name(&file.name).to_icon()).selectable(false).ui(ui);

                                                        let truncated_name = WidgetText::from(
                                                            WidgetText::from(&file.name)
                                                                .into_galley_impl(
                                                                    ui.ctx(),
                                                                    ui.style(),
                                                                    TextWrapping {
                                                                        max_width: ui
                                                                            .available_width(),
                                                                        max_rows: 2,
                                                                        break_anywhere: false,
                                                                        overflow_character: Some(
                                                                            '…',
                                                                        ),
                                                                    },
                                                                    Default::default(),
                                                                    Default::default(),
                                                                ),
                                                        );


                                                        Label::new(truncated_name).selectable(false).ui(ui);
                                                    });
                                                });
                                            }
                                        });
                                    });
                                } else {
                                    ui.label(WidgetText::from("Loading...").weak());
                                }

                                ui.add_space(50.);

                                ui.label(WidgetText::from(
                                    RichText::from("ACTIVITY").weak().small(),
                                ));

                                if let Some(files) = &mut self.files {
                                    // this is a hacky way to quickly get the most recently modified files
                                    // if someplace else we use the same technique but a different sort order, we will end up sorting every frame
                                    if !files.files.is_sorted_by_key(|f| f.last_modified) {
                                        files.files.sort_by_key(|f| f.last_modified);
                                    }

                                    for file in
                                        files.files.iter().rev().filter(|&f| !f.is_folder()).take(5)
                                    {
                                        ui.horizontal(|ui| {
                                            ui.style_mut().spacing.item_spacing.x = 0.0;
                                            ui.spacing_mut().button_padding.x = 0.;
                                            ui.spacing_mut().button_padding.y = 2.;

                                            // In a classic egui move, when rendering a shorter widget before a taller
                                            // widget in a horizontal layout, the shorter widget is vertically aligned
                                            // as if the taller widget was not there. To solve this, we pre-allocate a
                                            // zero-width rect the height of the button (referencing the button's
                                            // implementation).
                                            let button_height =
                                                ui.text_style_height(&TextStyle::Body);
                                            ui.allocate_exact_size(
                                                Vec2 {
                                                    x: 0.,
                                                    y: button_height
                                                        + 2. * ui.spacing().button_padding.y,
                                                },
                                                Sense::hover(),
                                            );

                                            ui.label(RichText::new("- ").color(weak_blue));

                                            // This is enough width to show the year and month of a pasted_image_...
                                            // but not the day, which seems sufficient
                                            let truncate_width = 200.;
                                            let truncated_name = WidgetText::from(
                                                WidgetText::from(&file.name).into_galley_impl(
                                                    ui.ctx(),
                                                    ui.style(),
                                                    TextWrapping::truncate_at_width(truncate_width),
                                                    Default::default(),
                                                    Default::default(),
                                                ),
                                            );

                                            ui.visuals_mut().widgets.inactive.fg_stroke.color =
                                                weak_blue;
                                            ui.visuals_mut().widgets.hovered.fg_stroke.color = blue;
                                            ui.visuals_mut().widgets.active.fg_stroke.color = blue;

                                            let icon = DocType::from_name(&file.name).to_icon();
                                            if Button::default()
                                                .icon(&icon)
                                                .text(truncated_name)
                                                .show(ui)
                                                .on_hover_text(&file.name)
                                                .clicked()
                                            {
                                                open_file = Some(file.id);
                                            }

                                            // The rest of the space is available for the modified_at/by text
                                            let modified_at = format!(
                                                " was edited {} by @{}",
                                                file.last_modified.elapsed_human_string(),
                                                file.last_modified_by,
                                            );
                                            let truncate_width = ui.available_width();
                                            let truncated_modified_at = WidgetText::from(
                                                WidgetText::from(&modified_at).into_galley_impl(
                                                    ui.ctx(),
                                                    ui.style(),
                                                    TextWrapping::truncate_at_width(truncate_width),
                                                    Default::default(),
                                                    Default::default(),
                                                ),
                                            );

                                            ui.label(truncated_modified_at);
                                        });
                                    }
                                } else {
                                    ui.label(WidgetText::from("Loading...").weak());
                                }

                                if let Some(open_file) = open_file {
                                    self.open_file(open_file, false, true);
                                }
                            });
                            strip.cell(|_| {});
                        });
                });
                strip.cell(|_| {});
            });
    }

    fn show_tabs(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

        ui.vertical(|ui| {
            if let Some(current_tab) = self.current_tab() {
                if self.show_tabs {
                    self.show_tab_strip(ui);
                } else {
                    self.out.tab_title_clicked = self.show_mobile_title(ui, current_tab);
                }
            }

            ui.centered_and_justified(|ui| {
                let mut rename_req = None;
                if let Some(tab) = self.current_tab_mut() {
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
                                    let resp = md.show(ui);
                                    // The editor signals a text change when the buffer is initially
                                    // loaded. Since we use that signal to trigger saves, we need to
                                    // check that this change was not from the initial frame.
                                    if resp.text_updated && md.past_first_frame() {
                                        tab.last_changed = Instant::now();
                                    }

                                    if let Some(new_name) = resp.suggest_rename {
                                        rename_req = tab.id().map(|id| (id, new_name));
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
                                TabContent::MindMap(mm) => {
                                    let response = mm.show(ui, false);
                                    if let Some(value) = response {
                                        self.open_file(value, false, true);
                                    }
                                }
                                TabContent::StorageViewer(sv) => {
                                    sv.show(ui);
                                }
                            };
                        }
                    }
                }
                if let Some(req) = rename_req {
                    self.rename_file(req, false);
                }
            });
        });
    }

    /// Shows the mobile title and returns true if clicked.
    fn show_mobile_title(&self, ui: &mut egui::Ui, tab: &Tab) -> bool {
        ui.horizontal(|ui| {
            let selectable_label =
                egui::widgets::Button::new(egui::RichText::new(self.tab_title(tab)))
                    .frame(false)
                    .wrap_mode(TextWrapMode::Truncate)
                    .fill(if ui.visuals().dark_mode {
                        egui::Color32::BLACK
                    } else {
                        egui::Color32::WHITE
                    }); // matches iOS native toolbar

            ui.allocate_ui(ui.available_size(), |ui| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    ui.add(selectable_label).clicked()
                })
                .inner
            })
            .inner
        })
        .inner
    }

    fn show_tab_strip(&mut self, ui: &mut egui::Ui) {
        let active_tab_changed = self.current_tab_changed;
        self.current_tab_changed = false;

        let cursor = ui
            .horizontal(|ui| {
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
                                        self.current_tab = i;
                                        self.current_tab_changed = true;
                                        self.ctx.send_viewport_cmd(ViewportCommand::Title(
                                            self.tab_title(&self.tabs[i]),
                                        ));
                                        self.out.selected_file = self.tabs[i].id();
                                    }
                                }
                                TabLabelResponse::Closed => {
                                    self.close_tab(i);
                                }
                                TabLabelResponse::Renamed(name) => {
                                    self.tabs[i].rename = None;
                                    if let Some(md) = self.current_tab_markdown_mut() {
                                        md.needs_name = false;
                                    }
                                    if let Some(id) = self.tabs[i].id() {
                                        self.rename_file((id, name.clone()), true);
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
        ui.painter()
            .hline(remaining_rect.x_range(), cursor.max.y, sep_stroke);
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
            self.save_tab(self.current_tab);
        }

        // Ctrl-M to open mind map
        let is_beta = self
            .core
            .get_account()
            .map(|a| a.is_beta())
            .unwrap_or_default();
        if is_beta && self.ctx.input_mut(|i| i.consume_key(COMMAND, egui::Key::M)) {
            self.upsert_mind_map(self.core.clone());
        }

        // Ctrl-W to close current tab.
        if self.ctx.input_mut(|i| i.consume_key(COMMAND, egui::Key::W)) && !self.is_empty() {
            self.close_tab(self.current_tab);
            self.ctx.send_viewport_cmd(ViewportCommand::Title(
                self.current_tab_title().unwrap_or("Lockbook".to_owned()),
            ));

            self.out.selected_file = self.current_tab_id();
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
            if input.consume_key_exact(COMMAND | SHIFT, Key::OpenBracket) && self.current_tab != 0 {
                goto_tab = Some(self.current_tab - 1);
            }

            // Cmd+Shift+] to go to next tab
            if input.consume_key_exact(COMMAND | SHIFT, Key::CloseBracket)
                && self.current_tab != self.tabs.len() - 1
            {
                goto_tab = Some(self.current_tab + 1);
            }
        });

        if let Some(goto_tab) = goto_tab {
            self.make_current(goto_tab);
        }
    }

    fn tab_label(
        &mut self, ui: &mut egui::Ui, t: usize, is_active: bool, active_tab_changed: bool,
    ) -> Option<TabLabelResponse> {
        let mut result = None;
        let icon_size = 16.0;
        let x_icon = Icon::CLOSE.size(icon_size);
        let status = self.tab_status(t);
        let status_icon = status.icon();

        let padding_x = 10.;
        let w = if self.tabs[t].is_closing { 40. } else { 160. };
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

        // closing (just draw status icon)
        if self.tabs[t].is_closing {
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
        // renaming
        else if let Some(ref mut str) = self.tabs[t].rename {
            let res = ui
                .allocate_ui_at_rect(tab_label_rect, |ui| {
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
                Id::new("tab label close button").with(t),
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
                Id::new("tab label status icon").with(t),
                Sense { click: false, drag: false, focusable: false },
            )
            .on_hover_ui(|ui| {
                let text = self.tab_status(t).summary();
                let text: egui::WidgetText = text.into();
                let text =
                    text.into_galley(ui, Some(TextWrapMode::Extend), 0., egui::TextStyle::Small);
                ui.add(egui::Label::new(text));

                let last_saved = self.tabs[t].last_saved.elapsed_human_string();
                let text: egui::WidgetText = format!("last saved {last_saved}").into();
                let text =
                    text.into_galley(ui, Some(TextWrapMode::Extend), 0., egui::TextStyle::Small);
                ui.add(egui::Label::new(text));

                ui.ctx().request_repaint_after_secs(1.0);
            });

            // draw text
            let text: egui::WidgetText = self.tab_title(&self.tabs[t]).into();
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
                Id::new("tab label text").with(t),
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

trait ElapsedHumanString {
    fn elapsed_human_string(&self) -> String;
}

impl ElapsedHumanString for time::Duration {
    fn elapsed_human_string(&self) -> String {
        let minutes = self.whole_minutes();
        let seconds = self.whole_seconds();
        if seconds > 0 && minutes == 0 {
            if seconds <= 1 {
                "1 second ago".to_string()
            } else {
                format!("{seconds} seconds ago")
            }
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

pub enum DocType {
    PlainText,
    Markdown,
    Drawing,
    Image,
    ImageUnsupported,
    Code,
    Unknown,
}

impl DocType {
    pub fn from_name(name: &str) -> Self {
        let ext = name.split('.').last().unwrap_or_default();
        match ext {
            "draw" | "svg" => Self::Drawing,
            "md" => Self::Markdown,
            "txt" => Self::PlainText,
            "cr2" => Self::ImageUnsupported,
            "go" => Self::Code,
            _ if image_viewer::is_supported_image_fmt(ext) => Self::Image,
            _ => Self::Unknown,
        }
    }
    pub fn to_icon(&self) -> Icon {
        match self {
            DocType::Markdown | DocType::PlainText => Icon::DOC_TEXT,
            DocType::Drawing => Icon::DRAW,
            DocType::Image => Icon::IMAGE,
            DocType::Code => Icon::CODE,
            _ => Icon::DOC_UNKNOWN,
        }
    }
}

const TIPS: [&str; 3] = [
    "Import files by dragging and dropping them into the app",
    "You can share and collaborate on files with other Lockbook users",
    "Lockbook is end-to-end encrypted and 100% open source",
];
