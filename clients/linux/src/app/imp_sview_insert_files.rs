use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

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
        {
            let tree = self.account.tree.clone();
            let errors = errors.clone();
            new_file_rx.attach(None, move |new_file| {
                if let Err(err) = tree.add_file(&new_file) {
                    errors.borrow_mut().push(err);
                }
                glib::Continue(true)
            });
        }

        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        let api = self.api.clone();
        std::thread::spawn(move || {
            // Get the parent id of the target file. The files will all be imported to this
            // directory.
            let parent_id = match api.file_by_id(target_file_id) {
                Ok(meta) => meta.parent,
                Err(err) => {
                    tx.send(Some(Err(format!("{:?}", err)))).unwrap();
                    tx.send(None).unwrap();
                    return;
                }
            };
            // Import each top-level file (with any children).
            for path in paths {
                let result = import_file_without_progress(&api, &path, parent_id, &new_file_tx);
                tx.send(Some(result)).unwrap();
            }
            tx.send(None).unwrap();
        });

        let buf = buf.clone();
        rx.attach(None, move |maybe_res: Option<Result<lb::FileMetadata, String>>| {
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
        let parent_id = match self.api.file_by_id(target_file_id) {
            Ok(meta) => meta.parent,
            Err(err) => {
                self.show_err_dialog(&format!("{:?}", err));
                return;
            }
        };

        // There's a bit of a chicken and egg situation when it comes to naming a new file based on
        // its id. First, we'll create a new file with a random (temporary) name.
        let tmp_name = format!("{}.png", lb::Uuid::new_v4());
        let mut png_meta = match self
            .api
            .create_file(&tmp_name, parent_id, lb::FileType::Document)
        {
            Ok(meta) => meta,
            Err(err) => {
                self.show_err_dialog(&format!("{:?}", err));
                return;
            }
        };

        // Then, the file is renamed to its id.
        let png_name = format!("{}.png", png_meta.id);
        if let Err(err) = self.api.rename_file(png_meta.id, &png_name) {
            self.show_err_dialog(&format!("{:?}", err));
            return;
        }
        png_meta.decrypted_name = png_name;

        // Convert the texture to PNG bytes and write them to the newly created lockbook file.
        let png_data = texture.save_to_png_bytes();
        if let Err(err) = self.api.write_document(png_meta.id, &png_data) {
            self.show_err_dialog(&format!("{:?}", err));
            return;
        }

        // Insert the new file entry in the tree and insert the markdown link at the cursor.
        if let Err(err) = self.account.tree.add_file(&png_meta) {
            self.show_err_dialog(&format!("{:?}", err));
            return;
        }
        buf.insert_at_cursor(&format!("[](lb://{})", png_meta.id));
    }
}

fn import_file_without_progress(
    api: &Arc<dyn lb::Api>, disk_path: &Path, dest: lb::Uuid,
    new_file_tx: &glib::Sender<lb::FileMetadata>,
) -> Result<lb::FileMetadata, String> {
    if !disk_path.exists() {
        return Err(format!("invalid disk path {:?}", disk_path));
    }

    let disk_file_name = disk_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(format!("invalid disk path {:?}", disk_path))?;

    let file_type = match disk_path.is_file() {
        true => lb::FileType::Document,
        false => lb::FileType::Folder,
    };

    let file_name = {
        let siblings = api.children(dest).map_err(|e| e.0)?;
        lb::get_non_conflicting_name(&siblings, disk_file_name)
    };

    let file_meta = api
        .create_file(&file_name, dest, file_type)
        .map_err(|e| format!("{:?}", e))?;
    new_file_tx.send(file_meta.clone()).unwrap();

    if file_type == lb::FileType::Document {
        let content = fs::read(&disk_path).map_err(|e| format!("{:?}", e))?;
        api.write_document(file_meta.id, &content)
            .map_err(|e| format!("{:?}", e))?;
    } else {
        let entries = fs::read_dir(disk_path).map_err(|e| format!("{:?}", e))?;
        for entry in entries {
            let child_path = entry.map_err(|e| format!("{:?}", e))?.path();
            import_file_without_progress(api, &child_path, file_meta.id, new_file_tx)
                .map_err(|e| format!("{:?}", e))?;
        }
    }

    Ok(file_meta)
}
