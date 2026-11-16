//! Shared action corpus and replay for every canvas property. Interactions are
//! generated as complete transactions; frame boundaries vary independently.

use egui::{Modifiers, Pos2, Vec2, pos2, vec2};
use lb_rs::model::svg::buffer::get_pen_colors;
use lb_rs::model::svg::element::EditableElement;

use super::super::tools::selection::{BuildPayload, SelectionOperation};
use super::super::{
    CanvasAction as A, EraseEvent, LayerMove, PathEvent, SelectionEvent, ShapeEvent, Tool,
};
use super::harness::TestCanvas;
use crate::tab::input_controller::ToolPayload;
use crate::test_utils::byte_source::ByteSource;

#[derive(Debug)]
pub(super) struct Program {
    pub steps: Vec<Step>,
    pub batches: Vec<usize>,
}

#[derive(Debug)]
pub(super) enum Step {
    Action(A),
    Select(usize),
    // Translation still needs egui's pointer delta. Viewport commands have not
    // been migrated to CanvasAction either, so retain their real input paths.
    Translate(usize, Vec2),
    Zoom(f32, Pos2),
    Pan(Vec2),
}

impl Program {
    pub fn actions(actions: impl IntoIterator<Item = A>) -> Self {
        Self { steps: actions.into_iter().map(Step::Action).collect(), batches: vec![1] }
    }

    pub fn run(&self, canvas: &mut TestCanvas) {
        let mut pending = Vec::new();
        let mut batch = 0;
        for step in &self.steps {
            if let Step::Action(action) = step {
                pending.push(*action);
                if pending.len() >= self.batches[batch % self.batches.len()] {
                    canvas.act_many(pending.drain(..));
                    batch += 1;
                }
                continue;
            }
            if !pending.is_empty() {
                canvas.act_many(pending.drain(..));
                batch += 1;
            }
            match *step {
                Step::Action(_) => unreachable!(),
                Step::Select(index) => {
                    select(canvas, index);
                }
                Step::Translate(index, delta) => {
                    let points = live_points(canvas);
                    if points.is_empty() {
                        continue;
                    }
                    let at = points[index % points.len()];
                    if !on_canvas(canvas, at) || !on_canvas(canvas, at + delta) {
                        continue;
                    }
                    canvas.tool(Tool::Selection);
                    canvas.drag(&[at, at + delta * 0.5, at + delta]);
                    if std::env::var("CANVAS_PROPS_PEN_AFTER_SELECTING").is_ok() {
                        canvas.tool(Tool::Pen);
                    }
                }
                Step::Zoom(factor, at) => canvas.zoom(factor, at),
                Step::Pan(delta) => canvas.pan(delta),
            }
        }
        if !pending.is_empty() {
            canvas.act_many(pending);
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
                .map(|group| pos2(group.anchor.x as f32, group.anchor.y as f32)),
            _ => None,
        })
        .collect()
}

fn on_canvas(canvas: &TestCanvas, at: Pos2) -> bool {
    canvas
        .svg_ref()
        .viewport_settings
        .working_rect
        .shrink(10.)
        .contains(at)
        && !canvas
            .svg_ref()
            .toolbar
            .get_rects()
            .iter()
            .any(|rect| rect.expand(10.).contains(at))
}

pub(super) fn select(canvas: &mut TestCanvas, index: usize) -> bool {
    let points = live_points(canvas);
    if points.is_empty() {
        canvas.act(A::Selection(SelectionEvent::SelectAll));
        return false;
    }
    canvas.act_many([
        A::Selection(SelectionEvent::StartLaso(BuildPayload {
            pos: points[index % points.len()],
            modifiers: Modifiers::NONE,
        })),
        A::Selection(SelectionEvent::EndTransform),
    ]);
    !canvas
        .svg_ref()
        .toolbar
        .selection
        .selected_elements
        .is_empty()
}

pub(super) fn generate(src: &mut ByteSource, history: bool, max_edits: usize) -> Program {
    let mut steps = Vec::new();
    for _ in 0..src.draw(max_edits + 1) {
        let step = match src.draw(14) {
            10 => Step::Select(src.draw(256)),
            11 => Step::Translate(
                src.draw(256),
                vec2(src.draw(81) as f32 - 40., src.draw(81) as f32 - 40.),
            ),
            12 => Step::Zoom([1.25, 0.8, 2., 0.5][src.draw(4)], point(src)),
            13 => Step::Pan(vec2(src.draw(101) as f32 - 50., src.draw(101) as f32 - 50.)),
            _ => {
                let mut actions = Vec::new();
                edit(src, &mut actions, history);
                // Exercise individual strokes as well as whole-selection edits.
                // Moving all strokes to the front/back would always be a no-op.
                if matches!(actions.first(), Some(A::Selection(SelectionEvent::SelectAll)))
                    && matches!(
                        actions.last(),
                        Some(
                            A::SetSelectionColor(_)
                                | A::SetSelectionWidth(_)
                                | A::SetSelectionOpacity(_)
                                | A::MoveSelectionLayer(_)
                        )
                    )
                    && src.draw(4) != 0
                {
                    steps.push(Step::Select(src.draw(256)));
                    steps.extend(actions.into_iter().skip(1).map(Step::Action));
                    continue;
                }
                steps.extend(actions.into_iter().map(Step::Action));
                continue;
            }
        };
        steps.push(step);
    }
    let batches = (0..1 + src.draw(8)).map(|_| 1 + src.draw(32)).collect();
    Program { steps, batches }
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
            actions.push(A::Selection(SelectionEvent::EndLaso));
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
            )));
            for _ in 0..1 + src.draw(16) {
                actions.push(A::Selection(SelectionEvent::Transform(point(src))));
            }
            actions.push(A::Selection(SelectionEvent::EndTransform));
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
