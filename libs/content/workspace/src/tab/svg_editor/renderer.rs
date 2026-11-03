use std::collections::HashMap;

use egui::Mesh;
use glam::f64::DVec2;
use lb_rs::Uuid;
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{Element, Path, WeakPathPressures};
use lyon::path::{AttributeIndex, LineCap, LineJoin};
use lyon::tessellation::{
    self, BuffersBuilder, FillVertexConstructor, StrokeOptions, StrokeTessellator,
    StrokeVertexConstructor, VertexBuffers,
};

use crate::widgets::document_image::DocumentImage;
use rayon::prelude::*;
use resvg::usvg::Transform;
use tracing::{Level, span};

use crate::theme::palette::ThemePalette;

use super::Buffer;
use super::util::{devc_to_point, transform_point, transform_rect};

const STROKE_WIDTH: AttributeIndex = 0;

pub struct VertexConstructor {
    pub(crate) color: epaint::Color32,
}
impl FillVertexConstructor<epaint::Vertex> for VertexConstructor {
    fn new_vertex(&mut self, vertex: tessellation::FillVertex) -> epaint::Vertex {
        let pos = egui::pos2(vertex.position().x, vertex.position().y);
        epaint::Vertex { pos, uv: epaint::WHITE_UV, color: self.color }
    }
}

impl StrokeVertexConstructor<epaint::Vertex> for VertexConstructor {
    fn new_vertex(&mut self, vertex: tessellation::StrokeVertex) -> epaint::Vertex {
        let pos = egui::pos2(vertex.position().x, vertex.position().y);
        epaint::Vertex { pos, uv: epaint::WHITE_UV, color: self.color }
    }
}

enum RenderOp {
    Delete,
    Paint(MeshShape),
    Transform(Transform),
}

#[derive(Default)]
pub struct Renderer {
    mesh_cache: HashMap<Uuid, MeshShape>,
    dark_mode: bool,
    viewport_transform: Transform,
    pub request_rerender: bool,
}

pub struct RendererOutput {
    pub diff_state: DiffState,
    pub absolute_transform: Transform,
}
struct MeshShape {
    shape: Mesh,
    scale: f32,
}
#[derive(Clone, Copy, Default)]
pub struct RenderOptions {
    pub viewport_transform: Option<Transform>,
}

impl Renderer {
    pub fn new(elements_count: usize) -> Self {
        Self {
            mesh_cache: HashMap::with_capacity(elements_count),
            dark_mode: false,
            viewport_transform: Transform::identity(),
            request_rerender: true,
        }
    }

