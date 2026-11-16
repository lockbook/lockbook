use std::collections::HashMap;

use egui::{Mesh, TextureHandle};
use lb_rs::Uuid;
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{Element, Image, Path, WeakPathPressures};
use lyon::path::{AttributeIndex, LineCap, LineJoin};
use lyon::tessellation::{
    self, BuffersBuilder, FillVertexConstructor, StrokeOptions, StrokeTessellator,
    StrokeVertexConstructor, VertexBuffers,
};
use rayon::prelude::*;
use resvg::usvg::ImageKind;

use super::util::devc_to_point;
use super::view::CanvasView;
use crate::theme::palette::ThemePalette;

const STROKE_WIDTH: AttributeIndex = 0;

pub struct VertexConstructor {
    pub(crate) color: epaint::Color32,
}

impl FillVertexConstructor<epaint::Vertex> for VertexConstructor {
    fn new_vertex(&mut self, vertex: tessellation::FillVertex) -> epaint::Vertex {
        epaint::Vertex {
            pos: egui::pos2(vertex.position().x, vertex.position().y),
            uv: epaint::WHITE_UV,
            color: self.color,
        }
    }
}

impl StrokeVertexConstructor<epaint::Vertex> for VertexConstructor {
    fn new_vertex(&mut self, vertex: tessellation::StrokeVertex) -> epaint::Vertex {
        epaint::Vertex {
            pos: egui::pos2(vertex.position().x, vertex.position().y),
            uv: epaint::WHITE_UV,
            color: self.color,
        }
    }
}

/// Meshes are cached in document space. Each consumer projects a copy into
/// its own viewport, without changing the buffer or generating document diffs.
#[derive(Default)]
pub struct Renderer {
    mesh_cache: HashMap<Uuid, MeshShape>,
    tex_cache: HashMap<Uuid, TextureHandle>,
    dark_mode: bool,
    pub request_rerender: bool,
}

struct MeshShape {
    shape: Mesh,
    zoom: f32,
}

impl Renderer {
    pub fn new(elements_count: usize) -> Self {
        Self {
            mesh_cache: HashMap::with_capacity(elements_count),
            request_rerender: true,
            ..Default::default()
        }
    }

    pub fn render_svg(
        &mut self, ui: &mut egui::Ui, view: CanvasView<'_>, painter: &mut egui::Painter,
    ) -> DiffState {
        let buffer = view.document();
        let coordinates = view.coordinates();
        let zoom = coordinates.zoom().max(0.001);
        let dark_mode = ui.visuals().dark_mode;
        let dark_mode_changed = dark_mode != self.dark_mode;
        self.dark_mode = dark_mode;

        // Document dirtiness comes exclusively from edits, never cache work.
        let mut diff_state = DiffState::default();
        for el in buffer.elements.values() {
            diff_state.data_changed |= el.data_changed();
            diff_state.delete_changed |= el.delete_changed();
            diff_state.opacity_changed |= el.opacity_changed();
            if let Some(transform) = el.transformed() {
                diff_state.transformed = Some(transform);
            }
        }

        self.mesh_cache
            .retain(|id, _| buffer.elements.get(id).is_some_and(|el| !el.deleted()));
        self.tex_cache
            .retain(|id, _| buffer.elements.get(id).is_some_and(|el| !el.deleted()));

        let paths: Vec<_> = buffer
            .elements
            .par_iter()
            .filter_map(|(id, el)| {
                let Element::Path(path) = el else { return None };
                if path.deleted {
                    return None;
                }
                let stale = self
                    .mesh_cache
                    .get(id)
                    .is_none_or(|mesh| zoom.max(mesh.zoom) / zoom.min(mesh.zoom) > 5.0);
                if !path.diff_state.is_dirty()
                    && !dark_mode_changed
                    && !stale
                    && !self.request_rerender
                {
                    return None;
                }
                Some((*id, tessellate_path(path, id, dark_mode, &buffer.weak_path_pressures, zoom)))
            })
            .collect();
        for (id, mesh) in paths {
            if let Some(shape) = mesh {
                self.mesh_cache.insert(id, MeshShape { shape, zoom });
            } else {
                self.mesh_cache.remove(&id);
            }
        }

        for (id, el) in &buffer.elements {
            if let Element::Image(image) = el {
                if !image.deleted
                    && (image.diff_state.is_dirty()
                        || self.request_rerender
                        || !self.mesh_cache.contains_key(id))
                {
                    self.alloc_image_mesh(*id, image, ui);
                }
            }
        }

        painter.extend(buffer.elements.iter().rev().filter_map(|(id, _)| {
            let shape = coordinates.screen_mesh(&self.mesh_cache.get(id)?.shape);
            if !painter.clip_rect().intersects(shape.calc_bounds())
                || shape.vertices.is_empty()
                || shape.indices.is_empty()
            {
                return None;
            }
            Some(egui::Shape::mesh(shape))
        }));
        self.request_rerender = false;

        diff_state
    }

