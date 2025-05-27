use lb_rs::{
    model::svg::{
        buffer::{u_transform_to_bezier, Buffer},
        diff::DiffState,
        element::{Element, Image},
    },
    Uuid,
};
use resvg::usvg::{NonZeroRect, Transform};

use crate::tab::{svg_editor::element::BoundedElement, ClipContent, ExtendedInput as _};

use super::{
    gesture_handler::get_rect_identity_transform, selection::SelectedElement, util::transform_rect,
    InsertElement, SVGEditor, Tool,
};

impl SVGEditor {
    pub fn handle_clip_input(&mut self, ui: &mut egui::Ui) {
        for custom_event in ui.ctx().pop_events() {
            match custom_event {
                crate::Event::Drop { content, .. } | crate::Event::Paste { content, .. } => {
                    for clip in content {
                        match clip {
                            ClipContent::Image(data) => {
                                let file =
                                    crate::tab::import_image(&self.lb, self.open_file, &data);

                                let img = image::load_from_memory(&data).unwrap();

                                let paste_pos = ui.input(|r| {
                                    r.pointer.hover_pos().unwrap_or(
                                        self.input_ctx
                                            .last_touch
                                            .unwrap_or(ui.available_rect_before_wrap().center()),
                                    )
                                });

                                let img_rect = egui::Rect::from_min_size(
                                    paste_pos,
                                    egui::vec2(img.width() as f32, img.height() as f32),
                                );

                                let transform = if ui.available_rect_before_wrap().width()
                                    < img_rect.width() * 2.0
                                    || ui.available_rect_before_wrap().height()
                                        < img_rect.height() * 2.0
                                {
                                    get_rect_identity_transform(
                                        ui.available_rect_before_wrap(),
                                        img_rect,
                                        0.5,
                                        paste_pos,
                                    )
                                } else {
                                    Some(Transform::identity().post_translate(
                                        paste_pos.x - img_rect.center().x,
                                        paste_pos.y - img_rect.center().y,
                                    ))
                                }
                                .unwrap_or_default();

                                let fitted_img_rect = transform_rect(img_rect, transform);

                                let id = Uuid::new_v4();
                                self.buffer.elements.insert(
                                    id,
                                    Element::Image(Image {
                                        data: resvg::usvg::ImageKind::PNG(data.into()),
                                        visibility: resvg::usvg::Visibility::Visible,
                                        transform,
                                        view_box: NonZeroRect::from_xywh(
                                            fitted_img_rect.min.x,
                                            fitted_img_rect.min.y,
                                            fitted_img_rect.width(),
                                            fitted_img_rect.height(),
                                        )
                                        .unwrap(),
                                        href: file.id,
                                        opacity: 1.0,
                                        diff_state: DiffState::new(),
                                        deleted: false,
                                    }),
                                );

                                self.toolbar.active_tool = Tool::Selection;

                                self.toolbar.selection.selected_elements =
                                    vec![SelectedElement { id, transform: Transform::identity() }];
                            }
                            ClipContent::Files(..) => unimplemented!(), // todo: support file drop & paste
                        }
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
                    let pasted_buffer = Buffer::new(payload);
                    if !pasted_buffer.elements.is_empty() || !pasted_buffer.weak_images.is_empty() {
                        let mut container = egui::Rect::NOTHING;
                        for el in pasted_buffer.elements.iter() {
                            let child = el.1.bounding_box();

                            container.min.x = container.min.x.min(child.min.x);
                            container.min.y = container.min.y.min(child.min.y);

                            container.max.x = container.max.x.max(child.max.x);
                            container.max.y = container.max.y.max(child.max.y);
                        }

                        let mut new_ids = Vec::with_capacity(pasted_buffer.elements.iter().count());

                        for (id, el) in pasted_buffer.elements.iter() {
                            let new_id = Uuid::new_v4();

                            let mut transformed_el = el.clone();

                            let paste_pos = r.pointer.hover_pos().unwrap_or(
                                self.input_ctx
                                    .last_touch
                                    .unwrap_or(ui.available_rect_before_wrap().center()),
                            );

                            let delta = paste_pos
                                - container.center() * self.viewport_settings.master_transform.sx;

                            let transform = Transform::identity()
                                .post_scale(
                                    self.viewport_settings.master_transform.sx,
                                    self.viewport_settings.master_transform.sy,
                                )
                                .post_translate(delta.x, delta.y);

                            match &mut transformed_el {
                                Element::Path(path) => {
                                    path.data.apply_transform(u_transform_to_bezier(&transform));

                                    let maybe_pressure = self
                                        .buffer
                                        .weak_path_pressures
                                        .get(id)
                                        .map(|vec| vec.to_owned());

                                    if let Some(pressures) = maybe_pressure {
                                        self.buffer.weak_path_pressures.insert(new_id, pressures);
                                    }
                                }
                                Element::Image(image) => {
                                    if let Some(new_vbox) = image.view_box.transform(transform) {
                                        image.view_box = new_vbox;
                                    }
                                }
                                Element::Text(_) => todo!(),
                            }

                            self.buffer
                                .elements
                                .insert_before(0, new_id, transformed_el);
                            new_ids.push(new_id);
                        }

                        //todo: support images

                        self.history.save(super::Event::Insert(
                            pasted_buffer
                                .elements
                                .iter()
                                .enumerate()
                                .map(|(i, _)| InsertElement {
                                    id: *new_ids.get(i).unwrap_or(&Uuid::new_v4()),
                                })
                                .collect(),
                        ));

                        self.toolbar.active_tool = Tool::Selection;

                        self.toolbar.selection.selected_elements = pasted_buffer
                            .elements
                            .iter()
                            .enumerate()
                            .map(|(i, _)| SelectedElement {
                                id: *new_ids.get(i).unwrap(),
                                transform: Default::default(),
                            })
                            .collect();
                    }
                }
            }
        });
    }
}
