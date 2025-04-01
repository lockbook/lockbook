use lb_rs::{
    model::svg::{
        self,
        buffer::{u_transform_to_bezier, Buffer},
        diff::DiffState,
        element::{Element, Image},
    },
    Uuid,
};
use resvg::usvg::{NonZeroRect, Transform};

use crate::tab::{svg_editor::element::BoundedElement, ClipContent, ExtendedInput as _};

use super::{selection::SelectedElement, InsertElement, SVGEditor, Tool};

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

                                let position = ui.input(|r| {
                                    r.pointer.hover_pos().unwrap_or(self.inner_rect.center())
                                });
                                self.buffer.elements.insert(
                                    Uuid::new_v4(),
                                    Element::Image(Image {
                                        data: resvg::usvg::ImageKind::PNG(data.into()),
                                        visibility: resvg::usvg::Visibility::Visible,
                                        transform: Transform::identity(),
                                        view_box: NonZeroRect::from_xywh(
                                            position.x,
                                            position.y,
                                            img.width() as f32,
                                            img.height() as f32,
                                        )
                                        .unwrap(),
                                        href: file.id,
                                        opacity: 1.0,
                                        diff_state: DiffState::new(),
                                        deleted: false,
                                    }),
                                );
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
                match event {
                    egui::Event::Paste(payload) => {
                        let pasted_buffer = Buffer::new(payload);
                        if !pasted_buffer.elements.is_empty()
                            || !pasted_buffer.weak_images.is_empty()
                        {
                            let mut container = egui::Rect::NOTHING;
                            for el in pasted_buffer.elements.iter() {
                                let child = el.1.bounding_box();

                                container.min.x = container.min.x.min(child.min.x);
                                container.min.y = container.min.y.min(child.min.y);

                                container.max.x = container.max.x.max(child.max.x);
                                container.max.y = container.max.y.max(child.max.y);
                            }

                            let mut new_ids =
                                Vec::with_capacity(pasted_buffer.elements.iter().count());

                            for (_, el) in pasted_buffer.elements.iter() {
                                let mut transformed_el = el.clone();
                                let center_pos = ui.available_rect_before_wrap().center();
                                let delta = r.pointer.hover_pos().unwrap_or(center_pos)
                                    - container.center() * self.buffer.master_transform.sx;

                                let transform = Transform::identity()
                                    .post_scale(
                                        self.buffer.master_transform.sx,
                                        self.buffer.master_transform.sy,
                                    )
                                    .post_translate(delta.x, delta.y);

                                match &mut transformed_el {
                                    Element::Path(path) => {
                                        path.data.apply_transform(u_transform_to_bezier(&transform))
                                    }
                                    Element::Image(image) => {
                                        if let Some(new_vbox) = image.view_box.transform(transform)
                                        {
                                            image.view_box = new_vbox;
                                        }
                                    }
                                    Element::Text(_) => todo!(),
                                }

                                let new_id = Uuid::new_v4();
                                self.buffer
                                    .elements
                                    .insert_before(0, new_id, transformed_el);
                                new_ids.push(new_id);
                            }

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
                    _ => {}
                }
            }
        });
    }
}
