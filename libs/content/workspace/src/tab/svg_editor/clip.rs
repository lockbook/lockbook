use lb_rs::{
    svg::{
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
    }
}
