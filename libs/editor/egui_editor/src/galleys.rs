use crate::appearance::Appearance;
use crate::ast::{Ast, AstTextRangeType};
use crate::bounds::Paragraphs;
use crate::buffer::SubBuffer;
use crate::element::Element;
use crate::images::ImageCache;
use crate::layouts::{Annotation, LayoutJobInfo};
use crate::offset_types::{DocCharOffset, RangeExt, RelCharOffset};
use crate::Editor;
use egui::epaint::text::cursor::Cursor;
use egui::text::{CCursor, LayoutJob};
use egui::{Galley, Pos2, Rect, Sense, TextFormat, TextureId, Ui, Vec2};
use std::ops::Index;
use std::sync::Arc;
use std::{cmp, mem};

#[derive(Default)]
pub struct Galleys {
    pub galleys: Vec<GalleyInfo>,
}

#[derive(Debug)]
pub struct GalleyInfo {
    pub range: (DocCharOffset, DocCharOffset),
    pub galley: Arc<Galley>,
    pub annotation: Option<Annotation>,

    // is it better to store this information in Annotation?
    pub head_size: RelCharOffset,
    pub tail_size: RelCharOffset,
    pub text_location: Pos2,
    pub galley_location: Rect,
    pub image: Option<ImageInfo>,

    pub annotation_text_format: TextFormat,
}

#[derive(Debug)]
pub struct ImageInfo {
    pub location: Rect,
    pub texture: TextureId,
}

