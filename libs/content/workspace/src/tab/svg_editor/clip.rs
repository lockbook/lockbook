use lb_rs::{
    svg::{
        diff::DiffState,
        element::{Element, Image},
    },
    Uuid,
};
use resvg::usvg::{AspectRatio, NonZeroRect, Transform, ViewBox};

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
                                    crate::tab::import_image(&self.core, self.open_file, &data);
                                let href = crate::tab::core_get_relative_path(
                                    &self.core,
                                    self.open_file,
                                    file.id,
                                );
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
                                        view_box: ViewBox {
                                            rect: NonZeroRect::from_xywh(
                                                position.x,
                                                position.y,
                                                img.width() as f32,
                                                img.height() as f32,
                                            )
                                            .unwrap(),
                                            aspect: AspectRatio::default(),
                                        },
                                        href: Some(href),
                                        opacity: 1.0,
                                        diff_state: DiffState::default(),
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
