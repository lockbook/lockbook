use std::collections::HashMap;
use std::fmt::Write;

use bezier_rs::{Bezier, Subpath};
use glam::{DAffine2, DMat2, DVec2};
use indexmap::IndexMap;
use usvg::{
    fontdb::Database,
    tiny_skia_path::{PathSegment, Point},
    Options, Transform,
};
use usvg::{Color, Paint};
use uuid::Uuid;

use super::element::{DynamicColor, Stroke, WeakImage, WeakImages};
use super::{
    diff::DiffState,
    element::{Element, ManipulatorGroupId, Path},
};

const ZOOM_G_ID: &str = "lb_master_transform";
const WEAK_IMAGE_G_ID: &str = "lb_images";

#[derive(Default, Clone)]
pub struct Buffer {
    pub elements: IndexMap<Uuid, Element>,
    pub weak_images: WeakImages,
    pub master_transform: Transform,
    id_map: HashMap<Uuid, String>,
}

impl Buffer {
    pub fn new(content: &str) -> Self {
        let mut elements = IndexMap::default();
        let mut master_transform = Transform::identity();
        let mut id_map = HashMap::default();
        let mut weak_images = WeakImages::default();

        let maybe_tree = usvg::Tree::from_str(content, &Options::default(), &Database::default());

        if let Err(err) = maybe_tree {
            println!("{:#?}", err);
        } else {
            let utree = maybe_tree.unwrap();

            utree.root().children().iter().for_each(|u_el| {
                parse_child(
                    u_el,
                    &mut elements,
                    &mut master_transform,
                    &mut id_map,
                    &mut weak_images,
                )
            });
        }

        Self { elements, master_transform, id_map, weak_images }
    }

