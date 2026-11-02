//! Read-only drawing previews using the canvas renderer. Meshes stay in
//! drawing coordinates; placement never changes the buffer or its cache.

use std::sync::Mutex;

#[cfg(not(target_arch = "wasm32"))]
use egui::ColorImage;
use egui::{Rect, Vec2};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::svg::buffer::Buffer;
use lb_rs::model::svg::element::{Element, Image};
use resvg::usvg::Transform;

use super::element::{BoundedElement, PromoteWeakImage};
use super::renderer::{RenderOptions, Renderer};

pub struct Drawing {
    pub bounds: Rect,
    references: Vec<Uuid>,
    render: Mutex<DrawingRender>,
}

struct DrawingRender {
    buffer: Buffer,
    renderer: Renderer,
}

impl std::fmt::Debug for Drawing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Drawing")
            .field("bounds", &self.bounds)
            .finish_non_exhaustive()
    }
}

impl Drawing {
    /// Generic SVG artwork stays with resvg: the canvas renderer implements
    /// Lockbook strokes and referenced images, rather than all SVG features.
    pub fn is_drawing(bytes: &[u8]) -> bool {
        std::str::from_utf8(bytes)
            .ok()
            .and_then(|s| roxmltree::Document::parse(s).ok())
            .is_some_and(|doc| {
                doc.descendants().any(|node| {
                    node.has_tag_name("g")
                        && matches!(
                            node.attribute("id"),
                            Some(
                                "lb_master_transform"
                                    | "lb_viewport_settings"
                                    | "lb_images"
                                    | "lb_path_pressures"
                            )
                        )
                })
            })
    }

    pub fn load(bytes: &[u8], core: &Lb) -> Result<Self, String> {
        let source = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let doc = roxmltree::Document::parse(source).map_err(|e| e.to_string())?;
        for node in doc.descendants().filter(|n| {
            matches!(
                n.attribute("id"),
                Some("lb_viewport_settings" | "lb_images" | "lb_path_pressures")
            )
        }) {
            if let Some(data) = node.first_element_child().and_then(|n| n.attribute("id")) {
                use base64::Engine as _;
                base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .map_err(|e| e.to_string())?;
            }
        }
        // Buffer::new is permissive about invalid SVG. A failed load should
        // show an error, not turn into an apparently empty drawing.
        resvg::usvg::Tree::from_str(source, &Default::default()).map_err(|e| e.to_string())?;
        let mut buffer = Buffer::new(source);
        let mut images: Vec<_> = buffer.weak_images.drain().collect();
        images.sort_by_key(|(_, weak)| weak.z_index);
        for (id, weak) in images {
            let image = Image::try_from_weak(weak, core)?;
            buffer.elements.shift_insert(
                weak.z_index.min(buffer.elements.len()),
                id,
                Element::Image(Box::new(image)),
            );
        }
        Ok(Self::new(buffer))
    }

