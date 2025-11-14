use std::ops::{Deref, DerefMut as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::{mem, thread};

use egui::{Id, Key, Modifiers};
use lb::Uuid;
use lb::blocking::Lb;
use lb::model::file::File;
use lb::subscribers::search::{ContentMatch, SearchConfig, SearchResult};
use workspace_rs::show::InputStateExt;
use workspace_rs::theme::icons::Icon;

use workspace_rs::show::DocType;

#[derive(Default)]
pub struct FullDocSearch {
    pub is_searching: Arc<AtomicBool>,
    pub query: Arc<Mutex<String>>,
    pub results: Arc<Mutex<Vec<SearchResult>>>,
}

#[derive(Default)]
pub struct Response {
    /// The user selected this file in search results. Open the file.
    pub file_to_open: Option<Uuid>,

    /// The user down-arrowed the focus out of the search widget. Advance the focus to the widget below.
    pub advance_focus: bool,

    pub search_box_rect: Option<egui::Rect>,
}

impl FullDocSearch {
    const X_MARGIN: f32 = 15.0;
    const ICON_WIDTH: f32 = 15.0;

    fn set_style(&self, ui: &mut egui::Ui) {
        ui.visuals_mut().extreme_bg_color = ui.visuals().code_bg_color;
        let rounding = 5.0.into();
        ui.visuals_mut().widgets.active.rounding = rounding;
        ui.visuals_mut().widgets.hovered.rounding = rounding;
        ui.visuals_mut().widgets.inactive.rounding = rounding;
        ui.visuals_mut().widgets.noninteractive.rounding = rounding;
        ui.visuals_mut().widgets.open.rounding = rounding;

        ui.visuals_mut().widgets.hovered.bg_stroke =
            egui::Stroke { width: 0.3, color: ui.visuals().weak_text_color() };

        ui.visuals_mut().selection.stroke =
            egui::Stroke { width: 0.7, color: ui.visuals().weak_text_color() };
    }

    fn show_search_icon(&self, ui: &mut egui::Ui, text_edit_rect: egui::Rect) {
        let search_icon_width = Self::ICON_WIDTH + 7.0; // account for margin
        let mut ui = ui.child_ui(
            text_edit_rect.translate(egui::vec2(-(search_icon_width), 0.0)),
            egui::Layout::default(),
            None,
        );
        ui.visuals_mut().override_text_color = Some(ui.visuals().weak_text_color());
        Icon::SEARCH.show(&mut ui);
    }

    fn show_x_icon(&self, ui: &mut egui::Ui, text_edit_rect: egui::Rect) -> egui::Response {
        let x_margin = 7.0; // account for margin

        let mut ui = ui.child_ui(
            text_edit_rect.translate(egui::vec2(text_edit_rect.width() + x_margin, 0.0)),
            egui::Layout {
                main_dir: egui::Direction::LeftToRight,
                main_wrap: false,
                main_align: egui::Align::LEFT,
                main_justify: false,
                cross_align: egui::Align::Center,
                cross_justify: true,
            },
            None,
        );
        ui.visuals_mut().override_text_color = Some(ui.visuals().widgets.active.bg_fill);
        ui.visuals_mut().widgets.active.bg_fill = ui.visuals().widgets.hovered.bg_fill;
        ui.spacing_mut().button_padding = egui::vec2(6.0, 2.0);

        let res = Icon::CLOSE.size(10.0).frame(true).show(&mut ui);

        res
    }
    pub fn show(&mut self, ui: &mut egui::Ui, core: &Lb) -> Response {
        let mut resp = Response::default();
        let results_resp = egui::Frame::default()
            .inner_margin(egui::Margin::symmetric(10.0, 10.0))
            .show(ui, |ui| {
                self.set_style(ui);
                // draw the UI, get the query, possibly clear the query & search results
                let Ok(mut query) = self.query.lock() else { return None };

                let id = Id::new("full_doc_search");
                if ui.memory(|m| m.has_focus(id))
                    && ui.input_mut(|i| {
                        i.consume_key(Modifiers::NONE, Key::ArrowDown)
                            || i.consume_key_exact(Modifiers::NONE, Key::Tab)
                    })
                    && query.is_empty()
                {
                    resp.advance_focus = true;
                }

                let x_margin_with_icon = Self::X_MARGIN + Self::ICON_WIDTH;
                let icon_shelf_width = 155.0;
                let output = egui::TextEdit::singleline(query.deref_mut())
                    .id(id)
                    .desired_width(ui.available_size_before_wrap().x - icon_shelf_width)
                    .hint_text("Search")
                    .margin(egui::Margin::symmetric(x_margin_with_icon, 6.0))
                    .show(ui);

                resp.search_box_rect = Some(output.response.interact_rect);

                self.show_search_icon(ui, output.response.rect);

                if !query.is_empty() && self.show_x_icon(ui, output.response.rect).clicked() {
                    if let Ok(mut results) = self.results.lock() {
                        // note: at this point both locks held simultaneously with order query -> results
                        query.clear();
                        results.clear();
                    }
                };

                mem::drop(query);

                let query_empty = self.query.lock().map(|q| q.is_empty()).unwrap_or_default();
                if !query_empty {
                    // show spinner while searching
                    if self.is_searching.load(Ordering::Relaxed) {
                        ui.add_space(20.0);
                        ui.spinner();
                    }

                    // launch search if query changed
                    if output.response.changed() {
                        let core = core.clone();
                        let is_searching = self.is_searching.clone();
                        let query = self.query.clone();
                        let results = self.results.clone();
                        let ctx = ui.ctx().clone();
                        thread::spawn(move || {
                            // get the query
                            let this_query = query.lock().unwrap().clone();

                            // run the search (no locks held)
                            is_searching.store(true, Ordering::Relaxed);
                            let these_results = core
                                .search(&this_query, SearchConfig::PathsAndDocuments)
                                .unwrap_or_default();

                            // update the results only if they are for the current query
                            // note: locks acquired in same order as above to prevent deadlock
                            let query = query.lock().unwrap();
                            let mut results = results.lock().unwrap();

                            if query.deref() == &this_query {
                                is_searching.store(false, Ordering::Relaxed);
                                *results = these_results;
                                ctx.request_repaint();
                            }
                        });
                    }
                    ui.add_space(5.0);
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| self.show_results(ui, core))
                        .inner
                } else {
                    None
                }
            })
            .inner;
        resp.file_to_open = results_resp;
        resp
    }

    pub fn show_results(&mut self, ui: &mut egui::Ui, core: &Lb) -> Option<Uuid> {
        ui.add_space(10.0);

        let Ok(results) = self.results.lock() else { return None };

        for sr in results.iter() {
            let sr_res = ui.vertical(|ui| {
                match sr {
                    SearchResult::DocumentMatch { id, path, content_matches } => {
                        let file = &core.get_file_by_id(*id).unwrap();
                        Self::show_file(ui, file, path);
                        ui.horizontal(|ui| {
                            ui.add_space(15.0);
                            ui.horizontal_wrapped(|ui| {
                                let font_size = 15.0;
                                self.show_content_match(ui, &content_matches[0], font_size);
                            });
                        });
                    }
                    SearchResult::PathMatch { id, path, matched_indices: _, score: _ } => {
                        let file = &core.get_file_by_id(*id).unwrap();
                        Self::show_file(ui, file, path);
                    }
                };
                ui.add_space(10.0);
            });
            ui.add(egui::Separator::default().shrink(ui.available_width() / 1.5));
            ui.add_space(10.0);

            let sr_res = ui.interact(sr_res.response.rect, ui.next_auto_id(), egui::Sense::click());
            if sr_res.hovered() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand)
            }

            if sr_res.clicked() {
                return Some(sr.id());
            };
        }

        None
    }

    fn show_file(ui: &mut egui::Ui, file: &File, path: &str) {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(Self::X_MARGIN);

            DocType::from_name(file.name.as_str()).to_icon().show(ui);

            ui.add_space(7.0);

            let mut job = egui::text::LayoutJob::single_section(
                file.name.clone(),
                egui::TextFormat::simple(
                    egui::FontId::proportional(18.0),
                    ui.visuals().text_color(),
                ),
            );
            job.wrap = egui::epaint::text::TextWrapping {
                overflow_character: Some('…'),
                max_rows: 1,
                break_anywhere: true,
                ..Default::default()
            };
            ui.label(job);
        });
        ui.horizontal_wrapped(|ui| {
            ui.add_space(Self::X_MARGIN);

            let mut job = egui::text::LayoutJob::single_section(
                path.to_owned(),
                egui::TextFormat::simple(egui::FontId::proportional(15.0), egui::Color32::GRAY),
            );

            job.wrap = egui::epaint::text::TextWrapping {
                overflow_character: Some('…'),
                max_rows: 1,
                break_anywhere: true,
                ..Default::default()
            };
            ui.label(job);
        });
    }

    fn show_content_match(&self, ui: &mut egui::Ui, content_match: &ContentMatch, font_size: f32) {
        let matched_indices = &content_match.matched_indices;
        let str = content_match.paragraph.clone();
        let highlight_color = ui.visuals().widgets.active.bg_fill.gamma_multiply(0.5);

        let mut curr = 0;
        let mut next;

        let pre: String = str.chars().take(matched_indices[0]).collect();

        ui.label(egui::RichText::new(pre).size(font_size));

        while curr < matched_indices.len() {
            next = curr;

            while next < matched_indices.len() - 1
                && matched_indices[next] + 1 == matched_indices[next + 1]
            {
                next += 1;
            }

            if next == curr || curr == matched_indices.len() - 1 {
                let h_str = str
                    .chars()
                    .nth(matched_indices[curr])
                    .unwrap_or_default()
                    .to_string();
                ui.label(
                    egui::RichText::new(h_str)
                        .size(font_size)
                        .background_color(highlight_color),
                );

                curr += 1;
            } else {
                let h_str: String = str
                    .chars()
                    .skip(matched_indices[curr])
                    .take(matched_indices[next] + 1)
                    .collect();

                ui.label(
                    egui::RichText::new(h_str)
                        .size(font_size)
                        .background_color(highlight_color),
                );
                curr = next + 1;
            }
            if curr < matched_indices.len() - 1 {
                let h_str: String = str
                    .chars()
                    .skip(matched_indices[next] + 1)
                    .take(matched_indices[curr])
                    .collect();

                ui.label(egui::RichText::new(h_str).size(font_size));
            }
        }
        let post: String = str
            .chars()
            .take(matched_indices[matched_indices.len() - 1] + 1)
            .collect();

        ui.label(egui::RichText::new(post).size(font_size));
    }
}
