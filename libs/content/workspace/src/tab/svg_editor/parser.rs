use std::{collections::HashMap, fmt::Write, path::PathBuf, str::FromStr, sync::Arc};

use bezier_rs::{Bezier, Identifier, Subpath};
use egui::TextureHandle;
use glam::{DAffine2, DMat2, DVec2};
use indexmap::IndexMap;
use lb_rs::Uuid;
use resvg::tiny_skia::Point;
use resvg::usvg::{
    self, fontdb::Database, Fill, ImageHrefResolver, ImageKind, Options, Paint, Text, Transform,
    Visibility,
};
use tracing::warn;

use crate::theme::palette::ThemePalette;

use super::selection::u_transform_to_bezier;
use super::SVGEditor;

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
    pub elements: IndexMap<Uuid, Element>,
    pub master_transform: Transform,
    pub needs_path_map_update: bool,
    id_map: HashMap<Uuid, String>,
}

#[derive(Clone)]
pub enum Element {
    Path(Path),
    Image(Image),
    Text(Text),
}

#[derive(Clone)]
pub struct Path {
    pub data: Subpath<ManipulatorGroupId>,
    pub visibility: Visibility,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    pub transform: Transform,
    pub opacity: f32,
    pub diff_state: DiffState,
    pub deleted: bool,
}

#[derive(Clone, Default, Debug)]
pub struct DiffState {
    pub opacity_changed: bool,
    pub transformed: Option<Transform>,
    pub delete_changed: bool,
    pub data_changed: bool,
}

