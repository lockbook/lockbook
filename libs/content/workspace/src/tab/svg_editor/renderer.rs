use std::collections::HashMap;

use glam::f64::DVec2;
use lyon::math::Point;
use lyon::path::{AttributeIndex, LineCap, LineJoin};
use lyon::tessellation::{
    self, BuffersBuilder, FillVertexConstructor, StrokeOptions, StrokeTessellator,
    StrokeVertexConstructor, VertexBuffers,
};

use rayon::prelude::*;
use resvg::usvg::Transform;

use crate::tab::svg_editor::selection::u_transform_to_bezier;
use crate::tab::svg_editor::util::bb_to_rect;

use super::parser::{self, DiffState};
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
    Transform(Transform, egui::Rect),
}

pub struct Renderer {
    mesh_cache: HashMap<String, egui::Shape>,
}

impl Renderer {
    pub fn new(elements_count: usize) -> Self {
        Self { mesh_cache: HashMap::with_capacity(elements_count) }
    }

    pub fn render_svg(&mut self, ui: &mut egui::Ui, buffer: &mut Buffer, painter: egui::Painter) {
        let mut elements = buffer.elements.clone();

        let paint_ops: Vec<(String, RenderOp)> = elements
            .par_iter_mut()
            .filter_map(|(id, el)| {
                if el.deleted() && el.delete_changed() {
                    println!("DEL el: {}", id);
                    return Some((id.clone(), RenderOp::Delete));
                };

                if el.deleted()
                    || (!el.opacity_changed()
                        && !el.data_changed()
                        && !el.delete_changed()
                        && el.transformed().is_none())
                {
                    return None;
                }

                if let Some(transform) = el.transformed() {
                    return Some((id.clone(), RenderOp::Transform(transform, egui::Rect::NOTHING)));

                    // let maybe_el_rect = match el {
                    //     parser::Element::Path(p) => {
                    //         if let Some(bb) = p.data.bounding_box() {
                    //             Some(bb_to_rect(bb))
                    //         } else {
                    //             None
                    //         }
                    //     }
                    //     parser::Element::Image(i) => Some(i.bounding_box()),
                    //     parser::Element::Text(_) => todo!(),
                    // };
                    // return if let Some(el_rect) = maybe_el_rect {
                    //     Some((id.clone(), RenderOp::Transform(transform, el_rect)))
                    // } else {
                    //     None
                    // };
                }

                tesselate_element(el, id, &painter, ui.visuals().dark_mode, buffer.master_transform)
            })
            .collect();

        for (id, paint_op) in paint_ops {
            match paint_op {
                RenderOp::Delete => {
                    self.mesh_cache.remove(&id);
                }
                RenderOp::Paint(m) => {
                    self.mesh_cache.insert(id.to_owned(), m);
                }
                RenderOp::Transform(t, rect) => {
                    if let Some(mesh) = self.mesh_cache.get_mut(&id) {
                        println!("TRANSFORM el : {}", id);
                        if let egui::Shape::Mesh(m) = mesh {
                            for v in &mut m.vertices {
                                v.pos.x = t.sx * v.pos.x + t.tx;
                                v.pos.y = t.sy * v.pos.y + t.ty;
                            }
                        }
                    }
                    // if painter.clip_rect().contains_rect(rect)
                    //     || painter.clip_rect().intersects(rect)
                    // {
                    //     if let Some(mesh) = self.mesh_cache.get_mut(&id) {
                    //         println!("TRANSFORM el : {}", id);
                    //         if let egui::Shape::Mesh(m) = mesh {
                    //             for v in &mut m.vertices {
                    //                 v.pos.x = t.sx * v.pos.x + t.tx;
                    //                 v.pos.y = t.sy * v.pos.y + t.ty;
                    //             }
                    //         }
                    //     } else {
                    //         if let Some(el) = buffer.elements.get_mut(&id) {
                    //             let mut out = tesselate_element(
                    //                 el,
                    //                 &id.clone(),
                    //                 &painter,
                    //                 ui.visuals().dark_mode,
                    //                 buffer.master_transform,
                    //             );
                    //             if let Some((_, RenderOp::Paint(shape))) = &mut out {
                    //                 if let egui::Shape::Mesh(m) = shape {
                    //                     for v in &mut m.vertices {
                    //                         v.pos.x = t.sx * v.pos.x + t.tx;
                    //                         v.pos.y = t.sy * v.pos.y + t.ty;
                    //                     }
                    //                 }
                    //                 self.mesh_cache.insert(id.to_owned(), shape.clone());
                    //             }
                    //         }
                    //     }
                    // } else {
                    //     if self.mesh_cache.remove(&id).is_some() {
                    //         println!("EVICT el : {}", id);
                    //     }
                    // }
                }
            }
        }
        if !self.mesh_cache.is_empty() {
            painter.extend(self.mesh_cache.clone().into_values().filter(|shape| {
                if let egui::Shape::Mesh(m) = shape {
                    !m.vertices.is_empty() && !m.indices.is_empty()
                } else {
                    true
                }
            }));
        }

        buffer
            .elements
            .iter_mut()
            .for_each(|(_, element)| match element {
                parser::Element::Path(p) => p.diff_state = DiffState::default(),
                parser::Element::Image(i) => i.diff_state = DiffState::default(),
                parser::Element::Text(_) => todo!(),
            })
    }
}
// todo: maybe impl this on element struct
fn tesselate_element(
    el: &mut parser::Element, id: &String, painter: &egui::Painter, dark_mode: bool,
    master_transform: Transform,
) -> Option<(String, RenderOp)> {
    let mut mesh: VertexBuffers<_, u32> = VertexBuffers::new();
    let mut stroke_tess = StrokeTessellator::new();

    match el {
        parser::Element::Path(p) => {
            println!(
                "TESS el with data len and pressure len: {}, {}, {} ",
                id,
                p.data.len_segments(),
                p.pressure.to_owned().unwrap_or_default().len()
            );

            if let Some(stroke) = p.stroke {
                if p.data.is_empty() {
                    return None;
                }
                let stroke_color = if dark_mode { stroke.color.1 } else { stroke.color.0 }
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
                            println!("OUT OF PRESSURE");
                            pressure.get(i - 1)
                        }
                    } else {
                        println!("NO PRESSURE");
                        None
                    };

                    let normalized_pressure =
                        if let Some(prsr) = pressure { *prsr * 2.0 + 0.1 } else { 1.0 };

                    let thickness = stroke.width * master_transform.sx * normalized_pressure;

                    let start = devc_to_point(seg.start());
                    let end = devc_to_point(seg.end());
                    if first.is_none() {
                        first = Some(start);
                        builder.begin(start, &[thickness]);
                    } else if seg.handle_end().is_some() && seg.handle_start().is_some() {
                        let handle_start = devc_to_point(seg.handle_start().unwrap());
                        let handle_end = devc_to_point(seg.handle_end().unwrap());

                        builder.cubic_bezier_to(handle_start, handle_end, end, &[thickness]);
                    } else if seg.handle_end().is_none() && seg.handle_start().is_none() {
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
                    Some((id.to_owned(), RenderOp::Paint(egui::Shape::Mesh(mesh))))
                }
            } else {
                None
            }
        }
        parser::Element::Image(_) => todo!(),
        parser::Element::Text(_) => todo!(),
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