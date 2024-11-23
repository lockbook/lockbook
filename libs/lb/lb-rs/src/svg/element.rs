use std::ops::{Deref, DerefMut};

use bezier_rs::{Identifier, Subpath};
use serde::{Deserialize, Serialize};

use usvg::{self, Color, Fill, ImageKind, Text, Transform, Visibility};
use uuid::Uuid;

use super::{buffer::u_transform_to_bezier, diff::DiffState};

#[derive(Clone)]
pub enum Element {
    Path(Path),
    Image(Image),
    Text(Text),
}

impl PartialEq for Element {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Path(l0), Self::Path(r0)) => l0 == r0,
            (Self::Image(_), Self::Image(_)) => todo!(),
            (Self::Text(_), Self::Text(_)) => todo!(),
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct Path {
    pub data: Subpath<ManipulatorGroupId>,
    pub visibility: Visibility,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    pub transform: Transform,
    pub diff_state: DiffState,
    pub deleted: bool,
    pub opacity: f32,
}

#[derive(Clone, Copy)]
pub struct Stroke {
    pub color: DynamicColor,
    pub opacity: f32,
    pub width: f32,
}

#[derive(Clone, Copy)]
pub struct DynamicColor {
    pub light: usvg::Color,
    pub dark: usvg::Color,
}

impl Default for DynamicColor {
    fn default() -> Self {
        Self { light: Color::black(), dark: Color::white() }
    }
}

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        self.data.len() == other.data.len()
            && self.visibility == other.visibility
            && self.transform == other.transform
            && self.deleted == other.deleted
    }
}

#[derive(Clone)]
pub struct Image {
    pub data: ImageKind,
    pub visibility: Visibility,
    pub transform: Transform,
    pub view_box: usvg::ViewBox,
    pub opacity: f32,
    pub href: Uuid,
    pub diff_state: DiffState,
    pub deleted: bool,
}

impl From<Transform> for WeakTransform {
    fn from(value: Transform) -> Self {
        WeakTransform {
            sx: value.sx,
            kx: value.kx,
            ky: value.ky,
            sy: value.sy,
            tx: value.tx,
            ty: value.ty,
        }
    }
}

impl Into<WeakImage> for &Image {
    fn into(self) -> WeakImage {
        WeakImage {
            href: self.href,
            transform: WeakTransform::from(self.transform),
            opacity: self.opacity,
            width: self.view_box.rect.width(),
            height: self.view_box.rect.height(),
            x: self.view_box.rect.x(),
            y: self.view_box.rect.y(),
            id: self.href,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default)]
pub struct WeakImages(Vec<WeakImage>);

impl Deref for WeakImages {
    type Target = Vec<WeakImage>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WeakImages {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// image that only contains a ref to the data but not the data itself.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy)]
pub struct WeakImage {
    id: Uuid,
    href: Uuid,
    transform: WeakTransform,
    opacity: f32,
    width: f32,
    height: f32,
    x: f32,
    y: f32,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy)]
struct WeakTransform {
    pub sx: f32,
    pub kx: f32,
    pub ky: f32,
    pub sy: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Identifier for ManipulatorGroupId {
    fn new() -> Self {
        ManipulatorGroupId
    }
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ManipulatorGroupId;

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
            }
            Element::Text(_) => todo!(),
        }
    }

    pub fn get_transform(&self) -> Transform {
        match self {
            Element::Path(path) => path.transform,
            Element::Image(img) => img.transform,
            Element::Text(_) => todo!(),
        }
    }

    pub fn opacity(&self) -> f32 {
        match self {
            Element::Path(path) => path.opacity,
            Element::Image(image) => image.opacity,
            Element::Text(_) => todo!(),
        }
    }
}
