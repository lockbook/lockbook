use crate::cursor_types::{DocByteOffset, DocCharOffset};
use crate::editor::Editor;
use crate::layout_job::{Annotation, LayoutJobInfo};
use egui::epaint::text::cursor::Cursor;
use egui::{Color32, Galley, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use std::ops::Range;
use std::sync::Arc;

// todo: maybe a nice VisualAppearence struct is in order?
pub static BULLET_RADIUS: f32 = 2.5;
pub static RULE_HEIGHT: f32 = 10.0;

pub struct GalleyInfo {
    pub range: Range<DocByteOffset>,
    pub galley: Arc<Galley>,
    pub annotation: Option<Annotation>,
    // is it better to store this information in Annotation?
    pub head_modification: usize,
    pub tail_modification: usize,
    pub text_location: Pos2,
    pub ui_location: Rect,
}

impl GalleyInfo {
    pub fn from(mut job: LayoutJobInfo, ui: &mut Ui) -> Self {
        let offset = Self::annotation_offset(&job.annotation);
        job.job.wrap.max_width = ui.available_width() - offset.x;
        let range = job.range;
        let annotation = job.annotation;
        let head_modification = job.head_modification;
        let tail_modification = job.tail_modification;

        let galley = ui.ctx().fonts().layout_job(job.job);
        let (ui_location, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), galley.size().y + offset.y),
            Sense::click_and_drag(),
        );

        let text_location = Pos2::new(offset.x + ui_location.min.x, ui_location.min.y);

        Self {
            range,
            galley,
            annotation,
            head_modification,
            tail_modification,
            text_location,
            ui_location,
        }
    }

    // todo: weird thing here, x dim refers to area before the text, y dim refers to area after the
    // text
    fn annotation_offset(annotation: &Option<Annotation>) -> Vec2 {
        let mut offset = Vec2::ZERO;
        if let Some(Annotation::Item(_, indent_level)) = annotation {
            offset.x = *indent_level as f32 * 20.0
        }

        if let Some(Annotation::Rule) = annotation {
            offset.y = RULE_HEIGHT;
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

    pub fn bullet_bounds(&self) -> Rect {
        let bullet_center = self.bullet_center();
        let mut min = bullet_center;
        let mut max = bullet_center;

        let bullet_padding = 2.0;

        min.x -= BULLET_RADIUS + bullet_padding;
        max.x += BULLET_RADIUS + bullet_padding;

        let cursor_height = self.cursor_height();
        min.y -= cursor_height / 2.0;
        max.y -= cursor_height / 2.0;

        Rect { min, max }
    }

    pub fn text<'a>(&self, doc_text: &'a str) -> &'a str {
        let text_start = self.range.start + self.head_modification;
        let text_end = self.range.end - self.tail_modification;
        &doc_text[text_start.0..text_end.0]
    }

    pub fn text_range(&self, editor: &Editor) -> Range<DocCharOffset> {
        let text_start = self.range.start + self.head_modification;
        let text_end = self.range.end - self.tail_modification;
        Range {
            start: editor.byte_offset_to_char(text_start),
            end: editor.byte_offset_to_char(text_end),
        }
    }
}

impl Editor {
    pub fn present_text(&mut self, ui: &mut Ui) {
        self.galleys.clear();
        for block in &self.layout {
            self.galleys.push(GalleyInfo::from(block.clone(), ui));
        }

        for galley in &self.galleys {
            // Draw Annotations
            if let Some(annotation) = &galley.annotation {
                match annotation {
                    Annotation::Item(_, indent_level) => {
                        let bullet_point = galley.bullet_center();

                        match indent_level {
                            1 => ui.painter().circle_filled(
                                bullet_point,
                                BULLET_RADIUS,
                                Color32::WHITE,
                            ),
                            _ => ui.painter().circle_stroke(
                                bullet_point,
                                BULLET_RADIUS,
                                Stroke::new(1.0, Color32::WHITE),
                            ),
                        }
                    }
                    Annotation::Rule => {
                        let mut max = galley.ui_location.max;
                        max.y -= 7.0;

                        let mut min = galley.ui_location.max;
                        min.y -= 7.0;
                        min.x = galley.ui_location.min.x;

                        ui.painter().line_segment(
                            [min, max],
                            Stroke::new(0.1, self.visual_appearance.heading_line()),
                        );
                    }
                    _ => {}
                }
            }

            // Draw Text
            ui.painter()
                .galley(galley.text_location, galley.galley.clone());
        }
    }
}
