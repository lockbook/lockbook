use std::fmt::Write;
use std::sync::Arc;
use std::{collections::HashMap, str::FromStr};

use crate::blocking::Lb;
use crate::model::file_metadata::DocumentHmac;

use bezier_rs::{Bezier, Subpath};
use glam::{DAffine2, DMat2, DVec2};
use indexmap::IndexMap;
use usvg::{
    fontdb::Database,
    tiny_skia_path::{PathSegment, Point},
    ImageHrefResolver, ImageHrefStringResolverFn, Options, Transform,
};
use usvg::{ImageKind, Paint};
use uuid::Uuid;

use super::element::Stroke;
use super::{
    diff::DiffState,
    element::{Element, Image, ManipulatorGroupId, Path},
};

const ZOOM_G_ID: &str = "lb_master_transform";

#[derive(Default)]
pub struct Buffer {
    pub open_file_hmac: Option<DocumentHmac>,
    pub opened_content: String,

    pub elements: IndexMap<Uuid, Element>,
    pub master_transform: Transform,
    id_map: HashMap<Uuid, String>,
}

impl Buffer {
    pub fn new(
        content: &str, maybe_core: Option<&Lb>, open_file_hmac: Option<DocumentHmac>,
    ) -> Self {
        let mut elements = IndexMap::default();
        let mut master_transform = Transform::identity();
        let mut id_map = HashMap::default();

        let opt = if let Some(core) = maybe_core {
            let lb_local_resolver = ImageHrefResolver {
                resolve_data: ImageHrefResolver::default_data_resolver(),
                resolve_string: lb_local_resolver(core),
            };
            Options { image_href_resolver: lb_local_resolver, ..Default::default() }
        } else {
            Options::default()
        };

        let maybe_tree = usvg::Tree::from_str(&content, &opt, &Database::default());

        if let Err(err) = maybe_tree {
            println!("{:#?}", err);
            println!("couldn't parse the base content");
        } else {
            let utree = maybe_tree.unwrap();

            utree
                .root()
                .children()
                .iter()
                .enumerate()
                .for_each(|(_, u_el)| {
                    parse_child(u_el, &mut elements, &mut master_transform, &mut id_map)
                });
        }

        Self {
            open_file_hmac,
            opened_content: content.to_string(),
            elements,
            master_transform,
            id_map,
        }
    }

    pub fn reload(
        local_elements: &mut IndexMap<Uuid, Element>, local_master_transform: Transform,
        base_content: &str, remote_content: &str,
    ) {
        let base_buffer = Buffer::new(&base_content, None, None);

        let remote_buffer = Buffer::new(remote_content, None, None);

        for (id, base_el) in base_buffer.elements.iter() {
            if let Some(remote_el) = remote_buffer.elements.get(id) {
                if remote_el != base_el {
                    // this element was changed remotly
                    let mut transformed_el = remote_el.clone();
                    if let Element::Path(path) = &mut transformed_el {
                        path.diff_state.transformed = Some(local_master_transform);
                        path.data
                            .apply_transform(u_transform_to_bezier(&local_master_transform));
                    }

                    match transformed_el {
                        Element::Path(ref mut path) => {
                            path.diff_state.data_changed = true;
                            path.diff_state.transformed = None
                        }
                        Element::Image(ref mut image) => {
                            image.diff_state.data_changed = true;
                            image.diff_state.transformed = None
                        }
                        _ => {}
                    }

                    local_elements.insert(*id, transformed_el);

                    println!("remote changed element {:#?}", id);
                }
            } else {
                // this was deletd remotly
                println!("remote delete element {:#?}", id);
                local_elements.shift_remove(id);
            }
        }

        for (id, remote_el) in remote_buffer.elements.iter() {
            if !base_buffer.elements.contains_key(id) {
                // this was created remotly

                let mut transformed_el = remote_el.clone();
                if let Element::Path(path) = &mut transformed_el {
                    path.diff_state.transformed = Some(local_master_transform);
                    path.data
                        .apply_transform(u_transform_to_bezier(&local_master_transform));
                }

                match transformed_el {
                    Element::Path(ref mut path) => {
                        path.diff_state.data_changed = true;
                        path.diff_state.transformed = None
                    }
                    Element::Image(ref mut image) => {
                        image.diff_state.data_changed = true;
                        image.diff_state.transformed = None
                    }
                    _ => {}
                }

                local_elements.insert(*id, transformed_el);
                println!("remote inserted element {:#?}", id);
            }
        }
    }

