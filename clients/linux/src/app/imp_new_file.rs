use gtk::prelude::*;

use crate::ui;

impl super::App {
    pub fn prompt_new_file(&self) {
        let selected_id = self.account.tree.get_selected_uuid();

        let (parent_id, parent_path) = match lb::parent_info(&self.api, selected_id) {
            Ok(v) => v,
            Err(err) => {
                self.show_err_dialog(&err);
                return;
            }
        };

        let parent_entry = gtk::Entry::builder()
            .primary_icon_name("folder-symbolic")
            .text(&parent_path)
            .sensitive(false)
            .hexpand(true)
            .build();

        let name_entry = gtk::Entry::builder()
            .activates_default(true)
            .hexpand(true)
            .build();

        let ext_lbl = gtk::Label::builder().visible(false).build();

        let name_and_ext = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        name_and_ext.append(&name_entry);
        name_and_ext.append(&ext_lbl);

        let ftype_choices = ui::ToggleGroup::with_buttons(&[
            ("Markdown", NewFileType::Markdown),
            ("Plain Text", NewFileType::PlainText),
            ("Folder", NewFileType::Folder),
        ]);
        ftype_choices.connect_changed(move |value: NewFileType| {
            if let Some(ext) = value.ext() {
                ext_lbl.set_text(ext);
                ext_lbl.show();
            } else {
                ext_lbl.hide();
            }
        });

        let form = gtk::Grid::builder()
            .column_spacing(16)
            .row_spacing(16)
            .build();
        form.attach(&form_lbl("Parent:"), 0, 0, 1, 1);
        form.attach(&parent_entry, 1, 0, 1, 1);
        form.attach(&form_lbl("Name:"), 0, 1, 1, 1);
        form.attach(&name_and_ext, 1, 1, 1, 1);

        let err_lbl = gtk::Label::builder().visible(false).name("err").build();

        let d = new_file_dialog(&self.window);
        let ca = d.content_area();
        ca.set_orientation(gtk::Orientation::Vertical);
        ca.set_spacing(16);
        ca.set_margin_top(16);
        ca.set_margin_bottom(16);
        ca.set_margin_start(16);
        ca.set_margin_end(16);
        ca.append(&ftype_choices.cntr);
        ca.append(&form);
        ca.append(&err_lbl);

        name_entry.grab_focus();

        let display_error = {
            let name_entry = name_entry.clone();

            move |err_msg: &str| {
                err_lbl.set_text(err_msg);
                err_lbl.show();
                name_entry.grab_focus();
            }
        };

        let app = self.clone();
        d.connect_response(move |d, resp| {
            if resp != gtk::ResponseType::Ok {
                d.close();
                return;
            }

            let ftype = ftype_choices.value();
            let mut fname = name_entry.text().to_string();
            if let Some(ext) = ftype.ext() {
                fname = format!("{}{}", fname, ext);
            }

            match app.api.create_file(&fname, parent_id, ftype.lb_type()) {
                Ok(new_file) => match app.account.tree.add_file(&new_file) {
                    Ok(_) => {
                        app.update_sync_status();
                        d.close();
                        if new_file.file_type == lb::FileType::Document
                            && app.settings.read().unwrap().open_new_files
                        {
                            app.open_file(new_file.id)
                        }
                    }
                    Err(err) => display_error(&err),
                },
                Err(err) => display_error(&{
                    use lb::CreateFileError::*;
                    match err {
                        lb::UiError(err) => match err {
                            DocumentTreatedAsFolder => {
                                "Can only create files within folders, not documents."
                            }
                            CouldNotFindAParent => "That parent folder does not exist.",
                            FileNameNotAvailable => "That file name is alrady taken.",
                            FileNameEmpty => "File names cannot be empty.",
                            FileNameContainsSlash => "File names cannot contain a slash (/).",
                        }
                        .to_string(),
                        lb::Unexpected(msg) => msg,
                    }
                }),
            }
        });

        d.show();
    }
}

fn form_lbl(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .margin_start(6)
        .halign(gtk::Align::Start)
        .build()
}

fn new_file_dialog(parent: &impl IsA<gtk::Window>) -> gtk::Dialog {
    let new_icon = gtk::Image::from_icon_name("emblem-new");
    new_icon.set_pixel_size(28);

    let title = gtk::Label::new(None);
    title.set_markup("<b>New File</b>");

    let titlebar = gtk::HeaderBar::new();
    titlebar.set_title_widget(Some(&title));
    titlebar.pack_start(&new_icon);

    let d = gtk::Dialog::builder()
        .transient_for(parent)
        .titlebar(&titlebar)
        .modal(true)
        .build();
    d.add_action_widget(&action_btn("Cancel"), gtk::ResponseType::Cancel);
    d.add_action_widget(&action_btn("Create"), gtk::ResponseType::Ok);
    d.set_default_response(gtk::ResponseType::Ok);
    d
}

fn action_btn(text: &str) -> gtk::Button {
    gtk::Button::builder()
        .margin_end(8)
        .margin_bottom(8)
        .label(text)
        .build()
}

#[derive(Clone, Copy, PartialEq)]
enum NewFileType {
    Markdown,
    PlainText,
    Folder,
}

impl Default for NewFileType {
    fn default() -> Self {
        Self::Markdown
    }
}

impl NewFileType {
    fn lb_type(&self) -> lb::FileType {
        match self {
            Self::PlainText | Self::Markdown => lb::FileType::Document,
            Self::Folder => lb::FileType::Folder,
        }
    }

    fn ext(&self) -> Option<&str> {
        match self {
            Self::Markdown => Some(".md"),
            Self::PlainText => Some(".txt"),
            Self::Folder => None,
        }
    }
}
