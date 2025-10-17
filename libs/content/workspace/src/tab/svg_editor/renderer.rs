use std::collections::HashMap;

use egui::{Mesh, TextureHandle};
use glam::f64::DVec2;
use lb_rs::Uuid;
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{Element, Image, Path, WeakPathPressures};
use lyon::path::{AttributeIndex, LineCap, LineJoin};
use lyon::tessellation::{
    self, BuffersBuilder, FillVertexConstructor, StrokeOptions, StrokeTessellator,
    StrokeVertexConstructor, VertexBuffers,
};

use rayon::prelude::*;
use resvg::usvg::{ImageKind, Transform};
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

enum RenderOp<'a> {
    Delete,
    Paint(MeshShape),
    Transform(Transform),
    ForwordImage(&'a mut Image),
}

pub struct Renderer {
    mesh_cache: HashMap<Uuid, MeshShape>,
    tex_cache: HashMap<Uuid, TextureHandle>,
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
            tex_cache: HashMap::new(),
            dark_mode: false,
            viewport_transform: Transform::identity(),
            request_rerender: true,
        }
    }

    pub fn render_svg(
        &mut self, ui: &mut egui::Ui, buffer: &mut Buffer, painter: &mut egui::Painter,
        render_options: RenderOptions, master_transform: Transform,
    ) -> RendererOutput {
        let frame = ui.ctx().frame_nr();
        let span = span!(Level::TRACE, "rendering svg", frame);
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
            .filter_map(|(id, el)| -> Option<(Uuid, RenderOp<'_>)> {
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
                            frame,
                            master_transform,
                            &buffer.weak_path_pressures,
                            self.viewport_transform,
                        )
                    }
                    Element::Image(image) => {
                        if image.deleted
                            || (!image.diff_state.opacity_changed
                                && !image.diff_state.data_changed
                                && !image.diff_state.delete_changed
                                && image.diff_state.transformed.is_none()
                                && !self.request_rerender
                                && !fit_content_transform_changed)
                        {
                            return None;
                        }

                        if let Some(transform) = image.diff_state.transformed {
                            let mut re_tess = false;
                            if !is_main_canvas_render {
                                if buffer.master_transform_changed {
                                    return None;
                                } else {
                                    re_tess = true;
                                }
                            };

                            if !re_tess && self.mesh_cache.contains_key(id) {
                                return Some((*id, RenderOp::Transform(transform)));
                            }
                        }

                        Some((*id, RenderOp::ForwordImage(image)))
                    }
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
                RenderOp::ForwordImage(img) => {
                    diff_state.data_changed = true;
                    self.alloc_image_mesh(id, img, ui, self.viewport_transform, master_transform);
                }
            }
        }
        if !self.mesh_cache.is_empty() {
            painter.extend(buffer.elements.iter_mut().rev().filter_map(|(id, _)| {
                if let Some(MeshShape { shape, .. }) = self.mesh_cache.get_mut(id) {
                    let shape_rect = shape.calc_bounds();
                    if !painter.clip_rect().contains_rect(shape_rect)
                        && !painter.clip_rect().intersects(shape_rect)
                    {
                        return None;
                    }

                    if !shape.vertices.is_empty() && !shape.indices.is_empty() {
                        Some(egui::Shape::mesh(shape.to_owned()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }));
        };

        self.request_rerender = false;

        RendererOutput {
            diff_state,
            absolute_transform: self
                .viewport_transform
                .pre_concat(master_transform.invert().unwrap_or_default()),
        }
    }

    fn alloc_image_mesh(
        &mut self, id: Uuid, img: &mut Image, ui: &mut egui::Ui, viewport_transform: Transform,
        master_transform: Transform,
    ) {
        match &img.data {
            ImageKind::JPEG(bytes) | ImageKind::PNG(bytes) => {
                let image = image::load_from_memory(bytes).unwrap();

                let egui_image = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    &image.to_rgba8(),
                );

                self.tex_cache.entry(id).or_insert_with(|| {
                    let texture = ui.ctx().load_texture(
                        format!("canvas_img_{id}"),
                        egui_image,
                        Default::default(),
                    );
                    texture
                });

                let texture = self.tex_cache.get(&id).unwrap();

                let rect = egui::Rect {
                    min: egui::pos2(img.view_box.left(), img.view_box.top()),
                    max: egui::pos2(img.view_box.right(), img.view_box.bottom()),
                };

                let t = master_transform.invert().unwrap_or_default();
                let rect = transform_rect(rect, t);

                let t = viewport_transform;
                let rect = transform_rect(rect, t);

                let uv = egui::Rect {
                    min: egui::Pos2 { x: 0.0, y: 0.0 },
                    max: egui::Pos2 { x: 1.0, y: 1.0 },
                };

                let mut mesh = egui::Mesh::with_texture(texture.id());

                mesh.add_rect_with_uv(rect, uv, egui::Color32::WHITE.linear_multiply(img.opacity));
                self.mesh_cache
                    .insert(id, MeshShape { shape: mesh, scale: img.transform.sx });
            }
            _ => {
                println!("image type is not supported")
            }
        }
    }
}

// todo: maybe impl this on element struct
fn tesselate_path<'a>(
    p: &'a mut Path, id: &'a Uuid, dark_mode: bool, frame: u64, master_transform: Transform,
    weak_path_pressures: &WeakPathPressures, viewport_transform: Transform,
) -> Option<(Uuid, RenderOp<'a>)> {
    let mut mesh: VertexBuffers<_, u32> = VertexBuffers::new();
    let mut stroke_tess = StrokeTessellator::new();

    let span = span!(Level::TRACE, "tessellating path", frame = frame);
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
