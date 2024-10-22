use crate::tab::markdown_editor::appearance::Appearance;
use crate::tab::markdown_editor::ast::{Ast, AstTextRangeType};
use crate::tab::markdown_editor::bounds::{self, Bounds, Text};
use crate::tab::markdown_editor::images::{ImageCache, ImageState};
use crate::tab::markdown_editor::layouts::{Annotation, LayoutJobInfo};
use crate::tab::markdown_editor::style::{MarkdownNode, RenderStyle};
use crate::tab::markdown_editor::Editor;
use egui::epaint::text::cursor::Cursor;
use egui::text::{CCursor, LayoutJob};
use egui::{Galley, Pos2, Rect, Response, Sense, TextFormat, Ui, Vec2};
use lb_rs::text::buffer::Buffer;
use lb_rs::text::offset_types::{DocCharOffset, RangeExt, RelCharOffset};
use std::mem;
use std::ops::{Deref, Index};
use std::sync::Arc;

#[derive(Default)]
pub struct Galleys {
    pub galleys: Vec<GalleyInfo>,
}

#[derive(Debug)]
pub struct GalleyInfo {
    pub range: (DocCharOffset, DocCharOffset),
    pub galley: Arc<Galley>,
    pub annotation: Option<Annotation>,

    // the head and tail size of a galley are always the head size of the first ast node and tail size of the last ast node
    pub head_size: RelCharOffset,
    pub tail_size: RelCharOffset,

    pub text_location: Pos2,
    pub rect: Rect,
    pub response: Response,
    pub image: Option<ImageInfo>,

    pub annotation_text_format: TextFormat,
}

#[derive(Debug)]
pub struct ImageInfo {
    pub location: Rect,
    pub image_state: ImageState,
}