    pub fn render_svg(
        &mut self, ui: &mut egui::Ui, buffer: &mut Buffer, painter: &mut egui::Painter,
        render_options: RenderOptions, master_transform: Transform, images: &DocumentImage,
    ) -> RendererOutput {
        let span = span!(Level::TRACE, "rendering svg");
        let _ = span.enter();
        let mut diff_state = DiffState::default();

        let dark_mode_changed = ui.visuals().dark_mode != self.dark_mode;
        self.dark_mode = ui.visuals().dark_mode;

        let new_viewport_transform =
            if let Some(viewport_transform) = render_options.viewport_transform {
                viewport_transform
            } else {
                master_transform
            };

        let mut fit_content_transform_changed = false;

        // todo: don't re-tess on viewport user pan or zoom.
        if new_viewport_transform != self.viewport_transform {
            self.viewport_transform = new_viewport_transform;
            fit_content_transform_changed = true;
        }

        let is_main_canvas_render = self.viewport_transform == master_transform;

        let paint_ops: Vec<(Uuid, RenderOp)> = buffer
            .elements
            .par_iter_mut()
            .filter_map(|(id, el)| -> Option<(Uuid, RenderOp)> {
                if el.deleted() {
                    return Some((*id, RenderOp::Delete));
                };

                match el {
                    Element::Path(path) => {
                        let stale_mesh = if let Some(MeshShape { scale, .. }) =
                            self.mesh_cache.get(id)
                        {
                            let current_el_scale = path.transform.sx * self.viewport_transform.sx;
                            let diff = current_el_scale.max(*scale) / current_el_scale.min(*scale);
                            diff > 5.0 && is_main_canvas_render
                        } else {
                            false
                        };

                        if path.deleted
                            || (!path.diff_state.opacity_changed
                                && !path.diff_state.data_changed
                                && !path.diff_state.delete_changed
                                && path.diff_state.transformed.is_none()
                                && !dark_mode_changed
                                && !stale_mesh
                                && !self.request_rerender
                                && !fit_content_transform_changed)
                        {
                            return None;
                        }

                        if let Some(transform) = path.diff_state.transformed {
                            let mut re_tess = false;
                            // if we're rendering using the master transform then this is  part of the main
                            // canvas render loop, not mini map or an external user like an editor preview.
                            // each transform is recorded on the master transform plane, and applying it
                            // to another plane would be incorrect. so just re-tesselate instead.
                            // todo: figure out how to convert the transform between planes to
                            // avoid re-tesselation.
                            if !is_main_canvas_render {
                                if buffer.master_transform_changed {
                                    return None;
                                } else {
                                    re_tess = true;
                                }
                            };

                            if !re_tess
                                && transform.sx == transform.sy
                                && self.mesh_cache.contains_key(id)
                            {
                                return Some((*id, RenderOp::Transform(transform)));
                            }
                        }
                        tesselate_path(
                            path,
                            id,
                            ui.visuals().dark_mode,
                            master_transform,
                            &buffer.weak_path_pressures,
                            self.viewport_transform,
                        )
                    }
                    Element::Image(_) => None,
                    Element::Text(_) => todo!(),
                }
            })
            .collect();

        for (id, paint_op) in paint_ops {
            match paint_op {
                RenderOp::Delete => {
                    if self.mesh_cache.remove(&id).is_some() {
                        diff_state.delete_changed = true;
                    }
                }
                RenderOp::Paint(m) => {
                    diff_state.data_changed = true;
                    self.mesh_cache.insert(id.to_owned(), m);
                }
                RenderOp::Transform(t) => {
                    diff_state.transformed = Some(t);
                    if let Some(MeshShape { shape, .. }) = self.mesh_cache.get_mut(&id) {
                        for v in &mut shape.vertices {
                            v.pos = transform_point(v.pos, t);
                        }
                    }
                }
            }
        }
        // Texture lifetime is owned by ImageCache. Look images up each frame,
        // including when their geometry has not changed and while loading/failed.
        for (id, element) in buffer.elements.iter().rev() {
            if let Element::Image(image) = element {
                diff_state.data_changed |= image.diff_state.is_dirty();
                if image.deleted {
                    continue;
                }
                if !image.visible {
                    continue;
                }
                let rect = egui::Rect::from_min_max(
                    egui::pos2(image.view_box.left(), image.view_box.top()),
                    egui::pos2(image.view_box.right(), image.view_box.bottom()),
                );
                let rect = transform_rect(
                    transform_rect(rect, master_transform.invert().unwrap_or_default()),
                    self.viewport_transform,
                );
                if painter.clip_rect().intersects(rect) {
                    let url = format!("lb://{}", image.href);
                    images.paint(ui, painter, &url, rect, egui::CornerRadius::ZERO, image.opacity);
                    if is_main_canvas_render
                        && images.withheld(&url)
                        && ui
                            .interact(
                                rect,
                                egui::Id::unique(("canvas_reveal_image", id)),
                                egui::Sense::click(),
                            )
                            .clicked()
                    {
                        images.load_anyway(&url);
                    }
                }
            } else if let Some(MeshShape { shape, .. }) = self.mesh_cache.get(id) {
                if !element.deleted()
                    && painter.clip_rect().intersects(shape.calc_bounds())
                    && !shape.is_empty()
                {
                    painter.add(egui::Shape::mesh(shape.clone()));
                }
            }
        }

        self.request_rerender = false;

        RendererOutput {
            diff_state,
            absolute_transform: self
                .viewport_transform
                .pre_concat(master_transform.invert().unwrap_or_default()),
        }
    }
}

