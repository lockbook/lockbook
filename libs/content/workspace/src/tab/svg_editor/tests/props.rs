//! One property suite over the shared mixed-action corpus. The same generator,
//! replay, seed loop and shrinker exercise persistence, history, rendering,
//! reloads and erasing. Only the undo invariant excludes generated undo/redo.

use std::fmt::Debug;

use bezier_rs::Subpath;
use egui::{Pos2, Rect, pos2, vec2};
use lb_rs::Uuid;
use lb_rs::model::svg::buffer::{Buffer, get_pen_colors};
use lb_rs::model::svg::element::{DynamicColor, EditableElement, ManipulatorGroupId};
use resvg::usvg::Transform;

use super::super::element::PromoteBufferWeakImages as _;
use super::super::{CanvasAction, EraseEvent, LayerMove, PathEvent, Tool};
use super::actions::{self, Program};
use super::harness::{TestCanvas, TestImages};
use crate::tab::input_controller::ToolPayload;
use crate::test_utils::{byte_source::ByteSource, prop};

// Shared exploration limits for every property.
const DEFAULT_SEEDS: u64 = 100;
const MAX_EDITS: usize = 32;
const CASE_BYTES: usize = 64;

#[test]
fn save_reopen_shows_the_same() {
    check(|b| gen_case(b, true), save_reopen_check);
}

#[test]
fn undo_all_restores_and_redo_all_replays() {
    check(|b| gen_case(b, false), undo_redo_check);
}
#[test]
fn reload_of_own_save_keeps_unsaved_edits() {
    check(|b| gen_case(b, true), reload_own_save_check);
}
#[test]
fn reload_shows_another_editors_save() {
    check(|b| gen_case(b, true), reload_other_check);
}
#[test]
fn untouched_editor_shows_each_save_in_a_chain() {
    check(gen_chain, chain_check);
}
#[test]
fn erasing_preserves_surviving_strokes() {
    check(gen_erase_case, |c| erase_check(c, false));
}
#[test]
fn undoing_erase_restores_the_exact_drawing() {
    check(gen_erase_case, |c| erase_check(c, true));
}

/// Imported content is shared by all scenarios and uses UUID element IDs.
#[derive(Debug, Clone)]
struct FileStroke {
    id: String,
    pts: Vec<(u32, u32)>,
    width: u32,
}

#[derive(Debug)]
struct Case {
    file: Vec<FileStroke>,
    edits: Program,
    later: Program,
}

fn gen_file(src: &mut ByteSource) -> Vec<FileStroke> {
    (0..src.draw(4))
        .map(|i| FileStroke {
            id: Uuid::from_u128(i as u128 + 1).to_string(),
            pts: (0..2 + src.draw(3))
                .map(|_| (150 + src.draw(256) as u32, 120 + src.draw(256) as u32))
                .collect(),
            width: [1, 2, 5, 10, 15][src.draw(5)],
        })
        .collect()
}

fn gen_case(bytes: &[u8], history: bool) -> Case {
    let mut src = ByteSource::new(bytes);
    let file = gen_file(&mut src);
    let bounds = file_bounds(&file);
    Case {
        file,
        edits: actions::generate(&mut src, history, MAX_EDITS, &bounds),
        later: actions::generate(&mut src, history, MAX_EDITS / 2, &bounds),
    }
}

fn file_bounds(file: &[FileStroke]) -> Vec<Rect> {
    file.iter()
        .map(|stroke| {
            let points: Vec<_> = stroke
                .pts
                .iter()
                .map(|&(x, y)| pos2(x as f32, y as f32))
                .collect();
            Rect::from_points(&points).expand(stroke.width as f32 / 2.)
        })
        .collect()
}

fn file_bytes(file: &[FileStroke]) -> String {
    let paths: String = file
        .iter()
        .map(|s| {
            let d: String = s
                .pts
                .iter()
                .enumerate()
                .map(|(i, (x, y))| format!("{} {x} {y} ", if i == 0 { "M" } else { "L" }))
                .collect();
            format!(
                r#"<path id="{}" d="{d}" stroke="rgb(0,0,0)" stroke-width="{}" fill="none"/>"#,
                s.id, s.width
            )
        })
        .collect();
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{paths}</svg>"#)
}

// ════════════════════════ what a person sees ════════════════════════
// live strokes in z order, in file coordinates, named by their svg id

#[derive(Debug, Clone, PartialEq)]
struct Seen {
    id: String,
    pts: Vec<(i64, i64)>,
    rgb: (u8, u8, u8),
    width: i64,
    opacity: i64,
    image: Option<Uuid>,
}

type View = Vec<Seen>;

