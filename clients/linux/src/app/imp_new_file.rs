use gtk::prelude::*;

impl super::App {
    pub fn new_file(&self, ftype: lb::FileType) {
        let ftype_str = format!("{:?}", ftype);

        let lbl = gtk::Label::builder()
            .label(&format!("Enter {} name:", ftype_str.to_lowercase()))
            .halign(gtk::Align::Start)
            .build();

        let errlbl = gtk::Label::builder()
            .name("err")
            .halign(gtk::Align::Start)
            .margin_top(16)
            .margin_bottom(8)
            .build();

        let entry = gtk::Entry::builder()
            .activates_default(true)
            .margin_top(16)
            .build();

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_start(8)
            .margin_end(8)
            .margin_top(8)
            .margin_bottom(8)
            .build();

        content.append(&lbl);
        content.append(&entry);

        let d = gtk::Dialog::builder()
            .title(&format!("New {}", ftype_str))
            .transient_for(&self.window)
            .default_width(300)
            .modal(true)
            .build();

        d.content_area().set_orientation(gtk::Orientation::Vertical);
        d.content_area().append(&content);
        d.add_button("Ok", gtk::ResponseType::Ok);
        d.set_default_response(gtk::ResponseType::Ok);

        let app = self.clone();
        d.connect_response(move |d, resp| {
            if resp != gtk::ResponseType::Ok {
                d.close();
                return;
            }

            let parent_id = match app.account.tree.get_selected_uuid() {
                Some(id) => match app.api.file_by_id(id) {
                    Ok(file) => match file.file_type {
                        lb::FileType::Document => file.parent,
                        lb::FileType::Folder => file.id,
                    },
                    Err(err) => {
                        errlbl.set_text(&format!("{:?}", err));
                        content.append(&errlbl);
                        entry.set_sensitive(false);
                        return;
                    }
                },
                None => {
                    eprintln!("no destination is selected to create from!");
                    return;
                }
            };

            let fname = entry.buffer().text();

            match app.api.create_file(&fname, parent_id, ftype) {
                Ok(new_file) => {
                    d.close();
                    match app.account.tree.add_file(&new_file) {
                        Ok(_) => {
                            app.update_sync_status();
                            //open the file if doc?
                        }
                        Err(err) => eprintln!("{}", err),
                    }
                }
                Err(err) => eprintln!("{:?}", err),
            }
        });

        d.show();
    }
}
