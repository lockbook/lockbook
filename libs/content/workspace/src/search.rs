use std::f32::INFINITY;

use egui::{
    Button, Color32, Context, CornerRadius, Frame, Key, Label, Margin, Modifiers, Pos2, RichText,
    Rounding, Spacing, Stroke, TextEdit, Ui, UiBuilder, Vec2, Widget,
};

use crate::{
    show::InputStateExt,
    theme::{icons::Icon, palette_v2::ThemeExt},
    widgets::GlyphonTextEdit,
    workspace::Workspace,
};

#[derive(Default)]
pub struct Search {
    search_shown: bool,
    search_type: SearchType,
    query: String,
}

#[derive(Default, Eq, PartialEq)]
pub enum SearchType {
    #[default]
    Path,
    Content,
    // inspo:
    // All,
    // Commands,
    // Semantic
}

impl Workspace {
    pub fn show_search_modal(&mut self) {
        self.search.process_keys(&self.ctx);
        let size = self.ctx.screen_rect();
        let theme = self.ctx.get_lb_theme();

        if self.search.search_shown {
            egui::Window::new("")
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .min_width(size.width() * 0.8)
                .min_height(size.height() * 0.8)
                .resizable(false)
                .fade_in(true)
                .frame(
                    Frame::window(&self.ctx.style())
                        .fill(theme.neutral_bg_secondary())
                        .stroke(Stroke::new(1., theme.neutral_bg()))
                        .corner_radius(CornerRadius::ZERO)
                        .inner_margin(Margin::ZERO),
                )
                .title_bar(false)
                .collapsible(false)
                .show(&self.ctx.clone(), |ui| self.show_search(ui));
        }
    }

    pub fn show_search(&mut self, ui: &mut Ui) {
        let theme = self.ctx.get_lb_theme();

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                for button in [SearchType::Path, SearchType::Content] {
                    let selected = self.search.search_type == button;

                    let button_resp = Button::selectable(
                        selected,
                        RichText::new(button.name()).color(if selected {
                            theme.fg().get_color(theme.prefs().secondary)
                        } else {
                            theme.neutral_fg()
                        }),
                    )
                    .corner_radius(CornerRadius::ZERO)
                    .frame_when_inactive(true)
                    .min_size(Vec2::new(85., 0.))
                    .fill(if selected {
                        //theme.bg().get_color(theme.prefs().primary)
                        theme.neutral_bg()
                    } else {
                        theme.neutral_bg_secondary()
                    })
                    .ui(ui);

                    if button_resp.clicked() {
                        self.search.search_type = button;
                    }
                }
                ui.allocate_space(Vec2::new(ui.available_width(), 0.));
            });

            Frame::new()
                .fill(theme.neutral_bg())
                .outer_margin(Margin::symmetric(5, 5))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.visuals_mut().widgets.hovered.bg_stroke =
                            egui::Stroke { width: 0.1, color: ui.visuals().weak_text_color() };
                        ui.visuals_mut().selection.stroke =
                            egui::Stroke { width: 0.3, color: ui.visuals().weak_text_color() };

                        // todo stick search icon like we do in full doc search
                        let resp = TextEdit::singleline(&mut self.search.query)
                            .text_color(theme.neutral_fg())
                            .frame(true)
                            .background_color(theme.neutral_bg_secondary())
                            .hint_text("Search")
                            .desired_width(ui.available_size_before_wrap().x)
                            .margin(Margin { left: 30, top: 5, bottom: 5, ..Margin::ZERO })
                            .show(ui)
                            .response;

                        resp.request_focus();

                    });
                    ui.allocate_space(ui.available_size());
                })
        });
    }
}

impl Search {
    fn process_keys(&mut self, ctx: &Context) {
        ctx.input_mut(|w| {
            if w.consume_key_exact(Modifiers::COMMAND | Modifiers::SHIFT, Key::O) {
                self.search_shown = !self.search_shown;
            }
        })
    }
}

impl SearchType {
    fn name(&self) -> &'static str {
        match &self {
            SearchType::Path => "Path",
            SearchType::Content => "Content",
        }
    }
}
