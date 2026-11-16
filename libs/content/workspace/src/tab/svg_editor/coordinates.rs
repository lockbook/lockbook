//! Coordinate conversions and document queries shared by tools and rendering.

use bezier_rs::Subpath;
use glam::DVec2;
use lb_rs::Uuid;
use lb_rs::model::svg::element::{EditableElement, ManipulatorGroupId};
use resvg::usvg::Transform;

use super::Buffer;
use super::element::BoundedElement;

use super::util::{pointer_intersects_element, transform_point, transform_rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentPoint(pub egui::Pos2);

impl DocumentPoint {
    pub const fn into_inner(self) -> egui::Pos2 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenPoint(pub egui::Pos2);

impl ScreenPoint {
    pub const fn into_inner(self) -> egui::Pos2 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentRect(pub egui::Rect);

impl DocumentRect {
    pub const fn into_inner(self) -> egui::Rect {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRect(pub egui::Rect);

impl ScreenRect {
    pub const fn into_inner(self) -> egui::Rect {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentLength(pub f32);

impl DocumentLength {
    pub const fn into_inner(self) -> f32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenLength(pub f32);

impl ScreenLength {
    pub const fn into_inner(self) -> f32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentVector(pub egui::Vec2);

impl DocumentVector {
    pub const fn into_inner(self) -> egui::Vec2 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenVector(pub egui::Vec2);

impl ScreenVector {
    pub const fn into_inner(self) -> egui::Vec2 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentTransform(pub Transform);

impl DocumentTransform {
    pub const fn into_inner(self) -> Transform {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenTransform(pub Transform);

impl ScreenTransform {
    pub const fn into_inner(self) -> Transform {
        self.0
    }
}

/// A snapshot of one view's document-to-screen mapping, independent of a buffer.
/// Obtain a fresh context after changing the viewport.
#[derive(Clone, Copy, Debug)]
pub struct CanvasCoordinates {
    to_screen: Transform,
    to_document: Transform,
}

impl CanvasCoordinates {
    pub fn new(to_screen: Transform) -> Self {
        Self { to_screen, to_document: to_screen.invert().unwrap_or_default() }
    }

    pub fn zoom(self) -> f32 {
        self.to_screen.sx.hypot(self.to_screen.ky)
    }

    pub fn screen_point(self, point: DocumentPoint) -> ScreenPoint {
        ScreenPoint(transform_point(point.into_inner(), self.to_screen))
    }

    pub fn document_point(self, point: ScreenPoint) -> DocumentPoint {
        DocumentPoint(transform_point(point.into_inner(), self.to_document))
    }

    pub fn screen_rect(self, rect: DocumentRect) -> ScreenRect {
        ScreenRect(transform_rect(rect.into_inner(), self.to_screen))
    }

    pub fn document_rect(self, rect: ScreenRect) -> DocumentRect {
        DocumentRect(transform_rect(rect.into_inner(), self.to_document))
    }

    // Canvas views use uniform zoom; lengths are independent of translation.
    pub fn screen_length(self, length: DocumentLength) -> ScreenLength {
        ScreenLength(length.into_inner() * self.zoom())
    }

    pub fn document_length(self, length: ScreenLength) -> DocumentLength {
        DocumentLength(length.into_inner() / self.zoom())
    }

    pub fn screen_vector(self, vector: DocumentVector) -> ScreenVector {
        ScreenVector(transform_vector(vector.into_inner(), self.to_screen))
    }

    pub fn document_vector(self, vector: ScreenVector) -> DocumentVector {
        DocumentVector(transform_vector(vector.into_inner(), self.to_document))
    }

    /// Express a screen-space edit in document space, for geometry and history.
    pub fn document_transform(self, transform: ScreenTransform) -> DocumentTransform {
        DocumentTransform(
            self.to_screen
                .post_concat(transform.into_inner())
                .post_concat(self.to_document),
        )
    }

    /// Project a copy for painting, leaving the document-space mesh cache intact.
    pub fn screen_mesh(self, mesh: &egui::Mesh) -> egui::Mesh {
        let mut mesh = mesh.clone();
        for vertex in &mut mesh.vertices {
            vertex.pos = self.screen_point(DocumentPoint(vertex.pos)).into_inner();
        }
        mesh
    }
}

fn transform_vector(vector: egui::Vec2, transform: Transform) -> egui::Vec2 {
    egui::vec2(
        transform.sx * vector.x + transform.kx * vector.y,
        transform.ky * vector.x + transform.sy * vector.y,
    )
}

/// A short-lived read context. Each canvas or minimap can borrow the same
/// document with its own viewport; no geometry is copied or cached here.
pub struct CanvasView<'a> {
    buffer: &'a Buffer,
    coordinates: CanvasCoordinates,
}

impl<'a> CanvasView<'a> {
    pub fn new(buffer: &'a Buffer, coordinates: CanvasCoordinates) -> Self {
        Self { buffer, coordinates }
    }

    pub fn document(&self) -> &'a Buffer {
        self.buffer
    }

    pub fn coordinates(&self) -> CanvasCoordinates {
        self.coordinates
    }

    pub fn screen_bounds(&self, id: Uuid) -> Option<egui::Rect> {
        let element = self.buffer.elements.get(&id).filter(|el| !el.deleted())?;
        Some(
            self.coordinates
                .screen_rect(DocumentRect(element.bounding_box()))
                .into_inner(),
        )
    }

    pub fn screen_bounds_for(&self, ids: impl IntoIterator<Item = Uuid>) -> egui::Rect {
        ids.into_iter()
            .filter_map(|id| self.screen_bounds(id))
            .fold(egui::Rect::NOTHING, |bounds, rect| bounds.union(rect))
    }

    /// Returns the topmost matching element in document paint order.
    pub fn hit_test(&self, point: ScreenPoint, radius: ScreenLength) -> Option<Uuid> {
        self.hit_test_all(point, radius).next()
    }

    pub fn hit_test_all(
        &self, point: ScreenPoint, radius: ScreenLength,
    ) -> impl Iterator<Item = Uuid> + '_ {
        let point = self.coordinates.document_point(point).into_inner();
        let radius = self.coordinates.document_length(radius).into_inner() as f64;
        self.buffer
            .elements
            .iter()
            .filter_map(move |(&id, element)| {
                (!element.deleted() && pointer_intersects_element(element, point, None, radius))
                    .then_some(id)
            })
    }

    /// Lasso selection: intersect the path outline or contain the whole path.
    pub fn elements_in_screen_rect(&self, rect: egui::Rect) -> impl Iterator<Item = Uuid> + '_ {
        let rect = self
            .coordinates
            .document_rect(ScreenRect(rect))
            .into_inner();
        let outline: Subpath<ManipulatorGroupId> = Subpath::new_rect(
            DVec2::new(rect.min.x as f64, rect.min.y as f64),
            DVec2::new(rect.max.x as f64, rect.max.y as f64),
        );
        self.buffer
            .elements
            .iter()
            .filter_map(move |(&id, element)| {
                if element.deleted() {
                    return None;
                }
                let bounds = element.bounding_box();
                let intersects = match element {
                    EditableElement::Path(path) => {
                        rect.intersects(bounds)
                            && (rect.contains_rect(bounds)
                                || !path
                                    .data
                                    .subpath_intersections(&outline, None, None)
                                    .is_empty())
                    }
                    EditableElement::Image(_) => {
                        rect.contains_rect(bounds) || rect.intersects(bounds)
                    }
                    EditableElement::Text(_) => todo!(),
                };
                intersects.then_some(id)
            })
    }
}