pub fn calc(
    ast: &Ast, buffer: &Buffer, bounds: &Bounds, images: &ImageCache, appearance: &Appearance,
    touch_mode: bool, ui: &mut Ui,
) -> Galleys {
    let max_rect = ui.max_rect();

    let mut result: Galleys = Default::default();

    let mut head_size: RelCharOffset = 0.into();
    let mut annotation: Option<Annotation> = Default::default();
    let mut annotation_text_format = Default::default();
    let mut layout: LayoutJob = Default::default();

    // join all bounds that affect rendering
    // emit one galley per paragraph; other data is used to determine style
    let ast_ranges = bounds
        .ast
        .iter()
        .map(|range| range.range)
        .collect::<Vec<_>>();
    for ([ast_idx, paragraph_idx, link_idx, selection_idx, text_idx], text_range_portion) in
        bounds::join([
            &ast_ranges,
            &bounds.paragraphs,
            &bounds.links,
            &[buffer.current.selection],
            &bounds.text,
        ])
    {
        // paragraphs cover all text; will always have at least one possibly-empty paragraph
        let paragraph_idx = if let Some(paragraph_idx) = paragraph_idx {
            paragraph_idx
        } else {
            continue;
        };
        let paragraph = bounds.paragraphs[paragraph_idx];

        if let Some(ast_idx) = ast_idx {
            let text_range = &bounds.ast[ast_idx];
            let maybe_link_range = link_idx.map(|link_idx| bounds.links[link_idx]);
            let in_selection = selection_idx.is_some() && !buffer.current.selection.is_empty();

            let captured = text_idx.is_none();

            // construct text format using styles for all ancestor nodes
            let mut text_format = TextFormat::default();
            for &node_idx in &text_range.ancestors[0..text_range.ancestors.len()] {
                RenderStyle::Markdown(ast.nodes[node_idx].node_type.clone()).apply_style(
                    &mut text_format,
                    appearance,
                    ui.visuals(),
                );
            }
            if in_selection && !cfg!(target_os = "ios") {
                // iOS draws its own selection rects
                RenderStyle::Selection.apply_style(&mut text_format, appearance, ui.visuals());
            }
            if maybe_link_range.is_some() {
                RenderStyle::PlaintextLink.apply_style(&mut text_format, appearance, ui.visuals());
            }

            let mut is_annotation = false;
            let annotate_text_ranges =
                matches!(text_range.annotation(ast, false), Some(Annotation::CodeBlock { .. }));
            let capture_unnecessary = matches!(
                text_range.annotation(ast, false),
                Some(Annotation::HeadingRule | Annotation::Image(..))
            );
            if (text_range.range_type == AstTextRangeType::Head || annotate_text_ranges) // code blocks annotate multiple paragraphs
                && (captured || capture_unnecessary || annotate_text_ranges) // heading rules, images, and code blocks drawn regardless of capture
                && (annotation.is_none())
            {
                annotation = text_range.annotation(ast, captured);
                annotation_text_format = text_format.clone();
                is_annotation = annotation.is_some();
            }

            let text = &buffer[text_range_portion];
            match text_range.range_type {
                AstTextRangeType::Head => {
                    if captured {
                        // need to append empty text to layout so that the style is applied
                        layout.append("", 0.0, text_format);
                    } else {
                        // uncaptured syntax characters have syntax style applied on top of node style
                        RenderStyle::Syntax.apply_style(&mut text_format, appearance, ui.visuals());
                        layout.append(text, 0.0, text_format);
                    }

                    if captured && is_annotation {
                        head_size = text_range.range.len();
                    }
                }
                AstTextRangeType::Tail => {
                    if captured {
                        // need to append empty text to layout so that the style is applied
                        layout.append("", 0.0, text_format);
                    } else {
                        // uncaptured syntax characters have syntax style applied on top of node style
                        RenderStyle::Syntax.apply_style(&mut text_format, appearance, ui.visuals());
                        layout.append(text, 0.0, text_format);
                    }
                }
                AstTextRangeType::Text => {
                    layout.append(text, 0.0, text_format);
                }
            }
        }

        let last_galley = text_range_portion.end() == buffer.current.segs.last_cursor_position();
        if text_range_portion.end() == paragraph.end() {
            // emit a galley
            if layout.is_empty() {
                // dummy text with document style
                let mut text_format = Default::default();
                RenderStyle::Markdown(MarkdownNode::Document).apply_style(
                    &mut text_format,
                    appearance,
                    ui.visuals(),
                );
                layout.append("", 0.0, text_format);
            }
            let layout_info = LayoutJobInfo {
                range: bounds.paragraphs[paragraph_idx],
                job: mem::take(&mut layout),
                annotation: mem::take(&mut annotation),
                head_size: mem::take(&mut head_size),
                tail_size: 0.into(),
                annotation_text_format: mem::take(&mut annotation_text_format),
            };
            result.galleys.push(GalleyInfo::from(
                layout_info,
                images,
                appearance,
                touch_mode,
                last_galley,
                max_rect,
                ui,
            ));
        };
    }

    result
}

impl Index<usize> for Galleys {
    type Output = GalleyInfo;

    fn index(&self, index: usize) -> &Self::Output {
        &self.galleys[index]
    }
}

impl Galleys {
    pub fn is_empty(&self) -> bool {
        self.galleys.is_empty()
    }

    pub fn len(&self) -> usize {
        self.galleys.len()
    }

    pub fn galley_at_char(&self, offset: DocCharOffset) -> usize {
        for i in 0..self.galleys.len() {
            let galley = &self.galleys[i];
            if galley.range.contains_inclusive(offset) {
                return i;
            }
        }
        self.galleys.len() - 1
    }

    pub fn galley_and_cursor_by_char_offset(
        &self, char_offset: DocCharOffset, text: &Text,
    ) -> (usize, Cursor) {
        let galley_index = self.galley_at_char(char_offset);
        let galley = &self.galleys[galley_index];
        let galley_text_range = galley.text_range();
        let char_offset = char_offset.clamp(galley_text_range.start(), galley_text_range.end());

        // adjust for captured syntax chars
        let mut rendered_chars: RelCharOffset = 0.into();
        for text_range in text {
            if text_range.end() <= galley_text_range.start() {
                continue;
            }
            if text_range.start() >= char_offset {
                break;
            }

            let text_range = (
                text_range.start().max(galley_text_range.start()),
                text_range.end().min(char_offset),
            );
            rendered_chars += text_range.len();
        }

        let cursor = galley
            .galley
            .from_ccursor(CCursor { index: rendered_chars.0, prefer_next_row: true });
        (galley_index, cursor)
    }