impl DiffState {
    /// is state dirty and require an i/o save
    pub fn is_dirty(&self) -> bool {
        self.data_changed
            || self.delete_changed
            || self.opacity_changed
            || self.transformed.is_some()
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Stroke {
    pub color: (egui::Color32, egui::Color32),
    pub width: f32,
}

impl Default for Stroke {
    fn default() -> Self {
        Self { color: (egui::Color32::BLACK, egui::Color32::WHITE), width: 1.0 }
    }
}

#[derive(Clone)]
pub struct Image {
    pub data: ImageKind,
    pub visibility: Visibility,
    pub transform: Transform,
    pub view_box: usvg::ViewBox,
    pub texture: Option<TextureHandle>,
    pub opacity: f32,
    pub href: Option<String>,
    pub diff_state: DiffState,
    pub deleted: bool,
}

impl Buffer {
    pub fn new(svg: &str, core: &lb_rs::Core, open_file: Uuid) -> Self {
        let fontdb = usvg::fontdb::Database::default();

        let lb_local_resolver = ImageHrefResolver {
            resolve_data: ImageHrefResolver::default_data_resolver(),
            resolve_string: lb_local_resolver(core, open_file),
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
            .for_each(|(_, u_el)| parse_child(u_el, &mut buffer));

        buffer
    }
}

impl SVGEditor {
    pub fn get_minimal_content(&self) -> String {
        self.buffer.to_string()
    }
}

fn parse_child(u_el: &usvg::Node, buffer: &mut Buffer) {
    match &u_el {
        usvg::Node::Group(group) => {
            if group.id().eq(ZOOM_G_ID) {
                buffer.master_transform = group.transform();
            }
            group
                .children()
                .iter()
                .enumerate()
                .for_each(|(_, u_el)| parse_child(u_el, buffer));
        }

        usvg::Node::Image(img) => {
            let diff_state = DiffState { data_changed: true, ..Default::default() };

            let id = get_internal_id(img.id(), buffer);

            buffer.elements.insert(
                id,
                Element::Image(Image {
                    data: img.kind().clone(),
                    visibility: img.visibility(),
                    transform: img.abs_transform(),
                    view_box: img.view_box(),
                    texture: None,
                    opacity: 1.0,
                    href: Some(img.id().to_string()),
                    diff_state,
                    deleted: false,
                }),
            );
        }
        usvg::Node::Path(path) => {
            let mut stroke = Stroke::default();
            let mut opacity = 1.0;
            if let Some(s) = path.stroke() {
                if let Paint::Color(c) = s.paint() {
                    let parsed_color = egui::Color32::from_rgb(c.red, c.green, c.blue);
                    let theme_colors = ThemePalette::as_array();
                    let maybe_dynamic_color = if let Some(dynamic_color) = theme_colors
                        .iter()
                        .find(|c| c.0.eq(&parsed_color) || c.1.eq(&parsed_color))
                    {
                        *dynamic_color
                    } else {
                        (parsed_color, parsed_color)
                    };
                    stroke.color = maybe_dynamic_color;
                }
                stroke.width = s.width().get();
                opacity = s.opacity().get();
            }
            let diff_state = DiffState { data_changed: true, ..Default::default() };

            let id = get_internal_id(path.id(), buffer);
            buffer.elements.insert(
                id,
                Element::Path(Path {
                    data: usvg_d_to_subpath(path),
                    visibility: path.visibility(),
                    fill: path.fill().cloned(),
                    stroke: Some(stroke),
                    transform: Transform::identity(),
                    opacity,
                    diff_state,
                    deleted: false,
                }),
            );
        }
        _ => {}
    }
}

fn get_internal_id(svg_id: &str, buffer: &mut Buffer) -> Uuid {
    let id: Uuid = svg_id.parse().unwrap_or(Uuid::new_v4());

    if buffer.id_map.insert(id, svg_id.to_owned()).is_some() {
        warn!(id = svg_id, "found elements  with duplicate id");
    }
    id
}

fn lb_local_resolver(core: &lb_rs::Core, open_file: Uuid) -> ImageHrefStringResolverFn {
    let core = core.clone();
    Box::new(move |href: &str, _opts: &Options, _db: &Database| {
        let id = if let Some(id) = href.strip_prefix("lb://") {
            lb_rs::Uuid::from_str(id).ok()?
        } else {
            crate::tab::core_get_by_relative_path(&core, open_file, &PathBuf::from(&href))
                .ok()?
                .id
        };

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
                    if p.deleted {
                        continue;
                    }
                    let mut curv_attrs = " ".to_string(); // if it's empty then the curve will not be converted to string via bezier_rs
                    if let Some(stroke) = p.stroke {
                        curv_attrs = format!(
                            "stroke-width='{}' stroke='rgba({},{},{},{})' fill='none' id='{}'",
                            stroke.width,
                            stroke.color.0.r(),
                            stroke.color.0.g(),
                            stroke.color.0.b(),
                            p.opacity,
                            self.id_map.get(el.0).unwrap_or(&el.0.to_string())
                        );
                    }
                    if p.data.len() > 1 {
                        p.data
                            .to_svg(&mut root, curv_attrs, "".into(), "".into(), "".into())
                    }
                }
                Element::Image(img) => {
                    let image_element = format!(
                        r#" <image id="{}" href="{}" width="{}" height="{}" x="{}" y="{}" />"#,
                        self.id_map.get(el.0).unwrap_or(&el.0.to_string()),
                        img.href.clone().unwrap_or_default(),
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

impl Path {
    pub fn bounding_box(&self) -> egui::Rect {
        let default_rect = egui::Rect::NOTHING;
        if self.data.len() < 2 {
            return default_rect;
        }
        let bb = match self.data.bounding_box() {
            Some(val) => val,
            None => return default_rect,
        };

        egui::Rect {
            min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
            max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
        }
    }
}
impl Element {
    pub fn opacity_changed(&self) -> bool {
        match self {
            Element::Path(p) => p.diff_state.opacity_changed,
            Element::Image(i) => i.diff_state.opacity_changed,
            Element::Text(_) => todo!(),
        }
    }
    pub fn delete_changed(&self) -> bool {
        match self {
            Element::Path(p) => p.diff_state.delete_changed,
            Element::Image(i) => i.diff_state.delete_changed,
            Element::Text(_) => todo!(),
        }
    }
    pub fn data_changed(&self) -> bool {
        match self {
            Element::Path(p) => p.diff_state.data_changed,
            Element::Image(i) => i.diff_state.data_changed,
            Element::Text(_) => todo!(),
        }
    }
    pub fn deleted(&self) -> bool {
        match self {
            Element::Path(p) => p.deleted,
            Element::Image(i) => i.deleted,
            Element::Text(_) => todo!(),
        }
    }
    pub fn transformed(&self) -> Option<Transform> {
        match self {
            Element::Path(p) => p.diff_state.transformed,
            Element::Image(i) => i.diff_state.transformed,
            Element::Text(_) => todo!(),
        }
    }
    pub fn transform(&mut self, transform: Transform) {
        match self {
            Element::Path(path) => {
                path.diff_state.transformed = Some(transform);
                path.transform = path.transform.post_concat(transform);
                path.data.apply_transform(u_transform_to_bezier(&transform));
            }
            Element::Image(img) => {
                img.diff_state.transformed = Some(transform);
                img.transform = img.transform.post_concat(transform);
                img.apply_transform(transform);
            }
            Element::Text(_) => todo!(),
        }
    }
    pub fn bounding_box(&self) -> egui::Rect {
        match self {
            Element::Path(p) => p.bounding_box(),
            Element::Image(image) => image.bounding_box(),

            Element::Text(_) => todo!(),
        }
    }
}
