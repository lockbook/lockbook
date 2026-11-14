//! Property tests driving the whole canvas through [`TestCanvas`]: gestures
//! in, file bytes, buffer and painted meshes out. Each property names the
//! gesture tier it is meant to hold for, the way the markdown properties
//! name a `Features` corpus.

use egui::{Key, Modifiers, Pos2, Vec2, pos2, vec2};
use lb_rs::model::svg::buffer::{Buffer, u_transform_to_bezier};
use lb_rs::model::svg::element::Element;
use resvg::usvg::Transform;

use super::super::Tool;
use super::super::keyboard_shortcuts::{CanvasAction, LayerMove};
use super::super::tools::pen::DEFAULT_HIGHLIGHTER_STROKE_WIDTH;
use super::harness::TestCanvas;
use crate::test_utils::byte_source::ByteSource;
use crate::test_utils::prop;
use lb_rs::model::svg::buffer::{get_highlighter_colors, get_pen_colors};

// ════════════════════════ gesture tiers ════════════════════════

#[derive(Clone, Copy, Debug)]
struct Gestures {
    erase: bool,
    /// drag strokes with the selection tool
    move_strokes: bool,
    undo: bool,
    /// pinch zoom and two-finger pan
    viewport: bool,
    /// toolbar controls: restyling and layering the selection, pen settings, the highlighter
    controls: bool,
    /// strokes whose points coincide: a pen tap
    dots: bool,
    /// files from other apps, with ids like Inkscape's `path12`
    readable_ids: bool,
}

impl Gestures {
    /// Draw, erase, move and undo at one zoom.
    const CORE: Gestures = Gestures {
        erase: true,
        move_strokes: true,
        undo: true,
        viewport: false,
        controls: false,
        dots: false,
        readable_ids: false,
    };
    /// Plus zooming, panning, taps and the toolbar's controls.
    const TOOLBAR: Gestures =
        Gestures { viewport: true, controls: true, dots: true, ..Gestures::CORE };
    /// Plus files that came from other apps.
    const ALL: Gestures = Gestures { readable_ids: true, ..Gestures::TOOLBAR };
}

// ════════════════════════ property audit table ════════════════════════
// property → tier → seeds (CANVAS_PROPS_SEEDS overrides)

#[test]
fn save_reopen_shows_the_same() {
    check(Gestures::TOOLBAR, 200, save_reopen_check);
}
#[test]
fn saved_file_renders_like_the_canvas() {
    check(Gestures::CORE, 200, |c| renders_check(c, false));
}
#[test]
fn saved_file_renders_like_the_canvas_given_round_caps() {
    check(Gestures::TOOLBAR, 200, |c| renders_check(c, true));
}
#[test]
fn undo_all_restores_and_redo_all_replays() {
    check(Gestures::TOOLBAR, 200, undo_redo_check);
}
#[test]
fn reload_of_own_save_keeps_unsaved_edits() {
    check(Gestures::TOOLBAR, 1000, reload_own_save_check);
}
#[test]
fn reload_shows_another_editors_save() {
    check(Gestures::ALL, 200, reload_other_check);
}
#[test]
fn untouched_editor_shows_each_save_in_a_chain() {
    check_chain(Gestures::TOOLBAR, 100);
}

// ════════════════════════ scenario ════════════════════════

#[derive(Debug, Clone)]
enum Gesture {
    /// pen stroke through these screen points
    Draw(Vec<Pos2>),
    /// eraser pressed on the nth live stroke's first point
    Erase(usize),
    /// selection tool dragging the nth live stroke by this much
    Move(usize, Vec2),
    Undo,
    Redo,
    Zoom(f32, Pos2),
    Pan(Vec2),
    /// select the nth live stroke, or everything, then set one style
    Restyle(Option<usize>, Style),
    /// select the nth live stroke and move it in the stacking order
    Layer(usize, LayerMove),
    /// a setting for strokes drawn from now on
    PenSetting(Style),
    /// draw with the pen or the highlighter from now on
    DrawWith(Tool),
}

