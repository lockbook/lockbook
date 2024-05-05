use std::{collections::HashMap, ops::Deref, sync::Arc};

use bezier_rs::{Bezier, Identifier, Subpath};
use glam::{DAffine2, DMat2, DVec2};
use resvg::{
    tiny_skia::Point,
    usvg::{self, Fill, ImageHrefStringResolverFn, ImageKind, Stroke, Text, Transform, Visibility},
};
use usvg_parser::Options;

use super::zoom::G_CONTAINER_ID;

impl Identifier for ManipulatorGroupId {
    fn new() -> Self {
        ManipulatorGroupId
    }
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ManipulatorGroupId;

#[derive(Default)]
pub struct Buffer {
    pub elements: HashMap<String, Element>,
    pub master_transform: Transform,
    pub needs_path_map_update: bool,
}

pub enum Element {
    Path(Path),
    Image(Image),
    Text(Text),
}

struct Path {
    pub data: Subpath<ManipulatorGroupId>,
    pub visibility: Visibility,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    pub transform: Transform,
}

struct Image {
    pub data: ImageKind,
    pub visibility: Visibility,
    pub transform: Transform,
}

impl Buffer {
    pub fn new(svg: &str) -> Self {
        let fontdb = usvg::fontdb::Database::new();
        let utree: usvg::Tree = usvg::Tree::from_str(
            svg,
            &Options { image_href_resolver: lb_local_resolver, ..Default::default() },
            &fontdb,
        )
        .unwrap();

        let mut buffer = Buffer::default();

        utree
            .root()
            .children()
            .iter()
            .enumerate()
            .for_each(|(i, u_el)| match &u_el {
                usvg::Node::Group(group) => {
                    if group.id().eq(G_CONTAINER_ID) {
                        buffer.master_transform = group.transform();
                    }
                }
                usvg::Node::Image(img) => {
                    buffer.elements.insert(
                        i.to_string(),
                        Element::Image(Image {
                            data: img.kind().clone(),
                            visibility: img.visibility(),
                            transform: img.abs_transform(),
                        }),
                    );
                }
                usvg::Node::Text(text) => {
                    buffer
                        .elements
                        .insert(i.to_string(), Element::Text(*text.to_owned().deref()));
                }
                usvg::Node::Path(path) => {
                    buffer.elements.insert(
                        i.to_string(),
                        Element::Path(Path {
                            data: usvg_d_to_subpath(path),
                            visibility: path.visibility(),
                            fill: path.fill().cloned(),
                            stroke: path.stroke().cloned(),
                            transform: path.abs_transform(),
                        }),
                    );
                }
                _ => {}
            });
        buffer
    }
}

fn usvg_d_to_subpath(path: &Box<usvg::Path>) -> Subpath<ManipulatorGroupId> {
    let mut prev = Point::default();
    let mut subpath: Subpath<ManipulatorGroupId> = Subpath::new(vec![], false);
    for segment in path.data().segments() {
        match segment {
            resvg::tiny_skia::PathSegment::MoveTo(p) => {
                prev = p;
            }
            resvg::tiny_skia::PathSegment::CubicTo(p1, p2, p3) => {
                let bez = Bezier::from_cubic_coordinates(
                    prev.x.into(),
                    prev.y.into(),
                    p1.x.into(),
                    p1.y.into(),
                    p2.x.into(),
                    p2.y.into(),
                    p3.x.into(),
                    p3.y.into(),
                );
                subpath.append_bezier(&bez, bezier_rs::AppendType::IgnoreStart);
                prev = p3;
            }
            resvg::tiny_skia::PathSegment::LineTo(p) => {
                let bez = Bezier::from_linear_coordinates(
                    prev.x.into(),
                    prev.y.into(),
                    p.x.into(),
                    p.y.into(),
                );
                subpath.append_bezier(&bez, bezier_rs::AppendType::IgnoreStart);
                prev = p;
            }
            _ => {}
        }
    }

    let t = path.abs_transform();

    subpath.apply_transform(DAffine2 {
        matrix2: DMat2 {
            x_axis: DVec2 { x: t.sx.into(), y: t.ky.into() },
            y_axis: DVec2 { x: t.kx.into(), y: t.sy.into() },
        },
        translation: DVec2 { x: t.tx.into(), y: t.ty.into() },
    });

    subpath
}

fn lb_local_resolver(core: &lb_rs::Core) -> ImageHrefStringResolverFn {
    let lb_link_prefix = "lb://";
    let core = core.clone();
    Box::new(move |href: &str, _opts: &Options| {
        if !href.starts_with(lb_link_prefix) {
            return None;
        }
        let id = &href[lb_link_prefix.len()..];
        let id = lb_rs::Uuid::from_str(id).ok()?;
        let raw = core.read_document(id).ok()?;

        let name = core.get_file_by_id(id).ok()?.name;
        let ext = name.split('.').last().unwrap_or_default();
        match ext {
            "jpg" | "jpeg" => Some(ImageKind::JPEG(Arc::new(raw))),
            "png" => Some(ImageKind::PNG(Arc::new(raw))),
            // "svg" => Some(ImageKind::SVG(Arc::new(raw))), todo: handle nested svg
            "gif" => Some(ImageKind::GIF(Arc::new(raw))),
            _ => None,
        }
    })
}

impl ToString for Buffer {
    fn to_string(&self) -> String {
        // let mut out = Vec::new();
        // if let Err(msg) = self.current.write_to(&mut out) {
        //     println!("{:#?}", msg);
        // }
        // let out = std::str::from_utf8(&out)
        //     .unwrap()
        //     .replace("href", "xlink:href"); // risky
        // let out = out.replace("xmlns='' ", "");
        // let out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\">{}</svg>", out);
        // out
        "todo serialize the buffer".to_string()
    }
}

// impl Debug for Buffer {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("Buffer")
//             .field("undo", &self.undo)
//             .field("redo", &self.redo)
//             .finish()
//     }
// }
