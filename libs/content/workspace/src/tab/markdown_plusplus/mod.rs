use std::io::{BufReader, Cursor};
use std::sync::Arc;

use bounds::Bounds;
use colored::Colorize;
use comrak::nodes::{AstNode, LineColumn, Sourcepos};
use comrak::{Arena, Options};
use core::time::Duration;
use egui::{
    Context, EventFilter, FontData, FontDefinitions, FontFamily, FontTweak, Frame, Id, Rect,
    ScrollArea, Sense, Stroke, Ui, Vec2,
};
use galleys::Galleys;
use input::cursor::CursorState;
use input::mutation::EventState;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{DocByteOffset, DocCharOffset};
use lb_rs::{blocking::Lb, Uuid};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use theme::Theme;
use widget::inline::image::cache::ImageCache;
use widget::leaf_block::code_block::cache::SyntaxHighlightCache;
use widget::{MARGIN, MAX_WIDTH};

mod bounds;
mod galleys;
mod input;
mod output;
mod style;
mod theme;
mod widget;

pub use input::Event;

pub struct MarkdownPlusPlus {
    // dependencies
    pub core: Lb,
    pub client: reqwest::blocking::Client,
    pub ctx: Context,

    // theme
    theme: Theme,
    syntax_set: SyntaxSet,
    syntax_light_theme: syntect::highlighting::Theme,
    syntax_dark_theme: syntect::highlighting::Theme,

    // input
    pub file_id: Uuid,
    // pub hmac: Option<DocumentHmac>,
    pub needs_name: bool,
    pub initialized: bool,

    // internal systems
    pub bounds: Bounds,
    pub buffer: Buffer,
    pub cursor: CursorState,
    pub event: EventState,
    pub galleys: Galleys,
    pub images: ImageCache,
    pub syntax: SyntaxHighlightCache,

    // widgets
    // pub toolbar: Toolbar,
    // pub find: Find,

    // ?
    pub virtual_keyboard_shown: bool,

    // populated at frame start
    image_max_size: Vec2,
}

impl Drop for MarkdownPlusPlus {
    fn drop(&mut self) {
        self.images.free(&self.ctx);
    }
}

impl MarkdownPlusPlus {
    pub fn new(core: Lb, md: &str, file_id: Uuid, ctx: Context) -> Self {
        let theme = Theme::new(ctx.clone());

        let syntax_set = SyntaxSet::load_defaults_newlines();

        let light_theme_bytes = include_bytes!("assets/mnemonic-light.tmTheme").as_ref();
        let cursor = Cursor::new(light_theme_bytes);
        let mut buffer = BufReader::new(cursor);
        let syntax_light_theme = ThemeSet::load_from_reader(&mut buffer).unwrap();

        let dark_theme_bytes = include_bytes!("assets/mnemonic-dark.tmTheme").as_ref();
        let cursor = Cursor::new(dark_theme_bytes);
        let mut buffer = BufReader::new(cursor);
        let syntax_dark_theme = ThemeSet::load_from_reader(&mut buffer).unwrap();

        let image_cache = Default::default();

        Self {
            core,
            client: Default::default(),
            buffer: md.into(),
            file_id,
            ctx,
            theme,
            syntax_set,
            syntax_light_theme,
            syntax_dark_theme,
            images: image_cache,
            image_max_size: Default::default(),
            bounds: Default::default(),
            needs_name: Default::default(),
            initialized: Default::default(),
            cursor: Default::default(),
            event: Default::default(),
            virtual_keyboard_shown: Default::default(),
            galleys: Default::default(),
            syntax: Default::default(),
        }
    }

    pub fn id(&self) -> Id {
        Id::new(self.file_id)
    }

    pub fn focus(&mut self, ctx: &Context) {
        ctx.memory_mut(|m| {
            m.request_focus(self.id());
        });
    }

    pub fn focus_lock(&mut self, ctx: &Context) {
        ctx.memory_mut(|m| {
            m.set_focus_lock_filter(
                self.id(),
                EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
    }

    pub fn focused(&self, ctx: &Context) -> bool {
        ctx.memory(|m| m.has_focus(self.id()))
    }

    pub fn surrender_focus(&self, ctx: &Context) {
        ctx.memory_mut(|m| {
            m.surrender_focus(self.id());
        });
    }

    pub fn show(&mut self, ui: &mut Ui) {
        // make sure the image can be viewed in full by capping its height and width to the viewport
        self.image_max_size = ui.available_size() - Vec2::splat(MARGIN);

        let start = std::time::Instant::now();

        self.process_events(ui.ctx());
        self.calc_source_lines();

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
        let root = comrak::parse_document(&arena, &self.buffer.current.text, &options);

        let ast_elapsed = start.elapsed();
        let start = std::time::Instant::now();

        println!(
            "{}",
            "================================================================================"
                .bright_black()
        );
        print_ast(root);

        let print_elapsed = start.elapsed();
        let start = std::time::Instant::now();

        if self.images.any_loading() {
            ui.ctx().request_repaint_after(Duration::from_millis(8));
        }
        self.images = widget::inline::image::cache::calc(
            root,
            &self.images,
            &self.client,
            &self.core,
            self.file_id,
            ui,
        );

        ui.painter()
            .rect_filled(ui.max_rect(), 0., self.theme.bg().neutral_primary);
        self.theme.apply(ui);
        ui.spacing_mut().item_spacing.x = 0.;

        self.bounds.paragraphs.clear();
        self.galleys.galleys.clear();
        ScrollArea::vertical()
            .id_source(format!("markdown{}", self.file_id))
            .show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    Frame::canvas(ui.style())
                        .inner_margin(MARGIN)
                        .stroke(Stroke::NONE)
                        .fill(self.theme.bg().neutral_primary)
                        .show(ui, |ui| self.render(ui, root));
                });
            });
        self.bounds.text = self.bounds.paragraphs.clone(); // todo: inline character capture
        self.syntax.garbage_collect();

