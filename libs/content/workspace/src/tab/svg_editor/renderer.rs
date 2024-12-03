use std::collections::HashMap;

use egui::{Mesh, TextureHandle};
use glam::f64::DVec2;
use lb_rs::svg::diff::DiffState;
use lb_rs::svg::element::{Element, Image, Path};
use lb_rs::Uuid;
use lyon::math::Point;
use lyon::path::{AttributeIndex, LineCap, LineJoin};
use lyon::tessellation::{
    self, BuffersBuilder, FillVertexConstructor, StrokeOptions, StrokeTessellator,
    StrokeVertexConstructor, VertexBuffers,
};

use rayon::prelude::*;
use resvg::usvg::{ImageKind, Transform};
use tracing::{span, Level};

use crate::theme::palette::ThemePalette;

use super::Buffer;

const STROKE_WIDTH: AttributeIndex = 0;

struct VertexConstructor {
    color: epaint::Color32,
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
}

struct MeshShape {
    shape: Mesh,
    scale: f32,
}

impl Renderer {
    pub fn new(elements_count: usize) -> Self {
        Self {
            mesh_cache: HashMap::with_capacity(elements_count),
            tex_cache: HashMap::new(),
            dark_mode: false,
        }
    }

    pub fn render_svg(
        &mut self, ui: &mut egui::Ui, buffer: &mut Buffer, painter: &mut egui::Painter,
    ) -> DiffState {
        let frame = ui.ctx().frame_nr();
        let span = span!(Level::TRACE, "rendering svg", frame);
        let _ = span.enter();
        let mut diff_state = DiffState::default();

        let dark_mode_changed = ui.visuals().dark_mode != self.dark_mode;
        self.dark_mode = ui.visuals().dark_mode;

        let paint_ops: Vec<(Uuid, RenderOp)> = buffer
            .elements
            .par_iter_mut()
            .filter_map(|(id, el)| -> Option<(Uuid, RenderOp<'_>)> {
                if el.deleted() && el.delete_changed() {
                    println!("issuning delete on el: {}", id);
                    return Some((*id, RenderOp::Delete));
                };

                match el {
                    Element::Path(path) => {
                        let stale_mesh = if let Some(MeshShape { scale, .. }) =
                            self.mesh_cache.get(id)
                        {
                            let current_el_scale = path.transform.sx * buffer.master_transform.sx;
                            let diff = current_el_scale.max(*scale) / current_el_scale.min(*scale);
                            diff > 5.0
                        } else {
                            false
                        };

                        if path.deleted
                            || (!path.diff_state.opacity_changed
                                && !path.diff_state.data_changed
                                && !path.diff_state.delete_changed
                                && path.diff_state.transformed.is_none()
                                && !dark_mode_changed
                                && !stale_mesh)
                        {
                            return None;
                        }

                        if let Some(transform) = path.diff_state.transformed {
                            if self.mesh_cache.contains_key(id) {
                                return Some((*id, RenderOp::Transform(transform)));
                            }
                        }
                        tesselate_path(
                            path,
                            id,
                            ui.visuals().dark_mode,
                            frame,
                            buffer.master_transform,
                        )
                    }
                    Element::Image(image) => {
                        // let image_clone = image.clone();

                        if image.deleted
                            || (!image.diff_state.opacity_changed
                                && !image.diff_state.data_changed
                                && !image.diff_state.delete_changed
                                && image.diff_state.transformed.is_none())
                        {
                            return None;
                        }

                        if let Some(transform) = image.diff_state.transformed {
                            if self.mesh_cache.contains_key(id) {
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
                    diff_state.delete_changed = true;
                    self.mesh_cache.remove(&id);
                }
                RenderOp::Paint(m) => {
                    diff_state.data_changed = true;
                    self.mesh_cache.insert(id.to_owned(), m);
                }
                RenderOp::Transform(t) => {
                    diff_state.transformed = Some(t);
                    if let Some(MeshShape { shape, .. }) = self.mesh_cache.get_mut(&id) {
                        for v in &mut shape.vertices {
                            v.pos.x = t.sx * v.pos.x + t.tx;
                            v.pos.y = t.sy * v.pos.y + t.ty;
                        }
                    }
                }
                RenderOp::ForwordImage(img) => {
                    diff_state.data_changed = true;
                    self.alloc_image_mesh(id, img, ui);
                }
            }
        }

        if !self.mesh_cache.is_empty() {
            painter.extend(buffer.elements.iter_mut().rev().filter_map(|(id, el)| {
                match el {
                    Element::Path(p) => p.diff_state = DiffState::default(),
                    Element::Image(i) => i.diff_state = DiffState::default(),
                    Element::Text(_) => todo!(),
                }
                if let Some(MeshShape { shape, .. }) = self.mesh_cache.get_mut(id) {
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

        diff_state
    }

    fn alloc_image_mesh(&mut self, id: Uuid, img: &mut Image, ui: &mut egui::Ui) {
        // if self.mesh_cache.contains_key(&id) {
        //     return;
        // }
        match &img.data {
            ImageKind::JPEG(bytes) | ImageKind::PNG(bytes) => {
                let image = image::load_from_memory(&bytes).unwrap();

                let egui_image = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    &image.to_rgba8(),
                );
                println!("created egui image");

                if !self.tex_cache.contains_key(&id) {
                    let texture = ui.ctx().load_texture(
                        format!("canvas_img_{}", id),
                        egui_image,
                        Default::default(),
                    );
                    self.tex_cache.insert(id, texture);
                }

                let texture = self.tex_cache.get(&id).unwrap();

                println!("created texture");

                let rect = egui::Rect {
                    min: egui::pos2(img.view_box.left(), img.view_box.top()),
                    max: egui::pos2(img.view_box.right(), img.view_box.bottom()),
                };
                let uv = egui::Rect {
                    min: egui::Pos2 { x: 0.0, y: 0.0 },
                    max: egui::Pos2 { x: 1.0, y: 1.0 },
                };

                let mut mesh = egui::Mesh::with_texture(texture.id());
                println!("created mesh");

                mesh.add_rect_with_uv(rect, uv, egui::Color32::WHITE.gamma_multiply(img.opacity));
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
            .gamma_multiply(stroke.opacity)
            .gamma_multiply(p.opacity);

        let mut builder = lyon::path::BuilderWithAttributes::new(1);

        let mut first = None;
        let mut i = 0;

        while let Some(seg) = p.data.get_segment(i) {
            let thickness = stroke.width * p.transform.sx * master_transform.sx;

            let start = devc_to_point(seg.start());
            let end = devc_to_point(seg.end());
            if first.is_none() {
                first = Some(start);
                builder.begin(start, &[thickness]);
                builder.line_to(start, &[thickness]);
            } else if seg.handle_end().is_some() && seg.handle_start().is_some() {
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
                .with_tolerance(0.1)
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

fn devc_to_point(dvec: DVec2) -> Point {
    Point::new(dvec.x as f32, dvec.y as f32)
}
