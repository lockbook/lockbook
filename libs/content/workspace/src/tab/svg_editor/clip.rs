use resvg::usvg::{AspectRatio, NonZeroRect, Transform, ViewBox};

use crate::tab::{ClipContent, EventManager as _};

use super::SVGEditor;

impl SVGEditor {
    pub fn handle_clip_input(&mut self, ui: &mut egui::Ui) {
        for custom_event in ui.ctx().pop_events() {
            match custom_event {
                crate::Event::Drop { content, .. } | crate::Event::Paste { content, .. } => {
                    for clip in content {
                        match clip {
                            ClipContent::Png(data) => {
                                let file =
                                    crate::tab::import_image(&self.core, self.open_file, &data);
                                // let image_href = format!("lb://{}", file.id);

                                let bytes = self.core.read_document(file.id).unwrap();
                                self.buffer.elements.insert(
                                    self.toolbar.pen.current_id.to_string(),
                                    crate::tab::svg_editor::parser::Element::Image(
                                        crate::tab::svg_editor::parser::Image {
                                            data: resvg::usvg::ImageKind::PNG(bytes.into()),
                                            visibility: resvg::usvg::Visibility::Visible,
                                            transform: Transform::identity(),
                                            view_box: ViewBox {
                                                rect: NonZeroRect::from_ltrb(
                                                    ui.available_rect_before_wrap().left(),
                                                    ui.available_rect_before_wrap().top(),
                                                    ui.available_rect_before_wrap().right(),
                                                    ui.available_rect_before_wrap().bottom(),
                                                )
                                                .unwrap(),
                                                aspect: AspectRatio::default(),
                                            },
                                            texture: None,
                                        },
                                    ),
                                );
                                self.toolbar.pen.current_id += 1;
                                println!("pasted image: {:?} bytes", data.len());
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