        let render_elapsed = start.elapsed();

        println!(
            "{}",
            "--------------------------------------------------------------------------------"
                .bright_black()
        );
        println!(
            "                                                                 ast: {:?}",
            ast_elapsed
        );
        println!(
            "                                                               print: {:?}",
            print_elapsed
        );
        println!(
            "                                                              render: {:?}",
            render_elapsed
        );
    }

    fn render<'a>(&mut self, ui: &mut Ui, root: &'a AstNode<'a>) {
        let max_rect = ui.max_rect();
        ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });

        let width = ui.max_rect().width().min(MAX_WIDTH);
        let padding = (ui.available_width() - ui.max_rect().width().min(MAX_WIDTH)) / 2.;

        let top_left = ui.max_rect().min + Vec2::new(padding, 0.);
        let height = self.height(root, width);
        let rect = Rect::from_min_size(top_left, Vec2::new(width, height));

        ui.ctx().check_for_id_clash(self.id(), rect, ""); // registers this widget so it's not forgotten by next frame
        ui.interact(rect, self.id(), Sense::click()); // catches pointer input missed by individual widgets e.g. clicking after line end to place cursor

        // shows the actual UI
        ui.allocate_ui_at_rect(rect, |ui| {
            self.show_block(ui, root, top_left, width);
        });

        let mut desired_size = Vec2::new(ui.max_rect().width(), max_rect.height());

        let min_rect = ui.min_rect();
        let fill_available_space = if min_rect.height() < max_rect.height() {
            // fill available space
            max_rect.height() - min_rect.height()
        } else {
            0.
        };
        let end_of_text_padding = max_rect.height() / 2.;
        desired_size.y = fill_available_space.max(end_of_text_padding);

        // debug
        // ui.painter().rect_stroke(
        //     Rect::from_min_size(ui.min_rect().left_bottom(), desired_size),
        //     1.,
        //     Stroke::new(1., self.theme.fg().accent_secondary),
        // );

        ui.allocate_space(desired_size);
    }

    // tired of writing ".buffer.current.segs" all the time
    // todo: find a better home
    pub fn offset_to_byte(&self, i: DocCharOffset) -> DocByteOffset {
        self.buffer.current.segs.offset_to_byte(i)
    }

    pub fn range_to_byte(
        &self, i: (DocCharOffset, DocCharOffset),
    ) -> (DocByteOffset, DocByteOffset) {
        self.buffer.current.segs.range_to_byte(i)
    }

    pub fn offset_to_char(&self, i: DocByteOffset) -> DocCharOffset {
        self.buffer.current.segs.offset_to_char(i)
    }

    pub fn range_to_char(
        &self, i: (DocByteOffset, DocByteOffset),
    ) -> (DocCharOffset, DocCharOffset) {
        self.buffer.current.segs.range_to_char(i)
    }

    pub fn last_cursor_position(&self) -> DocCharOffset {
        self.buffer.current.segs.last_cursor_position()
    }
}

fn print_ast<'a>(root: &'a AstNode<'a>) {
    print_recursive(root, "");
}

fn print_recursive<'a>(node: &'a AstNode<'a>, indent: &str) {
    let last_child = node.next_sibling().is_none();

    // convert cardinal (inc, inc) pairs to ordinal (inc, exc) pairs
    let sourcepos = node.data.borrow().sourcepos;
    let sourcepos = Sourcepos {
        start: LineColumn { line: sourcepos.start.line - 1, column: sourcepos.start.column - 1 },
        end: LineColumn { line: sourcepos.end.line - 1, ..sourcepos.end },
    };

    if indent.is_empty() {
        println!("{:?}", node.data.borrow().value);
    } else {
        println!(
            "{}{}{} {:?} {}{}{}",
            indent,
            if last_child { "└>" } else { "├>" }.bright_black(),
            if node.data.borrow().value.block() { "■" } else { "=" }.cyan(),
            node.data.borrow().value,
            format!("{}", sourcepos).bright_cyan(),
            if node.children().count() > 0 {
                format!(" +{}", node.children().count())
            } else {
                "".into()
            }
            .cyan(),
            if node.children().count() > 0 {
                if node.data.borrow().value.contains_inlines() { "=" } else { "■" }.bright_black()
            } else {
                "".into()
            }
            .bright_black(),
        );
    }
    for child in node.children() {
        print_recursive(
            child,
            &format!("{}{}", indent, if last_child { "  " } else { "│ " }.bright_black()),
        );
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
