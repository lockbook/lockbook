use crate::tab::{ClipContent, CustomEventer as _};

use super::Buffer;

pub fn handle_clip_input(ui: &mut egui::Ui, _buffer: &mut Buffer) {
    for custom_event in ui.ctx().pop_custom_events() {
        match custom_event {
            crate::Event::Drop { content, .. } | crate::Event::Paste { content, .. } => {
                for clip in content {
                    match clip {
                        ClipContent::Png(data) => {
                            // todo: make necessary changes to the buffer
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