    pub fn char_offset_by_galley_and_cursor(
        &self, galley_idx: usize, cursor: &Cursor, text: &Text,
    ) -> DocCharOffset {
        let galley = &self.galleys[galley_idx];
        let galley_text_range = galley.text_range();
        let mut result = galley_text_range.start() + cursor.ccursor.index;

        // adjust for captured syntax chars
        let mut last_range: Option<(DocCharOffset, DocCharOffset)> = None;
        for text_range in text {
            if text_range.end() <= galley_text_range.start() {
                continue;
            }

            let text_range = (
                text_range.start().max(galley_text_range.start()),
                text_range.end().min(galley_text_range.end()),
            );
            if let Some(last_range) = last_range {
                result += text_range.start() - last_range.end();
            } else {
                result += text_range.start() - galley_text_range.start();
            }

            if text_range.end() >= result {
                break;
            }

            last_range = Some(text_range);
        }

        // correct for prefer_next_row behavior
        let read_cursor = galley.galley.from_ccursor(CCursor {
            index: (result - galley_text_range.start()).0,
            prefer_next_row: true,
        });
        if read_cursor.rcursor.row > cursor.rcursor.row {
            result -= 1;
        }

        result
    }
}

// okay, it's not a rect in the literal sense, but it represents how much padding is around the text on the four sides
pub fn annotation_offset(annotation: &Option<Annotation>, appearance: &Appearance) -> Rect {
    let mut offset = Rect::ZERO;
    match annotation {
        Some(Annotation::Item(_, indent_level)) => offset.min.x = *indent_level as f32 * 20. + 30.,
        Some(Annotation::HeadingRule) => {
            offset.max.y = appearance.rule_height();
        }
        Some(Annotation::BlockQuote) => {
            offset.min.x = 15.;
        }
        Some(Annotation::CodeBlock { .. }) => {
            offset.min.x = 15.;
            offset.max.x = 15.;
        }
        _ => {}
    }
    offset
}

