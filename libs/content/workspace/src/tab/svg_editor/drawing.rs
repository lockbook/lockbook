//! Rasterize Lockbook drawings with the canvas renderer.

use egui::{ColorImage, Context, Rect, Vec2};
use egui_wgpu_renderer::{OffscreenFrame, OffscreenGpu, wgpu};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::svg::buffer::{Buffer, is_lockbook_group};
use lb_rs::model::svg::element::Element;
use resvg::usvg::Transform;

use super::element::PromoteBufferWeakImages;
use super::renderer::{RenderOptions, Renderer};
use super::viewport::calc_elements_bounds;

pub struct Drawing {
    pub bounds: Rect,
    buffer: Buffer,
}

impl Drawing {
    /// Generic SVG artwork stays with resvg: the canvas renderer implements
    /// Lockbook strokes and referenced images, rather than all SVG features.
    /// TODO: Make Lockbook drawings fully SVG compliant so standard SVG renderers
    /// can display them without special handling.
    pub fn is_lockbook_drawing(bytes: &[u8]) -> bool {
        std::str::from_utf8(bytes)
            .ok()
            .and_then(|s| roxmltree::Document::parse(s).ok())
            .is_some_and(|doc| {
                doc.descendants().any(|node| {
                    node.has_tag_name("g") && node.attribute("id").is_some_and(is_lockbook_group)
                })
            })
    }

    pub fn load(bytes: &[u8], core: &Lb) -> Result<Self, String> {
        let source = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let mut buffer = Buffer::parse(source)?;
        buffer.promote_weak_images(Transform::identity(), core)?;
        Ok(Self::new(buffer))
    }

    fn new(buffer: Buffer) -> Self {
        // Empty and degenerate drawings still need finite, nonzero dimensions.
        let mut bounds = calc_elements_bounds(&buffer, 1.0)
            .unwrap_or_else(|| Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(1.0)));
        if bounds.width() < 1.0 || bounds.height() < 1.0 {
            bounds = Rect::from_center_size(bounds.center(), bounds.size().max(Vec2::splat(1.0)));
        }
        Self { bounds, buffer }
    }

    pub fn references(&self) -> Vec<Uuid> {
        self.buffer
            .elements
            .values()
            .filter_map(|element| match element {
                Element::Image(image) => Some(image.href),
                _ => None,
            })
            .collect()
    }

    /// Clipboard pixels use the same GPU pipeline as cached previews.
    pub async fn image(&self, ctx: &Context, dark_mode: bool) -> Result<ColorImage, String> {
        self.rasterize(ctx, dark_mode, self.bounds.size().max_elem().min(2048.0))
            .await
    }

    /// Render through egui-wgpu on the live device. Pixels are temporary: the
    /// cache uploads them and drops the CPU copy; the clipboard owns its copy.
    pub async fn rasterize(
        &self, live: &Context, dark_mode: bool, max_side: f32,
    ) -> Result<ColorImage, String> {
        let gpu = OffscreenGpu::of(live).ok_or("drawing GPU renderer unavailable")?;
        let mut buffer = self.buffer.clone();
        let mut renderer = Renderer::new(buffer.elements.len());
        let scale = max_side / self.bounds.size().max_elem();
        let size = (self.bounds.size() * scale).ceil().max(Vec2::splat(1.0));
        let rect = Rect::from_min_size(egui::Pos2::ZERO, size);
        let ctx = egui::Context::default();
        ctx.set_visuals(if dark_mode { egui::Visuals::dark() } else { egui::Visuals::light() });
        let mut output =
            ctx.run_pass(egui::RawInput { screen_rect: Some(rect), ..Default::default() }, |ctx| {
                ctx.root_ui(|ui| {
                    let mut painter = ui.painter().clone();
                    painter.set_clip_rect(self.bounds);
                    renderer.render_svg(
                        ui,
                        &mut buffer,
                        &mut painter,
                        RenderOptions::default(),
                        Transform::identity(),
                    );
                });
            });
        let transform = egui::emath::TSTransform {
            scaling: scale,
            translation: rect.center().to_vec2() - self.bounds.center().to_vec2() * scale,
        };
        for clipped in &mut output.shapes {
            if let egui::Shape::Mesh(mesh) = &mut clipped.shape {
                std::sync::Arc::make_mut(mesh).transform(transform);
                clipped.clip_rect = rect;
            }
        }
        let size = [size.x as u32, size.y as u32];
        let frame = OffscreenFrame {
            paint_jobs: ctx.tessellate(output.shapes, 1.0),
            textures: output.textures_delta,
            size_in_pixels: size,
            pixels_per_point: 1.0,
            clear: wgpu::Color::TRANSPARENT,
        };
        let pixels = gpu
            .render(frame)
            .await
            .ok_or("the GPU returned no drawing pixels")?;
        Ok(ColorImage::from_rgba_premultiplied(size.map(|n| n as usize), &pixels))
    }
}