/// Coordinates to a tenth of a unit: below a pixel at any zoom we use.
fn view(b: &Buffer) -> View {
    b.elements
        .iter()
        .filter_map(|(id, el)| match el {
            EditableElement::Path(p) if !p.deleted && p.data.len() > 1 => {
                let data = &p.data;
                let s = p.stroke.unwrap();
                Some(Seen {
                    id: b.id_map.get(id).cloned().unwrap_or(id.to_string()),
                    pts: data
                        .manipulator_groups()
                        .iter()
                        .map(|g| {
                            ((g.anchor.x * 10.).round() as i64, (g.anchor.y * 10.).round() as i64)
                        })
                        .collect(),
                    rgb: (s.color.light.red, s.color.light.green, s.color.light.blue),
                    width: (s.width * p.stroke_scale() * 100.).round() as i64,
                    opacity: (s.opacity * 255.).round() as i64,
                    image: None,
                })
            }
            EditableElement::Image(image) if !image.deleted => Some(Seen {
                id: b.id_map.get(id).cloned().unwrap_or(id.to_string()),
                pts: vec![
                    (
                        (image.view_box.x() * 10.).round() as i64,
                        (image.view_box.y() * 10.).round() as i64,
                    ),
                    (
                        (image.view_box.width() * 10.).round() as i64,
                        (image.view_box.height() * 10.).round() as i64,
                    ),
                ],
                rgb: (0, 0, 0),
                width: 0,
                opacity: (image.opacity * 255.).round() as i64,
                image: Some(image.href),
            }),
            _ => None,
        })
        .collect()
}

fn canvas_view(c: &TestCanvas) -> View {
    view(&c.svg_ref().buffer)
}

fn file_view(bytes: &str, images: &TestImages) -> View {
    let mut buffer = Buffer::new(bytes);
    buffer.promote_weak_images(images);
    view(&buffer)
}

fn same(what: &str, a: &View, b: &View, ordered: bool) -> Result<(), String> {
    let (mut a, mut b) = (a.clone(), b.clone());
    if !ordered {
        a.sort_by(|x, y| x.id.cmp(&y.id));
        b.sort_by(|x, y| x.id.cmp(&y.id));
    }
    let ids = |v: &View| v.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    if ids(&a) != ids(&b) {
        return Err(format!("{what}: strokes {:?} vs {:?}", ids(&a), ids(&b)));
    }
    for (x, y) in a.iter().zip(&b) {
        // a tenth of a unit of rounding either way
        let near = x.pts.len() == y.pts.len()
            && x.pts
                .iter()
                .zip(&y.pts)
                .all(|(p, q)| (p.0 - q.0).abs() <= 1 && (p.1 - q.1).abs() <= 1);
        if !near
            || x.rgb != y.rgb
            || (x.width - y.width).abs() > 1
            || x.opacity != y.opacity
            || x.image != y.image
        {
            return Err(format!("{what}: {x:?} vs {y:?}"));
        }
    }
    Ok(())
}

// One runner and reproducer for every property scenario.
fn caught(f: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|e| {
        let message = e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
        Err(format!("panicked: {}", message.unwrap_or_default()))
    })
}

fn check<C: Debug>(generate: impl Fn(&[u8]) -> C, property: impl Fn(&C) -> Result<(), String>) {
    prop::run(
        DEFAULT_SEEDS,
        CASE_BYTES,
        |bytes| caught(|| property(&generate(bytes))),
        |bytes| {
            let case = generate(bytes);
            let reason = caught(|| property(&case)).err();
            format!("SHRUNK FAILURE: {reason:?}\n{case:#?}")
        },
    );
}

// ════════════════════════ property bodies ════════════════════════

/// Reopening what the canvas saved shows the same strokes and paints the same pixels.
fn save_reopen_check(c: &Case) -> Result<(), String> {
    let mut canvas = TestCanvas::open(&file_bytes(&c.file));
    c.edits.run(&mut canvas);
    let saved = canvas.save();
    let mut reopened = TestCanvas::open_with_images(&saved, canvas.images.clone());
    // View state is local: compare rendering through the same viewport.
    reopened.set_viewport(canvas.master());
    same("reopened", &canvas_view(&canvas), &canvas_view(&reopened), true)?;
    let (a, b) = (canvas_mask(&canvas), canvas_mask(&reopened));
    differs("reopened canvas", &a, &b)
}

/// A paste after arbitrary edits replaces selection and is one undo transaction.
#[test]
fn image_paste_selects_the_batch_and_undoes_once() {
    check(
        |bytes| {
            let mut src = ByteSource::new(bytes);
            (gen_case(bytes, true), actions::paste(&mut src))
        },
        |(case, paste)| {
            let mut canvas = TestCanvas::open(&file_bytes(&case.file));
            case.edits.run(&mut canvas);
            let before = canvas_view(&canvas);
            let before_ids: Vec<_> = canvas.svg_ref().buffer.elements.keys().copied().collect();
            canvas.act(paste.clone());
            let inserted: Vec<_> = canvas
                .svg_ref()
                .buffer
                .elements
                .keys()
                .filter(|id| !before_ids.contains(id))
                .copied()
                .collect();
            let selected: Vec<_> = canvas
                .svg_ref()
                .toolbar
                .selection
                .selected_elements
                .iter()
                .map(|element| element.id)
                .collect();
            let CanvasAction::PasteImages { images, .. } = paste else { unreachable!() };
            let expected_count = images.len();
            if inserted.len() != expected_count
                || selected != inserted
                || canvas.svg_ref().toolbar.active_tool != Tool::Selection
            {
                return Err(format!(
                    "paste selection: inserted {inserted:?}, selected {selected:?}"
                ));
            }
            let after = canvas_view(&canvas);
            canvas.act(CanvasAction::Undo);
            same("undo paste batch", &before, &canvas_view(&canvas), true)?;
            canvas.act(CanvasAction::Redo);
            same("redo paste batch", &after, &canvas_view(&canvas), true)
        },
    );
}

