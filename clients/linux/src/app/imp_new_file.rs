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

        let parent_lbl = gtk::Label::builder()
            .label("Parent:")
            .margin_start(6)
            .halign(gtk::Align::Start)
            .build();

        let parent_entry = gtk::Entry::builder()
            .text(&parent_path)
            .sensitive(false)
            .hexpand(true)
            .build();

        let name_lbl = gtk::Label::builder()
            .label("Name:")
            .margin_start(6)
            .halign(gtk::Align::Start)
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
            ("Folder", NewFileType::Folder),
            ("Plain Text", NewFileType::PlainText),
            ("Markdown", NewFileType::Markdown),
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
        form.attach(&parent_lbl, 0, 0, 1, 1);
        form.attach(&parent_entry, 1, 0, 1, 1);
        form.attach(&name_lbl, 0, 1, 1, 1);
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

        let display_error = move |err_msg: &str| {
            err_lbl.set_text(err_msg);
            err_lbl.show();
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
                Ok(new_file) => {
                    match app.account.tree.add_file(&new_file) {
                        Ok(_) => {
                            app.update_sync_status();
                            d.close();
                            //open the file if doc?
                        }
                        Err(err) => display_error(&err),
                    }
                }
                Err(err) => display_error(&format!("{:?}", err)), //todo
            }
        });

        d.show();
    }
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
    d.add_button("Ok", gtk::ResponseType::Ok);
    d.set_default_response(gtk::ResponseType::Ok);
    d
}

#[derive(Clone, Copy, PartialEq)]
enum NewFileType {
    Folder,
    PlainText,
    Markdown,
}

impl Default for NewFileType {
    fn default() -> Self {
        Self::Folder
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
            Self::PlainText | Self::Folder => None,
            Self::Markdown => Some(".md"),
        }
    }
}

/*
#[derive(Clone)]
struct FileTypeChoices {
    folder: gtk::ToggleButton,
    text: gtk::ToggleButton,
    markdown: gtk::ToggleButton,
    cntr: gtk::Box,
}

impl FileTypeChoices {
    fn new() -> Self {
        let type_group = gtk::ToggleButton::new();

        let folder = gtk::ToggleButton::builder()
            .group(&type_group)
            .can_focus(false)
            .label("Folder")
            .active(true)
            .build();

        let text = gtk::ToggleButton::builder()
            .group(&type_group)
            .can_focus(false)
            .label("Plain Text")
            .build();

        let markdown = gtk::ToggleButton::builder()
            .group(&type_group)
            .can_focus(false)
            .label("Markdown")
            .build();

        let sg = gtk::SizeGroup::new(gtk::SizeGroupMode::Horizontal);
        sg.add_widget(&folder);
        sg.add_widget(&text);
        sg.add_widget(&markdown);

        let cntr = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        cntr.add_css_class("toggle_btn_group");
        cntr.set_margin_bottom(4);
        cntr.append(&folder);
        cntr.append(&text);
        cntr.append(&markdown);

        Self { folder, text, markdown, cntr }
    }

    fn selected_type(&self) -> NewFileType {
        if self.text.is_active() {
            return NewFileType::PlainText;
        }
        if self.markdown.is_active() {
            return NewFileType::Markdown;
        }
        return NewFileType::Folder;
    }

    fn connect_clicked<F: Fn(&Self) + 'static>(&self, f: F) {
        use std::rc::Rc;
        let f = Rc::new(f);

        for btn in &[&self.folder, &self.text, &self.markdown] {
            let f = f.clone();
            let this = self.clone();
            btn.connect_clicked(move |_| f(&this));
        }
    }
}
*/
