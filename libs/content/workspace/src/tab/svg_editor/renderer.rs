use bezier_rs::{Bezier, Subpath};
use epaint::WHITE_UV;
use glam::f64::DVec2;
use lyon::tessellation::{
    self, BuffersBuilder, FillOptions, FillVertexConstructor, StrokeOptions, StrokeTessellator,
    StrokeVertexConstructor, VertexBuffers,
};
use resvg::usvg::{self, tiny_skia_path, ImageKind, Transform};

use crate::theme::palette::ThemePalette;
use lyon::path::PathEvent;
use lyon::{math::Point, tessellation::FillTessellator};

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

impl SVGEditor {
    pub fn render_svg(&mut self, ui: &mut egui::Ui) {
        let painter = ui
            .allocate_painter(self.inner_rect.size(), egui::Sense::click_and_drag())
            .1;
        let mut fill_tess = FillTessellator::new();
        let mut mesh: VertexBuffers<_, u32> = VertexBuffers::new();
        let mut primitives: Vec<GpuPrimitive> = Vec::new();
        let mut stroke_tess = StrokeTessellator::new();
        let transforms: Vec<GpuTransform> = Vec::new();

        let mut shapes = vec![];
        for (_, el) in self.buffer.elements.iter_mut() {
            match el {
                parser::Element::Path(p) => {
                    if p.data.len() < 3 {
                        continue;
                    }
                    let path_convertor = PathConvIter::new(p.data.clone());
                    if let Some(stroke) = p.stroke {
                        let (stroke_color, stroke_opts) =
                            convert_stroke(stroke, ui, self.buffer.master_transform);
                        primitives.push(GpuPrimitive::new(0, stroke_color, p.opacity));
                        let _ = stroke_tess.tessellate(
                            path_convertor,
                            &stroke_opts.with_tolerance(0.01),
                            &mut BuffersBuilder::new(
                                &mut mesh,
                                VertexCtor { prim_id: primitives.len() as u32 - 1 },
                            ),
                        );

                        // fill_tess
                        //     .tessellate(
                        //         path_convertor,
                        //         &FillOptions::tolerance(0.01),
                        //         &mut BuffersBuilder::new(
                        //             &mut mesh,
                        //             VertexCtor { prim_id: primitives.len() as u32 - 1 },
                        //         ),
                        //     )
                        //     .expect("Error during tessellation!");

                        shapes.push(egui::Shape::Mesh(egui::epaint::Mesh {
                            indices: mesh.indices.clone(),
                            vertices: mesh
                                .vertices
                                .iter()
                                .map(|v| egui::epaint::Vertex {
                                    pos: egui::pos2(v.position[0], v.position[1]),
                                    uv: WHITE_UV,
                                    color: stroke_color,
                                })
                                .collect(),
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

pub fn convert_stroke(
    s: Stroke, ui: &mut egui::Ui, master_transform: Transform,
) -> (egui::Color32, StrokeOptions) {
    let color = if ui.visuals().dark_mode { s.color.1 } else { s.color.0 };

    let linecap = tessellation::LineCap::Round;
    let linejoin = tessellation::LineJoin::Round;

    let opt = StrokeOptions::tolerance(0.01)
        .with_line_width(s.width * master_transform.sx)
        .with_line_cap(linecap)
        .with_line_join(linejoin);

    (color, opt)
}
