use indexmap::IndexMap;
use lb_rs::Uuid;
use lb_rs::model::svg::buffer::Buffer;
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{EditableElement, Image, WeakPathPressures};
use resvg::usvg::{NonZeroRect, Transform};

use crate::tab::svg_editor::history::History;
use crate::tab::svg_editor::tools::selection::SelectedElement;
use crate::tab::{ClipContent, ExtendedInput as _};

use super::coordinates::{
    CanvasCoordinates, DocumentRect, DocumentTransform, ScreenPoint, ScreenRect, ScreenTransform,
};
use super::element::PromoteBufferWeakImages;
use super::util::transform_rect;
use super::viewport::{calc_elements_bounds, get_rect_identity_transform};
use super::{InsertElement, SVGEditor, Tool};

impl SVGEditor {
    pub fn handle_clip_input(&mut self, ui: &mut egui::Ui) {
        for custom_event in ui.ctx().read_events() {
            match custom_event {
                crate::Event::Drop { content, .. } | crate::Event::Paste { content, .. } => {
                    for clip in content {
                        let (name, data) = match clip {
                            ClipContent::Image(data) => (None, data),
                            ClipContent::NamedImage { name, data } => (Some(name), data),
                            ClipContent::Files(..) => unimplemented!(), // todo: support file drop & paste
                        };
                        let Ok(href) = self.images.import(self.open_file, name.as_deref(), &data)
                        else {
                            continue;
                        };

                        let img = image::load_from_memory(&data).unwrap();

                        let paste_pos = ui.input(|r| {
                            r.pointer.hover_pos().unwrap_or(
                                self.input_ctx
                                    .last_touch
                                    .unwrap_or(ui.available_rect_before_wrap().center()),
                            )
                        });

                        let (fitted_img_rect, transform) = image_paste_placement(
                            self.viewport_settings.coordinates(),
                            ScreenRect(ui.available_rect_before_wrap()),
                            egui::vec2(img.width() as f32, img.height() as f32),
                            ScreenPoint(paste_pos),
                        );
                        let fitted_img_rect = fitted_img_rect.into_inner();

                        let id = Uuid::new_v4();
                        self.buffer.elements.insert(
                            id,
                            EditableElement::Image(Box::new(Image {
                                data: resvg::usvg::ImageKind::PNG(data.into()),
                                visible: true,
                                transform: transform.into_inner(),
                                view_box: NonZeroRect::from_xywh(
                                    fitted_img_rect.min.x,
                                    fitted_img_rect.min.y,
                                    fitted_img_rect.width(),
                                    fitted_img_rect.height(),
                                )
                                .unwrap(),
                                href,
                                opacity: 1.0,
                                diff_state: DiffState::new(),
                                deleted: false,
                            })),
                        );

                        // self.toolbar.active_tool = Tool::Selection;

                        // self.toolbar.selection.selected_elements =
                        //     vec![SelectedElement { id, transform: Transform::identity() }];
                    }
                }
                _ => {}
            }
        }
        ui.input(|r| {
            for event in &r.events {
                if let egui::Event::Paste(payload) = event {
                    if !payload.starts_with("<svg") {
                        continue;
                    }
                    let mut pasted_buffer = Buffer::new(payload);

                    pasted_buffer.promote_weak_images(&*self.images);

                    let paste_pos = r.pointer.hover_pos().unwrap_or(
                        self.input_ctx
                            .last_touch
                            .unwrap_or(ui.available_rect_before_wrap().center()),
                    );
                    let new_ids = paste_elements(
                        pasted_buffer,
                        &mut self.buffer,
                        &mut self.history,
                        self.viewport_settings.coordinates(),
                        ScreenPoint(paste_pos),
                    );
                    if !new_ids.is_empty() {
                        self.toolbar.active_tool = Tool::Selection;

                        self.toolbar.selection.selected_elements = new_ids
                            .iter()
                            .map(|&id| SelectedElement { id, transform: Default::default() })
                            .collect();
                    }
                }
            }
        });
    }
}

/// Raster imports retain their fitted screen size at the current zoom.
fn image_paste_placement(
    coordinates: CanvasCoordinates, container: ScreenRect, size: egui::Vec2, position: ScreenPoint,
) -> (DocumentRect, DocumentTransform) {
    let image_rect = egui::Rect::from_min_size(position.into_inner(), size);
    let transform = if container.into_inner().width() < size.x * 2.0
        || container.into_inner().height() < size.y * 2.0
    {
        get_rect_identity_transform(container.into_inner(), image_rect, 0.5, position.into_inner())
    } else {
        let delta = position.into_inner() - image_rect.center();
        Some(Transform::from_translate(delta.x, delta.y))
    }
    .unwrap_or_default();
    (
        coordinates.document_rect(ScreenRect(transform_rect(image_rect, transform))),
        coordinates.document_transform(ScreenTransform(transform)),
    )
}

/// SVG clipboard content retains its document size and is centered at the pointer.
fn paste_elements(
    source: Buffer, target: &mut Buffer, history: &mut History, coordinates: CanvasCoordinates,
    position: ScreenPoint,
) -> Vec<Uuid> {
    let Some(bounds) = calc_elements_bounds(&source) else { return Vec::new() };
    let delta = coordinates.document_point(position).into_inner() - bounds.center();
    duplicate_elements(
        source.elements,
        source.weak_path_pressures,
        target,
        history,
        Some(Transform::from_translate(delta.x, delta.y)),
    )
}

pub fn duplicate_elements(
    src_elements: IndexMap<Uuid, EditableElement>, src_pressure_map: WeakPathPressures,
    target: &mut Buffer, history: &mut History, maybe_transform: Option<Transform>,
) -> Vec<Uuid> {
    let mut new_ids = Vec::with_capacity(src_elements.iter().count());
    for (id, el) in src_elements.iter() {
        let new_id = Uuid::new_v4();
        let mut new_el = el.clone();

        if let EditableElement::Path(_) = el {
            let maybe_pressure = src_pressure_map.get(id).map(|vec| vec.to_owned());
            if let Some(pressures) = maybe_pressure {
                target.weak_path_pressures.insert(new_id, pressures);
            }
        }

        if let Some(transform) = maybe_transform {
            new_el.transform(transform);
        }

        new_el.mark_data_change();

        target.elements.insert_before(0, new_id, new_el);

        new_ids.push(new_id);
    }

    history.save(super::Event::Insert(
        src_elements
            .iter()
            .enumerate()
            .map(|(i, _)| InsertElement { id: *new_ids.get(i).unwrap_or(&Uuid::new_v4()) })
            .collect(),
    ));

    new_ids
}