    fn alloc_image_mesh(&mut self, id: Uuid, img: &Image, ui: &egui::Ui) {
        let (ImageKind::JPEG(bytes) | ImageKind::PNG(bytes)) = &img.data else { return };
        if !self.tex_cache.contains_key(&id) {
            let Ok(image) = image::load_from_memory(bytes) else { return };
            let pixels = egui::ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                &image.to_rgba8(),
            );
            self.tex_cache.insert(
                id,
                ui.ctx()
                    .load_texture(format!("canvas_img_{id}"), pixels, Default::default()),
            );
        }
        let mut mesh = Mesh::with_texture(self.tex_cache[&id].id());
        mesh.add_rect_with_uv(
            egui::Rect::from_min_size(
                egui::pos2(img.view_box.x(), img.view_box.y()),
                egui::vec2(img.view_box.width(), img.view_box.height()),
            ),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE.linear_multiply(img.opacity),
        );
        self.mesh_cache
            .insert(id, MeshShape { shape: mesh, zoom: 1.0 });
    }
}

fn tessellate_path(
    path: &Path, id: &Uuid, dark_mode: bool, pressures: &WeakPathPressures, zoom: f32,
) -> Option<Mesh> {
    let stroke = path.stroke?;
    if path.data.is_empty() {
        return None;
    }
    let color = ThemePalette::resolve_dynamic_color(stroke.color, dark_mode)
        .linear_multiply(stroke.opacity)
        .linear_multiply(path.opacity);
    let mut builder = lyon::path::BuilderWithAttributes::new(1);
    let mut started = false;
    for i in 0..path.data.len_segments() {
        let segment = path.data.get_segment(i)?;
        let mut thickness = stroke.width * path.stroke_scale();
        if let Some(forces) = pressures.get(id) {
            let force = forces
                .get(i)
                .or_else(|| i.checked_sub(1).and_then(|i| forces.get(i)))
                .copied()
                .unwrap_or(0.0);
            thickness *= 1.0 + force;
        }
        if !started {
            builder.begin(devc_to_point(segment.start()), &[thickness]);
            started = true;
        }
        match (segment.handle_start(), segment.handle_end()) {
            (Some(start), Some(end)) => builder.cubic_bezier_to(
                devc_to_point(start),
                devc_to_point(end),
                devc_to_point(segment.end()),
                &[thickness],
            ),
            _ => builder.line_to(devc_to_point(segment.end()), &[thickness]),
        };
    }
    if !started {
        return None;
    }
    builder.end(false);
    let mut mesh = VertexBuffers::new();
    StrokeTessellator::new()
        .tessellate_path(
            &builder.build(),
            &StrokeOptions::default()
                .with_line_cap(LineCap::Round)
                .with_line_join(LineJoin::Round)
                .with_tolerance((0.1 / zoom).max(0.00001))
                .with_variable_line_width(STROKE_WIDTH),
            &mut BuffersBuilder::new(&mut mesh, VertexConstructor { color }),
        )
        .ok()?;
    Some(Mesh { indices: mesh.indices, vertices: mesh.vertices, texture_id: Default::default() })
}
