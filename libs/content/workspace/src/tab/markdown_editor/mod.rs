use std::io::{BufReader, Cursor};
use std::mem;
use std::sync::Arc;

use bounds::Bounds;
use colored::Colorize as _;
use comrak::nodes::AstNode;
use comrak::{Arena, Options};
use core::time::Duration;
use egui::os::OperatingSystem;
use egui::scroll_area::{ScrollAreaOutput, ScrollBarVisibility};
use egui::{
    scroll_area, Context, EventFilter, FontData, FontDefinitions, FontFamily, FontTweak, Frame, Id, Margin, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder, Vec2
};
use galleys::Galleys;
use input::cursor::CursorState;
use input::mutation::EventState;
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::DocCharOffset;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect_assets::assets::HighlightingAssets;
use theme::Theme;
use widget::block::LayoutCache;
use widget::block::leaf::code_block::SyntaxHighlightCache;
use widget::find::Find;
use widget::inline::image::cache::ImageCache;
use widget::toolbar::{MOBILE_TOOL_BAR_SIZE, Toolbar};
use widget::{MARGIN, MAX_WIDTH};

pub mod bounds;
mod galleys;
pub mod input;
pub mod output;
mod style;
mod theme;
mod widget;

pub use input::Event;

#[derive(Debug, Default)]
pub struct Response {
    // state changes
    pub text_updated: bool,
    pub selection_updated: bool,
    pub scroll_updated: bool,
}

pub struct Editor {
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
    pub hmac: Option<DocumentHmac>,
    pub initialized: bool,
    pub plaintext_mode: bool,
    pub touch_mode: bool,
    pub readonly: bool,

    // internal systems
    pub bounds: Bounds,
    pub buffer: Buffer,
    pub cursor: CursorState,
    pub event: EventState,
    pub galleys: Galleys,
    pub images: ImageCache,
    pub layout_cache: LayoutCache,
    pub syntax: SyntaxHighlightCache,
    pub debug: bool,

    // widgets
    pub toolbar: Toolbar,
    pub find: Find,

    // selection state
    /// During drag operations, stores the selection that would be applied
    /// without actually updating the buffer selection (which would affect syntax reveal)
    pub in_progress_selection: Option<(DocCharOffset, DocCharOffset)>,

    // ?
    pub virtual_keyboard_shown: bool,
    scroll_to_cursor: bool,

    /// width used to render the root node, populated at frame start
    width: f32,
    /// height of the viewport, useful for image size constraints, populated at
    /// frame start
    height: f32,

    // outputs from drawing a frame need an additional frame to process before reporting
    next_resp: Response,
}

impl Drop for Editor {
    fn drop(&mut self) {
        self.images.free(&self.ctx);
    }
}

static PRINT: bool = false;