    pub fn insert(&mut self, id: Uuid, mut el: Element) {
        match el {
            Element::Path(ref mut path) => {
                path.diff_state.data_changed = true;
                path.diff_state.transformed = None
            }
            Element::Image(ref mut image) => {
                image.diff_state.data_changed = true;
                image.diff_state.transformed = None
            }
            _ => {}
        }
        self.elements.insert_before(0, id, el);
    }

    pub fn hard_remove(&mut self, id: Uuid) {
        self.elements.shift_remove(&id);
    }

    /// soft remove that marks the element as deleted but retains it in memeory
    pub fn remove(&mut self, id: Uuid) {
        if let Some(el) = self.elements.get_mut(&id) {
            match el {
                Element::Path(ref mut path) => {
                    path.deleted = true;
                    path.diff_state.delete_changed = true;
                }
                Element::Image(ref mut image) => {
                    image.deleted = true;
                    image.diff_state.delete_changed = true;
                }
                _ => {}
            }
        }
    }

    pub fn to_string(&self) -> String {
        let mut root = r#"<svg xmlns="http://www.w3.org/2000/svg">"#.into();
        for el in self.elements.iter() {
            match el.1 {
                Element::Path(p) => {
                    if p.deleted {
                        continue;
                    }
                    let mut curv_attrs = " ".to_string();
                    // if it's empty then the curve will not be converted to string via bezier_rs
                    if let Some(stroke) = p.stroke {
                        curv_attrs = format!(
                            "stroke-width='{}' stroke='rgba({},{},{},{})' fill='none' id='{}' transform='{}'",
                            stroke.width,
                            stroke.color.red,
                            stroke.color.green,
                            stroke.color.blue,
                            stroke.opacity,
                            self.id_map.get(el.0).unwrap_or(&el.0.to_string()),
                            to_svg_transform(p.transform)
                        );
                    }

                    let mut data = p.data.clone();
                    data.apply_transform(u_transform_to_bezier(
                        &self.master_transform.invert().unwrap_or_default(),
                    ));

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

pub fn parse_child(
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
                    opacity: 1.0,
                    href: Some(img.id().to_string()),
                    diff_state,
                    deleted: false,
                }),
            );
        }
        usvg::Node::Path(path) => {
            let diff_state = DiffState { data_changed: true, ..Default::default() };

            let id = get_internal_id(path.id(), id_map);

            let stroke = if let Some(s) = path.stroke() {
                if let Paint::Color(color) = *s.paint() {
                    Some(Stroke { color, opacity: s.opacity().get(), width: s.width().get() })
                } else {
                    None
                }
            } else {
                None
            };
            let mut data = usvg_d_to_subpath(path);

            data.apply_transform(u_transform_to_bezier(
                &path.abs_transform().invert().unwrap_or_default(),
            ));

            elements.insert(
                id,
                Element::Path(Path {
                    data,
                    visibility: path.visibility(),
                    fill: path.fill().cloned(),
                    stroke,
                    transform: path.abs_transform(),
                    diff_state,
                    deleted: false,
                    opacity: 1.0,
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

fn usvg_d_to_subpath(path: &usvg::Path) -> Subpath<ManipulatorGroupId> {
    let mut prev = Point::default();
    let mut subpath: Subpath<ManipulatorGroupId> = Subpath::new(vec![], false);
    for segment in path.data().segments() {
        match segment {
            PathSegment::MoveTo(p) => {
                prev = p;
            }
            PathSegment::CubicTo(p1, p2, p3) => {
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
            PathSegment::LineTo(p) => {
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

pub fn u_transform_to_bezier(src: &Transform) -> DAffine2 {
    glam::DAffine2 {
        matrix2: DMat2 {
            x_axis: DVec2 { x: src.sx.into(), y: src.ky.into() },
            y_axis: DVec2 { x: src.kx.into(), y: src.sy.into() },
        },
        translation: glam::DVec2 { x: src.tx.into(), y: src.ty.into() },
    }
}

fn lb_local_resolver(core: &Lb) -> ImageHrefStringResolverFn {
    let core = core.clone();
    Box::new(move |href: &str, _opts: &Options, _db: &Database| {
        let id = href.strip_prefix("lb://")?;
        let id = Uuid::from_str(id).ok()?;

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

fn to_svg_transform(transform: Transform) -> String {
    format!(
        "matrix({} {} {} {} {} {})",
        transform.sx, transform.ky, transform.kx, transform.sy, transform.tx, transform.ty
    )
}
