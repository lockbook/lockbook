use minidom::Element;

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
                                let image_href = format!("lb://{}", file.id);

                                let child = Element::builder("image", "")
                                    .attr("id", self.toolbar.pen.current_id)
                                    .attr("href", image_href)
                                    .build();

                                self.buffer.current.append_child(child);
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
