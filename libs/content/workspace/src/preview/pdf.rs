//! A printed note cut into pages and written as a PDF: its shapes as
//! paths, its textures as images, and its text as text in the fonts it was
//! shaped in, so it selects, copies, and searches.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use egui::epaint::{ClippedShape, ColorMode, CornerRadius, PathStroke, StrokeKind};
use egui::{Color32, Pos2, Rect, Shape, TextureId, Vec2};
use glyphon::FontSystem;
use glyphon::fontdb;
use krilla::color::rgb;
use krilla::geom::{Path, PathBuilder, Point, Size, Transform};
use krilla::image::Image;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule, Stroke};
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph, Tag};
use lb_rs::Uuid;

use super::print::{print_frame, textured};
use crate::tab::TabResources;
use crate::widgets::glyphon_render::{PrintText, TextBufferArea};

/// Room around a page's content, in PDF points: half an inch.
const MARGIN: f32 = 36.0;
/// PDF points per point of the note: what the app shows at 16 points
/// prints at 12.
const SCALE: f32 = 0.75;

/// A sheet of paper, in PDF points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper(pub Vec2);

impl Paper {
    pub const LETTER: Self = Self(Vec2::new(612.0, 792.0));
    pub const A4: Self = Self(Vec2::new(595.28, 841.89));

    /// Letter where the system's region uses it, A4 elsewhere.
    pub fn of_locale() -> Self {
        let locale = sys_locale::get_locale().unwrap_or_default();
        let region = locale.rsplit(['-', '_']).next().unwrap_or_default();
        const LETTER_REGIONS: [&str; 14] =
            ["US", "CA", "MX", "PH", "CL", "CO", "VE", "CR", "GT", "PA", "SV", "NI", "DO", "PR"];
        if LETTER_REGIONS.contains(&region) { Self::LETTER } else { Self::A4 }
    }

    /// The note's width in its own points, laid out to fill the page between margins.
    fn note_width(self) -> f32 {
        (self.0.x - 2.0 * MARGIN) / SCALE
    }

    /// How much of the note, in its own points, fits on one page.
    fn note_height(self) -> f32 {
        (self.0.y - 2.0 * MARGIN) / SCALE
    }
}

/// Prints the note `id` on `paper` and writes it as a PDF. `text` is an open
/// note's text, unsaved edits and all, instead of what's saved.
pub fn render_pdf(
    live: &TabResources, id: Uuid, text: Option<String>, paper: Paper,
) -> Result<Vec<u8>, String> {
    let size = Vec2::new(paper.note_width(), paper.note_height());
    let frame = print_frame(live, id, text, size)?;
    let cuts = page_cuts(&frame.unbreakable, paper.note_height(), frame.height);
    let mut writer = PdfWriter {
        shapes: frame.shapes,
        textures: frame.textures,
        images: HashMap::new(),
        font_system: crate::register_font_system(&live.ctx),
        faces: HashMap::new(),
        fonts: HashMap::new(),
    };
    writer.write(&cuts, paper)
}

/// Where pages end, in the note's points, from 0 to `bottom`: each page
/// holds at most `page` of the note and ends where no span shorter than a
/// page and starting on it is cut.
pub fn page_cuts(unbreakable: &[(f32, f32)], page: f32, bottom: f32) -> Vec<f32> {
    let unbreakable: Vec<_> = unbreakable
        .iter()
        .copied()
        .filter(|(top, end)| end - top <= page)
        .collect();
    let mut cuts = vec![0.0];
    let mut top = 0.0;
    while top < bottom {
        let limit = top + page;
        if limit >= bottom {
            cuts.push(bottom);
            break;
        }
        let mut cut = limit;
        while let Some(&(t, _)) = unbreakable
            .iter()
            .find(|&&(t, e)| top < t && t < cut && e > cut)
        {
            cut = t;
        }
        cuts.push(cut);
        top = cut;
    }
    cuts
}

/// A face's bytes, its index in them, and its name.
type Face = Option<(Arc<Vec<u8>>, u32, String)>;