impl Editor {
    pub fn new(
        ctx: Context, core: Lb, md: &str, file_id: Uuid, hmac: Option<DocumentHmac>,
        plaintext_mode: bool, readonly: bool,
    ) -> Self {
        let theme = Theme::new(ctx.clone());

        let dark_mode = ctx.style().visuals.dark_mode;
        let highlighting_assets = HighlightingAssets::from_binary();
        let syntax_set = highlighting_assets.get_syntax_set().unwrap().clone();

        let light_theme_bytes = include_bytes!("assets/mnemonic-light.tmTheme").as_ref();
        let cursor = Cursor::new(light_theme_bytes);
        let mut buffer = BufReader::new(cursor);
        let syntax_light_theme = ThemeSet::load_from_reader(&mut buffer).unwrap();

        let dark_theme_bytes = include_bytes!("assets/mnemonic-dark.tmTheme").as_ref();
        let cursor = Cursor::new(dark_theme_bytes);
        let mut buffer = BufReader::new(cursor);
        let syntax_dark_theme = ThemeSet::load_from_reader(&mut buffer).unwrap();

        let touch_mode = matches!(ctx.os(), OperatingSystem::Android | OperatingSystem::IOS);

        Self {
            core,
            client: Default::default(),
            ctx,

            dark_mode,
            theme,
            syntax_set,
            syntax_light_theme,
            syntax_dark_theme,

            toolbar: Default::default(),
            find: Default::default(),

            readonly,
            file_id,
            hmac,
            initialized: Default::default(),
            plaintext_mode,
            touch_mode,

            bounds: Default::default(),
            buffer: md.into(),
            cursor: Default::default(),
            event: Default::default(),
            galleys: Default::default(),
            images: Default::default(),
            layout_cache: Default::default(),
            syntax: Default::default(),
            debug: false,

            in_progress_selection: None,

            virtual_keyboard_shown: Default::default(),
            scroll_to_cursor: Default::default(),
            width: Default::default(),
            height: Default::default(),

            next_resp: Default::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn test(md: &str) -> Self {
        Self::new(
            Context::default(),
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
            None,
            false,
            false,
        )
    }

    pub fn id(&self) -> Id {
        Id::new(self.file_id)
    }

    pub fn focus(&self, ctx: &Context) {
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

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let mut resp: Response = mem::take(&mut self.next_resp);

        let height = ui.available_size().y;
        let width = ui.max_rect().width().min(MAX_WIDTH) - 2. * MARGIN;
        let height_updated = self.height != height;
        let width_updated = self.width != width;
        self.height = height;
        self.width = width;

        let dark_mode = ui.style().visuals.dark_mode;
        if dark_mode != self.dark_mode {
            self.syntax.clear();
            self.dark_mode = dark_mode;
        }

        self.calc_source_lines();

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
        options.extension.highlight = true;
        options.extension.math_code = true; // rendered as code for now
        options.extension.math_dollars = true; // rendered as code for now
        options.extension.multiline_block_quotes = false; // todo
        options.extension.shortcodes = true;
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
        let mut root = comrak::parse_document(&arena, &text_with_newline, &options);

        let ast_elapsed = start.elapsed();
        let start = std::time::Instant::now();

        if PRINT {
            println!(
                "{}",
                "================================================================================"
                    .bright_black()
            );
            print_ast(root);
        }

        let print_elapsed = start.elapsed();
        let start = std::time::Instant::now();

        // process events
        let prior_selection = self.buffer.current.selection;
        let images_updated = {
            let mut images_updated = self.images.updated.lock().unwrap();
            let result = *images_updated;
            *images_updated = false;
            result
        };
        if !self.initialized || self.process_events(ui.ctx(), root) {
            resp.text_updated = true;

            // need to re-parse ast to compute bounds which are referenced by mobile virtual keyboard between frames
            let text_with_newline = self.buffer.current.text.to_string() + "\n"; // todo: probably not okay but this parser quirky af sometimes
            root = comrak::parse_document(&arena, &text_with_newline, &options);

            self.bounds.paragraphs.clear();
            self.bounds.inline_paragraphs.clear();
            self.layout_cache.clear();

            self.calc_source_lines();
            self.compute_bounds(root);
            self.bounds.paragraphs.sort();
            self.bounds.inline_paragraphs.sort();

            self.calc_words();

            ui.ctx().request_repaint();
        }
        resp.selection_updated = prior_selection != self.buffer.current.selection;

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

        // these are computed during render
        self.galleys.galleys.clear();
        self.bounds.wrap_lines.clear();

        ui.vertical(|ui| {
            if self.touch_mode {
                // touch devices: show find...
                let find_resp = self.find.show(&self.buffer, ui);
                if let Some(term) = find_resp.term {
                    self.event
                        .internal_events
                        .push(Event::Find { term, backwards: find_resp.backwards });
                }

                // ...then show editor content...
                let available_width = ui.available_width();
                ui.allocate_ui(
                    egui::vec2(ui.available_width(), ui.available_height() - MOBILE_TOOL_BAR_SIZE),
                    |ui| {
                        let scroll_area_id = ui.id().with(egui::Id::new(self.file_id));
                        let scroll_area_offset = ui.data_mut(|d| {
                            d.get_persisted(scroll_area_id)
                                .map(|s: scroll_area::State| s.offset)
                                .unwrap_or_default()
                                .y
                        });

                        ui.ctx().style_mut(|style| {
                            style.spacing.scroll = egui::style::ScrollStyle::solid();
                            style.spacing.scroll.bar_width = 10.;
                        });

                        let scroll_area_output = self.show_scrollable_editor(ui, root);
                        self.next_resp.scroll_updated =
                            scroll_area_output.state.offset.y != scroll_area_offset;
                    },
                );

                // ...then show toolbar at the bottom
                if !self.readonly {
                    let (_, rect) =
                        ui.allocate_space(egui::vec2(available_width, MOBILE_TOOL_BAR_SIZE));
                    ui.allocate_new_ui(UiBuilder::new().max_rect(rect), |ui| {
                        self.show_toolbar(root, ui);
                    });
                }
            } else {
                let scroll_area_id = ui.id().with(egui::Id::new(self.file_id));
                let scroll_area_offset = ui.data_mut(|d| {
                    d.get_persisted(scroll_area_id)
                        .map(|s: scroll_area::State| s.offset)
                        .unwrap_or_default()
                        .y
                });

                // non-touch devices: show toolbar...
                if !self.readonly {
                    self.show_toolbar(root, ui);
                }

                // ...then show find...
                let find_resp = self.find.show(&self.buffer, ui);
                if let Some(term) = find_resp.term {
                    self.event
                        .internal_events
                        .push(Event::Find { term, backwards: find_resp.backwards });
                }

                // ...then show editor content
                let scroll_area_output = self.show_scrollable_editor(ui, root);
                self.next_resp.scroll_updated =
                    scroll_area_output.state.offset.y != scroll_area_offset;
            }
        });

        self.syntax.garbage_collect();

        let render_elapsed = start.elapsed();

        if PRINT {
            println!(
                "{}",
                "--------------------------------------------------------------------------------"
                    .bright_black()
            );
            println!("document: {:?}", self.buffer.current.text);
            self.print_paragraphs_bounds();
            println!(
                "{}",
                "--------------------------------------------------------------------------------"
                    .bright_black()
            );
            println!(
                "                                                                 ast: {ast_elapsed:?}"
            );
            println!(
                "                                                               print: {print_elapsed:?}"
            );
            println!(
                "                                                              render: {render_elapsed:?}"
            );
        }

        let all_selected = self.buffer.current.selection == (0.into(), self.last_cursor_position());
        if resp.selection_updated || images_updated || height_updated || width_updated {
            self.layout_cache.clear();
            ui.ctx().request_repaint();
        }
        if resp.selection_updated && !all_selected {
            self.scroll_to_cursor = true;
            ui.ctx().request_repaint();
        }
        if self.touch_mode && height_updated {
            self.scroll_to_cursor = true;
            ui.ctx().request_repaint();
        }
        if self.next_resp.scroll_updated {
            ui.ctx().request_repaint();
        }
        if !self.event.internal_events.is_empty() {
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

        self.initialized = true;

        resp
    }

    fn show_scrollable_editor<'a>(
        &mut self, ui: &mut Ui, root: &'a AstNode<'a>,
    ) -> ScrollAreaOutput<()> {
        let margin: Margin =
            if cfg!(target_os = "android") { Margin::symmetric(0.0, 60.0) } else { MARGIN.into() };
        ScrollArea::vertical()
            .drag_to_scroll(self.touch_mode)
            .id_salt(self.file_id)
            .scroll_bar_visibility(if self.touch_mode {
                ScrollBarVisibility::AlwaysVisible
            } else {
                ScrollBarVisibility::VisibleWhenNeeded
            })
            .show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    Frame::canvas(ui.style())
                        .inner_margin(margin)
                        .stroke(Stroke::NONE)
                        .fill(self.theme.bg().neutral_primary)
                        .show(ui, |ui| {
                            let scroll_view_height = ui.max_rect().height();
                            ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });

                            let padding = (ui.available_width() - self.width) / 2.;

                            let top_left = ui.max_rect().min + Vec2::new(padding, 0.);
                            let height = {
                                let document_height = self.height(root);
                                let unfilled_space = if document_height < scroll_view_height {
                                    scroll_view_height - document_height
                                } else {
                                    0.
                                };
                                let end_of_text_padding = scroll_view_height / 2.;

                                document_height + unfilled_space.max(end_of_text_padding)
                            };
                            let rect = Rect::from_min_size(top_left, Vec2::new(self.width, height));
                            let rect = rect.expand2(Vec2::X * margin.left); // clickable margins (more forgivable to click beginning of line)

                            ui.ctx().check_for_id_clash(self.id(), rect, ""); // registers this widget so it's not forgotten by next frame
                            let focused = self.focused(ui.ctx());
                            let response = ui.interact(
                                rect,
                                self.id(),
                                Sense { click: true, drag: !self.touch_mode, focusable: true },
                            );
                            if focused && !self.focused(ui.ctx()) {
                                // interact surrenders focus if we don't have sense focusable, but also if user clicks elsewhere, even on a child
                                self.focus(ui.ctx());
                            }
                            if response.hovered() || response.clicked() {
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Text);
                                // overridable by widgets
                            }

                            ui.advance_cursor_after_rect(rect);

                            ui.allocate_new_ui(UiBuilder::new().max_rect(rect), |ui| {
                                self.show_block(ui, root, top_left);
                            });
                        });
                });
                self.galleys.galleys.sort_by_key(|g| g.range);

                if ui.ctx().os() != OperatingSystem::IOS {
                    let selection = self
                        .in_progress_selection
                        .unwrap_or(self.buffer.current.selection);
                    let color = self.theme.fg().accent_secondary;
                    self.show_range(ui, selection, color);
                    self.show_offset(ui, selection.1, color);
                }
                if ui.ctx().os() == OperatingSystem::Android {
                    self.show_selection_handles(ui);
                }
                if mem::take(&mut self.scroll_to_cursor) {
                    self.scroll_to_cursor(ui);
                }
            })
    }
}

