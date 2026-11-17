//! Shared action corpus and replay for every canvas property. Interactions are
//! generated as complete transactions; frame boundaries vary independently.

use egui::{Modifiers, Pos2, Rect, pos2, vec2};
use lb_rs::Uuid;
use lb_rs::model::svg::buffer::get_pen_colors;
use lb_rs::model::svg::element::EditableElement;
use resvg::usvg::Transform;
use std::io::Cursor;

use super::super::tools::selection::{BuildPayload, SelectionOperation};
use super::super::{
    CanvasAction as A, EraseEvent, LayerMove, PathEvent, SelectionEvent, ShapeEvent, Tool,
};
use super::harness::TestCanvas;
use crate::tab::input_controller::ToolPayload;
use crate::test_utils::byte_source::ByteSource;

#[derive(Debug)]
pub(super) struct Program {
    pub actions: Vec<A>,
    pub batches: Vec<usize>,
}

impl Program {
    pub fn actions(actions: impl IntoIterator<Item = A>) -> Self {
        Self { actions: actions.into_iter().collect(), batches: vec![1] }
    }

    pub fn with_batches(mut self, batches: Vec<usize>) -> Self {
        self.batches = batches;
        self
    }

    pub fn run(&self, canvas: &mut TestCanvas) {
        let mut offset = 0;
        for size in self.batches.iter().cycle() {
            if offset == self.actions.len() {
                break;
            }
            let end = (offset + size).min(self.actions.len());
            canvas.act_many(self.actions[offset..end].iter().cloned());
            offset = end;
        }
    }
}

pub(super) fn live_points(canvas: &TestCanvas) -> Vec<Pos2> {
    canvas
        .svg_ref()
        .buffer
        .elements
        .values()
        .filter_map(|element| match element {
            EditableElement::Path(path) if !path.deleted => path
                .data
                .manipulator_groups()
                .first()
                .filter(|group| group.anchor.is_finite())
                .map(|group| {
                    canvas
                        .svg_ref()
                        .viewport_settings
                        .document_to_screen(pos2(group.anchor.x as f32, group.anchor.y as f32))
                }),
            _ => None,
        })
        .collect()
}

pub(super) fn click(at: Pos2) -> [A; 2] {
    [
        A::Selection(SelectionEvent::StartLaso(BuildPayload {
            pos: at,
            modifiers: Modifiers::NONE,
        })),
        A::Selection(SelectionEvent::EndTransform),
    ]
}

// Bounds are hints from the initial drawing, not targets resolved during replay.
// They can become stale after edits, just like any generated screen position.
fn select(src: &mut ByteSource, actions: &mut Vec<A>, bounds: &[Rect]) {
    actions.push(A::SetTool(Tool::Selection));
    if !bounds.is_empty() && src.draw(3) != 0 {
        let rect = bounds[src.draw(bounds.len())].expand(20.);
        actions.extend([
            A::Selection(SelectionEvent::StartLaso(BuildPayload {
                pos: rect.min,
                modifiers: Modifiers::NONE,
            })),
            A::Selection(SelectionEvent::LasoBuild(BuildPayload {
                pos: rect.max,
                modifiers: Modifiers::NONE,
            })),
            A::Selection(SelectionEvent::EndLaso),
        ]);
    } else {
        actions.extend(click(point(src)));
    }
}

pub(super) fn generate(
    src: &mut ByteSource, history: bool, max_edits: usize, bounds: &[Rect],
) -> Program {
    let mut actions = Vec::new();
    for _ in 0..src.draw(max_edits + 1) {
        match src.draw(15) {
            14 => actions.push(paste(src)),
            10 => select(src, &mut actions, bounds),
            11 => {
                if src.draw(4) == 0 {
                    actions.push(A::SelectAll);
                } else {
                    select(src, &mut actions, bounds);
                }
                let start = point(src);
                actions.push(A::Selection(SelectionEvent::StartTransform(
                    SelectionOperation::Translation,
                    start,
                )));
                let end = start + vec2(src.draw(81) as f32 - 40., src.draw(81) as f32 - 40.);
                actions.push(A::Selection(SelectionEvent::Transform(start + (end - start) * 0.5)));
                actions.push(A::Selection(SelectionEvent::Transform(end)));
                actions.push(A::Selection(if src.draw(5) == 0 {
                    SelectionEvent::Cancel
                } else {
                    SelectionEvent::EndTransform
                }));
            }
            12 => {
                let factor = [1.25, 0.8, 2., 0.5][src.draw(4)];
                let at = point(src);
                actions.push(A::TransformViewport(
                    Transform::identity()
                        .post_scale(factor, factor)
                        .post_translate((1. - factor) * at.x, (1. - factor) * at.y),
                ));
            }
            13 => actions.push(A::TransformViewport(Transform::from_translate(
                src.draw(101) as f32 - 50.,
                src.draw(101) as f32 - 50.,
            ))),
            _ => {
                let mut edit_actions = Vec::new();
                edit(src, &mut edit_actions, history);
                if matches!(edit_actions.first(), Some(A::Selection(SelectionEvent::SelectAll)))
                    && matches!(
                        edit_actions.last(),
                        Some(
                            A::SetSelectionColor(_)
                                | A::SetSelectionWidth(_)
                                | A::SetSelectionOpacity(_)
                                | A::MoveSelectionLayer(_)
                        )
                    )
                    && src.draw(4) != 0
                {
                    select(src, &mut actions, bounds);
                    actions.extend(edit_actions.into_iter().skip(1));
                } else {
                    actions.extend(edit_actions);
                }
            }
        }
    }
    let batches = (0..1 + src.draw(8)).map(|_| 1 + src.draw(16)).collect();
    Program { actions, batches }
}

