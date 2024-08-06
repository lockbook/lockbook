use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use glam::f64::DVec2;
use lyon::math::Point;
use lyon::path::{AttributeIndex, LineCap, LineJoin};
use lyon::tessellation::{
    self, BuffersBuilder, FillVertexConstructor, StrokeOptions, StrokeTessellator,
    StrokeVertexConstructor, VertexBuffers,
};

use rayon::prelude::*;

use super::parser::{self};
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

enum RenderOp {
    Delete,
    Paint(egui::Shape),
    Transform,
}

pub struct Renderer {
    mesh_tx: Sender<(String, RenderOp)>,
    mesh_rx: Receiver<(String, RenderOp)>,
    mesh_cache: HashMap<String, egui::Shape>,
    master_mesh: egui::Mesh,
}

impl Renderer {
    pub fn new(elements_count: usize) -> Self {
        let (mesh_tx, mesh_rx) = mpsc::channel();

        Self {
            mesh_rx,
            mesh_tx,
            mesh_cache: HashMap::default(),
            master_mesh: egui::Mesh::default(),
        }
    }

    pub fn render_svg_parralel(
        &mut self, ui: &mut egui::Ui, buffer: &mut Buffer, inner_rect: egui::Rect,
    ) {
        let painter = ui
            .allocate_painter(inner_rect.size(), egui::Sense::click_and_drag())
            .1;

        let elements = buffer.elements.clone();

        let paint_ops: Vec<(String, RenderOp)> = elements
            .par_iter()
            .filter_map(|(id, el)| {
                if !el.changed() {
                    return None;
                }
                if el.deleted() {
                    return Some((id.clone(), RenderOp::Delete));
                };
                let mut mesh: VertexBuffers<_, u32> = VertexBuffers::new();
                let mut stroke_tess = StrokeTessellator::new();

                match el {
                    parser::Element::Path(p) => {
                        // todo see if i can remove this so i can draw points

                        println!("TESS el: {}", id);

                        if let Some(stroke) = p.stroke {
                            let stroke_color = if ui.visuals().dark_mode {
                                stroke.color.1
                            } else {
                                stroke.color.0
                            }
                            .linear_multiply(p.opacity);

                            let mut builder = lyon::path::BuilderWithAttributes::new(1);

                            let mut first = None;
                            let mut i = 0;
                            while let Some(seg) = p.data.get_segment(i) {
                                let pressure = if let Some(ref pressure) = p.pressure {
                                    let pressure_at_segment = pressure.get(i);
                                    if pressure_at_segment.is_some() {
                                        pressure_at_segment
                                    } else {
                                        pressure.get(i - 1)
                                    }
                                } else {
                                    None
                                };

                                let normalized_pressure =
                                    if let Some(prsr) = pressure { *prsr * 2.0 + 0.1 } else { 1.0 };

                                let thickness =
                                    stroke.width * buffer.master_transform.sx * normalized_pressure;

                                let start = devc_to_point(seg.start());
                                let end = devc_to_point(seg.end());
                                if first.is_none() {
                                    first = Some(start);
                                    builder.begin(start, &[thickness]);
                                } else if seg.handle_end().is_some() && seg.handle_start().is_some()
                                {
                                    let handle_start = devc_to_point(seg.handle_start().unwrap());
                                    let handle_end = devc_to_point(seg.handle_end().unwrap());

                                    builder.cubic_bezier_to(
                                        handle_start,
                                        handle_end,
                                        end,
                                        &[thickness],
                                    );
                                } else if seg.handle_end().is_none() && seg.handle_start().is_none()
                                {
                                    builder.line_to(end, &[thickness]);
                                }
                                i = i + 1;
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
                                    .with_tolerance(1.0)
                                    .with_variable_line_width(STROKE_WIDTH),
                                &mut BuffersBuilder::new(
                                    &mut mesh,
                                    VertexConstructor { color: stroke_color },
                                ),
                            );

                            let mesh = egui::Shape::Mesh(egui::epaint::Mesh {
                                indices: mesh.indices.clone(),
                                vertices: mesh.vertices.clone(),
                                texture_id: Default::default(),
                            });

                            Some((id.to_owned(), RenderOp::Paint(mesh)))
                        } else {
                            None
                        }
                    }
                    parser::Element::Image(_) => todo!(),
                    parser::Element::Text(_) => todo!(),
                }
            })
            .collect();

        for (id, paint_op) in paint_ops {
            match paint_op {
                RenderOp::Delete => {
                    self.mesh_cache.remove(&id);
                }
                RenderOp::Paint(m) => {
                    self.mesh_cache.insert(id.to_owned(), m);
                    if let Some(el) = buffer.elements.get_mut(&id) {
                        match el {
                            parser::Element::Path(p) => p.changed = false,
                            parser::Element::Image(i) => i.changed = false,
                            parser::Element::Text(_) => todo!(),
                        }
                    }
                }
                RenderOp::Transform => todo!(),
            }
        }
        if !self.mesh_cache.is_empty() {
            painter.extend(self.mesh_cache.clone().into_values());
        }
    }