#[derive(Debug, Clone, Copy)]
enum Style {
    /// an index into the pen colors
    Color(usize),
    Width(f32),
    Opacity(f32),
}

fn gen_style(src: &mut ByteSource) -> Style {
    match src.draw(3) {
        0 => Style::Color(src.draw(get_pen_colors().len())),
        1 => Style::Width([1., 2., 5., 10.][src.draw(4)]),
        _ => Style::Opacity([1.0, 0.1, 0.5][src.draw(3)]),
    }
}

/// Away from the toolbar islands along the top and bottom edges.
fn spot(src: &mut ByteSource) -> Pos2 {
    pos2(150. + src.draw(500) as f32, 120. + src.draw(400) as f32)
}

fn gen_gesture(src: &mut ByteSource, g: &Gestures) -> Gesture {
    let w = |on: bool, w: u32| if on { w } else { 0 };
    match src.bias(&[
        4,
        w(g.erase, 2),
        w(g.move_strokes, 3),
        w(g.undo, 1),
        w(g.undo, 1),
        w(g.viewport, 1),
        w(g.viewport, 1),
        w(g.controls, 2),
        w(g.controls, 1),
        w(g.controls, 1),
        w(g.controls, 1),
    ]) {
        0 => {
            let mut p = spot(src);
            let mut pts = vec![p];
            for _ in 0..1 + src.draw(3) {
                let (mut dx, dy) = (src.draw(121) as f32 - 60., src.draw(121) as f32 - 60.);
                if !g.dots && dx == 0. && dy == 0. {
                    dx = 1.;
                }
                p = pos2((p.x + dx).clamp(100., 700.), (p.y + dy).clamp(100., 540.));
                pts.push(p);
            }
            Gesture::Draw(pts)
        }
        1 => Gesture::Erase(src.draw(256)),
        2 => {
            Gesture::Move(src.draw(256), vec2(src.draw(81) as f32 - 40., src.draw(81) as f32 - 40.))
        }
        3 => Gesture::Undo,
        4 => Gesture::Redo,
        5 => Gesture::Zoom([1.25, 0.8, 2.0, 0.5][src.draw(4)], spot(src)),
        6 => Gesture::Pan(vec2(src.draw(101) as f32 - 50., src.draw(101) as f32 - 50.)),
        7 => {
            let target = if src.draw(4) == 0 { None } else { Some(src.draw(256)) };
            Gesture::Restyle(target, gen_style(src))
        }
        8 => Gesture::Layer(
            src.draw(256),
            [LayerMove::ToBack, LayerMove::Backward, LayerMove::Forward, LayerMove::ToFront]
                [src.draw(4)],
        ),
        9 => Gesture::PenSetting(gen_style(src)),
        _ => Gesture::DrawWith([Tool::Pen, Tool::Highlighter][src.draw(2)]),
    }
}

fn gen_gestures(src: &mut ByteSource, g: &Gestures, max: usize) -> Vec<Gesture> {
    let n = src.draw(max + 1);
    (0..n).map(|_| gen_gesture(src, g)).collect()
}

/// A stroke as another app or an earlier session wrote it.
#[derive(Debug, Clone)]
struct FileStroke {
    id: String,
    pts: Vec<(u32, u32)>,
    width: u32,
}

#[derive(Debug)]
struct Case {
    file: Vec<FileStroke>,
    gestures: Vec<Gesture>,
    later: Vec<Gesture>,
}

fn gen_case(b: &[u8], g: &Gestures) -> Case {
    let mut src = ByteSource::new(b);
    let file = (0..src.draw(4))
        .map(|i| {
            let readable = g.readable_ids && src.draw(2) == 1;
            FileStroke {
                id: if readable {
                    format!("path{i}")
                } else {
                    lb_rs::Uuid::from_u128(i as u128 + 1).to_string()
                },
                pts: {
                    let mut pts: Vec<(u32, u32)> = (0..2 + src.draw(3))
                        .map(|_| (150 + src.draw(500) as u32, 120 + src.draw(400) as u32))
                        .collect();
                    if !g.dots && pts.iter().all(|p| *p == pts[0]) {
                        pts[1].0 += 1;
                    }
                    pts
                },
                width: [1, 2, 5, 10, 15][src.draw(5)],
            }
        })
        .collect();
    Case { file, gestures: gen_gestures(&mut src, g, 8), later: gen_gestures(&mut src, g, 4) }
}