pub fn calc(
    ast: &Ast, buffer: &SubBuffer, paragraphs: &Paragraphs, images: &ImageCache,
    appearance: &Appearance, ui: &mut Ui,
) -> Galleys {
    let mut result: Galleys = Default::default();

    let mut emit_galley = false;
    let mut paragraph_idx = 0;
    let mut past_selection_start = false;
    let mut past_selection_end = false;

    let mut head_size = Default::default();
    let mut tail_size = Default::default();
    let mut annotation: Option<Annotation> = Default::default();
    let mut annotation_text_format = Default::default();
    let mut layout: LayoutJob = Default::default();

    let mut text_range_iter = ast.iter_text_ranges();
    let mut maybe_text_range = text_range_iter.next();

    // combine ast text ranges, paragraphs, and selection
    // each paragraph gets a galley; ast text ranges and selection determine styles and captured characters
    loop {
        let paragraph = paragraphs.paragraphs[paragraph_idx];
        if let Some(text_range) = maybe_text_range.clone() {
            if paragraph.1 < text_range.range.0 {
                // paragraph ends before text_range starts -> emit galley
                emit_galley = true;
            } else if text_range.range.1 < paragraph.0 {
                // text range ends before paragraph starts -> skip text range
                maybe_text_range = text_range_iter.next();
            } else {
                // paragraph and text range overlap -> add non-syntax range text to layout
                let (text_range_portion, in_selection) = {
                    let mut text_range_portion = text_range.clone();
                    let mut in_selection = false;

                    // truncate text range to fit in paragraph
                    text_range_portion.range.0 = cmp::max(text_range_portion.range.0, paragraph.0);
                    text_range_portion.range.1 = cmp::min(text_range_portion.range.1, paragraph.1);

                    // truncate text range to fit before, in, or after selection
                    if !past_selection_start {
                        // before selection start
                        text_range_portion.range.1 =
                            cmp::min(text_range_portion.range.1, buffer.cursor.selection.start());
                    } else if !past_selection_end {
                        // in selection
                        text_range_portion.range.0 =
                            cmp::max(text_range_portion.range.0, buffer.cursor.selection.start());
                        text_range_portion.range.1 =
                            cmp::min(text_range_portion.range.1, buffer.cursor.selection.end());

                        in_selection = true;
                    } else {
                        // after selection end
                        text_range_portion.range.0 =
                            cmp::max(text_range_portion.range.0, buffer.cursor.selection.end());
                    }

                    // advance text range, paragraph, and cursor if they were completed
                    if text_range_portion.range.1 >= text_range.range.1 {
                        maybe_text_range = text_range_iter.next();
                    }
                    if text_range_portion.range.1 >= paragraph.1 {
                        emit_galley = true;
                    }
                    if text_range_portion.range.1 >= buffer.cursor.selection.start() {
                        past_selection_start = true;
                    }
                    if text_range_portion.range.1 >= buffer.cursor.selection.end() {
                        past_selection_end = true;
                    }

                    (text_range_portion, in_selection)
                };

                // construct text format using all styles except the last (current node)
                // only actual text (not head/tail) of each element gets the actual element style
                let mut text_format = TextFormat::default();
                for &node_idx in
                    &text_range_portion.ancestors[0..text_range_portion.ancestors.len() - 1]
                {
                    ast.nodes[node_idx]
                        .element
                        .apply_style(&mut text_format, appearance);
                }
                if in_selection {
                    Element::Selection.apply_style(&mut text_format, appearance);
                }

                // only the first portion of a text range gets that range's annotation
                if text_range.range.0 == text_range_portion.range.0 {
                    annotation = text_range_portion.annotation(ast).or(annotation);
                    annotation_text_format = text_format.clone();
                }

                match text_range_portion.range_type {
                    AstTextRangeType::Head => {
                        if matches!(
                            text_range_portion.element(ast),
                            Element::Heading(..) | Element::Item(..)
                        ) {
                            // these elements have syntax characters captured
                            head_size = text_range_portion.range.len();

                            // apply style e.g. so empty headers still have big font
                            layout.append("", 0.0, text_format);
                        } else {
                            // for other elements, apply the syntax style to head/tail characters
                            Element::Syntax.apply_style(&mut text_format, appearance);
                            layout.append(&buffer[text_range_portion.range], 0.0, text_format);
                        }
                        tail_size = 0.into();
                    }
                    AstTextRangeType::Tail => {
                        // there aren't any captured tail characters, so apply syntax style to all tail characters
                        Element::Syntax.apply_style(&mut text_format, appearance);
                        layout.append(&buffer[text_range_portion.range], 0.0, text_format);

                        // note, the tail of a galley is always zero
                        // it used to be nonzero when newlines were included in galleys and captured in tail
                        // now, newlines are omitted from galley ranges and exist in-between galleys
                        // this code is still here for when we start capturing more syntax characters e.g. line ends in `code`
                        // tail_size = text_range_portion.range.len();
                    }
                    AstTextRangeType::Text => {
                        text_range_portion
                            .element(ast)
                            .apply_style(&mut text_format, appearance);
                        layout.append(&buffer[text_range_portion.range], 0.0, text_format);

                        tail_size = 0.into();
                    }
                }
            }
        }

        if emit_galley || maybe_text_range.is_none() {
            if layout.is_empty() {
                // dummy text with document style
                let mut text_format = Default::default();
                Element::Document.apply_style(&mut text_format, appearance);
                layout.append("", 0.0, text_format);
            }
            let layout_info = LayoutJobInfo {
                range: paragraphs.paragraphs[paragraph_idx],
                job: mem::take(&mut layout),
                annotation: mem::take(&mut annotation),
                head_size: mem::take(&mut head_size),
                tail_size: mem::take(&mut tail_size),
                annotation_text_format: mem::take(&mut annotation_text_format),
            };
            result
                .galleys
                .push(GalleyInfo::from(layout_info, images, appearance, ui));

            paragraph_idx += 1;
            emit_galley = false;
        }

        if paragraph_idx == paragraphs.paragraphs.len() || maybe_text_range.is_none() {
            break;
        }
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
            if galley.range.contains(offset) {
                return i;
            }
        }
        self.galleys.len() - 1
    }

    pub fn galley_and_cursor_by_char_offset(&self, char_offset: DocCharOffset) -> (usize, Cursor) {
        let galley_index = self.galley_at_char(char_offset);
        let galley = &self.galleys[galley_index];
        let galley_text_range = galley.text_range();
        let cursor = galley.galley.from_ccursor(CCursor {
            index: (char_offset.clamp(galley_text_range.start(), galley_text_range.end())
                - galley_text_range.start())
            .0,
            prefer_next_row: true,
        });

        (galley_index, cursor)
    }

    pub fn char_offset_by_galley_and_cursor(
        &self, galley_idx: usize, cursor: &Cursor,
    ) -> DocCharOffset {
        let galley = &self.galleys[galley_idx];
        let galley_text_range = galley.text_range();
        let mut result = galley_text_range.start() + cursor.ccursor.index;

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

impl GalleyInfo {
    pub fn from(
        mut job: LayoutJobInfo, images: &ImageCache, appearance: &Appearance, ui: &mut Ui,
    ) -> Self {
        let offset = Self::annotation_offset(&job.annotation, appearance);
        job.job.wrap.max_width = ui.available_width() - offset.x;

        // allocate space for image
        let image = if let Some(Annotation::Image(_, url, _)) = &job.annotation {
            if let Some(&texture) = images.map.get(url) {
                let [image_width, image_height] =
                    ui.ctx().tex_manager().read().meta(texture).unwrap().size;
                let [image_width, image_height] = [image_width as f32, image_height as f32];
                let width =
                    f32::min(ui.available_width() - appearance.image_padding() * 2.0, image_width);
                let height = image_height * width / image_width + appearance.image_padding() * 2.0;
                let (location, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
                Some(ImageInfo { location, texture })
            } else {
                None
            }
        } else {
            None
        };

        let galley = ui.ctx().fonts(|f| f.layout_job(job.job));

        // allocate space for text and non-image annotations
        let (galley_location, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), galley.size().y + offset.y),
            Sense::hover(),
        );

        let text_location = Pos2::new(offset.x + galley_location.min.x, galley_location.min.y);

        Self {
            range: job.range,
            galley,
            annotation: job.annotation,
            head_size: job.head_size,
            tail_size: job.tail_size,
            text_location,
            galley_location,
            image,
            annotation_text_format: job.annotation_text_format,
        }
    }

    // todo: weird thing here, x dim refers to area before the text, y dim refers to area after the
    // text
    fn annotation_offset(annotation: &Option<Annotation>, appearance: &Appearance) -> Vec2 {
        let mut offset = Vec2::ZERO;
        if let Some(Annotation::Item(_, indent_level)) = annotation {
            offset.x = *indent_level as f32 * 20.0 + 20.0
        }

        if let Some(Annotation::Rule) = annotation {
            offset.y = appearance.rule_height();
        }

        offset
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

    pub fn head<'b>(&self, buffer: &'b SubBuffer) -> &'b str {
        &buffer[(self.range.start(), self.range.start() + self.head_size)]
    }

    pub fn text_range(&self) -> (DocCharOffset, DocCharOffset) {
        (self.range.0 + self.head_size, self.range.1 - self.tail_size)
    }
}

impl Editor {
    pub fn print_galleys(&self) {
        println!("layouts:");
        for galley in &self.galleys.galleys {
            println!(
                "galley: range: {:?}, annotation: {:?}, head: {:?}, tail: {:?}",
                &self.buffer.current[galley.range],
                galley.annotation,
                galley.head_size,
                galley.tail_size
            );
        }
    }
}

impl ImageInfo {
    pub fn image_bounds(&self, appearance: &Appearance, ui: &Ui) -> Rect {
        let [image_width, _] = ui
            .ctx()
            .tex_manager()
            .read()
            .meta(self.texture)
            .unwrap()
            .size;
        let width =
            f32::min(ui.available_width() - appearance.image_padding() * 2.0, image_width as f32);
        let mut result = self.location;
        let center_x = ui.available_width() / 2.0;
        result.min.x = center_x - width / 2.0;
        result.max.x = center_x + width / 2.0;
        result.min.y += appearance.image_padding();
        result.max.y -= appearance.image_padding();
        result
    }
}