    pub fn reload(
        local_elements: &mut IndexMap<Uuid, Element>, local_weak_images: &mut WeakImages,
        local_master_transform: Transform, base_buffer: &Self, remote_buffer: &Self,
    ) {
        // todo: convert weak images
        for (id, base_img) in base_buffer.weak_images.iter() {
            if let Some(remote_img) = remote_buffer.weak_images.get(id) {
                if remote_img != base_img {
                    local_weak_images.insert(*id, *remote_img);
                }
            } else {
                // this was deleted remotly
                local_weak_images.remove(id);
                local_elements.shift_remove(id);
            }
        }

        for (id, remote_img) in remote_buffer.weak_images.iter() {
            if !base_buffer.weak_images.contains_key(id) {
                local_weak_images.insert(*id, *remote_img);
            }
        }

        base_buffer
            .elements
            .iter()
            .filter_map(|(id, el)| if let Element::Path(p) = el { Some((id, p)) } else { None })
            .for_each(|(id, base_path)| {
                if let Some(Element::Path(remote_path)) = remote_buffer.elements.get(id) {
                    if remote_path != base_path {
                        // this element was changed remotly
                        let mut transformed_path = remote_path.clone();
                        transformed_path.diff_state.transformed = Some(local_master_transform);
                        transformed_path.diff_state.transformed = None;
                        transformed_path
                            .data
                            .apply_transform(u_transform_to_bezier(&local_master_transform));
                        transformed_path.diff_state.data_changed = true;

                        local_elements.insert(*id, Element::Path(transformed_path.clone()));
                    }
                } else {
                    // this was deletd remotly
                    local_elements.shift_remove(id);
                }
            });

        remote_buffer
            .elements
            .iter()
            .filter_map(|(id, el)| if let Element::Path(p) = el { Some((id, p)) } else { None })
            .enumerate()
            .for_each(|(i, (id, remote_el))| {
                if !base_buffer.elements.contains_key(id) {
                    let mut transformed_path = remote_el.clone();
                    transformed_path.diff_state.transformed = Some(local_master_transform);
                    transformed_path
                        .data
                        .apply_transform(u_transform_to_bezier(&local_master_transform));

                    transformed_path.diff_state.data_changed = true;
                    transformed_path.diff_state.transformed = None;

                    local_elements.insert_before(i, *id, Element::Path(transformed_path));
                }
            });
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

    pub fn serialize(&self) -> String {
        let mut root = r#"<svg xmlns="http://www.w3.org/2000/svg">"#.into();
        let mut weak_images = WeakImages::default();
        for (index, el) in self.elements.iter().enumerate() {
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
                            stroke.color.light.red,
                            stroke.color.light.green,
                            stroke.color.light.blue,
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
                    if img.deleted {
                        continue;
                    }

                    let mut weak_image: WeakImage = img.into_weak(index);

                    weak_image.transform(self.master_transform.invert().unwrap_or_default());

                    weak_images.insert(*el.0, weak_image);
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

        weak_images.extend(self.weak_images.iter());

        if !weak_images.is_empty() {
            let binary_data = bincode::serialize(&weak_images).expect("Failed to serialize");
            let base64_data = base64::encode(&binary_data);

            let _ = write!(
                &mut root,
                "<g id=\"{}\"> <g id=\"{}\"></g></g>",
                WEAK_IMAGE_G_ID, base64_data
            );
        }

        let _ = write!(&mut root, "{} </svg>", zoom_level);
        root
    }
}

pub fn parse_child(
    u_el: &usvg::Node, elements: &mut IndexMap<Uuid, Element>, master_transform: &mut Transform,
    id_map: &mut HashMap<Uuid, String>, weak_images: &mut WeakImages,
) {
    match &u_el {
        usvg::Node::Group(group) => {
            if group.id().eq(ZOOM_G_ID) {
                *master_transform = group.transform();
            } else if group.id().eq(WEAK_IMAGE_G_ID) {
                if let Some(usvg::Node::Group(weak_images_g)) = group.children().first() {
                    let base64 = base64::decode(weak_images_g.id().as_bytes())
                        .expect("Failed to decode base64");

                    let decoded: WeakImages = bincode::deserialize(&base64).unwrap();
                    *weak_images = decoded;
                }
            } else {
                group.children().iter().for_each(|u_el| {
                    parse_child(u_el, elements, master_transform, id_map, weak_images)
                });
            }
        }

        usvg::Node::Image(_) => {}
        usvg::Node::Path(path) => {
            let diff_state = DiffState { data_changed: true, ..Default::default() };

            let id = get_internal_id(path.id(), id_map);

            let stroke = if let Some(s) = path.stroke() {
                if let Paint::Color(color) = *s.paint() {
                    let canvas_colors = get_canvas_colors();

                    let maybe_dynamic_color = if let Some(dynamic_color) = canvas_colors
                        .iter()
                        .find(|c| c.light.eq(&color) || c.dark.eq(&color))
                    {
                        *dynamic_color
                    } else {
                        DynamicColor { light: color, dark: color }
                    };

                    Some(Stroke {
                        color: maybe_dynamic_color,
                        opacity: s.opacity().get(),
                        width: s.width().get(),
                    })
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

pub fn get_canvas_colors() -> Vec<DynamicColor> {
    let mut highlighter_colors = get_highlighter_colors();
    highlighter_colors.append(&mut get_pen_colors());

    highlighter_colors
}

pub fn get_highlighter_colors() -> Vec<DynamicColor> {
    let yellow =
        DynamicColor { light: Color::new_rgb(244, 250, 65), dark: Color::new_rgb(244, 250, 65) };
    let blue =
        DynamicColor { light: Color::new_rgb(65, 194, 250), dark: Color::new_rgb(65, 194, 250) };
    let pink =
        DynamicColor { light: Color::new_rgb(254, 110, 175), dark: Color::new_rgb(254, 110, 175) };
    vec![yellow, blue, pink]
}

pub fn get_pen_colors() -> Vec<DynamicColor> {
    let red =
        DynamicColor { light: Color::new_rgb(218, 21, 21), dark: Color::new_rgb(174, 33, 33) };
    let orange =
        DynamicColor { light: Color::new_rgb(255, 149, 0), dark: Color::new_rgb(255, 159, 10) };
    let yellow =
        DynamicColor { light: Color::new_rgb(255, 204, 0), dark: Color::new_rgb(255, 214, 10) };
    let green =
        DynamicColor { light: Color::new_rgb(42, 136, 49), dark: Color::new_rgb(56, 176, 65) };
    let teal =
        DynamicColor { light: Color::new_rgb(0, 128, 128), dark: Color::new_rgb(0, 147, 147) };
    let cyan =
        DynamicColor { light: Color::new_rgb(85, 190, 240), dark: Color::new_rgb(90, 200, 245) };
    let blue =
        DynamicColor { light: Color::new_rgb(62, 130, 230), dark: Color::new_rgb(54, 116, 207) };
    let indigo =
        DynamicColor { light: Color::new_rgb(75, 0, 130), dark: Color::new_rgb(64, 0, 110) };
    let purple =
        DynamicColor { light: Color::new_rgb(128, 0, 128), dark: Color::new_rgb(147, 0, 147) };
    let magenta =
        DynamicColor { light: Color::new_rgb(175, 82, 222), dark: Color::new_rgb(191, 90, 242) };
    let pink =
        DynamicColor { light: Color::new_rgb(255, 105, 180), dark: Color::new_rgb(255, 120, 190) };
    let brown =
        DynamicColor { light: Color::new_rgb(139, 69, 19), dark: Color::new_rgb(101, 53, 17) };

    let fg = DynamicColor { light: Color::black(), dark: Color::white() };

    vec![fg, red, orange, yellow, green, teal, cyan, blue, indigo, purple, brown, magenta, pink]
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

fn to_svg_transform(transform: Transform) -> String {
    format!(
        "matrix({} {} {} {} {} {})",
        transform.sx, transform.ky, transform.kx, transform.sy, transform.tx, transform.ty
    )
}
