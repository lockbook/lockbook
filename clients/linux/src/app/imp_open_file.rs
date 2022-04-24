use std::io;
use std::sync::Arc;

use gdk_pixbuf::Pixbuf;
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
        let doc = load_doc(&self.api, id)?;

        let tab = ui::Tab::new(doc.id);
        tab.set_name(&doc.name);
        tab.set_content(&match doc.ext.as_str() {
            "png" => self.image_content(doc)?,
            _ => self.text_content(doc),
        });

        self.account.tabs.append_page(&tab, Some(tab.tab_label()));
        self.account.focus_tab_by_id(id);

        Ok(())
    }

    fn text_content(&self, doc: Document) -> gtk::Widget {
        let txt_ed = ui::TextEditor::new();

        let buf = txt_ed.editor().buffer().downcast::<sv5::Buffer>().unwrap();
        buf.set_text(&String::from_utf8_lossy(&doc.data).to_string());
        buf.set_highlight_syntax(true);

        let lang_guess = self.account.lang_mngr.guess_language(Some(&doc.name), None);
        buf.set_language(lang_guess.as_ref());

        if doc.ext == "md" {
            connect_sview_clipboard_paste(self, &txt_ed, &buf, doc.id);
            connect_sview_drop_controller(self, &txt_ed, &buf, doc.id);
            connect_sview_click_controller(self, &txt_ed);
        }

        let id = doc.id;
        let edit_alert_tx = self.bg_state.track(id);
        buf.connect_changed(move |_| edit_alert_tx.send(id).unwrap());

        let scheme_name = self.account.scheme_name.get();
        if let Some(ref scheme) = sv5::StyleSchemeManager::default().scheme(scheme_name) {
            buf.set_style_scheme(Some(scheme));
        }

        txt_ed.upcast::<gtk::Widget>()
    }

    fn image_content(&self, doc: Document) -> Result<gtk::Widget, String> {
        let pbuf = Pixbuf::from_read(io::Cursor::new(doc.data)).map_err(|err| err.to_string())?;
        let pic = gtk::Picture::for_pixbuf(&pbuf);
        pic.set_halign(gtk::Align::Center);
        pic.set_valign(gtk::Align::Center);

        let img_content = ui::ImageTab::new();
        img_content.set_picture(&pic);

        Ok(img_content.upcast::<gtk::Widget>())
    }
}

struct Document {
    id: lb::Uuid,
    name: String,
    ext: String,
    data: Vec<u8>,
}

fn load_doc(api: &Arc<dyn lb::Api>, id: lb::Uuid) -> Result<Document, String> {
    use lb::GetFileByIdError::*;
    let name = api
        .file_by_id(id)
        .map(|fm| fm.decrypted_name)
        .map_err(|err| match err {
            lb::Error::UiError(NoFileWithThatId) => format!("no file with id '{}'", id),
            lb::Error::Unexpected(msg) => msg,
        })?;

    let ext = std::path::Path::new(&name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();

    use lb::ReadDocumentError::*;
    let data = api.read_document(id).map_err(|err| match err {
        lb::Error::UiError(err) => match err {
            TreatedFolderAsDocument => "treated folder as document",
            NoAccount => "no account",
            FileDoesNotExist => "file does not exist",
        }
        .to_string(),
        lb::Error::Unexpected(msg) => msg,
    })?;

    Ok(Document { id, name, ext, data })
}

fn connect_sview_clipboard_paste(
    app: &super::App, txt_ed: &ui::TextEditor, buf: &sv5::Buffer, id: lb::Uuid,
) {
    let app = app.clone();
    let buf = buf.clone();
    txt_ed.editor().connect_paste_clipboard(move |_| {
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
    app: &super::App, txt_ed: &ui::TextEditor, buf: &sv5::Buffer, id: lb::Uuid,
) {
    txt_ed.editor().add_controller(&{
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

fn connect_sview_click_controller(app: &super::App, txt_ed: &ui::TextEditor) {
    txt_ed.editor().add_controller(&{
        let g = gtk::GestureClick::new();
        g.set_button(gtk::gdk::ffi::GDK_BUTTON_PRIMARY as u32);

        let app = app.clone();
        let sview = txt_ed.editor().clone();
        g.connect_pressed(move |g, _, x, y| {
            if g.current_event_state() == gdk::ModifierType::CONTROL_MASK {
                app.handle_sview_ctrl_click(g, x as i32, y as i32, &sview);
            }
        });

        g
    });
}