// todo: maybe impl this on element struct
fn tesselate_path<'a>(
    p: &'a mut Path, id: &'a Uuid, dark_mode: bool, master_transform: Transform,
    weak_path_pressures: &WeakPathPressures, viewport_transform: Transform,
) -> Option<(Uuid, RenderOp)> {
    let mut mesh: VertexBuffers<_, u32> = VertexBuffers::new();
    let mut stroke_tess = StrokeTessellator::new();

    let span = span!(Level::TRACE, "tessellating path");
    let _ = span.enter();
    if let Some(stroke) = p.stroke {
        if p.data.is_empty() {
            return Some((*id, RenderOp::Delete));
        }
        let stroke_color = ThemePalette::resolve_dynamic_color(stroke.color, dark_mode)
            .linear_multiply(stroke.opacity)
            .linear_multiply(p.opacity);

        let mut builder = lyon::path::BuilderWithAttributes::new(1);

        let mut first = None;
        let mut i = 0;

        while let Some(mut seg) = p.data.get_segment(i) {
            let mut thickness =
                stroke.width * (p.transform.sx + p.transform.sy) / 2.0 * viewport_transform.sx;

            if let Some(forces) = weak_path_pressures.get(id) {
                let pressure_at_segment =
                    if let Some(p) = forces.get(i) { p } else { forces.get(i - 1).unwrap_or(&0.0) };

                thickness += thickness * pressure_at_segment;
            }

            if master_transform != viewport_transform {
                thickness = thickness.max(0.3);
            }

            let t = master_transform.invert().unwrap_or_default();
            seg = seg.apply_transformation(|p| DVec2 {
                x: t.sx as f64 * p.x + t.tx as f64,
                y: t.sy as f64 * p.y + t.ty as f64,
            });

            let t = viewport_transform;
            seg = seg.apply_transformation(|p| DVec2 {
                x: t.sx as f64 * p.x + t.tx as f64,
                y: t.sy as f64 * p.y + t.ty as f64,
            });

            let start = devc_to_point(seg.start());
            let end = devc_to_point(seg.end());
            if first.is_none() {
                first = Some(start);
                builder.begin(start, &[thickness]);
            }

            if seg.handle_end().is_some() && seg.handle_start().is_some() {
                let handle_start = devc_to_point(seg.handle_start().unwrap());
                let handle_end = devc_to_point(seg.handle_end().unwrap());

                builder.cubic_bezier_to(handle_start, handle_end, end, &[thickness]);
            } else if seg.handle_end().is_none() && seg.handle_start().is_none() {
                builder.line_to(end, &[thickness]);
            }
            i += 1;
        }
        if first.is_some() {
            builder.end(false);
        }
        let path = builder.build();

        let _ = stroke_tess.tessellate_path(
            &path,
            &StrokeOptions::default()
                .with_line_cap(LineCap::Round)
                .with_line_join(LineJoin::Round)
                .with_variable_line_width(STROKE_WIDTH),
            &mut BuffersBuilder::new(&mut mesh, VertexConstructor { color: stroke_color }),
        );

        let mesh = egui::epaint::Mesh {
            indices: mesh.indices.clone(),
            vertices: mesh.vertices.clone(),
            texture_id: Default::default(),
        };
        if mesh.is_empty() {
            None
        } else {
            Some((
                *id,
                RenderOp::Paint(MeshShape {
                    shape: mesh,
                    scale: master_transform.sx * p.transform.sx,
                }),
            ))
        }
    } else {
        None
    }
}