    fn new(mut buffer: Buffer) -> Self {
        let mut bounds = Rect::NOTHING;
        let mut references = Vec::new();
        for (id, element) in &mut buffer.elements {
            let mut bb = element.bounding_box();
            match element {
                Element::Path(path) => {
                    if let Some(stroke) = path.stroke {
                        let pressure = buffer
                            .weak_path_pressures
                            .get(id)
                            .map_or(0.0, |p| p.iter().copied().fold(0.0_f32, f32::max));
                        let radius = stroke.width * (path.transform.sx + path.transform.sy).abs()
                            / 4.0
                            * (1.0 + pressure);
                        bb = bb.expand(radius);
                    }
                    path.diff_state = Default::default();
                }
                Element::Image(image) => {
                    references.push(image.href);
                    image.diff_state = Default::default();
                }
                Element::Text(_) => {}
            }
            if bb.is_finite() {
                bounds = bounds.union(bb);
            }
        }
        // Empty and degenerate drawings still need finite, nonzero dimensions.
        if !bounds.is_finite() {
            bounds = Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(1.0));
        }
        if bounds.width() < 1.0 || bounds.height() < 1.0 {
            bounds = Rect::from_center_size(bounds.center(), bounds.size().max(Vec2::splat(1.0)));
        }
        let renderer = Renderer::new(buffer.elements.len());
        Self { bounds, references, render: Mutex::new(DrawingRender { buffer, renderer }) }
    }

    pub fn references(&self, id: Uuid) -> bool {
        self.references.contains(&id)
    }

    /// Paint only: the host owns layout, events, and clipping.
    pub fn show(&self, ui: &mut egui::Ui, rect: Rect) {
        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }
        let scale = (rect.width() / self.bounds.width()).min(rect.height() / self.bounds.height());
        let transform = egui::emath::TSTransform {
            scaling: scale,
            translation: rect.center().to_vec2() - self.bounds.center().to_vec2() * scale,
        };
        let mut painter = ui.painter().with_clip_rect(rect);
        let mut render = self.render.lock().unwrap();
        let DrawingRender { buffer, renderer } = &mut *render;
        renderer.render_svg(
            ui,
            buffer,
            &mut painter,
            RenderOptions { paint_transform: Some(transform), ..Default::default() },
            Transform::identity(),
        );
    }

    /// Copy the same meshes that previews paint, including theme and pressure,
    /// on a private context so texture IDs never cross contexts.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn image(&self, dark_mode: bool) -> Result<ColorImage, String> {
        let buffer = self.render.lock().unwrap().buffer.clone();
        let drawing = Self::new(buffer);
        let scale = (2048.0 / drawing.bounds.width().max(drawing.bounds.height())).min(1.0);
        let size = (drawing.bounds.size() * scale).ceil().max(Vec2::splat(1.0));
        let rect = Rect::from_min_size(egui::Pos2::ZERO, size);
        let ctx = egui::Context::default();
        ctx.set_visuals(if dark_mode { egui::Visuals::dark() } else { egui::Visuals::light() });
        let mut output =
            ctx.run_pass(egui::RawInput { screen_rect: Some(rect), ..Default::default() }, |ctx| {
                ctx.root_ui(|ui| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(ui, |ui| drawing.show(ui, rect));
                });
            });
        let result = rasterize_meshes(&output, size);
        output.textures_delta.clear();
        result
    }
}