fn file_bytes(case: &Case) -> String {
    let paths: String = case
        .file
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

fn live_first_points(c: &TestCanvas) -> Vec<Pos2> {
    c.svg_ref()
        .buffer
        .elements
        .values()
        .filter_map(|el| match el {
            Element::Path(p) if !p.deleted && !p.data.is_empty() => {
                let a = p.data.manipulator_groups()[0].anchor;
                Some(pos2(a.x as f32, a.y as f32))
            }
            _ => None,
        })
        .collect()
}

/// Strokes can drift under the toolbar or off screen as the view moves;
/// a press there would click the toolbar instead.
fn on_canvas(c: &TestCanvas, at: Pos2) -> bool {
    let working = c.svg_ref().viewport_settings.working_rect.shrink(10.);
    working.contains(at)
        && !c
            .svg_ref()
            .toolbar
            .get_rects()
            .iter()
            .any(|r| r.expand(10.).contains(at))
}

/// Clicks the nth live stroke with the selection tool.
fn select(c: &mut TestCanvas, i: usize) -> bool {
    let live = live_first_points(c);
    if live.is_empty() {
        return false;
    }
    let at = live[i % live.len()];
    if !on_canvas(c, at) {
        return false;
    }
    c.tool(Tool::Selection);
    c.drag(&[at, at]);
    !c.svg_ref().toolbar.selection.selected_elements.is_empty()
}

/// CANVAS_PROPS_PEN_AFTER_SELECTING keeps the selection overlay off screen,
/// past its known crash on strokes that are gone.
fn after_selecting(c: &mut TestCanvas) {
    if std::env::var("CANVAS_PROPS_PEN_AFTER_SELECTING").is_ok() {
        c.tool(Tool::Pen);
    }
}

fn perform(c: &mut TestCanvas, gesture: &Gesture) {
    match gesture {
        Gesture::Draw(pts) => {
            c.tool(c.drawing_with);
            c.drag(pts);
        }
        Gesture::Erase(i) => {
            let live = live_first_points(c);
            if live.is_empty() {
                return;
            }
            let at = live[i % live.len()];
            if !on_canvas(c, at) {
                return;
            }
            c.tool(Tool::Eraser);
            c.drag(&[at, at + vec2(1., 1.)]);
        }
        Gesture::Move(i, delta) => {
            let live = live_first_points(c);
            if live.is_empty() {
                return;
            }
            let at = live[i % live.len()];
            if !on_canvas(c, at) || !on_canvas(c, at + *delta) {
                return;
            }
            c.tool(Tool::Selection);
            c.drag(&[at, at + *delta * 0.5, at + *delta]);
            after_selecting(c);
        }
        Gesture::Undo => c.key(Key::Z, Modifiers::COMMAND),
        Gesture::Redo => c.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT),
        Gesture::Zoom(f, at) => c.zoom(*f, *at),
        Gesture::Pan(d) => c.pan(*d),
        Gesture::Restyle(target, style) => {
            let selected = match target {
                Some(i) => select(c, *i),
                None => {
                    c.key(Key::A, Modifiers::COMMAND);
                    true
                }
            };
            if !selected {
                return;
            }
            c.act(match *style {
                Style::Color(i) => CanvasAction::SetSelectionColor(get_pen_colors()[i]),
                Style::Width(w) => CanvasAction::SetSelectionWidth(w),
                Style::Opacity(o) => CanvasAction::SetSelectionOpacity(o),
            });
            after_selecting(c);
        }
        Gesture::Layer(i, layer) => {
            if select(c, *i) {
                c.act(CanvasAction::MoveSelectionLayer(*layer));
                after_selecting(c);
            }
        }
        Gesture::PenSetting(style) => {
            c.tool(c.drawing_with);
            // only what the active pen's popover offers
            let highlighter = c.drawing_with == Tool::Highlighter;
            c.act(match *style {
                Style::Color(i) if highlighter => {
                    let colors = get_highlighter_colors();
                    CanvasAction::SetPenColor(colors[i % colors.len()])
                }
                Style::Color(i) => CanvasAction::SetPenColor(get_pen_colors()[i]),
                Style::Width(w) if highlighter => {
                    CanvasAction::SetPenWidth(DEFAULT_HIGHLIGHTER_STROKE_WIDTH + 2.5 * w)
                }
                Style::Width(w) => CanvasAction::SetPenWidth(w),
                Style::Opacity(_) if highlighter => return,
                Style::Opacity(o) => CanvasAction::SetPenOpacity(o),
            });
        }
        Gesture::DrawWith(tool) => c.drawing_with = *tool,
    }
}