struct PdfWriter {
    shapes: Vec<ClippedShape>,
    textures: HashMap<TextureId, Arc<egui::ColorImage>>,
    images: HashMap<TextureId, Image>,
    /// The live context's, locked only to look up a face or a character.
    font_system: Arc<Mutex<FontSystem>>,
    faces: HashMap<fontdb::ID, Face>,
    fonts: HashMap<(fontdb::ID, u16), Option<Font>>,
}

impl PdfWriter {
    fn write(&mut self, cuts: &[f32], paper: Paper) -> Result<Vec<u8>, String> {
        let mut doc = krilla::Document::new();
        let shapes = std::mem::take(&mut self.shapes);
        for page_span in cuts.windows(2) {
            let (top, bottom) = (page_span[0], page_span[1]);
            let shown = Rect::from_x_y_ranges(f32::MIN..=f32::MAX, top..=bottom);
            let settings = PageSettings::from_wh(paper.0.x, paper.0.y).ok_or("paper size")?;
            let mut page = doc.start_page_with(settings);
            let mut surface = page.surface();
            surface.push_transform(&Transform::from_row(
                SCALE,
                0.0,
                0.0,
                SCALE,
                MARGIN,
                MARGIN - top * SCALE,
            ));
            let note = Rect::from_x_y_ranges(-1e5..=1e5, top..=bottom);
            surface.push_clip_path(&rect_path(note), &FillRule::NonZero);
            for clipped in &shapes {
                let bounds = clipped.shape.visual_bounding_rect();
                if !clipped.clip_rect.intersects(shown) || !bounds.intersects(shown) {
                    continue;
                }
                surface.push_clip_path(&rect_path(clipped.clip_rect), &FillRule::NonZero);
                self.draw(&mut surface, &clipped.shape, shown);
                surface.pop();
            }
            surface.pop();
            surface.pop();
            surface.finish();
            page.finish();
        }
        doc.finish().map_err(|e| format!("{e:?}"))
    }

    fn draw(&mut self, surface: &mut Surface, shape: &Shape, shown: Rect) {
        match shape {
            // a note draws all its text through glyphon
            Shape::Noop | Shape::Band(_) | Shape::Text(_) => {}
            Shape::Vec(shapes) => {
                for shape in shapes {
                    self.draw(surface, shape, shown);
                }
            }
            Shape::Callback(callback) => {
                if let Some(PrintText(areas)) = callback.callback.downcast_ref() {
                    for area in areas {
                        if area.clip_rect.intersects(shown) {
                            self.draw_text(surface, area, shown);
                        }
                    }
                }
            }
            Shape::Circle(c) => paint(
                surface,
                ellipse(c.center, Vec2::splat(c.radius)),
                fill(c.fill),
                stroke(c.stroke.width, c.stroke.color),
            ),
            Shape::Ellipse(e) => paint(
                surface,
                ellipse(e.center, e.radius),
                fill(e.fill),
                stroke(e.stroke.width, e.stroke.color),
            ),
            Shape::LineSegment { points, stroke: s } => {
                paint(surface, polyline(points, false), None, stroke(s.width, s.color))
            }
            Shape::Path(p) => {
                let fill = if p.closed { fill(p.fill) } else { None };
                paint(surface, polyline(&p.points, p.closed), fill, path_stroke(&p.stroke))
            }
            Shape::QuadraticBezier(q) => {
                let [a, b, c] = q.points;
                let mut pb = PathBuilder::new();
                pb.move_to(a.x, a.y);
                pb.quad_to(b.x, b.y, c.x, c.y);
                if q.closed {
                    pb.close();
                }
                let fill = if q.closed { fill(q.fill) } else { None };
                paint(surface, pb.finish(), fill, path_stroke(&q.stroke));
            }
            Shape::CubicBezier(q) => {
                let [a, b, c, d] = q.points;
                let mut pb = PathBuilder::new();
                pb.move_to(a.x, a.y);
                pb.cubic_to(b.x, b.y, c.x, c.y, d.x, d.y);
                if q.closed {
                    pb.close();
                }
                let fill = if q.closed { fill(q.fill) } else { None };
                paint(surface, pb.finish(), fill, path_stroke(&q.stroke));
            }
            Shape::Rect(r) if r.brush.is_none() => {
                let outline = match r.stroke_kind {
                    StrokeKind::Inside => r.rect.shrink(r.stroke.width / 2.0),
                    StrokeKind::Middle => r.rect,
                    StrokeKind::Outside => r.rect.expand(r.stroke.width / 2.0),
                };
                paint(surface, rounded(r.rect, r.corner_radius), fill(r.fill), None);
                let stroke = stroke(r.stroke.width, r.stroke.color);
                paint(surface, rounded(outline, r.corner_radius), None, stroke);
            }
            Shape::Mesh(mesh) if mesh.texture_id == TextureId::default() => {
                draw_mesh(surface, mesh)
            }
            shape => {
                if let Some((texture, rect)) = textured(shape) {
                    if let Some(image) = self.image(texture) {
                        surface.push_transform(&Transform::from_translate(rect.left(), rect.top()));
                        if let Some(size) = Size::from_wh(rect.width(), rect.height()) {
                            surface.draw_image(image, size);
                        }
                        surface.pop();
                    }
                }
            }
        }
    }