    pub fn render_svg(&mut self, ui: &mut egui::Ui, buffer: &mut Buffer, inner_rect: egui::Rect) {
        let painter = ui
            .allocate_painter(inner_rect.size(), egui::Sense::click_and_drag())
            .1;

        let mut mesh: VertexBuffers<_, u32> = VertexBuffers::new();
        let mut stroke_tess = StrokeTessellator::new();
        let elements = buffer.elements.clone();
        let mesh_tx = self.mesh_tx.clone();
        let dark_mode = ui.visuals().dark_mode;
        let master_transform = buffer.master_transform;

        thread::spawn(move || {
            for (id, el) in elements {
                if el.deleted() {
                    let _ = mesh_tx.send((id.clone(), RenderOp::Delete));
                    continue;
                }
                if !el.changed() {
                    continue;
                }
                match el {
                    parser::Element::Path(p) => {
                        // todo see if i can remove this so i can draw points

                        println!("TESS el: {}", id);

                        if let Some(stroke) = p.stroke {
                            let stroke_color =
                                if dark_mode { stroke.color.1 } else { stroke.color.0 }
                                    .linear_multiply(p.opacity);

                            let mut builder = lyon::path::BuilderWithAttributes::new(1);

                            let mut first = None;
                            let mut i = 0;
                            while let Some(seg) = p.data.get_segment(i) {
                                let pressure = if let Some(ref pressure) = p.pressure {
                                    let pressure_at_segment = pressure.get(i);
                                    if pressure_at_segment.is_some() {
                                        pressure_at_segment
                                    } else {
                                        pressure.get(i - 1)
                                    }
                                } else {
                                    None
                                };

                                let normalized_pressure =
                                    if let Some(prsr) = pressure { *prsr * 2.0 + 0.1 } else { 1.0 };

                                let thickness =
                                    stroke.width * master_transform.sx * normalized_pressure;

                                let start = devc_to_point(seg.start());
                                let end = devc_to_point(seg.end());
                                if first.is_none() {
                                    first = Some(start);
                                    builder.begin(start, &[thickness]);
                                } else if seg.handle_end().is_some() && seg.handle_start().is_some()
                                {
                                    let handle_start = devc_to_point(seg.handle_start().unwrap());
                                    let handle_end = devc_to_point(seg.handle_end().unwrap());

                                    builder.cubic_bezier_to(
                                        handle_start,
                                        handle_end,
                                        end,
                                        &[thickness],
                                    );
                                } else if seg.handle_end().is_none() && seg.handle_start().is_none()
                                {
                                    builder.line_to(end, &[thickness]);
                                }
                                i = i + 1;
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
                                    .with_tolerance(1.0)
                                    .with_variable_line_width(STROKE_WIDTH),
                                &mut BuffersBuilder::new(
                                    &mut mesh,
                                    VertexConstructor { color: stroke_color },
                                ),
                            );

                            let _ = mesh_tx.send((
                                id,
                                RenderOp::Paint(egui::Shape::Mesh(egui::epaint::Mesh {
                                    indices: mesh.indices.clone(),
                                    vertices: mesh.vertices.clone(),
                                    texture_id: Default::default(),
                                })),
                            ));
                        }
                    }
                    parser::Element::Image(_) => todo!(),
                    parser::Element::Text(_) => todo!(),
                }
            }
        });

        while let Ok((id, op)) = self.mesh_rx.try_recv() {
            match op {
                RenderOp::Delete => {
                    self.mesh_cache.remove(&id);
                }
                RenderOp::Paint(mesh) => {
                    self.mesh_cache.insert(id.to_owned(), mesh);
                    if let Some(el) = buffer.elements.get_mut(&id) {
                        match el {
                            parser::Element::Path(p) => p.changed = false,
                            parser::Element::Image(i) => i.changed = false,
                            parser::Element::Text(_) => todo!(),
                        }
                    }
                }
                RenderOp::Transform => todo!(),
            }
        }

        if !self.mesh_cache.is_empty() {
            painter.extend(self.mesh_cache.clone().into_values());
        }
    }
}

fn devc_to_point(dvec: DVec2) -> Point {
    Point::new(dvec.x as f32, dvec.y as f32)
}

// fn render_image(img: &mut parser::Image, ui: &mut egui::Ui, id: &String, painter: &egui::Painter) {
//     match &img.data {Iterator
//         ImageKind::JPEG(bytes) | ImageKind::PNG(bytes) => {
//             let image = image::load_from_memory(bytes).unwrap();

//             let egui_image = egui::ColorImage::from_rgba_unmultiplied(
//                 [image.width() as usize, image.height() as usize],
//                 &image.to_rgba8(),
//             );
//             if img.texture.is_none() {
//                 img.texture = Some(ui.ctx().load_texture(
//                     format!("canvas_img_{}", id),
//                     egui_image,
//                     egui::TextureOptions::LINEAR,
//                 ));
//             }

//             if let Some(texture) = &img.texture {
//                 let rect = egui::Rect {
//                     min: egui::pos2(img.view_box.rect.left(), img.view_box.rect.top()),
//                     max: egui::pos2(img.view_box.rect.right(), img.view_box.rect.bottom()),
//                 };
//                 let uv = egui::Rect {
//                     min: egui::Pos2 { x: 0.0, y: 0.0 },
//                     max: egui::Pos2 { x: 1.0, y: 1.0 },
//                 };

//                 let mut mesh = egui::Mesh::with_texture(texture.id());
//                 mesh.add_rect_with_uv(rect, uv, egui::Color32::WHITE.linear_multiply(img.opacity));
//                 painter.add(egui::Shape::mesh(mesh));
//             }
//         }
//         ImageKind::GIF(_) => todo!(),
//         ImageKind::SVG(_) => todo!(),
//     }
// }
