use image::ImageEncoder;
use lb::FileType;
use lbeguiapp::WgpuLockbook;
use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn handle(app: &mut WgpuLockbook) {
    // somewhat weird that app.from_host isn't involved here
    if let Ok(unicode) = clipboard_win::get_clipboard(clipboard_win::formats::Unicode) {
        app.raw_input.events.push(egui::Event::Paste(unicode));
    }
    if let Ok(bitmap) = clipboard_win::get_clipboard(clipboard_win::formats::Bitmap) {
        let bitmap: image::DynamicImage =
            image::load_from_memory(&bitmap).expect("load image from memory");
        let mut png_bytes = Vec::new();
        image::codecs::png::PngEncoder::new(Cursor::new(&mut png_bytes))
            .write_image(&bitmap.as_bytes(), bitmap.width(), bitmap.height(), bitmap.color())
            .expect("png encode pasted image");

        // todo: this certainly doesn't belong here
        // but also, what is this data modeling?
        let core = match &app.app {
            lbeguiapp::Lockbook::Splash(_) => {
                return;
            }
            lbeguiapp::Lockbook::Onboard(screen) => &screen.core,
            lbeguiapp::Lockbook::Account(screen) => &screen.core,
        };

        // todo: better filename
        // todo: use currently open folder
        // todo: use background thread
        // todo: refresh file tree view
        let file = core
            .create_file(
                &format!(
                    "pasted_image_{}.png",
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_micros()
                ),
                core.get_root().expect("get lockbook root").id,
                FileType::Document,
            )
            .expect("create lockbook file for image");
        core.write_document(file.id, &png_bytes)
            .expect("write lockbook file for image");

        let markdown_image_link = format!("![pasted image](lb://{})", file.id);
        app.raw_input
            .events
            .push(egui::Event::Paste(markdown_image_link));
    }
}