    fn image(&mut self, texture: TextureId) -> Option<Image> {
        if let Some(image) = self.images.get(&texture) {
            return Some(image.clone());
        }
        let pixels = self.textures.get(&texture)?;
        let rgba = pixels
            .pixels
            .iter()
            .flat_map(|p| p.to_srgba_unmultiplied())
            .collect();
        let [w, h] = pixels.size.map(|n| n as u32);
        let image = Image::from_rgba8(rgba, w, h);
        self.images.insert(texture, image.clone());
        Some(image)
    }

    /// Each run of `area` as text: consecutive glyphs in one font, size,
    /// and color go out together with the source text they show.
    fn draw_text(&mut self, surface: &mut Surface, area: &TextBufferArea, shown: Rect) {
        surface.push_clip_path(&rect_path(area.clip_rect), &FillRule::NonZero);
        let buffer = area.buffer.read().unwrap();
        for run in buffer.layout_runs() {
            let top = area.rect.top() + run.line_top;
            if top + run.line_height < shown.top() || top > shown.bottom() {
                continue;
            }
            let color_of =
                |g: &glyphon::cosmic_text::LayoutGlyph| g.color_opt.unwrap_or(area.default_color);
            for group in run.glyphs.chunk_by(|a, b| {
                a.font_id == b.font_id
                    && a.font_size == b.font_size
                    && a.font_weight == b.font_weight
                    && color_of(a) == color_of(b)
            }) {
                let first = &group[0];
                let Some(font) = self.font(first.font_id, first.font_weight) else { continue };
                let start = group.iter().map(|g| g.start).min().unwrap_or(0);
                let end = group.iter().map(|g| g.end).max().unwrap_or(0);
                let size = first.font_size;
                let glyphs: Vec<_> = group
                    .iter()
                    .enumerate()
                    .map(|(i, g)| {
                        let next = group.get(i + 1).map_or(g.x + g.w, |n| n.x);
                        KrillaGlyph::new(
                            GlyphId::new(g.glyph_id as u32),
                            (next - g.x) / size,
                            g.x_offset,
                            g.y_offset,
                            0.0,
                            g.start - start..g.end - start,
                            None,
                        )
                    })
                    .collect();
                let c = color_of(first);
                surface.set_stroke(None);
                surface.set_fill(fill(Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), c.a())));
                let origin =
                    Point::from_xy(area.rect.left() + first.x, area.rect.top() + run.line_y);
                surface.draw_glyphs(origin, &glyphs, font, &run.text[start..end], size, false);
            }
        }
        surface.pop();
    }

    /// The face `id` at `weight`, its variable axes set as shaping set them.
    fn font(&mut self, id: fontdb::ID, weight: fontdb::Weight) -> Option<Font> {
        if let Some(font) = self.fonts.get(&(id, weight.0)) {
            return font.clone();
        }
        let font_system = &self.font_system;
        let face = self
            .faces
            .entry(id)
            .or_insert_with(|| {
                // only a lookup and a clone under the shared lock: the bytes load outside it
                let face = font_system.lock().unwrap().db().face(id).cloned();
                let Some(face) = face else {
                    tracing::warn!("pdf text in font {id} left out: it isn't in the font database");
                    return None;
                };
                let data = match &face.source {
                    fontdb::Source::Binary(data) => Ok(data.as_ref().as_ref().to_vec()),
                    fontdb::Source::File(path) => std::fs::read(path).map_err(|e| e.to_string()),
                    fontdb::Source::SharedFile(_, data) => Ok(data.as_ref().as_ref().to_vec()),
                };
                match data {
                    Ok(data) => Some((Arc::new(data), face.index, face.post_script_name)),
                    Err(why) => {
                        tracing::warn!("pdf text in {} left out: {why}", face.post_script_name);
                        None
                    }
                }
            })
            .clone();
        let font = face.and_then(|(data, index, name)| {
            let font =
                Font::new_variable(data.into(), index, &[(Tag::new(b"wght"), weight.0 as f32)]);
            if font.is_none() {
                tracing::warn!("pdf text in {name} left out: krilla can't read the font");
            }
            font
        });
        self.fonts.insert((id, weight.0), font.clone());
        font
    }
}