fn perform_all(c: &mut TestCanvas, gestures: &[Gesture]) {
    for g in gestures {
        perform(c, g);
    }
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
    /// the element's own opacity, apart from its stroke's
    element_opacity: i64,
}

type View = Vec<Seen>;

/// Coordinates to a tenth of a unit: below a pixel at any zoom we use.
fn view(b: &Buffer, to_file: Transform) -> View {
    b.elements
        .iter()
        .filter_map(|(id, el)| match el {
            Element::Path(p) if !p.deleted && p.data.len() > 1 => {
                let mut data = p.data.clone();
                data.apply_transform(u_transform_to_bezier(&to_file));
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
                    width: (s.width * 100.).round() as i64,
                    opacity: (s.opacity * 255.).round() as i64,
                    element_opacity: (p.opacity * 255.).round() as i64,
                })
            }
            _ => None,
        })
        .collect()
}

fn canvas_view(c: &TestCanvas) -> View {
    view(&c.svg_ref().buffer, c.master().invert().unwrap())
}

fn file_view(bytes: &str) -> View {
    view(&Buffer::new(bytes), Transform::identity())
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
            || x.element_opacity != y.element_opacity
        {
            return Err(format!("{what}: {x:?} vs {y:?}"));
        }
    }
    Ok(())
}

// ════════════════════════ runner ════════════════════════

fn seeds(default: u64) -> u64 {
    std::env::var("CANVAS_PROPS_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn caught(f: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|e| {
        let m = e.downcast_ref::<String>().cloned();
        let m = m.or(e.downcast_ref::<&str>().map(|s| s.to_string()));
        Err(format!("panicked: {}", m.unwrap_or_default()))
    })
}

fn check(g: Gestures, default_seeds: u64, f: impl Fn(&Case) -> Result<(), String>) {
    prop::run(
        seeds(default_seeds),
        128,
        |b| caught(|| f(&gen_case(b, &g))),
        |b| {
            let case = gen_case(b, &g);
            let why = caught(|| f(&case)).err();
            format!("SHRUNK FAILURE: {why:?}\n{case:?}")
        },
    );
}

// ════════════════════════ property bodies ════════════════════════

/// Reopening what the canvas saved shows the same strokes and paints the same pixels.
fn save_reopen_check(c: &Case) -> Result<(), String> {
    let mut canvas = TestCanvas::open(&file_bytes(c));
    perform_all(&mut canvas, &c.gestures);
    let saved = canvas.save();
    let reopened = TestCanvas::open(&saved);
    same("reopened", &canvas_view(&canvas), &canvas_view(&reopened), true)?;
    let (a, b) = (canvas_mask(&canvas), canvas_mask(&reopened));
    differs("reopened canvas", &a, &b)
}

