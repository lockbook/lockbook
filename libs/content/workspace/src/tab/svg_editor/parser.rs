use std::{collections::HashMap, fmt::Write, str::FromStr, sync::Arc};

use bezier_rs::{Bezier, Identifier, Subpath};
use egui::TextureHandle;
use glam::{DAffine2, DMat2, DVec2};
use lb_rs::Uuid;
use resvg::{
    tiny_skia::Point,
    usvg::{
        self, fontdb::Database, Fill, ImageHrefResolver, ImageKind, Options, Paint, Text,
        Transform, Visibility,
    },
};

const ZOOM_G_ID: &str = "lb_master_transform";

/// A shorthand for [ImageHrefResolver]'s string function.
pub type ImageHrefStringResolverFn =
    Box<dyn Fn(&str, &Options, &Database) -> Option<ImageKind> + Send + Sync>;

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
    pub deleted_elements: HashMap<String, Element>,
    pub master_transform: Transform,
    pub needs_path_map_update: bool,
}

pub enum Element {
    Path(Path),
    Image(Image),
    Text(Text),
}

pub struct Path {
    pub data: Subpath<ManipulatorGroupId>,
    pub visibility: Visibility,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    pub transform: Transform,
    pub opacity: f32,
}

#[derive(Clone, Copy)]
pub struct Stroke {
    pub color: egui::Color32,
    opacity: f32,
    pub width: f32,
}

impl Default for Stroke {
    fn default() -> Self {
        Self { color: egui::Color32::BLACK, opacity: 1.0, width: 1.0 }
    }
}

pub struct Image {
    pub data: ImageKind,
    pub visibility: Visibility,
    pub transform: Transform,
    pub view_box: usvg::ViewBox,
    pub texture: Option<TextureHandle>,
    pub opacity: f32,
    pub href: Option<Uuid>, // todo: change data modeling when impl remote images. this assumes that all images are resolved to an lb file
}

impl Buffer {
    pub fn new(svg: &str, core: &lb_rs::Core) -> Self {
        let fontdb = usvg::fontdb::Database::default();

        let lb_local_resolver = ImageHrefResolver {
            resolve_data: ImageHrefResolver::default_data_resolver(),
            resolve_string: lb_local_resolver(core),
        };

        let options =
            usvg::Options { image_href_resolver: lb_local_resolver, ..Default::default() };

        let maybe_tree = usvg::Tree::from_str(svg, &options, &fontdb);
        if let Err(err) = maybe_tree {
            println!("{:#?}", err);
            return Self::default();
        }
        let utree = maybe_tree.unwrap();

        let mut buffer = Buffer::default();
        utree
            .root()
            .children()
            .iter()
            .enumerate()
            .for_each(|(i, u_el)| parse_child(u_el, &mut buffer, i));
        buffer
    }
}

fn parse_child(u_el: &usvg::Node, buffer: &mut Buffer, i: usize) {
    match &u_el {
        usvg::Node::Group(group) => {
            if group.id().eq(ZOOM_G_ID) {
                buffer.master_transform = group.transform();
            }
            group
                .children()
                .iter()
                .enumerate()
                .for_each(|(i, u_el)| parse_child(u_el, buffer, i));
        }
        usvg::Node::Image(img) => {
            buffer.elements.insert(
                i.to_string(),
                Element::Image(Image {
                    data: img.kind().clone(),
                    visibility: img.visibility(),
                    transform: img.abs_transform(),
                    view_box: img.view_box(),
                    texture: None,
                    opacity: 1.0,
                    href: Uuid::from_str(img.id()).ok(),
                }),
            );
        }
        usvg::Node::Path(path) => {
            let mut stroke = Stroke::default();
            if let Some(s) = path.stroke() {
                if let Paint::Color(c) = s.paint() {
                    stroke.color = egui::Color32::from_rgb(c.red, c.green, c.blue);
                }
                stroke.width = s.width().get();
                stroke.opacity = s.opacity().get();
            }
            buffer.elements.insert(
                i.to_string(),
                Element::Path(Path {
                    data: usvg_d_to_subpath(path),
                    visibility: path.visibility(),
                    fill: path.fill().cloned(),
                    stroke: Some(stroke),
                    transform: path.abs_transform(),
                    opacity: 1.0,
                }),
            );
        }
        usvg::Node::Text(_) => {}
    }
}

fn lb_local_resolver(core: &lb_rs::Core) -> ImageHrefStringResolverFn {
    let lb_link_prefix = "lb://";
    let core = core.clone();
    Box::new(move |href: &str, _opts: &Options, _db: &Database| {
        if !href.starts_with(lb_link_prefix) {
            // todo: resolve remote links
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

fn usvg_d_to_subpath(path: &usvg::Path) -> Subpath<ManipulatorGroupId> {
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

impl ToString for Buffer {
    fn to_string(&self) -> String {
        let mut root = r#"<svg xmlns="http://www.w3.org/2000/svg">"#.into();
        for el in self.elements.iter() {
            match el.1 {
                Element::Path(p) => {
                    let mut curv_attrs = " ".to_string(); // if it's empty then the curve might not be converted to string via bezier_rs
                    if let Some(stroke) = p.stroke {
                        curv_attrs = format!(
                            r###"stroke-width="{}" stroke="#{:02X}{:02X}{:02X}" fill="none""###,
                            stroke.width,
                            stroke.color.r(),
                            stroke.color.g(),
                            stroke.color.b(),
                        );
                    }
                    if p.data.len() > 1 {
                        p.data
                            .to_svg(&mut root, curv_attrs, "".into(), "".into(), "".into())
                    }
                }
                Element::Image(img) => {
                    let image_element = format!(
                        r#" <image id= "{}" href ="lb://{}" width="{}" height="{}" x="{}" y="{}" />"#,
                        img.href.unwrap_or_default(),
                        img.href.unwrap_or_default(),
                        img.view_box.rect.width(),
                        img.view_box.rect.height(),
                        img.view_box.rect.left(),
                        img.view_box.rect.top(),
                    );

                    let _ = write!(root, "{image_element}");
                }
                Element::Text(_) => {}
            }
        }
        let zoom_level = format!(
            r#"<g id="{}" transform="matrix({} {} {} {} {} {})"></g>"#,
            ZOOM_G_ID,
            self.master_transform.sx,
            self.master_transform.kx,
            self.master_transform.ky,
            self.master_transform.sy,
            self.master_transform.tx,
            self.master_transform.ty
        );
        let _ = write!(&mut root, "{} </svg>", zoom_level);
        root
    }
}

impl Image {
    pub fn bounding_box(&self) -> egui::Rect {
        egui::Rect {
            min: egui::pos2(self.view_box.rect.left(), self.view_box.rect.top()),
            max: egui::pos2(self.view_box.rect.right(), self.view_box.rect.bottom()),
        }
    }
    pub fn apply_transform(&mut self, transform: Transform) {
        if let Some(new_vb) = self.view_box.rect.transform(transform) {
            self.view_box.rect = new_vb;
        }
    }
}