/// Undoing every gesture gives back the opened drawing; redoing them all
/// gives back the edited one.
fn undo_redo_check(c: &Case) -> Result<(), String> {
    let mut canvas = TestCanvas::open(&file_bytes(&c.file));
    let opened = canvas_view(&canvas);
    c.edits.run(&mut canvas);
    let edited = canvas_view(&canvas);
    while canvas.svg_ref().history.has_undo() {
        canvas.act(CanvasAction::Undo);
    }
    same("after undoing everything", &opened, &canvas_view(&canvas), true)?;
    while canvas.svg_ref().history.has_redo() {
        canvas.act(CanvasAction::Redo);
    }
    same("after redoing everything", &edited, &canvas_view(&canvas), true)
}

/// After a save, sync hands the same file back; gestures made meanwhile
/// survive the reload.
fn reload_own_save_check(c: &Case) -> Result<(), String> {
    let mut canvas = TestCanvas::open(&file_bytes(&c.file));
    c.edits.run(&mut canvas);
    let saved = canvas.save();
    c.later.run(&mut canvas);
    let before = canvas_view(&canvas);
    canvas.reload(&saved);
    same("after reload", &before, &canvas_view(&canvas), true)
}

/// Two editors open the same file; after the first saves, the second,
/// untouched, reloads it and shows exactly what was saved.
fn reload_other_check(c: &Case) -> Result<(), String> {
    let bytes = file_bytes(&c.file);
    let mut first = TestCanvas::open(&bytes);
    let mut second = TestCanvas::open_with_images(&bytes, first.images.clone());
    c.edits.run(&mut first);
    let saved = first.save();
    second.reload(&saved);
    same(
        "second editor after reload",
        &file_view(&saved, &second.images),
        &canvas_view(&second),
        false,
    )
}

// ════════════════════════ rendering ════════════════════════

const SIDE: u32 = super::harness::SCREEN.x as u32;
const TALL: u32 = super::harness::SCREEN.y as u32;

/// What the canvas painted last frame: its stroke meshes, filled.
fn canvas_mask(c: &TestCanvas) -> Vec<bool> {
    use resvg::tiny_skia::{FillRule, Paint, PathBuilder, Pixmap};
    let mut pixmap = Pixmap::new(SIDE, TALL).unwrap();
    let mut paint = Paint { anti_alias: true, ..Paint::default() };
    for clipped in &c.shapes {
        let egui::Shape::Mesh(mesh) = &clipped.shape else { continue };
        if mesh.texture_id != egui::TextureId::default() {
            continue;
        }
        let v = &mesh.vertices;
        for tri in mesh.indices.chunks(3) {
            paint.set_color_rgba8(255, 255, 255, v[tri[0] as usize].color.a());
            let mut pb = PathBuilder::new();
            pb.move_to(v[tri[0] as usize].pos.x, v[tri[0] as usize].pos.y);
            pb.line_to(v[tri[1] as usize].pos.x, v[tri[1] as usize].pos.y);
            pb.line_to(v[tri[2] as usize].pos.x, v[tri[2] as usize].pos.y);
            pb.close();
            if let Some(tri) = pb.finish() {
                pixmap.fill_path(&tri, &paint, FillRule::Winding, Transform::identity(), None);
            }
        }
    }
    pixmap.pixels().iter().map(|px| px.alpha() > 64).collect()
}

/// What any SVG viewer paints for the file, at the canvas's zoom.
fn viewer_mask(file: &str, master: Transform, round: bool) -> Vec<bool> {
    let mut file = file.to_string();
    if round && !file.contains("stroke-linecap=") {
        file = file
            .replace("fill='none'", "fill='none' stroke-linecap='round' stroke-linejoin='round'");
    }
    let tree = resvg::usvg::Tree::from_str(&file, &resvg::usvg::Options::default()).unwrap();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(SIDE, TALL).unwrap();
    resvg::render(&tree, master, &mut pixmap.as_mut());
    pixmap.pixels().iter().map(|px| px.alpha() > 64).collect()
}

/// Pixels set in `a` with nothing set in `b` within `r` pixels.
fn uncovered(a: &[bool], b: &[bool], r: i32) -> Vec<(i32, i32)> {
    let (w, h) = (SIDE as i32, TALL as i32);
    let mut out = vec![];
    for y in 0..h {
        for x in 0..w {
            if !a[(y * w + x) as usize] {
                continue;
            }
            let near = (-r..=r).any(|dy| {
                (-r..=r).any(|dx| {
                    let (xx, yy) = (x + dx, y + dy);
                    xx >= 0 && yy >= 0 && xx < w && yy < h && b[(yy * w + xx) as usize]
                })
            });
            if !near {
                out.push((x, y));
            }
        }
    }
    out
}

fn bbox(px: &[(i32, i32)]) -> String {
    let (x0, x1) = (px.iter().map(|p| p.0).min().unwrap(), px.iter().map(|p| p.0).max().unwrap());
    let (y0, y1) = (px.iter().map(|p| p.1).min().unwrap(), px.iter().map(|p| p.1).max().unwrap());
    format!("{} px in x {x0}..{x1}, y {y0}..{y1}", px.len())
}

