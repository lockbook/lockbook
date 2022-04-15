use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

use crate::lbutil;

impl super::App {
    pub fn cut_selected_files(&self) {
        let t = &self.account.tree;

        let entries = t.get_selected_ids();
        if entries.len() != 1 {
            t.show_msg("A single file must be selected in order to cut.");
            return;
        }

        let selected_id = *entries.get(0).unwrap();
        let lb_uri = format!("lb://{}", selected_id);
        gtk::gdk::Display::default()
            .unwrap()
            .clipboard()
            .set_text(&lb_uri);
        t.show_msg("File cut!");
    }

    pub fn copy_selected_files(&self) {
        self.account
            .tree
            .show_msg("Copying files is currently unsupported.");
    }

    pub fn paste_into_selected_file(&self) {
        let t = &self.account.tree;

        let entries = t.get_selected_ids();
        if entries.len() != 1 {
            t.show_msg("A single file must be selected in order to paste.");
            return;
        }

        let selected_id = *entries.get(0).unwrap();
        let dest_id = match self.api.file_by_id(selected_id) {
            Ok(meta) => match meta.file_type {
                lb::FileType::Document => meta.parent,
                lb::FileType::Folder => meta.id,
            },
            Err(err) => {
                self.show_err_dialog(&format!("{:?}", err));
                return;
            }
        };

        // First, check if there's an image being pasted.
        gdk::Display::default()
            .unwrap()
            .clipboard()
            .read_texture_async(None::<gio::Cancellable>.as_ref(), {
                let app = self.clone();

                move |res| match res {
                    Ok(Some(texture)) => app.import_texture(dest_id, texture),
                    _ => app.try_pasting_file_list(dest_id),
                }
            });
    }

    fn try_pasting_file_list(&self, dest_id: lb::Uuid) {
        gdk::Display::default()
            .unwrap()
            .clipboard()
            .read_value_async(
                gdk::FileList::static_type(),
                glib::PRIORITY_DEFAULT,
                None::<gio::Cancellable>.as_ref(),
                {
                    let app = self.clone();

                    move |res| {
                        if let Ok(value) = res {
                            if let Ok(flist) = value.get::<gdk::FileList>() {
                                app.import_file_list(flist, dest_id);
                            }
                            return;
                        }
                        app.try_pasting_uris(dest_id);
                    }
                },
            );
    }

    fn try_pasting_uris(&self, dest_id: lb::Uuid) {
        gdk::Display::default()
            .unwrap()
            .clipboard()
            .read_text_async(None::<gio::Cancellable>.as_ref(), {
                let app = self.clone();

                move |res| match res {
                    Ok(maybe_str) => app.parse_clipboard_content(dest_id, maybe_str),
                    Err(err) => app.show_err_dialog(&format!("Failed to read clipboard: {}", err)),
                }
            });
    }

    fn import_texture(&self, dest_id: lb::Uuid, texture: gdk::Texture) {
        let png_meta = match lbutil::save_texture_to_png(&self.api, dest_id, texture) {
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
    }

    fn import_file_list(&self, flist: gdk::FileList, dest_id: lb::Uuid) {
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

        let api = self.api.clone();
        std::thread::spawn(move || {
            // Import each top-level file (with any children).
            for path in paths {
                let result =
                    lbutil::import_file_without_progress(&api, &path, dest_id, &new_file_tx);
                tx.send(Some(result)).unwrap();
            }
            tx.send(None).unwrap();
        });

        rx.attach(None, move |maybe_res: Option<Result<lb::FileMetadata, String>>| {
            match maybe_res {
                Some(res) => match res {
                    Ok(_new_file) => {}
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

    fn parse_clipboard_content(&self, dest_id: lb::Uuid, maybe_gstr: Option<glib::GString>) {
        let t = &self.account.tree;

        let clipboard_text = maybe_gstr.unwrap_or_else(|| "".into()).to_string();
        if clipboard_text.is_empty() {
            t.show_msg("Clipboard is empty, nothing to paste!");
            return;
        }

        let uris = clipboard_text
            .split("\r\n")
            .filter_map(|s| match s.is_empty() {
                false => Some(s.to_string()),
                true => None,
            })
            .collect::<Vec<String>>();
        if uris.is_empty() {
            return;
        }
        for uri in &uris {
            if !uri.starts_with("lb://") {
                let scheme = match &uri[..uri.find(':').unwrap_or(0)] {
                    "" => "unknown".to_string(),
                    other => format!("`{}`", other),
                };
                self.show_err_dialog(&format!("Cannot paste {} URIs.", scheme));
                return;
            }
        }

        self.move_lb_files(&uris, dest_id);
    }

    fn move_lb_files(&self, uris: &[String], dest_id: lb::Uuid) {
        let mut ids = Vec::new();
        for uri in uris {
            let id_str = &uri[5..];
            let id = match lb::Uuid::parse_str(id_str) {
                Ok(id) => id,
                Err(err) => {
                    self.show_err_dialog(&format!("Unable to parse ID '{}': {:?}", id_str, err));
                    return;
                }
            };
            match self.api.file_by_id(id) {
                Ok(_) => ids.push(id),
                Err(err) => {
                    self.show_err_dialog(&format!(
                        "Unable to load file with ID '{}': {:?}",
                        id, err
                    ));
                    return;
                }
            }
        }

        let t = &self.account.tree;
        for id in ids {
            match self.api.move_file(id, dest_id) {
                Ok(_) => {
                    let iter = t.search(id).unwrap();
                    t.model.remove(&iter);

                    let children = self.api.file_and_all_children(id).unwrap();
                    let parent_iter = t.search(dest_id).unwrap();
                    t.append_any_children(dest_id, &parent_iter, &children);
                }
                Err(err) => self.show_err_dialog(&format!("{:?}", err)),
            }
        }
    }
}
