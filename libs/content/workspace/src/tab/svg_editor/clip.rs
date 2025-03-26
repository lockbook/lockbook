use lb_rs::{
    model::svg::{
        self,
        buffer::Buffer,
        diff::DiffState,
        element::{Element, Image},
    },
    Uuid,
};
use resvg::usvg::{NonZeroRect, Transform};

use crate::tab::{ClipContent, ExtendedInput as _};

use super::SVGEditor;

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
                            println!("{:#?}", pasted_buffer.elements.len());
                            for (id, el) in pasted_buffer.elements.iter() {
                                self.buffer
                                    .elements
                                    .insert_before(0, Uuid::new_v4(), el.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }
}
