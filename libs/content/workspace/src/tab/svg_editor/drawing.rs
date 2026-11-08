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

#[cfg(test)]
pub(crate) fn install_test_gpu(ctx: &Context) {
    static GPU: std::sync::OnceLock<OffscreenGpu> = std::sync::OnceLock::new();
    GPU.get_or_init(|| {
        futures::executor::block_on(async {
            let instance = wgpu::Instance::new(
                wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            );
            let adapter = instance
                .request_adapter(&Default::default())
                .await
                .expect("drawing tests require a GPU");
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    required_limits: adapter.limits(),
                    ..Default::default()
                })
                .await
                .unwrap();
            OffscreenGpu::new(&device, &queue, 4)
        })
    })
    .install(ctx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Color32;
    use lb_rs::model::svg::element::Image;

    fn image(drawing: &Drawing, dark: bool) -> Result<ColorImage, String> {
        let ctx = Context::default();
        install_test_gpu(&ctx);
        futures::executor::block_on(drawing.image(&ctx, dark))
    }

    fn strokes() -> Buffer {
        Buffer::new(
            r##"<svg xmlns="http://www.w3.org/2000/svg">
            <path id="00000000-0000-0000-0000-000000000001" d="M-100 -20 L0 -20" stroke="#000000" stroke-width="8" fill="none"/>
            <path id="00000000-0000-0000-0000-000000000002" d="M-100 20 L0 20" stroke="#123456" stroke-width="8" fill="none"/>
        </svg>"##,
        )
    }

    #[test]
    fn rasterization_scales_and_uses_current_theme() {
        let ctx = Context::default();
        install_test_gpu(&ctx);
        let drawing = Drawing::new(strokes());
        for dark in [false, true, false] {
            for scale in [1.0, 2.0] {
                let image = futures::executor::block_on(drawing.rasterize(
                    &ctx,
                    dark,
                    drawing.bounds.size().max_elem() * scale,
                ))
                .unwrap();
                assert_eq!(image.size, [108 * scale as usize, 48 * scale as usize]);
                let ink = if dark { Color32::WHITE } else { Color32::BLACK };
                assert!(image.pixels.contains(&ink));
                assert!(image.pixels.contains(&Color32::from_rgb(0x12, 0x34, 0x56)));
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
        let light = image(&drawing, false).unwrap();
        let dark = image(&drawing, true).unwrap();
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
        assert_eq!(image(&empty, true).unwrap().size, [1, 1]);
        let mut buffer = strokes();
        let id = *buffer.elements.first().unwrap().0;
        buffer.weak_path_pressures.insert(id, vec![]);
        assert!(image(&Drawing::new(buffer), false).is_ok());
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
        let image = self::image(&Drawing::new(buffer), true).unwrap();
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
        let id = *buffer.elements.first().unwrap().0;
        buffer.weak_path_pressures.insert(id, vec![0.5, 0.75]);
        let before = image(&Drawing::new(buffer.clone()), false).unwrap();
        let master = Transform::from_scale(2.0, 2.0).post_translate(400.0, 300.0);
        buffer.weak_viewport_settings.master_transform = master.into();
        for element in buffer.elements.values_mut() {
            if let Element::Path(path) = element {
                path.data.apply_transform(u_transform_to_bezier(&master));
            }
        }
        let serialized = buffer.serialize();
        assert!(Drawing::is_lockbook_drawing(serialized.as_bytes()));
        let core = crate::tab::markdown_editor::tests::harness::build_lb();
        let loaded = Drawing::load(serialized.as_bytes(), &core).unwrap();
        assert_eq!(loaded.buffer.weak_path_pressures, buffer.weak_path_pressures);
        let after = image(&loaded, false).unwrap();
        assert_eq!(before, after);
        assert!(!Drawing::is_lockbook_drawing(
            b"<svg xmlns='http://www.w3.org/2000/svg'><rect width='20' height='20'/></svg>"
        ));
        assert!(
            Drawing::load(b"<svg xmlns='http://www.w3.org/2000/svg'><g id='lb_images'><g id='!invalid'/></g></svg>", &core).is_err()
        );
    }
}