/// Canvas meshes have a single stroke color, or a rectangular image texture.
/// Rasterizing the mesh triangles keeps clipboard output on the same drawing
/// pipeline without requiring a GPU on the worker thread.
#[cfg(not(target_arch = "wasm32"))]
fn rasterize_meshes(output: &egui::FullOutput, size: Vec2) -> Result<ColorImage, String> {
    use resvg::tiny_skia::{self as sk, FillRule, Paint, PathBuilder, Pixmap};
    let mut target = Pixmap::new(size.x as u32, size.y as u32).ok_or("invalid drawing size")?;
    for clipped in &output.shapes {
        let egui::Shape::Mesh(mesh) = &clipped.shape else { continue };
        let Some(vertex) = mesh.vertices.first() else { continue };
        let mut builder = PathBuilder::new();
        for indices in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [
                mesh.vertices[indices[0] as usize].pos,
                mesh.vertices[indices[1] as usize].pos,
                mesh.vertices[indices[2] as usize].pos,
            ];
            // Consistent winding merges the triangles into one coverage mask.
            let ab = b - a;
            let ac = c - a;
            let (b, c) = if ab.x * ac.y - ab.y * ac.x < 0.0 { (c, b) } else { (b, c) };
            builder.move_to(a.x, a.y);
            builder.line_to(b.x, b.y);
            builder.line_to(c.x, c.y);
            builder.close();
        }
        let Some(path) = builder.finish() else { continue };
        let mut paint = Paint { anti_alias: true, ..Default::default() };
        if mesh.texture_id == egui::TextureId::default() {
            let [r, g, b, a] = vertex.color.to_srgba_unmultiplied();
            paint.set_color_rgba8(r, g, b, a);
            target.fill_path(&path, &paint, FillRule::Winding, sk::Transform::identity(), None);
        } else {
            let image = output
                .textures_delta
                .set
                .iter()
                .find(|(id, _)| **id == mesh.texture_id)
                .and_then(|(_, deltas)| deltas.first())
                .ok_or("drawing texture missing")?;
            let egui::ImageData::Color(image) = &image.image;
            let mut texture = Pixmap::new(image.width() as u32, image.height() as u32)
                .ok_or("invalid image size")?;
            for (dst, src) in texture.data_mut().chunks_exact_mut(4).zip(&image.pixels) {
                dst.copy_from_slice(&src.to_array());
            }
            let bounds = mesh.calc_bounds();
            paint.shader = sk::Pattern::new(
                texture.as_ref(),
                sk::SpreadMode::Pad,
                sk::FilterQuality::Bilinear,
                vertex.color.a() as f32 / 255.0,
                sk::Transform::from_scale(
                    bounds.width() / image.width() as f32,
                    bounds.height() / image.height() as f32,
                )
                .post_translate(bounds.left(), bounds.top()),
            );
            target.fill_path(&path, &paint, FillRule::Winding, sk::Transform::identity(), None);
        }
    }
    let pixels: Vec<_> = target
        .pixels()
        .iter()
        .flat_map(|p| {
            let p = p.demultiply();
            [p.red(), p.green(), p.blue(), p.alpha()]
        })
        .collect();
    Ok(ColorImage::from_rgba_unmultiplied(
        [target.width() as usize, target.height() as usize],
        &pixels,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, Pos2};

    fn strokes() -> Buffer {
        Buffer::new(
            r##"<svg xmlns="http://www.w3.org/2000/svg">
            <path d="M-100 -20 L0 -20" stroke="#000000" stroke-width="8" fill="none"/>
            <path d="M-100 20 L0 20" stroke="#123456" stroke-width="8" fill="none"/>
        </svg>"##,
        )
    }

    fn paint(
        ctx: &egui::Context, drawing: &Drawing, rects: &[Rect],
    ) -> Vec<egui::epaint::ClippedShape> {
        let mut output = ctx.run_pass(Default::default(), |ctx| {
            ctx.root_ui(|ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let cursor = ui.cursor();
                    for rect in rects {
                        drawing.show(ui, *rect);
                    }
                    assert_eq!(
                        ui.cursor(),
                        cursor,
                        "painting must not advance the editor's layout"
                    );
                })
            });
        });
        let shapes = std::mem::take(&mut output.shapes);
        output.textures_delta.clear();
        shapes
    }

    #[test]
    fn previews_switch_theme_and_keep_placements_independent() {
        let drawing = Drawing::new(strokes());
        let ctx = egui::Context::default();
        let rects = [
            Rect::from_min_size(Pos2::new(20.0, 20.0), drawing.bounds.size()),
            Rect::from_min_size(Pos2::new(200.0, 200.0), drawing.bounds.size() * 2.0),
        ];
        let mut original = None;
        for dark in [false, true, false] {
            ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
            let shapes = paint(&ctx, &drawing, &rects);
            let meshes: Vec<_> = shapes
                .iter()
                .filter_map(|s| {
                    if let egui::Shape::Mesh(m) = &s.shape { Some((s.clip_rect, m)) } else { None }
                })
                .collect();
            assert_eq!(meshes.len(), 4);
            for (i, (clip, mesh)) in meshes.iter().enumerate() {
                let rect = rects[i / 2];
                assert_eq!(*clip, rect);
                assert!(rect.expand(0.01).contains_rect(mesh.calc_bounds()));
                let color = mesh.vertices[0].color;
                assert!(
                    color == Color32::from_rgb(0x12, 0x34, 0x56)
                        || color == if dark { Color32::WHITE } else { Color32::BLACK }
                );
            }
            assert!(
                meshes.iter().any(|(_, m)| m.vertices[0].color
                    == if dark { Color32::WHITE } else { Color32::BLACK })
            );
            let positions: Vec<_> = meshes
                .iter()
                .flat_map(|(_, m)| m.vertices.iter().map(|v| v.pos))
                .collect();
            if let Some(original) = &original {
                assert_eq!(original, &positions);
            } else {
                original = Some(positions);
            }
        }
    }

    #[test]
    fn clipboard_uses_theme_opacity_and_pressure() {
        let mut buffer = strokes();
        let id = *buffer.elements.first().unwrap().0;
        buffer.weak_path_pressures.insert(id, vec![1.0, 1.0]);
        let Element::Path(path) = buffer.elements.get_mut(&id).unwrap() else { panic!() };
        path.opacity = 0.5;
        let drawing = Drawing::new(buffer);
        assert_eq!(drawing.bounds.height(), 52.0); // [-28, 24], including pressure
        let light = drawing.image(false).unwrap();
        let dark = drawing.image(true).unwrap();
        assert_eq!(light.size, dark.size);
        let sample = |img: &ColorImage, y: usize| img.pixels[y * img.width() + img.width() / 2];
        let ink = sample(&light, 8);
        assert!(ink.a() > 120 && ink.a() < 135);
        assert_eq!(ink.to_srgba_unmultiplied()[..3], [0, 0, 0]);
        assert_eq!(sample(&dark, 8).to_srgba_unmultiplied()[..3], [255, 255, 255]);
        assert_eq!(sample(&dark, 48), sample(&light, 48));
        assert!(sample(&light, 1).a() > 0, "pressure-expanded stroke must survive copying");
    }

    #[test]
    fn empty_drawing_and_empty_pressure_samples_are_valid() {
        let empty = Drawing::new(Buffer::default());
        assert_eq!(empty.image(true).unwrap().size, [1, 1]);
        let mut buffer = strokes();
        let id = *buffer.elements.first().unwrap().0;
        buffer.weak_path_pressures.insert(id, vec![]);
        assert!(Drawing::new(buffer).image(false).is_ok());
    }

    #[test]
    fn clipboard_preserves_image_pixels_and_order() {
        let mut buffer = strokes();
        let image = ColorImage::filled([2, 2], Color32::RED);
        let png = crate::widgets::image_cache::encode_png(&image).unwrap();
        buffer.elements.insert(
            Uuid::new_v4(),
            Element::Image(Box::new(Image {
                data: resvg::usvg::ImageKind::PNG(png.into()),
                visible: true,
                transform: Transform::identity(),
                view_box: resvg::usvg::NonZeroRect::from_xywh(-80.0, -30.0, 60.0, 60.0).unwrap(),
                opacity: 0.5,
                href: Uuid::new_v4(),
                diff_state: Default::default(),
                deleted: false,
            })),
        );
        let image = Drawing::new(buffer).image(true).unwrap();
        let pixel = image.pixels[30 * image.width() + image.width() / 2];
        assert_eq!(pixel.to_srgba_unmultiplied()[..3], [255, 0, 0]);
        assert!(pixel.a() > 120 && pixel.a() < 135);
        // The first stroke is above the image (canvas paints in reverse order).
        assert!(image.pixels[10 * image.width() + image.width() / 2].a() > pixel.a());
    }

    #[test]
    fn saved_viewport_does_not_change_preview_geometry() {
        use lb_rs::model::svg::buffer::u_transform_to_bezier;
        let mut buffer = strokes();
        let before = Drawing::new(buffer.clone()).image(false).unwrap();
        let master = Transform::from_scale(2.0, 2.0).post_translate(400.0, 300.0);
        buffer.weak_viewport_settings.master_transform = master.into();
        for element in buffer.elements.values_mut() {
            if let Element::Path(path) = element {
                path.data.apply_transform(u_transform_to_bezier(&master));
            }
        }
        let serialized = buffer.serialize();
        assert!(Drawing::is_drawing(serialized.as_bytes()));
        let core = crate::tab::markdown_editor::tests::harness::build_lb();
        let after = Drawing::load(serialized.as_bytes(), &core)
            .unwrap()
            .image(false)
            .unwrap();
        assert_eq!(before, after);
        assert!(!Drawing::is_drawing(
            b"<svg xmlns='http://www.w3.org/2000/svg'><rect width='20' height='20'/></svg>"
        ));
        assert!(
            Drawing::load(b"<svg><g id='lb_images'><g id='!invalid'/></g></svg>", &core).is_err()
        );
    }
}
