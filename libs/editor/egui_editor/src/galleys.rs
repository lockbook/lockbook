use crate::appearance::Appearance;
use crate::buffer::SubBuffer;
use crate::images::ImageCache;
use crate::layouts::{Annotation, LayoutJobInfo, Layouts};
use crate::offset_types::{DocByteOffset, DocCharOffset, RelByteOffset};
use crate::unicode_segs::UnicodeSegs;
use egui::epaint::text::cursor::Cursor;
use egui::text::CCursor;
use egui::{Galley, Pos2, Rect, Sense, TextFormat, TextureId, Ui, Vec2};
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
    pub head_size: RelByteOffset,
    pub tail_size: RelByteOffset,
    pub text_location: Pos2,
    pub galley_location: Rect,
    pub image: Option<ImageInfo>,

    pub annotation_text_format: TextFormat,
}

pub struct ImageInfo {
    pub location: Rect,
    pub texture: TextureId,
}

pub fn calc(
    layouts: &Layouts, images: &ImageCache, appearance: &Appearance, ui: &mut Ui,
) -> Galleys {
    Galleys {
        galleys: layouts
            .layouts
            .iter()
            .map(|layout| GalleyInfo::from(layout.clone(), images, appearance, ui))
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
            index: (char_offset.clamp(galley_text_range.start, galley_text_range.end)
                - galley_text_range.start)
                .0,
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
                let (location, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), height),
                    Sense::click_and_drag(),
                );
                Some(ImageInfo { location, texture })
            } else {
                None
            }
        } else {
            None
        };

        let galley = ui.ctx().fonts().layout_job(job.job);

        // allocate space for text and non-image annotations
        let (galley_location, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), galley.size().y + offset.y),
            Sense::click_and_drag(),
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
            offset.x = *indent_level as f32 * 20.0 + 15.0
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

    pub fn text<'a>(&self, buffer: &'a SubBuffer) -> &'a str {
        let text_start = self.range.start + self.head_size;
        let text_end = self.range.end - self.tail_size;
        &buffer.text[text_start.0..text_end.0]
    }

    pub fn byte_range(&self) -> Range<DocByteOffset> {
        let text_start = self.range.start + self.head_size;
        let text_end = self.range.end - self.tail_size;
        Range { start: text_start, end: text_end }
    }

    pub fn text_range(&self, segs: &UnicodeSegs) -> Range<DocCharOffset> {
        let byte_range = self.byte_range();
        Range {
            start: segs.byte_offset_to_char(byte_range.start),
            end: segs.byte_offset_to_char(byte_range.end),
        }
    }

    pub fn size(&self) -> RelByteOffset {
        self.range.end - self.range.start
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
