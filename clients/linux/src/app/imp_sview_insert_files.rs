use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

use crate::lbutil;

impl super::App {
    pub fn sview_insert_file_list(
        &self, target_file_id: lb::Uuid, buf: &sv5::Buffer, flist: gdk::FileList,
    ) {
        let paths = flist
            .files()
            .iter()
            .map(|f| f.path().unwrap())
            .collect::<Vec<PathBuf>>();

        let caption = gtk::Label::new(Some(&format!("Importing {} files...", paths.len())));

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .margin_top(16)
            .margin_start(16)
            .margin_end(16)
            .margin_bottom(16)
            .spacing(16)
            .build();
        content.append(&gtk::Spinner::builder().spinning(true).build());
        content.append(&caption);

        let dialog = gtk::Dialog::builder()
            .transient_for(&self.window)
            .deletable(false)
            .modal(true)
            .title(". . .")
            .build();
        dialog.content_area().append(&content);
        dialog.show();

        let errors = Rc::new(RefCell::new(Vec::new()));

        // Setup a separate receiver to add file entries to the tree as they are created via import.
        let (new_file_tx, new_file_rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        new_file_rx.attach(None, {
            let tree = self.account.tree.clone();
            let errors = errors.clone();

            move |new_file| {
                if let Err(err) = tree.add_file(&new_file) {
                    errors.borrow_mut().push(err);
                }
                glib::Continue(true)
            }
        });

        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        let core = self.core.clone();
        std::thread::spawn(move || {
            // Get the parent id of the target file. The files will all be imported to this
            // directory.
            let parent_id = match core.get_file_by_id(target_file_id) {
                Ok(meta) => meta.parent,
                Err(err) => {
                    tx.send(Some(Err(format!("{:?}", err)))).unwrap();
                    tx.send(None).unwrap();
                    return;
                }
            };
            // Import each top-level file (with any children).
            for path in paths {
                let result = lbutil::import_file(&core, &path, parent_id, &new_file_tx);
                tx.send(Some(result)).unwrap();
            }
            tx.send(None).unwrap();
        });

        let buf = buf.clone();
        rx.attach(None, move |maybe_res: Option<Result<lb::DecryptedFileMetadata, String>>| {
            match maybe_res {
                Some(res) => match res {
                    Ok(new_file) => {
                        let md_link =
                            format!("[{}](lb://{})", new_file.decrypted_name, new_file.id);
                        buf.insert_at_cursor(&md_link);
                    }
                    Err(err) => errors.borrow_mut().push(err),
                },
                None => match errors.borrow().len() {
                    0 => dialog.close(),
                    _ => {
                        let err_list = gtk::Box::new(gtk::Orientation::Vertical, 8);
                        for err in errors.borrow().iter() {
                            err_list.append(&gtk::Label::new(Some(err)));
                        }
                        dialog.content_area().remove(&content);
                        dialog.content_area().append(&err_list);
                        dialog.set_deletable(true);
                        dialog.add_button("Close", gtk::ResponseType::Cancel);
                    }
                },
            }
            glib::Continue(true)
        });
    }

    pub fn sview_insert_texture(
        &self, target_file_id: lb::Uuid, buf: &sv5::Buffer, texture: gdk::Texture,
    ) {
        // Get the parent id of the target file. The image will be inserted under the same
        // directory.
        let parent_id = match self.core.get_file_by_id(target_file_id) {
            Ok(meta) => meta.parent,
            Err(err) => {
                self.show_err_dialog(&format!("{:?}", err));
                return;
            }
        };

        let png_meta = match lbutil::save_texture_to_png(&self.core, parent_id, texture) {
            Ok(meta) => meta,
            Err(err) => {
                self.show_err_dialog(&err);
                return;
            }
        };

        // Insert the new file entry in the tree and insert the markdown link at the cursor.
        if let Err(err) = self.account.tree.add_file(&png_meta) {
            self.show_err_dialog(&format!("{:?}", err));
            return;
        }
        buf.insert_at_cursor(&format!("[](lb://{})", png_meta.id));
    }
}
