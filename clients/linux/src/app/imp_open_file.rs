use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use sv5::prelude::*;

use crate::ui;

impl super::App {
    pub fn open_file_from_id_str(&self, id_str: &str) {
        match lb::Uuid::parse_str(id_str) {
            Ok(id) => self.open_file(id),
            Err(err) => self.show_err_dialog(&format!("invalid uuid: {}", err)),
        }
    }

    pub fn open_file(&self, id: lb::Uuid) {
        if self.account.focus_tab_by_id(id) {
            return;
        }
        if let Err(err) = self.read_file_and_open_tab(id) {
            self.show_err_dialog(&format!("error opening file: {}", err));
        }
    }

    fn read_file_and_open_tab(&self, id: lb::Uuid) -> Result<(), String> {
        use lb::GetFileByIdError::*;
        let name = self
            .api
            .file_by_id(id)
            .map(|fm| fm.decrypted_name)
            .map_err(|err| match err {
                lb::Error::UiError(NoFileWithThatId) => format!("no file with id '{}'", id),
                lb::Error::Unexpected(msg) => msg,
            })?;

        use lb::ReadDocumentError::*;
        let data = self
            .api
            .read_document(id)
            .map(|data| String::from_utf8_lossy(&data).to_string())
            .map_err(|err| match err {
                lb::Error::UiError(err) => match err {
                    TreatedFolderAsDocument => "treated folder as document",
                    NoAccount => "no account",
                    FileDoesNotExist => "file does not exist",
                }
                .to_string(),
                lb::Error::Unexpected(msg) => msg,
            })?;

        let tab_page = ui::TextEditor::new(id);
        tab_page.set_name(&name);

        let buf = tab_page
            .editor()
            .buffer()
            .downcast::<sv5::Buffer>()
            .unwrap();
        buf.set_text(&data);
        buf.set_highlight_syntax(true);

        let lang_guess = self.account.lang_mngr.guess_language(Some(&name), None);
        buf.set_language(lang_guess.as_ref());

        if lang_guess.map(|l| l.name().to_string()) == Some("Markdown".to_string()) {
            connect_sview_clipboard_paste(self, &tab_page, &buf, id);
            connect_sview_drop_controller(self, &tab_page, &buf, id);
            connect_sview_click_controller(self, &tab_page);
        }

        let edit_alert_tx = self.bg_state.track(id);
        tab_page.connect_edit_alert_chan(edit_alert_tx);

        self.account
            .tabs
            .append_page(&tab_page, Some(tab_page.tab_label()));
        tab_page.editor().grab_focus();

        let scheme_name = self.account.scheme_name.get();
        if let Some(ref scheme) = sv5::StyleSchemeManager::default().scheme(scheme_name) {
            buf.set_style_scheme(Some(scheme));
        }

        Ok(())
    }
}

fn connect_sview_clipboard_paste(
    app: &super::App, tab_page: &ui::TextEditor, buf: &sv5::Buffer, id: lb::Uuid,
) {
    let app = app.clone();
    let buf = buf.clone();
    tab_page.editor().connect_paste_clipboard(move |_| {
        let clip = gdk::Display::default().unwrap().clipboard();
        let app = app.clone();
        let buf = buf.clone();
        clip.clone()
            .read_texture_async(None::<gio::Cancellable>.as_ref(), move |res| {
                if let Ok(Some(texture)) = res {
                    app.sview_insert_texture(id, &buf, texture);
                    return;
                }
                clip.read_value_async(
                    gdk::FileList::static_type(),
                    glib::PRIORITY_DEFAULT,
                    None::<gio::Cancellable>.as_ref(),
                    move |res| {
                        if let Ok(value) = res {
                            if let Ok(flist) = value.get::<gdk::FileList>() {
                                buf.undo();
                                app.sview_insert_file_list(id, &buf, flist);
                            }
                        }
                    },
                );
            });
    });
}

fn connect_sview_drop_controller(
    app: &super::App, tab_page: &ui::TextEditor, buf: &sv5::Buffer, id: lb::Uuid,
) {
    tab_page.editor().add_controller(&{
        let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gtk::gdk::DragAction::COPY);

        let app = app.clone();
        let buf = buf.clone();
        drop.connect_drop(move |_, value, _x, _y| {
            if let Ok(flist) = value.get::<gdk::FileList>() {
                app.sview_insert_file_list(id, &buf, flist);
                true
            } else {
                false
            }
        });

        drop
    });
}

fn connect_sview_click_controller(app: &super::App, tab_page: &ui::TextEditor) {
    tab_page.editor().add_controller(&{
        let g = gtk::GestureClick::new();
        g.set_button(gtk::gdk::ffi::GDK_BUTTON_PRIMARY as u32);

        let app = app.clone();
        let sview = tab_page.editor().clone();
        g.connect_pressed(move |g, _, x, y| {
            if g.current_event_state() == gdk::ModifierType::CONTROL_MASK {
                app.handle_sview_ctrl_click(g, x as i32, y as i32, &sview);
            }
        });

        g
    });
}
