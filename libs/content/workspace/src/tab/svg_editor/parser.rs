use std::{collections::HashMap, fmt::Write, path::PathBuf, str::FromStr, sync::Arc};

use bezier_rs::{Bezier, Identifier, Subpath};
use egui::TextureHandle;
use glam::{DAffine2, DMat2, DVec2};
use indexmap::IndexMap;
use lb_rs::{DocumentHmac, Uuid};
use resvg::tiny_skia::Point;
use resvg::usvg::{
    self, fontdb::Database, Fill, ImageHrefResolver, ImageKind, Options, Paint, Text, Transform,
    Visibility,
};
use tracing::warn;

use super::selection::u_transform_to_bezier;
use super::toolbar::{get_highlighter_colors, get_pen_colors};
use super::SVGEditor;

const ZOOM_G_ID: &str = "lb_master_transform";

/// A shorthand for [ImageHrefResolver]'s string function.
pub type ImageHrefStringResolverFn =
    Box<dyn Fn(&str, &Options, &Database) -> Option<ImageKind> + Send + Sync>;

impl Identifier for ManipulatorGroupId {
    fn new() -> Self {
        ManipulatorGroupId { is_predicted: false }
    }
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ManipulatorGroupId {
    pub is_predicted: bool,
}

#[derive(Default)]
pub struct Buffer {
    pub open_file_hmac: Option<DocumentHmac>,
    pub opened_content: String,

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
    pub fn new(svg: &str, core: &lb_rs::Core, open_file: Uuid, hmac: Option<DocumentHmac>) -> Self {
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

        let mut buffer =
            Buffer { open_file_hmac: hmac, opened_content: svg.to_string(), ..Default::default() };

        utree
            .root()
            .children()
            .iter()
            .enumerate()
            .for_each(|(_, u_el)| {
                parse_child(
                    u_el,
                    &mut buffer.elements,
                    &mut buffer.master_transform,
                    &mut buffer.id_map,
                )
            });

        buffer.elements.iter_mut().for_each(|(_, el)| {
            el.transform(buffer.master_transform);
        });

        buffer
    }

    pub fn reload(&mut self, content: &[u8], hmac: Option<DocumentHmac>) {
        // construct diff from buffer.opened_content to content

        //
        // CONSTRCUT THE ELS FROM BASE MAP
        //
        let maybe_tree =
            usvg::Tree::from_str(&self.opened_content, &Options::default(), &Database::default());
        if let Err(err) = maybe_tree {
            println!("{:#?}", err);
            panic!("couldn't parse the base content");
        }

        let utree = maybe_tree.unwrap();

        let mut base_eles = IndexMap::default();
        let mut base_master_transform = Transform::identity();
        let mut base_id_map = HashMap::default();

        utree
            .root()
            .children()
            .iter()
            .enumerate()
            .for_each(|(_, u_el)| {
                parse_child(u_el, &mut base_eles, &mut base_master_transform, &mut base_id_map)
            });

        base_eles.iter_mut().for_each(|(_, el)| {
            el.transform(base_master_transform);
        });

        //
        // CONSTRCUT THE ELS FROM REMOTE
        //
        let maybe_tree = usvg::Tree::from_str(
            String::from_utf8_lossy(content).to_string().as_str(),
            &Options::default(),
            &Database::default(),
        );
        if let Err(err) = maybe_tree {
            println!("{:#?}", err);
            panic!("couldn't parse the base content");
        }

        let utree = maybe_tree.unwrap();

        let mut remote_eles = IndexMap::default();
        let mut remote_master_transform = Transform::identity();
        let mut remote_id_map = HashMap::default();

        utree
            .root()
            .children()
            .iter()
            .enumerate()
            .for_each(|(_, u_el)| {
                parse_child(
                    u_el,
                    &mut remote_eles,
                    &mut remote_master_transform,
                    &mut remote_id_map,
                )
            });

        // remote_eles.iter_mut().for_each(|(_, el)| {
        //     el.transform(remote_master_transform);
        // });

        for (id, base_el) in base_eles.iter() {
            if let Some(remote_el) = remote_eles.get(id) {
                if let Element::Path(remote_path) = remote_el {
                    if let Element::Path(base_path) = base_el {
                        let first_remote_mg = &remote_path.data.manipulator_groups()[0].anchor;
                        let first_base_mg = &base_path.data.manipulator_groups()[0].anchor;

                        if first_base_mg != first_remote_mg {
                            // this was changed remotly
                            self.elements.insert(*id, remote_el.clone());
                        }
                    }
                }
            } else {
                // this was deletd remotly
                self.elements.shift_remove(id);
            }
        }

        for (id, remote_el) in remote_eles.iter() {
            if !base_eles.contains_key(id) {
                // this was created remotly
                self.elements.insert(*id, remote_el.clone());
            }
        }

        self.open_file_hmac = hmac;
    }
}

impl SVGEditor {
    pub fn get_minimal_content(&self) -> String {
        self.buffer.to_string()
    }
}

fn parse_child(
    u_el: &usvg::Node, elements: &mut IndexMap<Uuid, Element>, master_transform: &mut Transform,
    id_map: &mut HashMap<Uuid, String>,
) {
    match &u_el {
        usvg::Node::Group(group) => {
            if group.id().eq(ZOOM_G_ID) {
                *master_transform = group.transform();
            }
            group
                .children()
                .iter()
                .enumerate()
                .for_each(|(_, u_el)| parse_child(u_el, elements, master_transform, id_map));
        }

        usvg::Node::Image(img) => {
            let diff_state = DiffState { data_changed: true, ..Default::default() };

            let id = get_internal_id(img.id(), id_map);

            elements.insert(
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

                    let mut pen_colors = get_pen_colors();
                    pen_colors.append(&mut get_highlighter_colors());

                    let maybe_dynamic_color = if let Some(dynamic_color) = pen_colors
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

            let id = get_internal_id(path.id(), id_map);
            elements.insert(
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

fn get_internal_id(svg_id: &str, id_map: &mut HashMap<Uuid, String>) -> Uuid {
    let id: Uuid = svg_id.parse().unwrap_or(Uuid::new_v4());

    if id_map.insert(id, svg_id.to_owned()).is_some() {
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

impl Buffer {
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
                    let transform =
                        u_transform_to_bezier(&p.transform.invert().unwrap_or_default());
                    let mut data = p.data.clone();
                    data.apply_transform(transform);

                    if data.len() > 1 {
                        data.to_svg(&mut root, curv_attrs, "".into(), "".into(), "".into())
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
