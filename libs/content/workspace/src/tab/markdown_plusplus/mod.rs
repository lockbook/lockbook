use std::io::{BufReader, Cursor};
use std::sync::Arc;

use bounds::Bounds;
use colored::Colorize as _;
use comrak::nodes::AstNode;
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
use lb_rs::{blocking::Lb, Uuid};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use theme::Theme;
use widget::block::leaf::code_block::SyntaxHighlightCache;
use widget::block::LayoutCache;
use widget::inline::image::cache::ImageCache;
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
    dark_mode: bool, // supports change detection
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
    pub layout_cache: LayoutCache,
    pub syntax: SyntaxHighlightCache,

    // widgets
    // pub toolbar: Toolbar,
    // pub find: Find,

    // ?
    pub virtual_keyboard_shown: bool,

    /// width used to render the root node, populated at frame start
    width: f32,
    /// height of the viewport, useful for image size constraints, populated at
    /// frame start
    height: f32,
}

impl Drop for MarkdownPlusPlus {
    fn drop(&mut self) {
        self.images.free(&self.ctx);
    }
}

impl MarkdownPlusPlus {
    pub fn new(core: Lb, md: &str, file_id: Uuid, ctx: Context) -> Self {
        let theme = Theme::new(ctx.clone());

        let dark_mode = ctx.style().visuals.dark_mode;
        let syntax_set = SyntaxSet::load_defaults_newlines();

        let light_theme_bytes = include_bytes!("assets/mnemonic-light.tmTheme").as_ref();
        let cursor = Cursor::new(light_theme_bytes);
        let mut buffer = BufReader::new(cursor);
        let syntax_light_theme = ThemeSet::load_from_reader(&mut buffer).unwrap();

        let dark_theme_bytes = include_bytes!("assets/mnemonic-dark.tmTheme").as_ref();
        let cursor = Cursor::new(dark_theme_bytes);
        let mut buffer = BufReader::new(cursor);
        let syntax_dark_theme = ThemeSet::load_from_reader(&mut buffer).unwrap();

        Self {
            core,
            client: Default::default(),
            ctx,

            dark_mode,
            theme,
            syntax_set,
            syntax_light_theme,
            syntax_dark_theme,

            file_id,
            needs_name: Default::default(),
            initialized: Default::default(),

            bounds: Default::default(),
            buffer: md.into(),
            cursor: Default::default(),
            event: Default::default(),
            galleys: Default::default(),
            images: Default::default(),
            layout_cache: Default::default(),
            syntax: Default::default(),

            virtual_keyboard_shown: Default::default(),
            width: Default::default(),
            height: Default::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn test(md: &str) -> Self {
        Self::new(
            Lb::init(lb_rs::model::core_config::Config {
                writeable_path: format!("/tmp/{}", Uuid::new_v4()),
                logs: false,
                stdout_logs: false,
                colored_logs: false,
                background_work: false,
            })
            .unwrap(),
            md,
            Uuid::new_v4(),
            Context::default(),
        )
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
        self.height = ui.available_size().y;
        self.width = ui.max_rect().width().min(MAX_WIDTH);

        // todo: more thoughtful cache invalidation
        self.layout_cache.clear();
        let dark_mode = ui.style().visuals.dark_mode;
        if dark_mode != self.dark_mode {
            self.syntax.clear();
            self.dark_mode = dark_mode;
        }

        self.calc_source_lines();
        self.print_source_lines_bounds();

        let start = std::time::Instant::now();

        let arena = Arena::new();
        let mut options = Options::default();
        options.parse.smart = true;
        options.extension.alerts = true;
        options.extension.autolink = true;
        options.extension.description_lists = false; // todo: is this a good way to power workspace-wide term definitions?
        options.extension.footnotes = true;
        options.extension.front_matter_delimiter = None; // todo: is this a good place for metadata?
        options.extension.greentext = false;
        options.extension.header_ids = None; // intended for HTML renderers
        options.extension.math_code = true; // rendered as code for now
        options.extension.math_dollars = true; // rendered as code for now
        options.extension.multiline_block_quotes = false; // todo
        options.extension.spoiler = true;
        options.extension.strikethrough = true;
        options.extension.subscript = true;
        options.extension.superscript = true;
        options.extension.table = true;
        options.extension.tagfilter = false; // intended for HTML renderers
        options.extension.tasklist = true;
        options.extension.underline = true;
        options.extension.wikilinks_title_after_pipe = true; // matches obsidian
        options.extension.wikilinks_title_before_pipe = false; // would not match obsidian
        options.render.escaped_char_spans = true;

        let text_with_newline = self.buffer.current.text.to_string() + "\n"; // todo: probably not okay but this parser quirky af sometimes

        let root = comrak::parse_document(&arena, &text_with_newline, &options);

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
        self.bounds.inline_paragraphs.clear();
        self.galleys.galleys.clear();
        ScrollArea::vertical()
            .drag_to_scroll(false)
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
        self.bounds.paragraphs.sort();
        self.galleys.galleys.sort_by_key(|g| g.range);
        self.show_selection(ui);

        self.bounds.text = self.bounds.paragraphs.clone(); // todo: inline character capture
        self.bounds.words = self.bounds.paragraphs.clone(); // todo: real words
        self.bounds.lines = self.bounds.paragraphs.clone(); // todo: real lines
        self.syntax.garbage_collect();

        let render_elapsed = start.elapsed();

        println!(
            "{}",
            "--------------------------------------------------------------------------------"
                .bright_black()
        );
        println!("document: {:?}", self.buffer.current.text);
        self.print_paragraphs_bounds();
        self.print_inline_paragraphs_bounds();
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

        if self.process_events(ui.ctx(), root) {
            ui.ctx().request_repaint();
        }
        if self.images.any_loading() {
            ui.ctx().request_repaint_after(Duration::from_millis(8));
        }

        // focus editor by default
        if ui.memory(|m| m.focused().is_none()) {
            self.focus(ui.ctx());
        }
        if self.focused(ui.ctx()) {
            self.focus_lock(ui.ctx());
        }
    }

    fn render<'a>(&mut self, ui: &mut Ui, root: &'a AstNode<'a>) {
        let max_rect = ui.max_rect();
        ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });

        let padding = (ui.available_width() - ui.max_rect().width().min(MAX_WIDTH)) / 2.;

        let top_left = ui.max_rect().min + Vec2::new(padding, 0.);
        let height = self.height(root);
        let rect = Rect::from_min_size(top_left, Vec2::new(self.width, height));

        ui.ctx().check_for_id_clash(self.id(), rect, ""); // registers this widget so it's not forgotten by next frame
        ui.interact(rect, self.id(), Sense::click_and_drag()); // catches pointer input missed by individual widgets e.g. clicking after line end to place cursor

        // compute bounds for all nodes
        self.compute_bounds(root);

        // shows the actual UI
        ui.allocate_ui_at_rect(rect, |ui| {
            self.show_block(ui, root, top_left);
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
}

fn print_ast<'a>(root: &'a AstNode<'a>) {
    print_recursive(root, "");
}

fn print_recursive<'a>(node: &'a AstNode<'a>, indent: &str) {
    let last_child = node.next_sibling().is_none();
    let sourcepos = node.data.borrow().sourcepos;

    if indent.is_empty() {
        println!(
            "{} {:?} {}{}{}",
            if node.data.borrow().value.block() { "□" } else { "☰" }.blue(),
            node.data.borrow().value,
            format!("{}", sourcepos).yellow(),
            if node.children().count() > 0 {
                format!(" +{} ", node.children().count())
            } else {
                "".into()
            }
            .blue(),
            if node.children().count() > 0 {
                if !node.data.borrow().value.block() || node.data.borrow().value.contains_inlines()
                {
                    "☰"
                } else {
                    "□"
                }
            } else {
                ""
            }
            .bright_magenta(),
        );
    } else {
        println!(
            "{}{}{} {:?} {}{}{}",
            indent,
            if last_child { "└>" } else { "├>" }.bright_black(),
            if node.data.borrow().value.block() { "□" } else { "☰" }.blue(),
            node.data.borrow().value,
            format!("{}", sourcepos).yellow(),
            if node.children().count() > 0 {
                format!(" +{} ", node.children().count())
            } else {
                "".into()
            }
            .blue(),
            if node.children().count() > 0 {
                if !node.data.borrow().value.block() || node.data.borrow().value.contains_inlines()
                {
                    "☰"
                } else {
                    "□"
                }
            } else {
                ""
            }
            .bright_magenta(),
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
    fonts.font_data.insert("mono".into(), {
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
    fonts.font_data.insert("super".into(), {
        FontData {
            tweak: FontTweak { y_offset_factor: -1. / 4., scale: 3. / 4., ..Default::default() },
            ..FontData::from_static(sans)
        }
    });
    fonts.font_data.insert("sub".into(), {
        FontData {
            tweak: FontTweak { y_offset_factor: 1. / 4., scale: 3. / 4., ..Default::default() },
            ..FontData::from_static(sans)
        }
    });
    fonts.font_data.insert("material_icons".into(), {
        let mut font = FontData::from_static(icons);
        font.tweak.y_offset_factor = -0.1;
        font
    });

    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Bold")), vec!["bold".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Super")), vec!["super".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Sub")), vec!["sub".into()]);

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