pub fn print_ast<'a>(root: &'a AstNode<'a>) {
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
            format!("{sourcepos}").yellow(),
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
            format!("{sourcepos}").yellow(),
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
    let (sans, mono, bold, base_scale) = if cfg!(target_vendor = "apple") {
        (lb_fonts::SF_PRO_REGULAR, lb_fonts::SF_MONO_REGULAR, lb_fonts::SF_PRO_TEXT_BOLD, 0.9)
    } else {
        (lb_fonts::PT_SANS_REGULAR, lb_fonts::JETBRAINS_MONO, lb_fonts::PT_SANS_BOLD, 1.)
    };

    let mono_scale = 0.9 * base_scale;
    let mono_y_offset_factor = 0.1;
    let mono_baseline_offset_factor = -0.1;

    let super_y_offset_factor = -1. / 4.;
    let sub_y_offset_factor = 1. / 4.;
    let super_scale = 3. / 4.;
    let sub_scale = 3. / 4.;

    fonts.font_data.insert(
        "sans".to_string(),
        FontData {
            tweak: FontTweak { scale: base_scale, ..FontTweak::default() },
            ..FontData::from_static(sans)
        }
        .into(),
    );
    fonts.font_data.insert("mono".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: mono_y_offset_factor,
                scale: mono_scale,
                baseline_offset_factor: mono_baseline_offset_factor,
                ..Default::default()
            },
            ..FontData::from_static(mono)
        }.into()
    });
    fonts.font_data.insert(
        "bold".to_string(),
        FontData {
            tweak: FontTweak { scale: base_scale, ..FontTweak::default() },
            ..FontData::from_static(bold)
        }.into(),
    );

    fonts.font_data.insert("sans_super".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: super_y_offset_factor,
                scale: super_scale * base_scale,
                ..Default::default()
            },
            ..FontData::from_static(sans)
        }.into()
    });
    fonts.font_data.insert("bold_super".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: super_y_offset_factor,
                scale: super_scale * base_scale,
                ..Default::default()
            },
            ..FontData::from_static(bold)
        }.into()
    });
    fonts.font_data.insert("mono_super".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: super_y_offset_factor + mono_y_offset_factor,
                scale: super_scale * mono_scale,
                baseline_offset_factor: mono_baseline_offset_factor,
                ..Default::default()
            },
            ..FontData::from_static(mono)
        }.into()
    });

    fonts.font_data.insert("sans_sub".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: sub_y_offset_factor,
                scale: sub_scale * base_scale,
                ..Default::default()
            },
            ..FontData::from_static(sans)
        }.into()
    });
    fonts.font_data.insert("bold_sub".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: sub_y_offset_factor,
                scale: sub_scale * base_scale,
                ..Default::default()
            },
            ..FontData::from_static(bold)
        }.into()
    });
    fonts.font_data.insert("mono_sub".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: sub_y_offset_factor + mono_y_offset_factor,
                scale: sub_scale * mono_scale,
                baseline_offset_factor: mono_baseline_offset_factor,
                ..Default::default()
            },
            ..FontData::from_static(mono)
        }.into()
    });

    fonts.font_data.insert("icons".into(), {
        FontData {
            tweak: FontTweak { y_offset: -0.1, scale: mono_scale, ..Default::default() },
            ..FontData::from_static(lb_fonts::NERD_FONTS_MONO_SYMBOLS)
        }.into()
    });

    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Bold")), vec!["bold".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("SansSuper")), vec!["sans_super".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("BoldSuper")), vec!["bold_super".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("MonoSuper")), vec!["mono_super".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("SansSub")), vec!["sans_sub".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("BoldSub")), vec!["bold_sub".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("MonoSub")), vec!["mono_sub".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Icons")), vec!["icons".into()]);

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
        .push("icons".to_owned());
}
