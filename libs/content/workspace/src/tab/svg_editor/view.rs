//! Read document geometry through a particular viewport without changing it.

use bezier_rs::Subpath;
use glam::DVec2;
use lb_rs::Uuid;
use lb_rs::model::svg::element::{EditableElement, ManipulatorGroupId};

use super::Buffer;
use super::coordinates::{CanvasCoordinates, DocumentRect, ScreenLength, ScreenPoint, ScreenRect};
use super::element::BoundedElement;
use super::util::pointer_intersects_element;

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
                .0,
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
        let point = self.coordinates.document_point(point).0;
        let radius = self.coordinates.document_length(radius).0 as f64;
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
        let rect = self.coordinates.document_rect(ScreenRect(rect)).0;
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