/// Undoing every gesture gives back the opened drawing; redoing them all
/// gives back the edited one.
fn undo_redo_check(c: &Case) -> Result<(), String> {
    let mut canvas = TestCanvas::open(&file_bytes(c));
    let opened = canvas_view(&canvas);
    let edits: Vec<_> = c
        .gestures
        .iter()
        .filter(|g| !matches!(g, Gesture::Undo | Gesture::Redo))
        .cloned()
        .collect();
    perform_all(&mut canvas, &edits);
    let edited = canvas_view(&canvas);
    for _ in 0..64 {
        if !canvas.svg_ref().history.has_undo() {
            break;
        }
        perform(&mut canvas, &Gesture::Undo);
    }
    same("after undoing everything", &opened, &canvas_view(&canvas), true)?;
    for _ in 0..64 {
        if !canvas.svg_ref().history.has_redo() {
            break;
        }
        perform(&mut canvas, &Gesture::Redo);
    }
    same("after redoing everything", &edited, &canvas_view(&canvas), true)
}

/// After a save, sync hands the same file back; gestures made meanwhile
/// survive the reload.
fn reload_own_save_check(c: &Case) -> Result<(), String> {
    let mut canvas = TestCanvas::open(&file_bytes(c));
    perform_all(&mut canvas, &c.gestures);
    let saved = canvas.save();
    perform_all(&mut canvas, &c.later);
    let before = canvas_view(&canvas);
    canvas.reload(&saved);
    same("after reload", &before, &canvas_view(&canvas), true)
}

/// Two editors open the same file; after the first saves, the second,
/// untouched, reloads it and shows exactly what was saved.
fn reload_other_check(c: &Case) -> Result<(), String> {
    let bytes = file_bytes(c);
    let mut first = TestCanvas::open(&bytes);
    let mut second = TestCanvas::open(&bytes);
    perform_all(&mut first, &c.gestures);
    let saved = first.save();
    second.reload(&saved);
    same("second editor after reload", &file_view(&saved), &canvas_view(&second), false)
}

// ════════════════════════ rendering ════════════════════════

const SIDE: u32 = super::harness::SCREEN.x as u32;
const TALL: u32 = super::harness::SCREEN.y as u32;

