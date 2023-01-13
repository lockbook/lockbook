use crate::appearance::Appearance;
use crate::buffer::Buffer;
use crate::layouts::{Annotation, LayoutJobInfo};
use crate::offset_types::{DocByteOffset, DocCharOffset};
use crate::unicode_segs::UnicodeSegs;
use egui::epaint::text::cursor::Cursor;
use egui::text::CCursor;
use egui::{Galley, Pos2, Rect, Sense, TextFormat, Ui, Vec2};
use std::ops::{Index, Range};
use std::sync::Arc;

#[derive(Default)]
pub struct Galleys {
    pub galleys: Vec<GalleyInfo>,
}

pub struct GalleyInfo {
    pub range: Range<DocByteOffset>,
    pub galley: Arc<Galley>,
    pub annotation: Option<Annotation>,

    // is it better to store this information in Annotation?
    pub head_size: usize,
    pub tail_size: usize,
    pub text_location: Pos2,
    pub ui_location: Rect,

    pub annotation_text_format: TextFormat,
}

pub fn calc(layouts: &[LayoutJobInfo], appearance: &Appearance, ui: &mut Ui) -> Galleys {
    Galleys {
        galleys: layouts
            .iter()
            .map(|layout| GalleyInfo::from(layout.clone(), appearance, ui))
            .collect(),
    }
}

impl Index<usize> for Galleys {
    type Output = GalleyInfo;

    fn index(&self, index: usize) -> &Self::Output {
        &self.galleys[index]
    }
}

// todo: simplify by storing DocByteOffset in GalleyInfo range
// todo: simplify fn and parameter names
impl Galleys {
    pub fn is_empty(&self) -> bool {
        self.galleys.is_empty()
    }

    pub fn len(&self) -> usize {
        self.galleys.len()
    }

    pub fn galley_at_char(&self, char_index: DocCharOffset, segs: &UnicodeSegs) -> usize {
        let byte_offset = segs.char_offset_to_byte(char_index);
        for i in 0..self.galleys.len() {
            let galley = &self.galleys[i];
            if galley.range.start <= byte_offset && byte_offset < galley.range.end {
                return i;
            }
        }
        self.galleys.len() - 1
    }

    pub fn galley_and_cursor_by_char_offset(
        &self, char_offset: DocCharOffset, segs: &UnicodeSegs,
    ) -> (usize, Cursor) {
        let galley_index = self.galley_at_char(char_offset, segs);
        let galley = &self.galleys[galley_index];
        let galley_text_range = galley.text_range(segs);
        let cursor = galley.galley.from_ccursor(CCursor {
            index: (char_offset - galley_text_range.start).0,
            prefer_next_row: true,
        });

        (galley_index, cursor)
    }

    pub fn char_offset_by_galley_and_cursor(
        &self, galley_idx: usize, cursor: &Cursor, segs: &UnicodeSegs,
    ) -> DocCharOffset {
        let galley = &self.galleys[galley_idx];
        let galley_text_range = galley.text_range(segs);
        let mut result = galley_text_range.start + cursor.ccursor.index;

        // correct for prefer_next_row behavior
        let read_cursor = galley.galley.from_ccursor(CCursor {
            index: (result - galley_text_range.start).0,
            prefer_next_row: true,
        });
        if read_cursor.rcursor.row > cursor.rcursor.row {
            result -= 1;
        }

        result
    }
}

impl GalleyInfo {
    pub fn from(mut job: LayoutJobInfo, appearance: &Appearance, ui: &mut Ui) -> Self {
        let offset = Self::annotation_offset(&job.annotation, appearance);
        job.job.wrap.max_width = ui.available_width() - offset.x;

        let galley = ui.ctx().fonts().layout_job(job.job);
        // todo: do this during draw
        let (ui_location, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), galley.size().y + offset.y),
            Sense::click_and_drag(),
        );

        let text_location = Pos2::new(offset.x + ui_location.min.x, ui_location.min.y);

        Self {
            range: job.range,
            galley,
            annotation: job.annotation,
            head_size: job.head_size,
            tail_size: job.tail_size,
            text_location,
            ui_location,
            annotation_text_format: job.annotation_text_format,
        }
    }

    // todo: weird thing here, x dim refers to area before the text, y dim refers to area after the
    // text
    fn annotation_offset(annotation: &Option<Annotation>, appearance: &Appearance) -> Vec2 {
        let mut offset = Vec2::ZERO;
        if let Some(Annotation::Item(_, indent_level)) = annotation {
            offset.x = *indent_level as f32 * 20.0
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

    pub fn text<'a>(&self, buffer: &'a Buffer) -> &'a str {
        let text_start = self.range.start + self.head_size;
        let text_end = self.range.end - self.tail_size;
        &buffer.raw[text_start.0..text_end.0]
    }

    pub fn text_range(&self, segs: &UnicodeSegs) -> Range<DocCharOffset> {
        let text_start = self.range.start + self.head_size;
        let text_end = self.range.end - self.tail_size;
        Range {
            start: segs.byte_offset_to_char(text_start),
            end: segs.byte_offset_to_char(text_end),
        }
    }
}