/// Two masks agree to within 2 pixels; a pixel-sized dot is below what
/// this comparison judges.
fn differs(what: &str, a: &[bool], b: &[bool]) -> Result<(), String> {
    let (only_a, only_b) = (uncovered(a, b, 2), uncovered(b, a, 2));
    if only_a.len() <= 6 && only_b.len() <= 6 {
        return Ok(());
    }
    let mut why = format!("{what}: ");
    if !only_a.is_empty() {
        why += &format!("first only: {}. ", bbox(&only_a));
    }
    if !only_b.is_empty() {
        why += &format!("second only: {}", bbox(&only_b));
    }
    Err(why)
}

// Editors share the same file and action corpus, taking turns writing.
const WRITERS: usize = 2;

#[derive(Debug)]
struct Chain {
    file: Vec<FileStroke>,
    turns: Vec<(usize, Program)>,
}

fn gen_chain(bytes: &[u8]) -> Chain {
    let mut src = ByteSource::new(bytes);
    let file = gen_file(&mut src);
    let bounds = file_bounds(&file);
    let turns = (0..1 + src.draw(5))
        .map(|_| (src.draw(WRITERS), actions::generate(&mut src, true, MAX_EDITS, &bounds)))
        .collect();
    Chain { file, turns }
}

fn chain_check(chain: &Chain) -> Result<(), String> {
    let mut latest = file_bytes(&chain.file);
    let images = TestImages::default();
    let mut writers: Vec<TestCanvas> = (0..WRITERS)
        .map(|_| TestCanvas::open_with_images(&latest, images.clone()))
        .collect();
    let mut watcher = TestCanvas::open_with_images(&latest, images.clone());
    for (turn, (writer, edits)) in chain.turns.iter().enumerate() {
        let writer = &mut writers[*writer];
        writer.reload(&latest);
        edits.run(writer);
        latest = writer.save();
        watcher.reload(&latest);
        same(
            &format!("untouched editor after turn {turn}"),
            &file_view(&latest, &images),
            &canvas_view(&watcher),
            true,
        )?;
    }
    Ok(())
}

#[derive(Debug)]
struct EraseCase {
    file: Vec<FileStroke>,
    rounds: Vec<EraseRound>,
}

#[derive(Debug)]
struct EraseRound {
    setup: Program,
    erase: Vec<ErasePoint>,
    /// Vary how many actions arrive in each frame, independently of gestures.
    batches: Vec<usize>,
}

#[derive(Debug)]
enum ErasePoint {
    Anchor(usize, usize),
    Position(Pos2),
    Repeat,
}

fn gen_erase_case(bytes: &[u8]) -> EraseCase {
    let mut src = ByteSource::new(bytes);
    let file = gen_file(&mut src);
    let bounds = file_bounds(&file);
    let rounds = (0..1 + src.draw(6))
        .map(|_| {
            let setup = actions::generate(&mut src, true, MAX_EDITS, &bounds);
            let mut erase = vec![ErasePoint::Anchor(src.draw(256), src.draw(256))];
            for _ in 0..src.draw(64) {
                erase.push(match src.draw(4) {
                    0 | 1 => ErasePoint::Anchor(src.draw(256), src.draw(256)),
                    2 => ErasePoint::Position(actions::point(&mut src) - vec2(200., 200.)),
                    _ => ErasePoint::Repeat,
                });
            }
            let batches = (0..1 + src.draw(8)).map(|_| 1 + src.draw(32)).collect();
            EraseRound { setup, erase, batches }
        })
        .collect();
    EraseCase { file, rounds }
}

/// Exact editable path state: includes Bézier handles and pressure, unlike the
/// rounded anchor-only view used by the save/reload properties. These actions
/// create unfilled paths; renderer caches and dirty flags are not drawing state.
#[derive(Clone, Debug)]
struct StrokeSnapshot {
    id: Uuid,
    data: Subpath<ManipulatorGroupId>,
    visible: bool,
    transform: Transform,
    stroke: Option<(DynamicColor, f32, f32)>,
    pressures: Option<Vec<f32>>,
}

impl PartialEq for StrokeSnapshot {
    fn eq(&self, other: &Self) -> bool {
        // Earlier actions can expose NaN geometry bugs. Compare exact bits so
        // unchanged invalid geometry is not falsely attributed to erasing.
        let point = |p: glam::DVec2| (p.x.to_bits(), p.y.to_bits());
        let geometry = |data: &Subpath<ManipulatorGroupId>| {
            data.manipulator_groups()
                .iter()
                .map(|group| {
                    (point(group.anchor), group.in_handle.map(point), group.out_handle.map(point))
                })
                .collect::<Vec<_>>()
        };
        let transform = |t: Transform| [t.sx, t.ky, t.kx, t.sy, t.tx, t.ty].map(f32::to_bits);
        let stroke = |s: Option<(DynamicColor, f32, f32)>| {
            s.map(|(color, width, opacity)| (color, width.to_bits(), opacity.to_bits()))
        };
        let pressures = |p: &Option<Vec<f32>>| {
            p.as_ref()
                .map(|values| values.iter().map(|v| v.to_bits()).collect::<Vec<_>>())
        };
        self.id == other.id
            && self.visible == other.visible
            && self.data.closed() == other.data.closed()
            && geometry(&self.data) == geometry(&other.data)
            && transform(self.transform) == transform(other.transform)
            && stroke(self.stroke) == stroke(other.stroke)
            && pressures(&self.pressures) == pressures(&other.pressures)
    }
}