/// What the canvas painted last frame: its stroke meshes, filled.
fn canvas_mask(c: &TestCanvas) -> Vec<bool> {
    use resvg::tiny_skia::{FillRule, Paint, PathBuilder, Pixmap};
    let mut pixmap = Pixmap::new(SIDE, TALL).unwrap();
    let paint = Paint { anti_alias: true, ..Paint::default() };
    for clipped in &c.shapes {
        let egui::Shape::Mesh(mesh) = &clipped.shape else { continue };
        if mesh.texture_id != egui::TextureId::default() {
            continue;
        }
        let v = &mesh.vertices;
        for tri in mesh.indices.chunks(3) {
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
    if round {
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

/// The saved file looks the same in any SVG viewer as on the canvas.
/// `round` renders the file as if it said round caps and joins.
fn renders_check(c: &Case, round: bool) -> Result<(), String> {
    let mut canvas = TestCanvas::open(&file_bytes(c));
    perform_all(&mut canvas, &c.gestures);
    let file = canvas.save();
    differs("canvas vs viewer", &canvas_mask(&canvas), &viewer_mask(&file, canvas.master(), round))
}

#[test]
fn smoke_each_gesture_reaches_the_buffer() {
    let mut c = TestCanvas::open(r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#);
    perform(&mut c, &Gesture::Draw(vec![pos2(300., 300.), pos2(350., 320.), pos2(400., 300.)]));
    let drawn = canvas_view(&c);
    perform(&mut c, &Gesture::Move(0, vec2(20., 10.)));
    let moved = canvas_view(&c);
    perform(&mut c, &Gesture::Zoom(2.0, pos2(400., 300.)));
    let zoom = c.master();
    perform(&mut c, &Gesture::Erase(0));
    let erased = canvas_view(&c);
    println!("drawn {drawn:?}\nmoved {moved:?}\nzoom {zoom:?}\nerased {erased:?}");
    assert_eq!(drawn.len(), 1);
    assert_ne!(drawn, moved);
    assert_ne!(zoom, Transform::identity());
    assert!(erased.is_empty());
}

// ════════════════════════ a chain of saves ════════════════════════

const WRITERS: usize = 2;

/// Editors take turns: each reloads the latest file, makes its gestures and
/// saves, the way edits reach each other through sync.
#[derive(Debug)]
struct Chain {
    file: Vec<FileStroke>,
    turns: Vec<(usize, Vec<Gesture>)>,
}

fn gen_chain(b: &[u8], g: &Gestures) -> Chain {
    let case = gen_case(b, g);
    let mut src = ByteSource::new(&b[b.len() / 2..]);
    let turns = (0..1 + src.draw(5))
        .map(|_| (src.draw(WRITERS), gen_gestures(&mut src, g, 3)))
        .collect();
    Chain { file: case.file, turns }
}

/// An editor that never edits shows exactly each file it reloads, stacking
/// order included.
fn chain_check(chain: &Chain) -> Result<(), String> {
    let mut latest =
        file_bytes(&Case { file: chain.file.clone(), gestures: vec![], later: vec![] });
    let mut writers: Vec<TestCanvas> = (0..WRITERS).map(|_| TestCanvas::open(&latest)).collect();
    let mut watcher = TestCanvas::open(&latest);
    for (turn, (w, gestures)) in chain.turns.iter().enumerate() {
        let writer = &mut writers[*w];
        writer.reload(&latest);
        perform_all(writer, gestures);
        latest = writer.save();
        watcher.reload(&latest);
        let what = format!("untouched editor after turn {turn}");
        same(&what, &file_view(&latest), &canvas_view(&watcher), true)?;
    }
    Ok(())
}

fn check_chain(g: Gestures, default_seeds: u64) {
    prop::run(
        seeds(default_seeds),
        192,
        |b| caught(|| chain_check(&gen_chain(b, &g))),
        |b| {
            let chain = gen_chain(b, &g);
            let why = caught(|| chain_check(&chain)).err();
            format!("SHRUNK FAILURE: {why:?}\n{chain:?}")
        },
    );
}

#[test]
fn smoke_each_control_reaches_the_buffer() {
    let mut c = TestCanvas::open(r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#);
    perform(&mut c, &Gesture::Draw(vec![pos2(300., 300.), pos2(350., 320.)]));
    perform(&mut c, &Gesture::Draw(vec![pos2(400., 300.), pos2(450., 320.)]));
    let before = canvas_view(&c);
    perform(&mut c, &Gesture::Restyle(Some(0), Style::Color(1)));
    perform(&mut c, &Gesture::Restyle(Some(0), Style::Width(5.)));
    perform(&mut c, &Gesture::Restyle(Some(0), Style::Opacity(0.5)));
    perform(&mut c, &Gesture::Layer(0, LayerMove::ToBack));
    perform(&mut c, &Gesture::DrawWith(Tool::Highlighter));
    perform(&mut c, &Gesture::PenSetting(Style::Color(2)));
    perform(&mut c, &Gesture::Draw(vec![pos2(300., 400.), pos2(350., 420.)]));
    let after = canvas_view(&c);
    assert_eq!(after.len(), 3);
    let restyled = after.iter().position(|s| s.id == before[0].id).unwrap();
    assert_eq!(restyled, 2, "the layer move put the restyled stroke at the back");
    assert_ne!(after[restyled].rgb, before[0].rgb);
    assert_eq!((after[restyled].width, after[restyled].element_opacity), (500, 128));
    assert_eq!((after[0].width, after[0].opacity), (1500, 26), "highlighter stroke");
}

/// A stroke drawn under one that arrived by reload keeps its layer.
#[test]
fn chain_keeps_stacking_order() {
    let chain = Chain {
        file: vec![FileStroke {
            id: lb_rs::Uuid::from_u128(1).to_string(),
            pts: vec![(500, 400), (560, 420)],
            width: 2,
        }],
        turns: vec![
            (0, vec![Gesture::Draw(vec![pos2(200., 200.), pos2(260., 230.)])]),
            (
                1,
                vec![
                    Gesture::Draw(vec![pos2(300., 300.), pos2(360., 330.)]),
                    Gesture::Layer(0, LayerMove::Backward),
                ],
            ),
        ],
    };
    chain_check(&chain).unwrap();
}
