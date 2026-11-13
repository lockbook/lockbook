//! A note drawn off screen whole, for a file: in the app's theme made light,
//! folds open, tables and code wrapped to the width. Its text paints as
//! [`PrintText`] and its formulas as [`PrintSvg`], for a writer to keep as
//! text and paths.

use std::collections::HashMap;
use std::sync::Arc;

use egui::epaint::{ClippedShape, ImageData};
use egui::{ColorImage, Rect, Shape, TextureId, Vec2};
use lb_rs::Uuid;

use super::{Headless, Look, PreviewRequest};
use crate::tab::{ContentState, Destination, Place, Presentation, TabContent, TabResources};
use crate::theme::palette_v2::Mode;
use crate::widgets::diagram_cache::PrintSvg;
use crate::widgets::glyphon_render::PrintText;

/// The whole note in one frame, in its own points.
pub struct PrintFrame {
    pub shapes: Vec<ClippedShape>,
    pub textures: HashMap<TextureId, Arc<ColorImage>>,
    /// Spans, top to bottom, that a break shouldn't cut: lines of text,
    /// images, formulas, and what the renderer found unbreakable.
    pub unbreakable: Vec<(f32, f32)>,
    pub height: f32,
}

/// Draws the note `id` whole, `size.x` wide and at least `size.y` tall.
/// `text` is an open note's text, unsaved edits and all, instead of what's
/// saved.
pub fn print_frame(
    live: &TabResources, id: Uuid, text: Option<String>, size: Vec2,
) -> Result<PrintFrame, String> {
    let mut look = Look::of(&live.ctx);
    look.theme.current = Mode::Light;
    look.zoom = 1.0;
    let req = PreviewRequest {
        place: Place::new(Destination::File(id)),
        size,
        scale: 1.0,
        padding: 0.0,
        text,
    };
    let mut headless = Headless::open(live, look, &req);
    match &headless.tab.content {
        ContentState::Open(TabContent::Markdown(_)) => {}
        ContentState::Failed(failure) => return Err(failure.msg()),
        _ => return Err("only notes print".into()),
    }
    headless.tab.presentation = Presentation::Print;

    // grow to the note's height until what loads at that height, images and
    // embeds, stops growing it
    let mut height = 0.0;
    loop {
        if !headless.settle() {
            return Err("the note didn't settle".into());
        }
        let measured = headless.tab.region_height(None, size.x).unwrap_or(0.0);
        if measured <= height {
            break;
        }
        height = measured;
        headless.points.y = headless.points.y.max(height);
    }
    let renderer_spans = match &headless.tab.content {
        ContentState::Open(TabContent::Markdown(md)) => md.edit.renderer.unbreakable.clone(),
        _ => vec![],
    };

    let textures = headless
        .textures
        .set
        .iter()
        .flat_map(|(id, deltas)| deltas.iter().map(move |d| (id, d)))
        .filter(|(_, delta)| delta.pos.is_none())
        .map(|(id, delta)| {
            let ImageData::Color(image) = &delta.image;
            (*id, image.clone())
        })
        .collect();
    let shapes = std::mem::take(&mut headless.shapes);
    let unbreakable = unbreakable(&shapes, &renderer_spans);
    Ok(PrintFrame { shapes, textures, unbreakable, height })
}

fn unbreakable(shapes: &[ClippedShape], renderer: &[Rect]) -> Vec<(f32, f32)> {
    let mut spans: Vec<_> = renderer.iter().map(|r| (r.top(), r.bottom())).collect();
    for clipped in shapes {
        for_each_shape(&clipped.shape, &mut |shape| match shape {
            Shape::Callback(callback) => {
                if callback.callback.is::<PrintSvg>() {
                    spans.push((callback.rect.top(), callback.rect.bottom()));
                }
                let Some(PrintText(areas)) = callback.callback.downcast_ref() else { return };
                for area in areas {
                    let buffer = area.buffer.read().unwrap();
                    for run in buffer.layout_runs() {
                        let top = area.rect.top() + run.line_top;
                        spans.push((top, top + run.line_height));
                    }
                }
            }
            shape => {
                if let Some((_, rect)) = textured(shape) {
                    spans.push((rect.top(), rect.bottom()));
                }
            }
        });
    }
    spans
}

pub fn for_each_shape(shape: &Shape, f: &mut impl FnMut(&Shape)) {
    match shape {
        Shape::Vec(shapes) => shapes.iter().for_each(|s| for_each_shape(s, f)),
        shape => f(shape),
    }
}

/// The texture a shape shows and where: an image.
pub fn textured(shape: &Shape) -> Option<(TextureId, Rect)> {
    match shape {
        Shape::Rect(r) => r.brush.as_ref().map(|b| (b.fill_texture_id, r.rect)),
        Shape::Mesh(m) if m.texture_id != TextureId::default() => {
            Some((m.texture_id, m.calc_bounds()))
        }
        _ => None,
    }
}
