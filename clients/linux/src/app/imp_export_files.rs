use std::path::PathBuf;

use gtk::glib;
use gtk::prelude::*;

use crate::ui;
use crate::ui::icons;

impl super::App {
    pub fn export_files(&self) {
        let lb_file = match self.account.tree.get_selected_uuid() {
            Some(id) => id,
            None => return,
        };

        let fc = gtk::FileChooserDialog::builder()
            .transient_for(&self.window)
            .title("Choose Export Destination")
            .modal(true)
            .action(gtk::FileChooserAction::SelectFolder)
            .build();
        fc.add_button("Ok", gtk::ResponseType::Ok);

        let app = self.clone();
        fc.connect_response(move |fc, resp| {
            fc.close();
            if resp == gtk::ResponseType::DeleteEvent {
                return;
            }

            let g_file = match fc.file() {
                Some(f) => f,
                None => return,
            };

            let dest = match g_file.path() {
                Some(path_buf) => path_buf,
                None => {
                    app.show_err_dialog(&format!("invalid disk file '{}'", g_file));
                    return;
                }
            };

            app.do_exporting(lb_file, dest);
        });

        fc.show();
    }

    fn do_exporting(&self, lb_file: lb::Uuid, dest: PathBuf) {
        let prog_bar = gtk::ProgressBar::new();
        let status_lbl = gtk::Label::new(Some("Preparing to export files..."));

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

        let total = match self.api.file_and_all_children(lb_file) {
            Ok(children) => children.len(),
            Err(err) => {
                self.show_err_dialog(&format!("{:?}", err));
                return;
            }
        };

        let (info_tx, info_rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let update_status = {
            let info_tx = info_tx.clone();
            move |export_info| info_tx.send(Info::Progress(export_info)).unwrap()
        };

        let api = self.api.clone();
        std::thread::spawn(move || {
            let result = api.export_file(lb_file, dest, false, Some(Box::new(update_status)));
            info_tx.send(Info::Final(result)).unwrap();
        });

        let mut count = 0;
        info_rx.attach(None, move |info| {
            match info {
                Info::Progress(file_info) => {
                    count += 1;
                    status_lbl.set_text(&format!(
                        "({}/{}) Exporting: {}... ",
                        count, total, file_info.lockbook_path,
                    ));
                    prog_bar.set_fraction(count as f64 / total as f64);
                }
                Info::Final(result) => match result {
                    Ok(_) => {
                        menu_btn.set_icon_name(icons::CHECK_MARK);
                        let msg = format!(
                            "Successfully exported {} file{}!",
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
                    }
                    Err(err) => {
                        menu_btn.set_icon_name(icons::ERROR_RED);
                        let err_msg = format!("error: {}", export_err_to_string(err));
                        let err_lbl = gtk::Label::new(Some(&err_msg));
                        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
                        content.append(&err_lbl);
                        //content.append(&dismiss_errs);
                        p.set_child(Some(&content));
                    }
                },
            }
            glib::Continue(true)
        });
    }
}

fn export_err_to_string(err: lb::Error<lb::ExportFileError>) -> String {
    use lb::ExportFileError::*;

    match err {
        lb::UiError(err) => match err {
            ParentDoesNotExist => "parent lockbook file does not exist",
            DiskPathTaken => "destination path is taken",
            DiskPathInvalid => "destination path is invalid",
        }
        .to_string(),
        lb::Unexpected(msg) => msg,
    }
}

enum Info {
    Progress(lb::ImportExportFileInfo),
    Final(Result<(), lb::Error<lb::ExportFileError>>),
}
