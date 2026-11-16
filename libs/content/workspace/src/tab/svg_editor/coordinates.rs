//! Coordinate conversions shared by tools, queries, clipboard, and rendering.

use resvg::usvg::Transform;

use super::util::{transform_point, transform_rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentPoint(pub egui::Pos2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenPoint(pub egui::Pos2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentRect(pub egui::Rect);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRect(pub egui::Rect);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentLength(pub f32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenLength(pub f32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentVector(pub egui::Vec2);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenVector(pub egui::Vec2);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentTransform(pub Transform);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenTransform(pub Transform);

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
        ScreenPoint(transform_point(point.0, self.to_screen))
    }

    pub fn document_point(self, point: ScreenPoint) -> DocumentPoint {
        DocumentPoint(transform_point(point.0, self.to_document))
    }

    pub fn screen_rect(self, rect: DocumentRect) -> ScreenRect {
        ScreenRect(transform_rect(rect.0, self.to_screen))
    }

    pub fn document_rect(self, rect: ScreenRect) -> DocumentRect {
        DocumentRect(transform_rect(rect.0, self.to_document))
    }

    // Canvas views use uniform zoom; lengths are independent of translation.
    pub fn screen_length(self, length: DocumentLength) -> ScreenLength {
        ScreenLength(length.0 * self.zoom())
    }

    pub fn document_length(self, length: ScreenLength) -> DocumentLength {
        DocumentLength(length.0 / self.zoom())
    }

    pub fn screen_vector(self, vector: DocumentVector) -> ScreenVector {
        ScreenVector(transform_vector(vector.0, self.to_screen))
    }

    pub fn document_vector(self, vector: ScreenVector) -> DocumentVector {
        DocumentVector(transform_vector(vector.0, self.to_document))
    }

    /// Express a screen-space edit in document space, for geometry and history.
    pub fn document_transform(self, transform: ScreenTransform) -> DocumentTransform {
        DocumentTransform(
            self.to_screen
                .post_concat(transform.0)
                .post_concat(self.to_document),
        )
    }

    /// Project a copy for painting, leaving the document-space mesh cache intact.
    pub fn screen_mesh(self, mesh: &egui::Mesh) -> egui::Mesh {
        let mut mesh = mesh.clone();
        for vertex in &mut mesh.vertices {
            vertex.pos = self.screen_point(DocumentPoint(vertex.pos)).0;
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
