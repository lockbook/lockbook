use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use bezier_rs::{Identifier, Subpath};
use serde::{Deserialize, Serialize};

use usvg::{self, Fill, ImageKind, NonZeroRect, Text, Transform};
use uuid::Uuid;

use super::buffer::u_transform_to_bezier;
use super::diff::DiffState;

#[derive(Clone)]
pub enum EditableElement {
    Path(Path),
    Image(Box<Image>),
    Text(Text),
}

/// Document information needed to serialize an element. Images are collected
/// separately because they are stored in the drawing's shared metadata.
pub struct SvgSerializationContext<'a> {
    pub id: Uuid,
    pub svg_id: &'a str,
    pub z_index: usize,
    pub weak_images: &'a mut WeakImages,
}

/// Serializes an editable element in document coordinates.
pub trait SvgSerialize {
    fn to_string(&self, context: &mut SvgSerializationContext<'_>) -> String;
}

impl SvgSerialize for EditableElement {
    fn to_string(&self, context: &mut SvgSerializationContext<'_>) -> String {
        match self {
            Self::Path(path) => path.to_string(context),
            Self::Image(image) => image.to_string(context),
            Self::Text(text) => SvgSerialize::to_string(text, context),
        }
    }
}

impl SvgSerialize for Path {
    fn to_string(&self, context: &mut SvgSerializationContext<'_>) -> String {
        if self.deleted {
            return String::new();
        }
        let mut curve_attrs = " ".to_string();
        if let Some(stroke) = self.stroke {
            curve_attrs = format!(
                "stroke-width='{}' stroke='rgba({},{},{},{})' fill='none' id='{}'",
                stroke.width * self.stroke_scale(),
                stroke.color.light.red,
                stroke.color.light.green,
                stroke.color.light.blue,
                stroke.opacity,
                context.svg_id
            );
        }

        let data = &self.data;
        let mut svg = String::new();
        if data.len() > 1 {
            data.to_svg(&mut svg, curve_attrs, "".into(), "".into(), "".into());
        }
        svg
    }
}

impl SvgSerialize for Image {
    fn to_string(&self, context: &mut SvgSerializationContext<'_>) -> String {
        if !self.deleted {
            let image = self.into_weak(context.z_index);
            context.weak_images.insert(context.id, image);
        }
        String::new()
    }
}

impl SvgSerialize for Text {
    fn to_string(&self, _context: &mut SvgSerializationContext<'_>) -> String {
        // Text serialization is not supported by the editor yet.
        String::new()
    }
}

#[derive(Clone)]
pub struct Path {
    /// Geometry in document coordinates; independent of every editor viewport.
    pub data: Subpath<ManipulatorGroupId>,
    pub visible: bool,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    /// Accumulated element edits, used to scale stroke width. Geometry already
    /// contains these edits; rendering must not apply this matrix again.
    pub transform: Transform,
    pub diff_state: DiffState,
    pub deleted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub color: DynamicColor,
    pub opacity: f32,
    pub width: f32,
}

impl Default for Stroke {
    fn default() -> Self {
        Self { color: Default::default(), opacity: 1.0, width: 1.0 }
    }
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub struct DynamicColor {
    pub light: Color,
    pub dark: Color,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Color {
    pub fn new_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub fn black() -> Self {
        Self::new_rgb(0, 0, 0)
    }

    pub fn white() -> Self {
        Self::new_rgb(255, 255, 255)
    }
}

impl Default for DynamicColor {
    fn default() -> Self {
        Self { light: Color::black(), dark: Color::white() }
    }
}

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.visible == other.visible
            && self.transform == other.transform
            && self.deleted == other.deleted
            && self.stroke == other.stroke
    }
}

impl Path {
    pub fn stroke_scale(&self) -> f32 {
        (self.transform.sx.hypot(self.transform.ky) + self.transform.kx.hypot(self.transform.sy))
            / 2.0
    }
}

#[derive(Clone)]
pub struct Image {
    pub data: ImageKind,
    pub visible: bool,
    pub transform: Transform,
    pub view_box: NonZeroRect,
    pub opacity: f32,
    pub href: Uuid,
    pub diff_state: DiffState,
    pub deleted: bool,
}

impl Image {
    pub fn into_weak(&self, z_index: usize) -> WeakImage {
        WeakImage {
            href: self.href,
            transform: WeakTransform::from(self.transform),
            opacity: self.opacity,
            width: self.view_box.width(),
            height: self.view_box.height(),
            x: self.view_box.x(),
            y: self.view_box.y(),
            z_index,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct WeakImages(HashMap<Uuid, WeakImage>);

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct WeakPathPressures(HashMap<Uuid, Vec<f32>>);

impl Deref for WeakPathPressures {
    type Target = HashMap<Uuid, Vec<f32>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WeakPathPressures {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Deref for WeakImages {
    type Target = HashMap<Uuid, WeakImage>;

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
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct WeakImage {
    pub href: Uuid,
    pub transform: WeakTransform,
    pub opacity: f32,
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
    pub z_index: usize,
}

impl PartialEq for WeakImage {
    fn eq(&self, other: &Self) -> bool {
        self.href == other.href
            && self.transform == other.transform
            && self.z_index == other.z_index
            && self.x == other.x
            && self.y == other.y
            && self.width == other.width
            && self.height == other.height
    }
}

impl WeakImage {
    pub fn transform(&mut self, transform: Transform) {
        if transform.is_identity() {
            return;
        }
        if let Some(view_box) = NonZeroRect::from_xywh(self.x, self.y, self.width, self.height) {
            if let Some(ts_view_box) = view_box.transform(transform) {
                self.x = ts_view_box.x();
                self.y = ts_view_box.y();
                self.width = ts_view_box.width();
                self.height = ts_view_box.height();
            }
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy)]
pub struct WeakTransform {
    pub sx: f32,
    pub kx: f32,
    pub ky: f32,
    pub sy: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Default for WeakTransform {
    fn default() -> Self {
        Self { sx: 1.0, kx: 0.0, ky: 0.0, sy: 1.0, tx: 0.0, ty: 0.0 }
    }
}

impl From<WeakTransform> for usvg::Transform {
    fn from(wt: WeakTransform) -> Self {
        usvg::Transform { sx: wt.sx, kx: wt.kx, ky: wt.ky, sy: wt.sy, tx: wt.tx, ty: wt.ty }
    }
}

impl From<usvg::Transform> for WeakTransform {
    fn from(t: usvg::Transform) -> Self {
        WeakTransform { sx: t.sx, kx: t.kx, ky: t.ky, sy: t.sy, tx: t.tx, ty: t.ty }
    }
}

impl Identifier for ManipulatorGroupId {
    fn new() -> Self {
        ManipulatorGroupId
    }
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ManipulatorGroupId;

impl EditableElement {
    pub fn opacity_changed(&self) -> bool {
        match self {
            EditableElement::Path(p) => p.diff_state.opacity_changed,
            EditableElement::Image(i) => i.diff_state.opacity_changed,
            EditableElement::Text(_) => todo!(),
        }
    }
    pub fn delete_changed(&self) -> bool {
        match self {
            EditableElement::Path(p) => p.diff_state.delete_changed,
            EditableElement::Image(i) => i.diff_state.delete_changed,
            EditableElement::Text(_) => todo!(),
        }
    }
    pub fn data_changed(&self) -> bool {
        match self {
            EditableElement::Path(p) => p.diff_state.data_changed,
            EditableElement::Image(i) => i.diff_state.data_changed,
            EditableElement::Text(_) => todo!(),
        }
    }
    pub fn mark_data_change(&mut self) {
        match self {
            EditableElement::Path(p) => p.diff_state.data_changed = true,
            EditableElement::Image(i) => i.diff_state.data_changed = true,
            EditableElement::Text(_) => todo!(),
        }
    }
    pub fn deleted(&self) -> bool {
        match self {
            EditableElement::Path(p) => p.deleted,
            EditableElement::Image(i) => i.deleted,
            EditableElement::Text(_) => todo!(),
        }
    }
    pub fn transformed(&self) -> Option<Transform> {
        match self {
            EditableElement::Path(p) => p.diff_state.transformed,
            EditableElement::Image(i) => i.diff_state.transformed,
            EditableElement::Text(_) => todo!(),
        }
    }
    pub fn transform(&mut self, transform: Transform) {
        match self {
            EditableElement::Path(path) => {
                path.diff_state.transformed = Some(transform);
                path.transform = path.transform.post_concat(transform);
                path.data.apply_transform(u_transform_to_bezier(&transform));
            }
            EditableElement::Image(img) => {
                img.diff_state.transformed = Some(transform);
                img.transform = img.transform.post_concat(transform);
                if let Some(new_vbox) = img.view_box.transform(transform) {
                    img.view_box = new_vbox;
                }
            }
            EditableElement::Text(_) => todo!(),
        }
    }

    pub fn get_transform(&self) -> Transform {
        match self {
            EditableElement::Path(path) => path.transform,
            EditableElement::Image(img) => img.transform,
            EditableElement::Text(_) => todo!(),
        }
    }

    pub fn opacity(&self) -> f32 {
        match self {
            EditableElement::Path(path) => path.stroke.map_or(1.0, |stroke| stroke.opacity),
            EditableElement::Image(image) => image.opacity,
            EditableElement::Text(_) => todo!(),
        }
    }

    pub fn set_opacity(&mut self, opacity: f32) {
        match self {
            EditableElement::Path(path) => {
                if let Some(stroke) = &mut path.stroke {
                    stroke.opacity = opacity;
                }
            }
            EditableElement::Image(image) => image.opacity = opacity,
            EditableElement::Text(_) => todo!(),
        }
        match self {
            EditableElement::Path(path) => path.diff_state.opacity_changed = true,
            EditableElement::Image(image) => image.diff_state.opacity_changed = true,
            EditableElement::Text(_) => todo!(),
        }
    }

    pub fn stroke(&self) -> Option<Stroke> {
        match self {
            EditableElement::Path(path) => path.stroke,
            EditableElement::Image(_) => None,
            EditableElement::Text(_) => todo!(),
        }
    }

    pub fn set_stroke(&mut self, stroke: Stroke) {
        match self {
            EditableElement::Path(path) => path.stroke = Some(stroke),
            EditableElement::Image(_) => {}
            EditableElement::Text(_) => {}
        }
        self.mark_data_change();
    }
}