impl GalleyInfo {
    pub fn from(
        mut job: LayoutJobInfo, images: &ImageCache, appearance: &Appearance, touch_mode: bool,
        last_galley: bool, max_rect: Rect, ui: &mut Ui,
    ) -> Self {
        let offset = annotation_offset(&job.annotation, appearance);
        let text_width = ui.available_width().min(800.);
        let padding_width = (ui.available_width() - text_width) / 2.;
        job.job.wrap.max_width = text_width - (offset.min.x + offset.max.x);

        // allocate space for image
        let image = if let Some(Annotation::Image(_, url, _)) = &job.annotation {
            if let Some(image_state) = images.map.get(url) {
                let image_state = image_state.lock().unwrap().deref().clone();
                let (location, _) = if let ImageState::Loaded(texture) = image_state {
                    let [image_width, image_height] =
                        ui.ctx().tex_manager().read().meta(texture).unwrap().size;
                    let [image_width, image_height] = [image_width as f32, image_height as f32];
                    let width = f32::min(
                        ui.available_width() - appearance.image_padding() * 2.0,
                        image_width,
                    );
                    let height =
                        image_height * width / image_width + appearance.image_padding() * 2.0;
                    ui.allocate_exact_size(Vec2::new(width, height), Sense::hover())
                } else {
                    ui.allocate_exact_size(Vec2::new(200.0, 200.0), Sense::hover())
                };
                Some(ImageInfo { location, image_state })
            } else {
                None
            }
        } else {
            None
        };

        let galley = ui.ctx().fonts(|f| f.layout_job(job.job));

        // allocate space for text and non-image annotations, including end-of-text padding for the last galley
        let mut desired_size =
            Vec2::new(ui.available_width(), galley.size().y + (offset.min.y + offset.max.y));
        let padding_height = if last_galley {
            let min_rect = ui.min_rect();
            let height_to_fill = if min_rect.height() < max_rect.height() {
                // fill available space
                max_rect.height() - min_rect.height()
            } else {
                // end of text padding
                max_rect.height() / 2.
            };
            let padding_height = height_to_fill - desired_size.y;
            desired_size.y = height_to_fill;
            padding_height
        } else {
            0.
        };
        if let Some(Annotation::CodeBlock { text_range, language, .. }) = &job.annotation {
            if text_range == &job.range && !language.is_empty() {
                // the first galley in a code block with a language badge gets extra height for the badge
                desired_size.y += 100.
            }
        }
        let response = ui.allocate_response(
            desired_size,
            Sense { click: true, drag: !touch_mode, focusable: false },
        );

        let text_location =
            Pos2::new(padding_width + response.rect.min.x + offset.min.x, response.rect.min.y);

        let mut text_rect = response.rect;
        text_rect.min.x += offset.min.x + padding_width;
        text_rect.min.y += offset.min.y;
        text_rect.max.x -= offset.max.x + padding_width;
        text_rect.max.y -= offset.max.y + padding_height;

        Self {
            range: job.range,
            galley,
            annotation: job.annotation,
            head_size: job.head_size,
            tail_size: job.tail_size,
            text_location,
            rect: text_rect,
            response,
            image,
            annotation_text_format: job.annotation_text_format,
        }
    }

    pub fn cursor_height(&self) -> f32 {
        self.galley.pos_from_cursor(&Cursor::default()).height()
    }

    pub fn bullet_center(&self) -> Pos2 {
        let mut point = self.text_location;
        point.x -= 10.0;
        point.y += self.cursor_height() / 2.0;
        point
    }

    pub fn bullet_bounds(&self, appearance: &Appearance) -> Rect {
        let bullet_center = self.bullet_center();
        let mut min = bullet_center;
        let mut max = bullet_center;

        let bullet_padding = 2.0;

        let bullet_radius = appearance.bullet_radius();
        min.x -= bullet_radius + bullet_padding;
        max.x += bullet_radius + bullet_padding;

        let cursor_height = self.cursor_height();
        min.y -= cursor_height / 2.0;
        max.y -= cursor_height / 2.0;

        Rect { min, max }
    }

    pub fn checkbox_bounds(&self, appearance: &Appearance) -> Rect {
        let bullet_center = self.bullet_center();
        let mut min = bullet_center;
        let mut max = bullet_center;

        let dim = appearance.checkbox_dim();
        min.x -= dim / 2.0;
        max.x += dim / 2.0;
        min.y -= dim / 2.0;
        max.y += dim / 2.0;

        Rect { min, max }
    }

    pub fn checkbox_slash(&self, appearance: &Appearance) -> [Pos2; 2] {
        let bounds = self.checkbox_bounds(appearance);
        [Pos2 { x: bounds.min.x, y: bounds.max.y }, Pos2 { x: bounds.max.x, y: bounds.min.y }]
    }

    pub fn size(&self) -> RelCharOffset {
        self.range.end() - self.range.start()
    }

    pub fn head<'b>(&self, buffer: &'b Buffer) -> &'b str {
        &buffer[(self.range.start(), self.range.start() + self.head_size)]
    }

    pub fn text_range(&self) -> (DocCharOffset, DocCharOffset) {
        (self.range.0 + self.head_size, self.range.1 - self.tail_size)
    }
}

impl Editor {
    pub fn print_galleys(&self) {
        println!("galleys:");
        for galley in &self.galleys.galleys {
            println!(
                "galley: range: {:?}, annotation: {:?}, head: {:?}, tail: {:?}",
                &self.buffer[galley.range], galley.annotation, galley.head_size, galley.tail_size
            );
        }
    }
}
