mod styling;

use eframe::egui;
use egui_extras::{Size, StripBuilder};
use pulldown_cmark::{Alignment, Event, HeadingLevel, LinkType, Tag};

use crate::theme::Icon;
use crate::widgets::ButtonGroup;

use self::styling::Styling;

pub struct Markdown {
    pub content: String,
    events: Vec<MdEvent>,
    view_mode: ViewMode,
    single_view: SingleView,
}

impl Markdown {
    pub fn boxed(bytes: &[u8]) -> Box<Self> {
        let content = String::from_utf8_lossy(bytes).to_string();

        Box::new(Self {
            events: parse(&content),
            content,
            view_mode: ViewMode::Dual,
            single_view: SingleView::Editor,
        })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(50.0))
            .vertical(|mut strip| {
                strip.cell(|ui| match &self.view_mode {
                    ViewMode::Single => match &self.single_view {
                        SingleView::Editor => self.draw_editor(ui),
                        SingleView::Rendered => self.draw_rendered(ui),
                    },
                    ViewMode::Dual => {
                        ui.columns(2, |uis| {
                            self.draw_editor(&mut uis[0]);
                            self.draw_rendered(&mut uis[1]);
                        });
                    }
                });
                strip.cell(|ui| self.draw_toolbar(ui));
            });
    }

    fn draw_editor(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_source("editor")
            .show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    let out = egui::TextEdit::multiline(&mut self.content)
                        .desired_width(f32::INFINITY)
                        .code_editor()
                        .show(ui);

                    if out.response.changed() {
                        self.events = parse(&self.content);
                    }
                });
            });
    }

    fn draw_rendered(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_source("render")
            .show(ui, |ui| {
                let initial_size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);

                let layout = egui::Layout::left_to_right()
                    .with_main_wrap(true)
                    .with_cross_align(egui::Align::Center);

                ui.allocate_ui_with_layout(initial_size, layout, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.set_row_height(17.0);

                    let mut s = Styling::default();
                    let mut in_list = false;

                    for el in &self.events {
                        match el {
                            MdEvent::Start(tag) => match tag {
                                MdTag::Heading(lvl, _, _) => s.set_for_heading(lvl),
                                MdTag::BlockQuote => s.set_for_blockquote(),
                                MdTag::List(_) => in_list = true,
                                MdTag::Item => {
                                    ui.label(&Icon::CIRCLE.size(6.0));
                                    ui.label("  ");
                                }
                                _ => {}
                            },
                            MdEvent::End(tag) => match tag {
                                MdTag::Paragraph => {
                                    if !in_list {
                                        ui.end_row();
                                        ui.end_row();
                                    }
                                }
                                MdTag::Heading(_, _, _) => {
                                    s.unset_heading();
                                    ui.end_row();
                                    ui.set_row_height(17.0);
                                    ui.end_row();
                                }
                                MdTag::BlockQuote => {
                                    s.unset_blockquote();
                                }
                                MdTag::List(_) => {
                                    in_list = false;
                                    ui.end_row();
                                }
                                MdTag::Item => {
                                    ui.end_row();
                                }
                                _ => {}
                            },
                            MdEvent::Text(txt) => {
                                ui.label(s.gen_rich_text(txt));
                            }
                            MdEvent::Code(txt) => {
                                s.set_for_code();
                                ui.label(s.gen_rich_text(txt));
                                s.unset_code();
                            }
                            MdEvent::SoftBreak => {
                                ui.label(" ");
                            }
                            MdEvent::HardBreak => {}
                            MdEvent::Rule => {
                                let initial_size = egui::vec2(ui.available_width(), 0.0);
                                ui.allocate_ui_with_layout(
                                    initial_size,
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui.add(egui::Separator::default().horizontal());
                                    },
                                );
                                ui.end_row();
                            }
                            _ => {}
                        }
                    }
                });
            });
    }

    fn draw_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ButtonGroup::toggle_mut(&mut self.view_mode)
                .btn_icon(ViewMode::Single, &Icon::VIDEO_LABEL.size(24.0))
                .btn_icon(ViewMode::Dual, &Icon::VERTICAL_SPLIT.size(24.0))
                .show(ui);

            if self.view_mode == ViewMode::Single {
                ui.add_space(10.0);

                ButtonGroup::toggle_mut(&mut self.single_view)
                    .btn_icon(SingleView::Editor, &Icon::EDIT.size(24.0))
                    .btn_icon(SingleView::Rendered, &Icon::VISIBILITY_ON.size(24.0))
                    .show(ui);
            }
        });
    }
}

