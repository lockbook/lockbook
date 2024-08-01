use std::f64::consts::PI;

use bezier_rs::{Bezier, Subpath};
use chrono::Utc;
use epaint::WHITE_UV;
use glam::f64::DVec2;
use lyon::path::traits::{Build, PathBuilder, PathIterator};
use lyon::path::{AttributeIndex, LineCap, LineJoin, PathEvent};
use lyon::tessellation::geometry_builder::simple_builder;
use lyon::tessellation::{
    self, BuffersBuilder, FillOptions, FillVertexConstructor, StrokeOptions, StrokeTessellator,
    StrokeVertexConstructor, TessellationError, VertexBuffers,
};
use lyon::{math::Point, tessellation::FillTessellator};
use resvg::usvg::{self, tiny_skia_path, ImageKind, Transform};

use super::parser::{ManipulatorGroupId, Stroke};
use super::{
    parser::{self, Path},
    SVGEditor,
};

// A 2x3 matrix (last two members of data1 unused).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct GpuTransform {
    pub data0: [f32; 4],
    pub data1: [f32; 4],
}

pub struct VertexCtor {
    pub prim_id: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GpuVertex {
    pub position: [f32; 2],
    pub prim_id: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GpuPrimitive {
    pub transform: u32,
    pub color: u32,
    pub _pad: [u32; 2],
}
const STROKE_WIDTH: AttributeIndex = 0;

impl GpuPrimitive {
    pub fn new(transform_idx: u32, color: egui::Color32, alpha: f32) -> Self {
        GpuPrimitive {
            transform: transform_idx,
            color: ((color.r() as u32) << 24)
                + ((color.g() as u32) << 16)
                + ((color.b() as u32) << 8)
                + (alpha * 255.0) as u32,
            _pad: [0; 2],
        }
    }
}

impl FillVertexConstructor<GpuVertex> for VertexCtor {
    fn new_vertex(&mut self, vertex: tessellation::FillVertex) -> GpuVertex {
        GpuVertex { position: vertex.position().to_array(), prim_id: self.prim_id }
    }
}

impl StrokeVertexConstructor<GpuVertex> for VertexCtor {
    fn new_vertex(&mut self, vertex: tessellation::StrokeVertex) -> GpuVertex {
        GpuVertex { position: vertex.position().to_array(), prim_id: self.prim_id }
    }
}

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
fn get_oscillating_factor() -> f64 {
    // Get the current time in seconds since the Unix epoch
    let now = Utc::now().timestamp() as f64;

    // Calculate the sine of the time, scaled to oscillate
    // Here we use a period of 2*PI seconds for a smooth oscillation
    let sine_value = (now).sin();

    // Scale and shift the sine value to oscillate between 0.5 and 1.5
    // (sine_value oscillates between -1 and 1)
    // We want the result to oscillate between 0.5 and 1.5
    let factor = 0.5 + (sine_value + 1.0) / 2.0;

    factor
}

impl SVGEditor {
    pub fn render_svg(&mut self, ui: &mut egui::Ui) {
        let painter = ui
            .allocate_painter(self.inner_rect.size(), egui::Sense::click_and_drag())
            .1;
        // let mut fill_tess = FillTessellator::new();
        let mut mesh: VertexBuffers<_, u32> = VertexBuffers::new();
        let mut stroke_tess = StrokeTessellator::new();

        let mut shapes = vec![];
        for (_, el) in self.buffer.elements.iter_mut() {
            match el {
                parser::Element::Path(p) => {
                    if p.data.len() < 3 {
                        continue;
                    }
                    // let path_convertor = PathConvIter::new(p.data.clone());
                    if let Some(stroke) = p.stroke {
                        let stroke_color =
                            if ui.visuals().dark_mode { stroke.color.1 } else { stroke.color.0 }
                                .gamma_multiply(p.opacity);

                        let mut builder = lyon::path::BuilderWithAttributes::new(1);

                        let mut first = None;
                        let mut i = 0;
                        while let Some(seg) = p.data.get_segment(i) {
                            let thickness = stroke.width
                                * self.buffer.master_transform.sx
                                * (((i as f32) % 40.0 + 1.0).ln() * 2.0);

                            let start = devc_to_point(seg.start());
                            let end = devc_to_point(seg.end());
                            if first.is_none() {
                                first = Some(start);
                                builder.begin(start, &[thickness]);
                            } else if seg.handle_end().is_some() && seg.handle_start().is_some() {
                                let handle_start = devc_to_point(seg.handle_start().unwrap());
                                let handle_end = devc_to_point(seg.handle_end().unwrap());

                                builder.cubic_bezier_to(
                                    handle_start,
                                    handle_end,
                                    end,
                                    &[thickness],
                                );
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
                                .with_tolerance(0.01)
                                .with_variable_line_width(STROKE_WIDTH),
                            &mut BuffersBuilder::new(
                                &mut mesh,
                                VertexConstructor { color: stroke_color },
                            ),
                        );

                        shapes.push(egui::Shape::Mesh(egui::epaint::Mesh {
                            indices: mesh.indices.clone(),
                            vertices: mesh.vertices.clone(),
                            texture_id: Default::default(),
                        }));
                    }
                }
                parser::Element::Image(_) => todo!(),
                parser::Element::Text(_) => todo!(),
            }
        }
        painter.extend(shapes);
    }
}

pub struct PathConvIter {
    iter: Subpath<ManipulatorGroupId>,
    first: Option<Point>,
    needs_end: bool,
    deferred: Option<PathEvent>,
    index: usize,
}
impl PathConvIter {
    fn new(path: Subpath<ManipulatorGroupId>) -> Self {
        Self { iter: path, first: None, deferred: None, needs_end: false, index: 0 }
    }
}
fn devc_to_point(dvec: DVec2) -> Point {
    Point::new(dvec.x as f32, dvec.y as f32)
}

// fn graduated_path_outline(
//     path: Subpath<ManipulatorGroupId>, base_width: f32, master_transform: Transform,
// ) -> Vec<Subpath<ManipulatorGroupId>> {
//     let ceil = base_width * 1.3;
//     let floor = base_width * 0.5;
//     let mut is_increasing = true;
//     let mut prev_width = base_width;

//     let mut new_path: Vec<Subpath<ManipulatorGroupId>> = vec![];

//     for m in path.iter() {
//         let new_width = if is_increasing { prev_width * 1.05 } else { prev_width / 1.05 };
//         let new_subpath: Subpath<ManipulatorGroupId> = m.graduated_outline(
//             (prev_width * master_transform.sx) as f64,
//             (new_width * master_transform.sx) as f64,
//             bezier_rs::Cap::Round,
//         );
//         new_path.push(new_subpath);

//         if new_width > ceil {
//             is_increasing = false;
//         } else if new_width < floor {
//             is_increasing = true;
//         }
//         prev_width = new_width;
//     }
//     new_path
// }

impl Iterator for PathConvIter {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
        if self.deferred.is_some() {
            return self.deferred.take();
        }
        let out = if self.index < self.iter.len_segments() {
            let next = self.iter.get_segment(self.index).unwrap();

            let start = devc_to_point(next.start());
            let end = devc_to_point(next.end());
            if self.first.is_none() {
                self.first = Some(start);
            }

            if self.index == 0 {
                Some(PathEvent::Begin { at: start })
            } else if next.handle_end().is_some() && next.handle_start().is_some() {
                self.needs_end = true;
                let handle_start = devc_to_point(next.handle_start().unwrap());
                let handle_end = devc_to_point(next.handle_end().unwrap());

                Some(PathEvent::Cubic {
                    from: start,
                    ctrl1: handle_start,
                    ctrl2: handle_end,
                    to: end,
                })
            } else if next.handle_end().is_none() && next.handle_start().is_none() {
                Some(PathEvent::Line { from: start, to: end })
            } else {
                //quadratic case not handled could cause the path conversion to stop
                None
            }
        } else if self.index == self.iter.len_segments() && self.iter.len_segments() > 1 {
            Some(PathEvent::End {
                last: devc_to_point(
                    self.iter
                        .get_segment(self.iter.len_segments() - 1)
                        .unwrap()
                        .end(),
                ),
                first: self.first.unwrap(),
                close: false,
            })
        } else {
            None
        };
        self.index = self.index + 1;
        return out;
    }
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
//                 mesh.add_rect_with_uv(rect, uv, egui::Color32::WHITE.gamma_multiply(img.opacity));
//                 painter.add(egui::Shape::mesh(mesh));
//             }
//         }
//         ImageKind::GIF(_) => todo!(),
//         ImageKind::SVG(_) => todo!(),
//     }
// }