fn snapshot(canvas: &TestCanvas) -> Vec<StrokeSnapshot> {
    let buffer = &canvas.svg_ref().buffer;
    buffer
        .elements
        .iter()
        .filter_map(|(id, element)| match element {
            EditableElement::Path(path) if !path.deleted => Some(StrokeSnapshot {
                id: *id,
                data: path.data.clone(),
                visible: path.visible,
                transform: path.transform,
                stroke: path
                    .stroke
                    .map(|stroke| (stroke.color, stroke.width, stroke.opacity)),
                pressures: buffer.weak_path_pressures.get(id).cloned(),
            }),
            _ => None,
        })
        .collect()
}

fn erase_check(case: &EraseCase, undo: bool) -> Result<(), String> {
    let mut canvas = TestCanvas::open(&file_bytes(&case.file));
    for (round_index, round) in case.rounds.iter().enumerate() {
        round.setup.run(&mut canvas);
        let before = snapshot(&canvas);
        let targets: Vec<_> = before
            .iter()
            .filter(|stroke| {
                stroke.data.len_segments() > 0
                    && stroke
                        .data
                        .manipulator_groups()
                        .iter()
                        .all(|group| group.anchor.is_finite())
            })
            .collect();
        let mut last = Pos2::ZERO;
        let points: Vec<_> = round
            .erase
            .iter()
            .map(|point| {
                last = match point {
                    ErasePoint::Anchor(stroke, anchor) if !targets.is_empty() => {
                        let anchors = targets[stroke % targets.len()].data.manipulator_groups();
                        let anchor = anchors[anchor % anchors.len()].anchor;
                        canvas
                            .svg_ref()
                            .viewport_settings
                            .document_to_screen(pos2(anchor.x as f32, anchor.y as f32))
                    }
                    ErasePoint::Anchor(..) => Pos2::ZERO,
                    ErasePoint::Position(pos) => *pos,
                    ErasePoint::Repeat => last,
                };
                last
            })
            .collect();
        let mut actions: Vec<_> = points
            .iter()
            .map(|pos| CanvasAction::Erase(EraseEvent::Build(*pos)))
            .collect();
        actions.push(CanvasAction::Erase(EraseEvent::End));
        Program::actions(actions)
            .with_batches(round.batches.clone())
            .run(&mut canvas);
        let after = snapshot(&canvas);

        if undo {
            // A miss does not add history; undoing it would undo an earlier edit.
            if after.len() < before.len() {
                canvas.act(CanvasAction::Undo);
                let restored = snapshot(&canvas);
                if restored != before {
                    return Err(format!(
                        "round {round_index}: undo erase changed the drawing\nbefore: {before:?}\nrestored: {restored:?}"
                    ));
                }
            }
        } else {
            let survivors: Vec<_> = before
                .iter()
                .filter(|stroke| after.iter().any(|s| s.id == stroke.id))
                .cloned()
                .collect();
            if after != survivors {
                return Err(format!(
                    "round {round_index}: erase changed surviving strokes\nexpected: {survivors:?}\nafter: {after:?}"
                ));
            }
            // A disjoint bounding box guarantees a stroke was never touched.
            // This also catches accidental deletion of unrelated strokes.
            let radius = canvas.svg_ref().toolbar.eraser.radius;
            for stroke in &before {
                if stroke.data.is_empty() {
                    continue;
                }
                if let Some([min, max]) = stroke.data.bounding_box() {
                    if !min.is_finite() || !max.is_finite() {
                        continue;
                    }
                    let bounds = canvas.svg_ref().viewport_settings.document_rect_to_screen(
                        Rect::from_min_max(
                            pos2(min.x as f32, min.y as f32),
                            pos2(max.x as f32, max.y as f32),
                        ),
                    );
                    if points.iter().all(|pos| {
                        !bounds.intersects(Rect::from_center_size(
                            *pos,
                            vec2(2. * radius, 2. * radius),
                        ))
                    }) && !after.iter().any(|s| s.id == stroke.id)
                    {
                        return Err(format!(
                            "round {round_index}: erase deleted untouched stroke {}",
                            stroke.id
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn draw(canvas: &mut TestCanvas, tool: Tool, points: &[Pos2]) {
    canvas.tool(tool);
    canvas.drag(points);
}

fn erase_first(canvas: &mut TestCanvas) {
    let points = actions::live_points(canvas);
    if let Some(at) = points.first() {
        canvas.act_many([
            CanvasAction::Erase(EraseEvent::Build(*at)),
            CanvasAction::Erase(EraseEvent::End),
        ]);
    }
}

#[test]
fn selection_opacity_survives_save_reopen_and_undo() {
    for tool in [Tool::Pen, Tool::Highlighter] {
        let mut canvas = TestCanvas::open(r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#);
        draw(&mut canvas, tool, &[pos2(300., 300.), pos2(350., 320.)]);
        let original = canvas_view(&canvas);
        canvas.act(CanvasAction::SelectAll);
        canvas.act(CanvasAction::SetSelectionOpacity(0.5));
        let edited = canvas_view(&canvas);
        assert_eq!(edited[0].opacity, 128);
        let saved = canvas.save();
        same("saved stroke opacity", &edited, &file_view(&saved, &canvas.images), true).unwrap();
        let reopened = TestCanvas::open(&saved);
        same("reopened stroke opacity", &edited, &canvas_view(&reopened), true).unwrap();
        canvas.act(CanvasAction::Undo);
        same("undo opacity", &original, &canvas_view(&canvas), true).unwrap();
        canvas.act(CanvasAction::Redo);
        same("redo opacity", &edited, &canvas_view(&canvas), true).unwrap();
        erase_first(&mut canvas);
        assert!(canvas_view(&canvas).is_empty());
        canvas.act(CanvasAction::Undo);
        same("undo erase preserves opacity", &edited, &canvas_view(&canvas), true).unwrap();
    }
}

fn fixed_stroke(points: &[Pos2]) -> Vec<CanvasAction> {
    let payload = |pos| ToolPayload { pos, force: None, id: None };
    points
        .iter()
        .map(|pos| CanvasAction::Pen(PathEvent::Draw(payload(*pos))))
        .chain(std::iter::once(CanvasAction::Pen(PathEvent::End(payload(*points.last().unwrap())))))
        .collect()
}

#[test]
fn chain_keeps_stacking_order() {
    let mut second = fixed_stroke(&[pos2(300., 300.), pos2(360., 330.)]);
    second.extend(actions::click(pos2(300., 300.)));
    second.push(CanvasAction::MoveSelectionLayer(LayerMove::Backward));
    let chain = Chain {
        file: vec![FileStroke {
            id: Uuid::from_u128(1).to_string(),
            pts: vec![(500, 400), (560, 420)],
            width: 2,
        }],
        turns: vec![
            (0, Program::actions(fixed_stroke(&[pos2(200., 200.), pos2(260., 230.)]))),
            (1, Program::actions(second)),
        ],
    };
    chain_check(&chain).unwrap();
}

// A viewport change must affect subsequent screen-space input, including
// actions in the same frame, without leaking view state into the saved SVG.
#[test]
fn save_reopen_after_zoom_and_drawing_in_one_frame() {
    for batch in [1, 32] {
        let mut canvas = TestCanvas::open(r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#);
        let mut actions = vec![CanvasAction::TransformViewport(
            Transform::from_scale(2., 2.).post_translate(-100., -100.),
        )];
        actions.extend(fixed_stroke(&[pos2(300., 300.), pos2(400., 400.)]));
        Program::actions(actions)
            .with_batches(vec![batch])
            .run(&mut canvas);
        let drawing = canvas_view(&canvas);
        assert_eq!(drawing.len(), 1);
        assert_eq!(drawing[0].pts.first(), Some(&(2000, 2000)));
        assert_eq!(drawing[0].pts.last(), Some(&(2500, 2500)));
        // End the final frame with another viewport change, then save immediately.
        canvas.act(CanvasAction::TransformViewport(Transform::from_translate(20., 10.)));
        let saved = canvas.save();
        assert!(!saved.contains("lb_viewport_settings"));
        assert!(!saved.contains("lb_master_transform"));
        let mut reopened = TestCanvas::open_with_images(&saved, canvas.images.clone());
        reopened.set_viewport(canvas.master());
        same("zoom then draw", &drawing, &canvas_view(&reopened), true).unwrap();
        differs("zoom then draw", &canvas_mask(&canvas), &canvas_mask(&reopened)).unwrap();
    }
}

// Position-based transforms must behave identically regardless of frame batching.
#[test]
fn selection_translation_uses_each_action_position() {
    use super::super::SelectionEvent as S;
    use super::super::tools::selection::SelectionOperation;
    let file = file_bytes(&[FileStroke {
        id: Uuid::from_u128(1).to_string(),
        pts: vec![(300, 300), (360, 330)],
        width: 2,
    }]);
    for batch in [1, 32] {
        let mut c = TestCanvas::open(&file);
        // Fixed screen positions below use a one-to-one document view.
        c.set_viewport(Transform::identity());
        let before = canvas_view(&c);
        Program::actions([
            CanvasAction::SelectAll,
            CanvasAction::Selection(S::StartTransform(
                SelectionOperation::Translation,
                pos2(300., 300.),
            )),
            CanvasAction::Selection(S::Transform(pos2(310., 305.))),
            CanvasAction::Selection(S::Transform(pos2(320., 308.))),
            CanvasAction::Selection(S::EndTransform),
        ])
        .with_batches(vec![batch])
        .run(&mut c);
        let mut expected = before.clone();
        for stroke in &mut expected {
            for (x, y) in &mut stroke.pts {
                *x += 200;
                *y += 80;
            }
        }
        same("translation", &expected, &canvas_view(&c), true).unwrap();
        c.act(CanvasAction::Undo);
        same("undo whole drag", &before, &canvas_view(&c), true).unwrap();
        assert!(!c.svg_ref().history.has_undo());

        c.act_many([
            CanvasAction::Selection(S::StartTransform(
                SelectionOperation::Translation,
                pos2(300., 300.),
            )),
            CanvasAction::Selection(S::Transform(pos2(320., 308.))),
            CanvasAction::Selection(S::Cancel),
            CanvasAction::Selection(S::Transform(pos2(500., 500.))),
        ]);
        same("cancel ends drag", &expected, &canvas_view(&c), true).unwrap();
        c.act(CanvasAction::Undo);
        same("cancel preserves undo", &before, &canvas_view(&c), true).unwrap();
    }
}

#[test]
fn viewport_actions_preserve_read_only_drawing() {
    let mut c = TestCanvas::open(&file_bytes(&[FileStroke {
        id: Uuid::from_u128(1).to_string(),
        pts: vec![(300, 300), (360, 330)],
        width: 2,
    }]));
    let before = canvas_view(&c);
    let master = c.master();
    c.svg().read_only = true;
    c.act_many([
        CanvasAction::SelectAll,
        CanvasAction::DeleteSelection,
        CanvasAction::TransformViewport(Transform::from_translate(20., 10.)),
        CanvasAction::TransformViewport(Transform::from_scale(2., 2.)),
    ]);
    assert_ne!(master, c.master());
    assert!(c.svg_ref().toolbar.selection.selected_elements.is_empty());
    same("viewport keeps document coordinates", &before, &canvas_view(&c), true).unwrap();
}

#[test]
fn pointer_moves_in_one_frame_translate_once_per_event() {
    use egui::{Event, Modifiers, PointerButton};
    let mut c = TestCanvas::open(&file_bytes(&[FileStroke {
        id: Uuid::from_u128(1).to_string(),
        pts: vec![(300, 300), (360, 330)],
        width: 2,
    }]));
    c.set_viewport(Transform::identity());
    let mut expected = canvas_view(&c);
    for stroke in &mut expected {
        for (x, y) in &mut stroke.pts {
            *x += 200;
            *y += 80;
        }
    }
    c.tool(Tool::Selection);
    c.frame(vec![Event::PointerMoved(pos2(300., 300.))]);
    c.frame(vec![Event::PointerButton {
        pos: pos2(300., 300.),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    c.frame(vec![Event::PointerMoved(pos2(310., 305.)), Event::PointerMoved(pos2(320., 308.))]);
    c.frame(vec![Event::PointerButton {
        pos: pos2(320., 308.),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    same("pointer translation", &expected, &canvas_view(&c), true).unwrap();
}

#[test]
fn recoloring_preserves_individual_width_and_opacity_through_history() {
    let mut canvas = TestCanvas::open(r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#);
    canvas.act(CanvasAction::SetPenWidth(2.));
    canvas.act(CanvasAction::SetPenOpacity(1.));
    canvas.act_many(fixed_stroke(&[pos2(300., 300.), pos2(350., 320.)]));
    canvas.act(CanvasAction::SetPenWidth(8.));
    canvas.act(CanvasAction::SetPenOpacity(0.5));
    canvas.act_many(fixed_stroke(&[pos2(400., 300.), pos2(450., 320.)]));
    canvas.act(CanvasAction::SelectAll);
    let before = canvas_view(&canvas);
    let color = get_pen_colors()[1];
    let mut expected = before.clone();
    for stroke in &mut expected {
        stroke.rgb = (color.light.red, color.light.green, color.light.blue);
    }
    canvas.act(CanvasAction::SetSelectionColor(color));
    same("color only", &expected, &canvas_view(&canvas), true).unwrap();
    canvas.act(CanvasAction::Undo);
    same("undo color", &before, &canvas_view(&canvas), true).unwrap();
    canvas.act(CanvasAction::Redo);
    same("redo color", &expected, &canvas_view(&canvas), true).unwrap();
}

#[test]
fn invalid_selection_resizes_do_not_change_geometry_or_history() {
    use super::super::SelectionEvent as S;
    use super::super::tools::selection::SelectionOperation as Op;
    let mut canvas = TestCanvas::open(&file_bytes(&[FileStroke {
        id: Uuid::from_u128(1).to_string(),
        pts: vec![(300, 300), (360, 330)],
        width: 2,
    }]));
    canvas.set_viewport(Transform::identity());
    canvas.act(CanvasAction::SelectAll);
    let before = snapshot(&canvas);
    for operation in [
        Op::EastScale,
        Op::WestScale,
        Op::NorthScale,
        Op::SouthScale,
        Op::NorthWestScale,
        Op::NorthEastScale,
        Op::SouthEastScale,
        Op::SouthWestScale,
    ] {
        for pos in [pos2(f32::NAN, 320.), pos2(320., f32::INFINITY), pos2(f32::MAX, f32::MAX)] {
            canvas.act_many([
                CanvasAction::Selection(S::StartTransform(operation, pos2(360., 330.))),
                CanvasAction::Selection(S::Transform(pos)),
                CanvasAction::Selection(S::EndTransform),
            ]);
            assert_eq!(snapshot(&canvas), before);
            assert!(!canvas.svg_ref().history.has_undo());
        }
    }
    // Zero-size selections would divide by zero even with finite pointer input.
    let mut canvas = TestCanvas::open(&file_bytes(&[FileStroke {
        id: Uuid::from_u128(1).to_string(),
        pts: vec![(300, 300), (300, 300)],
        width: 2,
    }]));
    canvas.set_viewport(Transform::identity());
    canvas.act(CanvasAction::SelectAll);
    let before = snapshot(&canvas);
    for operation in [Op::EastScale, Op::SouthEastScale] {
        canvas.act_many([
            CanvasAction::Selection(S::StartTransform(operation, pos2(300., 300.))),
            CanvasAction::Selection(S::Transform(pos2(320., 320.))),
            CanvasAction::Selection(S::EndTransform),
        ]);
        assert_eq!(snapshot(&canvas), before);
        assert!(!canvas.svg_ref().history.has_undo());
    }
}

#[test]
fn selection_ui_handles_undo_removing_all_selected_geometry() {
    let mut canvas = TestCanvas::open(r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#);
    canvas.act_many(fixed_stroke(&[pos2(300., 300.), pos2(350., 320.)]));
    let drawn = canvas_view(&canvas);
    canvas.act(CanvasAction::SelectAll);
    canvas.act(CanvasAction::Undo);
    canvas.frame(vec![]);
    assert!(canvas_view(&canvas).is_empty());
    canvas.act(CanvasAction::Redo);
    same("redo selected stroke", &drawn, &canvas_view(&canvas), true).unwrap();
}

#[test]
fn taps_are_centered_dots_and_survive_prediction_save_and_history() {
    for tool in [Tool::Pen, Tool::Highlighter] {
        for predicted in [false, true] {
            let mut canvas = TestCanvas::open(r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#);
            canvas.tool(tool);
            canvas.act(CanvasAction::SetPenWidth(10.));
            canvas.act(CanvasAction::SetPenOpacity(1.));
            let payload = |pos| ToolPayload { pos, force: Some(0.5), id: None };
            let wrap =
                if tool == Tool::Pen { CanvasAction::Pen } else { CanvasAction::Highlighter };
            let center = pos2(300., 300.);
            canvas.act(wrap(PathEvent::Draw(payload(center))));
            if predicted {
                canvas.act(wrap(PathEvent::PredictedDraw(payload(pos2(320., 310.)))));
                canvas.act(wrap(PathEvent::Draw(payload(center))));
            }
            canvas.act(wrap(PathEvent::End(payload(center))));
            let drawn = canvas_view(&canvas);
            assert_eq!(drawn.len(), 1);
            assert_eq!(drawn[0].pts, vec![(3000, 3000), (3000, 3000)]);
            let mask = canvas_mask(&canvas);
            assert!(mask[300 * SIDE as usize + 300], "tap must paint its center");
            assert!(!mask[300 * SIDE as usize + 307], "tap must not retain a diagonal tail");
            let saved = canvas.save();
            differs("SVG tap", &mask, &viewer_mask(&saved, canvas.master(), false)).unwrap();
            differs("round-capped SVG tap", &mask, &viewer_mask(&saved, canvas.master(), true))
                .unwrap();
            let mut reopened = TestCanvas::open_with_images(&saved, canvas.images.clone());
            reopened.set_viewport(canvas.master());
            same("reopened tap", &drawn, &canvas_view(&reopened), true).unwrap();
            differs("reopened tap", &mask, &canvas_mask(&reopened)).unwrap();
            canvas.act(CanvasAction::Undo);
            assert!(canvas_view(&canvas).is_empty());
            canvas.act(CanvasAction::Redo);
            same("redo tap", &drawn, &canvas_view(&canvas), true).unwrap();
        }
    }
}

#[test]
fn eraser_preview_does_not_change_saved_opacity() {
    let mut canvas = TestCanvas::open(
        "<svg xmlns='http://www.w3.org/2000/svg'><path fill='none' stroke='black' \
         stroke-width='6' d='M300 300 L350 320'/></svg>",
    );
    canvas.tool(Tool::Eraser);
    let before = canvas.file();
    let point = canvas
        .svg_ref()
        .viewport_settings
        .coordinates()
        .screen_point(super::super::coordinates::DocumentPoint(pos2(300., 300.)))
        .into_inner();
    canvas.act(CanvasAction::Erase(EraseEvent::Build(point)));
    for _ in 0..3 {
        canvas.frame(vec![]);
        assert_eq!(canvas.file(), before);
        assert!(!canvas.svg_ref().history.has_undo());
    }
    let mesh_alpha = |canvas: &TestCanvas| {
        canvas
            .shapes
            .iter()
            .filter_map(|shape| {
                let egui::Shape::Mesh(mesh) = &shape.shape else { return None };
                mesh.vertices.first().map(|vertex| vertex.color.a())
            })
            .max()
            .unwrap()
    };
    assert!(mesh_alpha(&canvas) < 100, "the preview should still fade on screen");
    canvas.act(CanvasAction::Erase(EraseEvent::Cancel));
    assert_eq!(canvas.file(), before);
    assert_eq!(mesh_alpha(&canvas), 255);
    canvas.act(CanvasAction::Erase(EraseEvent::Build(point)));
    canvas.act(CanvasAction::Erase(EraseEvent::End));
    assert!(canvas_view(&canvas).is_empty());
    canvas.act(CanvasAction::Undo);
    assert_eq!(canvas.file(), before);
}