fn parse(src: &str) -> Vec<MdEvent> {
    use pulldown_cmark::{Options, Parser};

    let opts = Options::empty()
        .union(Options::ENABLE_STRIKETHROUGH)
        .union(Options::ENABLE_TASKLISTS);

    Parser::new_ext(src, opts).map(MdEvent::from).collect()
}

#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    Single,
    Dual,
}

#[derive(Clone, Copy, PartialEq)]
enum SingleView {
    Editor,
    Rendered,
}

#[derive(Debug)]
enum MdEvent {
    Start(MdTag),
    End(MdTag),
    Text(String),
    Code(String),
    Html(String),
    FootnoteReference(String),
    SoftBreak,
    HardBreak,
    Rule,
    TaskListMarker(bool),
}

impl From<Event<'_>> for MdEvent {
    fn from(v: Event<'_>) -> Self {
        match v {
            Event::Start(tag) => Self::Start(MdTag::from(tag)),
            Event::End(tag) => Self::End(MdTag::from(tag)),
            Event::Text(txt) => Self::Text(txt.into_string()),
            Event::Code(txt) => Self::Code(txt.into_string()),
            Event::Html(txt) => Self::Html(txt.into_string()),
            Event::FootnoteReference(txt) => Self::FootnoteReference(txt.into_string()),
            Event::SoftBreak => Self::SoftBreak,
            Event::HardBreak => Self::HardBreak,
            Event::Rule => Self::Rule,
            Event::TaskListMarker(v) => Self::TaskListMarker(v),
        }
    }
}

#[derive(Debug)]
enum MdTag {
    Paragraph,
    Heading(HeadingLevel, Option<String>, Vec<String>),
    BlockQuote,
    CodeBlockIndented,
    CodeBlockFenced(String),
    List(Option<u64>),
    Item,
    FootnoteDefinition(String),
    Table(Vec<Alignment>),
    TableHead,
    TableRow,
    TableCell,
    Emphasis,
    Strong,
    Strikethrough,
    Link(LinkType, String, String),
    Image(LinkType, String, String),
}

impl From<Tag<'_>> for MdTag {
    fn from(v: Tag<'_>) -> Self {
        use pulldown_cmark::CodeBlockKind;

        match v {
            Tag::Paragraph => Self::Paragraph,
            Tag::Heading(lvl, maybe_id, classes) => Self::Heading(
                lvl,
                maybe_id.map(|s| s.to_string()),
                classes.iter().map(|s| s.to_string()).collect(),
            ),
            Tag::BlockQuote => Self::BlockQuote,
            Tag::CodeBlock(kind) => match kind {
                CodeBlockKind::Indented => Self::CodeBlockIndented,
                CodeBlockKind::Fenced(txt) => Self::CodeBlockFenced(txt.into_string()),
            },
            Tag::List(maybe_first_number) => Self::List(maybe_first_number),
            Tag::Item => Self::Item,
            Tag::FootnoteDefinition(txt) => Self::FootnoteDefinition(txt.into_string()),
            Tag::Table(col_aligns) => Self::Table(col_aligns),
            Tag::TableHead => Self::TableHead,
            Tag::TableRow => Self::TableRow,
            Tag::TableCell => Self::TableCell,
            Tag::Emphasis => Self::Emphasis,
            Tag::Strong => Self::Strong,
            Tag::Strikethrough => Self::Strikethrough,
            Tag::Link(typ, dest, title) => Self::Link(typ, dest.into_string(), title.into_string()),
            Tag::Image(typ, dest, title) => {
                Self::Image(typ, dest.into_string(), title.into_string())
            }
        }
    }
}