/// An untextured mesh: a fade, which is a quad whose two edges have their
/// own colors, as a gradient; anything else filled in its first color.
fn draw_mesh(surface: &mut Surface, mesh: &egui::Mesh) {
    let rect = mesh.calc_bounds();
    let v = &mesh.vertices;
    if let [a, b, c, d] = v.as_slice() {
        let (from, to, end) = if a.color == b.color && c.color == d.color {
            (a.color, c.color, rect.left_bottom())
        } else if a.color == c.color && b.color == d.color {
            (a.color, b.color, rect.right_top())
        } else {
            (a.color, a.color, rect.right_top())
        };
        let stop = |offset: f32, c: Color32| {
            let [r, g, b, _] = c.to_srgba_unmultiplied();
            krilla::paint::Stop {
                offset: NormalizedF32::new(offset).unwrap_or(NormalizedF32::ZERO),
                color: rgb::Color::new(r, g, b).into(),
                opacity: opacity(c),
            }
        };
        let gradient = krilla::paint::LinearGradient {
            x1: rect.left(),
            y1: rect.top(),
            x2: end.x,
            y2: end.y,
            transform: Transform::default(),
            spread_method: Default::default(),
            stops: vec![stop(0.0, from), stop(1.0, to)],
            anti_alias: false,
        };
        let fill =
            Fill { paint: gradient.into(), opacity: NormalizedF32::ONE, rule: FillRule::NonZero };
        paint(surface, Some(rect_path(rect)), Some(fill), None);
        return;
    }
    let mut pb = PathBuilder::new();
    for t in mesh.indices.as_chunks::<3>().0 {
        let [p, q, r] = t.map(|i| v[i as usize].pos);
        pb.move_to(p.x, p.y);
        pb.line_to(q.x, q.y);
        pb.line_to(r.x, r.y);
        pb.close();
    }
    paint(surface, pb.finish(), v.first().and_then(|v| fill(v.color)), None);
}

fn paint(surface: &mut Surface, path: Option<Path>, fill: Option<Fill>, stroke: Option<Stroke>) {
    let Some(path) = path else { return };
    if fill.is_none() && stroke.is_none() {
        return;
    }
    surface.set_fill(fill);
    surface.set_stroke(stroke);
    surface.draw_path(&path);
}

