use resvg::usvg::{AspectRatio, NonZeroRect, Transform, ViewBox};

use crate::tab::{self, ClipContent, EventManager as _};

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

                                let img = image::load_from_memory(&data).unwrap();

                                let position = ui.input(|r| {
                                    r.pointer.hover_pos().unwrap_or(self.inner_rect.center())
                                });
                                self.buffer.elements.insert(
                                    self.toolbar.pen.current_id.to_string(),
                                    crate::tab::svg_editor::parser::Element::Image(
                                        crate::tab::svg_editor::parser::Image {
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
                                            texture: None,
                                            href: Some(file.id),
                                            opacity: 1.0,
                                        },
                                    ),
                                );
                                self.toolbar.pen.current_id += 1;
                            }
                            ClipContent::Files(..) => unimplemented!(), // todo: support file drop & paste
                        }
                    }
                }
                crate::Event::Markdown(..) => {}
            }
        }
    }
}
