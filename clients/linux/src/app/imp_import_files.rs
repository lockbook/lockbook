use std::path::PathBuf;

use gtk::glib;
use gtk::prelude::*;

use crate::ui;
use crate::ui::icons;

impl super::App {
    pub fn import_files(&self, uris: Vec<String>, dest: lb::Uuid) {
        let prog_bar = gtk::ProgressBar::new();
        let status_lbl = gtk::Label::new(Some("Preparing to import files..."));

        let panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
        panel.append(&prog_bar);
        panel.append(&status_lbl);

        let p = gtk::Popover::builder()
            .halign(gtk::Align::Start)
            .width_request(300)
            .child(&panel)
            .build();

        let menu_btn = gtk::MenuButton::builder()
            .child(&gtk::Spinner::builder().spinning(true).build())
            .popover(&p)
            .build();

        let titlebar = self
            .window
            .titlebar()
            .expect("app window should have a titlebar")
            .downcast::<ui::Titlebar>()
            .expect("app window titlebar should be a `ui::Titlebar`");
        titlebar.base().pack_start(&menu_btn);

        let (file_paths, unsupported_uris) = process_uris(uris);
        if !unsupported_uris.is_empty() {
            eprintln!("unsupported uris: {:?}", unsupported_uris);
            return;
        }

        let (info_tx, info_rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let update_status = {
            let info_tx = info_tx.clone();
            move |import_status| info_tx.send(Info::Progress(import_status)).unwrap()
        };

        let api = self.api.clone();
        std::thread::spawn(move || {
            let result = api.import_files(&file_paths, dest, Box::new(update_status));
            info_tx.send(Info::Final(result)).unwrap();
        });

        let app = self.clone();
        let mut total = 0;
        let mut count = 0;
        let mut errors = Vec::new();
        info_rx.attach(None, move |info| {
            match info {
                Info::Progress(import_status) => match import_status {
                    lb::ImportStatus::CalculatedTotal(n_files) => total = n_files,
                    lb::ImportStatus::Error(disk_path, err) => errors.push(match err {
                        lb::CoreError::DiskPathInvalid => {
                            format!("invalid disk path {:?}", disk_path)
                        }
                        _ => format!("unexpected error: {:#?}", err),
                    }),
                    lb::ImportStatus::StartingItem(disk_path) => {
                        status_lbl.set_text(&format!(
                            "({}/{}) Importing: {}... ",
                            count + 1,
                            total,
                            disk_path
                        ));
                        prog_bar.set_fraction(count as f64 / total as f64);
                    }
                    lb::ImportStatus::FinishedItem(meta) => {
                        count += 1;
                        if let Some(iter) = app.account.tree.search(meta.parent) {
                            app.account.tree.append(Some(&iter), &meta);
                        }
                    }
                },
                Info::Final(result) => {
                    let dismiss_errs = gtk::Button::with_label("Dismiss");
                    dismiss_errs.connect_clicked({
                        let p = p.clone();
                        let titlebar = titlebar.clone();
                        let menu_btn = menu_btn.clone();
                        move |_| {
                            p.popdown();
                            titlebar.base().remove(&menu_btn);
                        }
                    });

                    match result {
                        Ok(_) => {
                            if errors.is_empty() {
                                menu_btn.set_icon_name(icons::CHECK_MARK);
                                let msg = format!(
                                    "Successfully imported {} file{}!",
                                    count,
                                    if count == 1 { "" } else { "s" }
                                );
                                p.set_child(Some(&gtk::Label::new(Some(&msg))));
                                p.popdown();
                                p.connect_closed({
                                    let titlebar = titlebar.clone();
                                    let menu_btn = menu_btn.clone();
                                    move |_| titlebar.base().remove(&menu_btn)
                                });
                            } else {
                                menu_btn.set_icon_name(icons::ERROR_RED);

                                let msg = format!(
                                    "Imported {} / {} files with the following errors:",
                                    count, total
                                );

                                let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
                                content.append(&gtk::Label::new(Some(&msg)));

                                for err_msg in &errors {
                                    content.append(&gtk::Label::new(Some(err_msg)));
                                }

                                content.append(&dismiss_errs);
                                p.set_child(Some(&content));
                                p.popup();
                            }
                        }
                        Err(err) => {
                            menu_btn.set_icon_name(icons::ERROR_RED);
                            let err_msg = format!("error: {}", import_err_to_string(err));
                            let err_lbl = gtk::Label::new(Some(&err_msg));
                            let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
                            content.append(&err_lbl);
                            content.append(&dismiss_errs);
                            p.set_child(Some(&content));
                        }
                    }

                    app.update_sync_status();
                }
            }
            glib::Continue(true)
        });
    }
}

fn process_uris(uris: Vec<String>) -> (Vec<PathBuf>, Vec<String>) {
    let mut file_paths = Vec::new();
    let mut unsupported_uris = Vec::new();

    for uri in uris {
        match uri.strip_prefix("file://") {
            Some(path) => {
                let unescaped_path = glib::uri_unescape_string(path, None).unwrap().to_string();
                let path_buf = PathBuf::from(&unescaped_path);
                file_paths.push(path_buf.clone());
            }
            None => unsupported_uris.push(uri),
        }
    }

    (file_paths, unsupported_uris)
}

enum Info {
    Progress(lb::ImportStatus),
    Final(Result<(), lb::Error<lb::ImportFileError>>),
}

fn import_err_to_string(err: lb::Error<lb::ImportFileError>) -> String {
    use lb::ImportFileError::*;
    match err {
        lb::UiError(err) => match err {
            ParentDoesNotExist => "destination does not exist",
            DocumentTreatedAsFolder => "destination is a document",
        }
        .to_string(),
        lb::Unexpected(err) => err,
    }
}