fn opacity(c: Color32) -> NormalizedF32 {
    NormalizedF32::new(c.a() as f32 / 255.0).unwrap_or(NormalizedF32::ONE)
}

fn fill(c: Color32) -> Option<Fill> {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    (c.a() > 0).then(|| Fill {
        paint: rgb::Color::new(r, g, b).into(),
        opacity: opacity(c),
        rule: FillRule::NonZero,
    })
}

fn stroke(width: f32, c: Color32) -> Option<Stroke> {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    (width > 0.0 && c.a() > 0).then(|| Stroke {
        paint: rgb::Color::new(r, g, b).into(),
        width,
        opacity: opacity(c),
        ..Default::default()
    })
}

fn path_stroke(s: &PathStroke) -> Option<Stroke> {
    match s.color {
        ColorMode::Solid(c) => stroke(s.width, c),
        ColorMode::UV(_) => None,
    }
}

/// `r` as a path, an empty rect as a sliver at its corner so a clip to it
/// shows nothing.
fn rect_path(r: Rect) -> Path {
    let r = r.intersect(Rect::from_min_max(Pos2::new(-1e6, -1e6), Pos2::new(1e6, 1e6)));
    let (w, h) = (r.width().max(1e-3), r.height().max(1e-3));
    let mut pb = PathBuilder::new();
    pb.push_rect(krilla::geom::Rect::from_xywh(r.left(), r.top(), w, h).expect("finite"));
    pb.finish().expect("a rect")
}

/// `r` with its corners rounded by `radius`, each a quarter ellipse drawn as
/// one cubic.
fn rounded(r: Rect, radius: CornerRadius) -> Option<Path> {
    let most = r.width().min(r.height()) / 2.0;
    let [nw, ne, sw, se] =
        [radius.nw, radius.ne, radius.sw, radius.se].map(|v| (v as f32).min(most));
    // the control point's distance from a corner, for a quarter circle
    let k = 1.0 - 0.552_284_8;
    let (left, right, top, bottom) = (r.left(), r.right(), r.top(), r.bottom());
    let mut pb = PathBuilder::new();
    pb.move_to(left + nw, top);
    pb.line_to(right - ne, top);
    pb.cubic_to(right - ne * k, top, right, top + ne * k, right, top + ne);
    pb.line_to(right, bottom - se);
    pb.cubic_to(right, bottom - se * k, right - se * k, bottom, right - se, bottom);
    pb.line_to(left + sw, bottom);
    pb.cubic_to(left + sw * k, bottom, left, bottom - sw * k, left, bottom - sw);
    pb.line_to(left, top + nw);
    pb.cubic_to(left, top + nw * k, left + nw * k, top, left + nw, top);
    pb.close();
    pb.finish()
}

fn ellipse(center: Pos2, radius: Vec2) -> Option<Path> {
    let r = Rect::from_center_size(center, radius * 2.0);
    let k = 0.552_284_8;
    let (rx, ry) = (radius.x, radius.y);
    let mut pb = PathBuilder::new();
    pb.move_to(r.right(), center.y);
    pb.cubic_to(r.right(), center.y + ry * k, center.x + rx * k, r.bottom(), center.x, r.bottom());
    pb.cubic_to(center.x - rx * k, r.bottom(), r.left(), center.y + ry * k, r.left(), center.y);
    pb.cubic_to(r.left(), center.y - ry * k, center.x - rx * k, r.top(), center.x, r.top());
    pb.cubic_to(center.x + rx * k, r.top(), r.right(), center.y - ry * k, r.right(), center.y);
    pb.close();
    pb.finish()
}

