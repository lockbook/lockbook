use std::f32;
use std::sync::Arc;

use comrak::nodes::AstNode;
use comrak::{Arena, Options};
use egui::{
    Context, FontData, FontDefinitions, FontFamily, FontTweak, Frame, Rect, ScrollArea, Stroke,
    TextFormat, Ui, Vec2,
};
use lb_rs::Uuid;
use theme::Theme;
use widget::{Ast, Block as _, MARGIN, MAX_WIDTH, ROW_HEIGHT};

mod theme;
mod widget;

pub struct MarkdownPlusPlus {
    pub md: String,
    pub file_id: Uuid,
    pub ctx: Context,
}

impl MarkdownPlusPlus {
    pub fn theme(&self) -> Theme {
        Theme::new(self.ctx.clone())
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let start = std::time::Instant::now();

        let arena = Arena::new();
        let mut options = Options::default();
        options.parse.smart = true;
        options.extension.strikethrough = true;
        options.extension.tagfilter = false; // intended for HTML renderers
        options.extension.table = true;
        options.extension.autolink = true;
        options.extension.tasklist = true;
        options.extension.superscript = true;
        options.extension.header_ids = None; // intended for HTML renderers
        options.extension.footnotes = true;
        options.extension.description_lists = false; // not GFM https://github.com/github/cmark-gfm/issues/135
        options.extension.front_matter_delimiter = None; // todo: is this a good place for metadata?
        options.extension.multiline_block_quotes = true;
        options.extension.math_dollars = true; // rendered as code for now
        options.extension.math_code = true; // rendered as code for now
        options.extension.wikilinks_title_after_pipe = true; // matches obsidian
        options.extension.wikilinks_title_before_pipe = false; // would not match obsidian
        options.extension.underline = true;
        options.extension.spoiler = true;
        options.extension.greentext = true;
        let root = comrak::parse_document(&arena, &self.md, &options);

        let ast_elapsed = start.elapsed();
        let start = std::time::Instant::now();

        println!("========================================");
        print_ast(root);

        let print_elapsed = start.elapsed();
        let start = std::time::Instant::now();

        let theme = Theme::new(ui.ctx().clone());

        ui.painter()
            .rect_filled(ui.max_rect(), 0., theme.bg().neutral_primary);
        theme.apply(ui);
        ui.spacing_mut().item_spacing.x = 0.;

        ScrollArea::vertical()
            .id_source(format!("markdown{}", self.file_id))
            .show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    Frame::canvas(ui.style())
                        .inner_margin(MARGIN)
                        .stroke(Stroke::NONE)
                        .fill(theme.bg().neutral_primary)
                        .show(ui, |ui| self.render(ui, &theme, root));
                });
            });

        let render_elapsed = start.elapsed();

        println!("----------------------------------------");
        println!("                          ast: {:?}", ast_elapsed);
        println!("                        print: {:?}", print_elapsed);
        println!("                       render: {:?}", render_elapsed);
    }

    fn render<'a>(&mut self, ui: &mut Ui, theme: &Theme, root: &'a AstNode<'a>) {
        let max_rect = ui.max_rect();
        ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });

        let width = ui.max_rect().width().min(MAX_WIDTH);
        let padding = (ui.available_width() - ui.max_rect().width().min(MAX_WIDTH)) / 2.;

        let top_left = ui.max_rect().min + Vec2::new(padding, 0.);
        let rect = Rect::from_min_size(top_left, Vec2::new(width, f32::INFINITY));
        ui.allocate_ui_at_rect(rect, |ui| {
            Ast::new(root, TextFormat::default(), theme, ui.ctx()).show(
                ui.available_width(),
                ui.max_rect().left_top(),
                ui,
            );
        });

        let mut desired_size = Vec2::new(ui.max_rect().width(), max_rect.height());
        let min_rect = ui.min_rect();
        desired_size.y = if min_rect.height() < max_rect.height() {
            // fill available space
            max_rect.height() - min_rect.height()
        } else {
            // end of text padding
            max_rect.height() / 2.
        };
        ui.allocate_space(max_rect.size() - Vec2::new(0., ROW_HEIGHT));
    }
}

fn print_ast<'a>(root: &'a AstNode<'a>) {
    print_recursive(root, "");
}

fn print_recursive<'a>(node: &'a AstNode<'a>, indent: &str) {
    let last_child = node.next_sibling().is_none();
    if indent.is_empty() {
        println!("{:?}", node.data.borrow().value);
    } else {
        println!(
            "{}{}{} {:?} {}{}",
            indent,
            if last_child { "└>" } else { "├>" },
            if node.data.borrow().value.block() { "☐" } else { "~" },
            node.data.borrow().value,
            node.data.borrow().sourcepos,
            if node.children().count() > 0 {
                format!(
                    " +{}{}",
                    node.children().count(),
                    if node.data.borrow().value.contains_inlines() { "~" } else { "☐" },
                )
            } else {
                "".into()
            }
        );
    }
    for child in node.children() {
        print_recursive(child, &format!("{}{}", indent, if last_child { "  " } else { "│ " }));
    }
}

pub fn register_fonts(fonts: &mut FontDefinitions) {
    let (sans, mono, bold, icons) = (
        lb_fonts::PT_SANS_REGULAR,
        lb_fonts::JETBRAINS_MONO,
        lb_fonts::PT_SANS_BOLD,
        lb_fonts::MATERIAL_SYMBOLS_OUTLINED,
    );

    fonts
        .font_data
        .insert("sans".to_string(), FontData::from_static(sans));
    fonts.font_data.insert("mono".to_owned(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: 0.1,
                scale: 0.9,
                baseline_offset_factor: -0.1,
                ..Default::default()
            },
            ..FontData::from_static(mono)
        }
    });
    fonts
        .font_data
        .insert("bold".to_string(), FontData::from_static(bold));
    fonts.font_data.insert("material_icons".to_owned(), {
        let mut font = FontData::from_static(icons);
        font.tweak.y_offset_factor = -0.1;
        font
    });

    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Bold")), vec!["bold".to_string()]);

    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "sans".to_string());

    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, "mono".to_string());

    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .push("material_icons".to_owned());
}
