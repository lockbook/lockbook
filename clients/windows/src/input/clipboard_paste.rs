use egui::Pos2;
use image::ImageEncoder;
use lbeguiapp::WgpuLockbook;
use std::io::Cursor;
use workspace_rs::tab::{ClipContent, ExtendedInput as _};

pub fn handle(app: &mut WgpuLockbook) {
    if let Ok(unicode) = clipboard_win::get_clipboard(clipboard_win::formats::Unicode) {
        app.renderer
            .raw_input
            .events
            .push(egui::Event::Paste(unicode));
    }
    if let Ok(bitmap) = clipboard_win::get_clipboard(clipboard_win::formats::Bitmap) {
        let bitmap: image::DynamicImage =
            image::load_from_memory(&bitmap).expect("load image from memory");
        let mut png_bytes = Vec::new();
        image::codecs::png::PngEncoder::new(Cursor::new(&mut png_bytes))
            .write_image(bitmap.as_bytes(), bitmap.width(), bitmap.height(), bitmap.color())
            .expect("png encode pasted image");

        app.renderer.context.push_event(workspace_rs::Event::Paste {
            content: vec![ClipContent::Image(png_bytes)],
            position: Pos2::ZERO, // todo: support position
        });
    }
}