fn polyline(points: &[Pos2], closed: bool) -> Option<Path> {
    let (first, rest) = points.split_first()?;
    let mut pb = PathBuilder::new();
    pb.move_to(first.x, first.y);
    for p in rest {
        pb.line_to(p.x, p.y);
    }
    if closed {
        pb.close();
    }
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::print::for_each_shape;
    use crate::test_utils::ws::TestWs;

    /// Pages hold at most a page each, run from the top to the bottom, and
    /// cut no span shorter than a page, for spans that nest or stand apart
    /// as the editor's do: lines, inside rows, inside tables.
    #[test]
    fn page_cuts_keep_spans_whole() {
        let mut seed = 0x9e37_79b9_u32;
        let mut next = move |n: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % n) as f32
        };
        for _ in 0..500 {
            let page = 50.0 + next(200);
            let mut spans = vec![];
            let mut y = 0.0;
            for _ in 0..next(30) as usize {
                y += next(20);
                let top = y;
                for _ in 0..1 + next(8) as usize {
                    let line = (y, y + 1.0 + next(40));
                    y = line.1 + next(4);
                    spans.push(line);
                }
                spans.push((top, y));
            }
            let bottom = y + next(20);
            let cuts = page_cuts(&spans, page, bottom);
            assert_eq!(cuts[0], 0.0);
            assert_eq!(*cuts.last().unwrap(), bottom);
            for pair in cuts.windows(2) {
                assert!(pair[0] < pair[1], "{cuts:?}");
                assert!(pair[1] - pair[0] <= page, "{cuts:?}");
            }
            for &(top, end) in spans.iter().filter(|(t, e)| e - t <= page) {
                assert!(!cuts.iter().any(|&c| top < c && c < end), "{top}..{end} cut by {cuts:?}");
            }
        }
    }

    /// Everything a note shows prints: a printed note's frame holds no egui
    /// text and no GPU callback, which the PDF leaves out.
    #[test]
    fn a_printed_note_paints_nothing_the_pdf_drops() {
        let mut t = TestWs::new();
        let mut png = vec![];
        image::RgbaImage::from_pixel(40, 30, image::Rgba([200, 80, 40, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let picture = t.doc("picture.png", "");
        t.ws.core.write_document(picture, &png).unwrap();
        t.doc("Plan.md", "# Plan\n\n## Goals\n\nShip the release.\n");
        let broken = t.doc("broken.png", "");
        t.ws.core.write_document(broken, b"not a png").unwrap();
        let broken = t.doc("broken.pdf", "");
        t.ws.core.write_document(broken, b"not a pdf").unwrap();
        let wide = (0..12).map(|i| format!("column {i}")).collect::<Vec<_>>();
        let alerts = ["NOTE", "TIP", "IMPORTANT", "WARNING", "CAUTION"]
            .map(|kind| format!("> [!{kind}]\n> An alert.\n\n"))
            .concat();
        let note = format!(
            "# Title 🧾\n\n{alerts}- [ ] open\n- [x] done\n\nA [link](https://lockbook.net), a footnote[^1], `code` and $x^2$.\n\n## Folded {fold}\n\nHidden until printed.\n\n| {} |\n|{}|\n| {} |\n\n```\n{}\n```\n\n![picture](picture.png)\n\n![[Plan#Goals]]\n\n![[Plan#Gone]]\n\n![[broken.png]]\n\n![[broken.pdf]]\n\n![broken](broken.png)\n\n[^1]: The footnote.\n",
            wide.join(" | "),
            ["---"; 12].join("|"),
            wide.join(" | "),
            "code ".repeat(80),
            fold = crate::tab::markdown_editor::fold::FOLD_TAG,
        );
        let id = t.doc("note.md", &note);
        let live = t.ws.resources();

        let size = Vec2::new(Paper::LETTER.note_width(), Paper::LETTER.note_height());
        let frame = print_frame(&live, id, None, size).unwrap();
        let mut dropped = vec![];
        for clipped in &frame.shapes {
            for_each_shape(&clipped.shape, &mut |shape| match shape {
                Shape::Text(text) => dropped.push(text.galley.text().to_string()),
                Shape::Callback(callback) if !callback.callback.is::<PrintText>() => {
                    dropped.push("a GPU callback".into())
                }
                _ => {}
            });
        }
        assert_eq!(dropped, Vec::<String>::new());

        let pdf = render_pdf(&live, id, None, Paper::LETTER).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }
}
