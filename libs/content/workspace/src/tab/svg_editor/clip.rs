use indexmap::IndexMap;
use lb_rs::Uuid;
use lb_rs::model::svg::buffer::{Buffer, u_transform_to_bezier};
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{Element, Image, WeakPathPressures};
use resvg::usvg::{NonZeroRect, Transform};

use crate::tab::svg_editor::element::BoundedElement;
use crate::tab::svg_editor::history::History;
use crate::tab::svg_editor::tools::selection::SelectedElement;
use crate::tab::{ClipContent, ExtendedInput as _};

use super::element::PromoteBufferWeakImages;
use super::viewport::get_rect_identity_transform;
// use super::tools::selection::SelectedElement;
use super::util::transform_rect;
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
                            ClipContent::Files(..) => continue, // same unsupported-content behavior as Markdown
                        };
                        let imported = match crate::image_import::import(
                            &self.images,
                            &self.lb,
                            self.open_file,
                            name.as_deref(),
                            &data,
                        ) {
                            Ok(imported) => imported,
                            Err(error) => {
                                self.failure_messages.push(error);
                                continue;
                            }
                        };

                        let paste_pos = ui.input(|r| {
                            r.pointer.hover_pos().unwrap_or(
                                self.input_ctx
                                    .last_touch
                                    .unwrap_or(ui.available_rect_before_wrap().center()),
                            )
                        });

                        let img_rect = egui::Rect::from_min_size(paste_pos, imported.size);

                        let transform = if ui.available_rect_before_wrap().width()
                            < img_rect.width() * 2.0
                            || ui.available_rect_before_wrap().height() < img_rect.height() * 2.0
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

                        let Some(view_box) = NonZeroRect::from_xywh(
                            fitted_img_rect.min.x,
                            fitted_img_rect.min.y,
                            fitted_img_rect.width(),
                            fitted_img_rect.height(),
                        ) else {
                            self.failure_messages
                                .push("Could not place image in the canvas".into());
                            continue;
                        };
                        let id = Uuid::new_v4();
                        self.buffer.elements.insert_before(
                            0,
                            id,
                            Element::Image(Box::new(Image {
                                visible: true,
                                transform,
                                view_box,
                                href: imported.file.id,
                                opacity: 1.0,
                                diff_state: DiffState::new(),
                                deleted: false,
                            })),
                        );

                        self.buffer
                            .image_hrefs
                            .insert(imported.file.id, imported.destination);
                        self.history
                            .save(super::Event::Insert(vec![InsertElement { id }]));
                        self.toolbar.active_tool = Tool::Selection;
                        self.toolbar.selection.selected_elements =
                            vec![SelectedElement { id, transform: Transform::identity() }];
                        self.has_queued_save_request = true;
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

                    pasted_buffer.promote_weak_images(Transform::identity());

                    if !pasted_buffer.elements.is_empty() {
                        self.buffer
                            .image_hrefs
                            .extend(pasted_buffer.image_hrefs.clone());
                        let mut container = egui::Rect::NOTHING;
                        for el in pasted_buffer.elements.iter() {
                            let child = el.1.bounding_box();

                            container.min.x = container.min.x.min(child.min.x);
                            container.min.y = container.min.y.min(child.min.y);

                            container.max.x = container.max.x.max(child.max.x);
                            container.max.y = container.max.y.max(child.max.y);
                        }

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

                        let new_ids = duplicate_elements(
                            pasted_buffer.elements,
                            pasted_buffer.weak_path_pressures,
                            &mut self.buffer,
                            &mut self.history,
                            Some(transform),
                        );

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

pub fn duplicate_elements(
    src_elements: IndexMap<Uuid, Element>, src_pressure_map: WeakPathPressures,
    target: &mut Buffer, history: &mut History, maybe_transform: Option<Transform>,
) -> Vec<Uuid> {
    let mut new_ids = Vec::with_capacity(src_elements.len());
    let mut new_elements = Vec::with_capacity(src_elements.len());
    for (id, el) in src_elements.iter() {
        let new_id = Uuid::new_v4();
        let mut new_el = el.clone();

        if let Element::Path(_) = el {
            let maybe_pressure = src_pressure_map.get(id).map(|vec| vec.to_owned());
            if let Some(pressures) = maybe_pressure {
                target.weak_path_pressures.insert(new_id, pressures);
            }
        }

        if let Some(transform) = maybe_transform {
            match &mut new_el {
                Element::Path(path) => {
                    path.data.apply_transform(u_transform_to_bezier(&transform));
                    path.diff_state = DiffState::new();
                }
                Element::Image(image) => {
                    if let Some(new_vbox) = image.view_box.transform(transform) {
                        image.view_box = new_vbox;
                    };
                    image.diff_state = DiffState::new();
                }
                Element::Text(_) => {}
            }
        }

        new_elements.push((new_id, new_el));

        new_ids.push(new_id);
    }

    for (id, element) in new_elements.into_iter().rev() {
        target.elements.insert_before(0, id, element);
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