pub(super) fn point(src: &mut ByteSource) -> Pos2 {
    pos2(100. + 2. * src.draw(256) as f32, 100. + src.draw(256) as f32)
}

fn payload(src: &mut ByteSource, pos: Pos2) -> ToolPayload {
    ToolPayload {
        pos,
        force: [None, Some(0.), Some(0.25), Some(0.5), Some(1.)][src.draw(5)],
        id: None,
    }
}

fn stroke(src: &mut ByteSource, actions: &mut Vec<A>, highlighter: bool, cancel: bool) {
    let wrap = if highlighter { A::Highlighter } else { A::Pen };
    actions.push(A::SetTool(if highlighter { Tool::Highlighter } else { Tool::Pen }));
    actions.push(A::SetPenColor(get_pen_colors()[src.draw(get_pen_colors().len())]));
    actions.push(A::SetPenWidth([1., 2., 5., 10., 20.][src.draw(5)]));
    actions.push(A::SetPenOpacity([0.1, 0.25, 0.5, 1.][src.draw(4)]));
    let mut pos = point(src);
    actions.push(wrap(PathEvent::Draw(payload(src, pos))));
    for _ in 0..1 + src.draw(16) {
        if src.draw(3) == 0 {
            let predicted = point(src);
            actions.push(wrap(PathEvent::PredictedDraw(payload(src, predicted))));
        }
        pos += vec2(src.draw(61) as f32 - 30., src.draw(61) as f32 - 30.);
        actions.push(wrap(PathEvent::Draw(payload(src, pos))));
    }
    actions.push(wrap(if cancel {
        PathEvent::CancelStroke
    } else {
        PathEvent::End(payload(src, pos))
    }));
}

fn edit(src: &mut ByteSource, actions: &mut Vec<A>, history: bool) {
    match src.draw(10) {
        0 | 1 => {
            let highlighter = src.draw(2) == 1;
            let cancel = src.draw(5) == 0;
            stroke(src, actions, highlighter, cancel);
        }
        2 => {
            let origin = point(src);
            actions.push(A::Shape(ShapeEvent::Build(origin)));
            for _ in 0..1 + src.draw(16) {
                actions.push(A::Shape(ShapeEvent::Build(point(src))));
            }
            actions.push(A::Shape(if src.draw(5) == 0 {
                ShapeEvent::Cancel
            } else {
                ShapeEvent::End
            }));
        }
        3 => {
            actions.push(A::Selection(SelectionEvent::SelectAll));
            actions.push(match src.draw(4) {
                0 => A::SetSelectionColor(get_pen_colors()[src.draw(get_pen_colors().len())]),
                1 => A::SetSelectionWidth([1., 3., 8.][src.draw(3)]),
                2 => A::SetSelectionOpacity([0.1, 0.5, 1.][src.draw(3)]),
                _ => A::MoveSelectionLayer(
                    [
                        LayerMove::ToBack,
                        LayerMove::Backward,
                        LayerMove::Forward,
                        LayerMove::ToFront,
                    ][src.draw(4)],
                ),
            });
        }
        4 => {
            actions.push(A::Selection(SelectionEvent::StartLaso(BuildPayload {
                pos: pos2(20., 20.),
                modifiers: Modifiers::NONE,
            })));
            for _ in 0..1 + src.draw(16) {
                actions.push(A::Selection(SelectionEvent::LasoBuild(BuildPayload {
                    pos: point(src),
                    modifiers: Modifiers::NONE,
                })));
            }
            actions.push(A::Selection(if src.draw(5) == 0 {
                SelectionEvent::Cancel
            } else {
                SelectionEvent::EndLaso
            }));
        }
        5 => {
            actions.push(A::Selection(SelectionEvent::SelectAll));
            actions.push(A::Selection(SelectionEvent::StartTransform(
                [
                    SelectionOperation::EastScale,
                    SelectionOperation::WestScale,
                    SelectionOperation::NorthScale,
                    SelectionOperation::SouthScale,
                    SelectionOperation::NorthWestScale,
                    SelectionOperation::NorthEastScale,
                    SelectionOperation::SouthEastScale,
                    SelectionOperation::SouthWestScale,
                ][src.draw(8)],
                point(src),
            )));
            for _ in 0..1 + src.draw(16) {
                actions.push(A::Selection(SelectionEvent::Transform(point(src))));
            }
            actions.push(A::Selection(if src.draw(5) == 0 {
                SelectionEvent::Cancel
            } else {
                SelectionEvent::EndTransform
            }));
        }
        6 => {
            for _ in 0..1 + src.draw(16) {
                actions.push(A::Erase(EraseEvent::Build(point(src))));
            }
            actions.push(A::Erase(if src.draw(2) == 0 {
                EraseEvent::Cancel
            } else {
                EraseEvent::End
            }));
        }
        7 => {
            actions.push(A::Selection(SelectionEvent::SelectAll));
            actions.push(A::Selection(
                [SelectionEvent::Copy, SelectionEvent::Cut, SelectionEvent::Delete][src.draw(3)],
            ));
        }
        _ if history => actions.push(if src.draw(2) == 0 { A::Undo } else { A::Redo }),
        _ => stroke(src, actions, false, false),
    }
}

pub(super) fn paste(src: &mut ByteSource) -> A {
    let count = 1 + src.draw(3);
    let images = (0..count)
        .map(|_| {
            let width = 16 + src.draw(512) as u32;
            let height = 16 + src.draw(256) as u32;
            let mut bytes = Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                width,
                height,
                image::Rgba([90, 140, 210, 255]),
            ))
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
            (Uuid::new_v4(), bytes.into_inner().into())
        })
        .collect();
    A::PasteImages { images, position: point(src) }
}
