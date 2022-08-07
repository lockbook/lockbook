use std::io;

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
        let info = load_doc_info(&self.core, id)?;

        let tab = ui::Tab::new(id);
        tab.set_name(&info.name);

        let tab_lbl = tab.tab_label();
        tab_lbl.connect_closed({
            let tabs = self.account.tabs.clone();
            let tab = tab.clone();

            move || tabs.remove_page(tabs.page_num(&tab))
        });

        self.account.tabs.append_page(&tab, Some(&tab_lbl.cntr));
        self.account.focus_tab_by_id(id);

        // Load the document's content in a separate thread to prevent any UI locking.
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let core = self.core.clone();
        let ext = info.ext.clone();
        std::thread::spawn(move || {
            let result = match ext.as_str() {
                "draw" => core
                    .export_drawing(id, lb::SupportedImageFormats::Png, None)
                    .map_err(export_drawing_err_to_string),
                _ => core.read_document(id).map_err(read_doc_err_to_string),
            };
            tx.send(result).unwrap();
        });

        // Receive the Result of reading the document and set the tab page content accordingly.
        let app = self.clone();
        let tab = tab.clone();
        rx.attach(None, move |read_doc_result: Result<Vec<u8>, String>| {
            match read_doc_result {
                Ok(data) => {
                    if info.ext == "draw" || ui::SUPPORTED_IMAGE_FORMATS.contains(&info.ext) {
                        tab.set_content(&image_content(data));
                    } else {
                        tab.set_content(&text_content(&app, &info, &data));
                    }
                }
                Err(msg) => tab.set_content(&err_content(&msg)),
            }
            glib::Continue(false)
        });

        Ok(())
    }
}

fn text_content(app: &super::App, info: &DocInfo, data: &[u8]) -> ui::TextEditor {
    let txt_ed = ui::TextEditor::new();

    let buf = txt_ed.editor().buffer().downcast::<sv5::Buffer>().unwrap();
    buf.set_text(&String::from_utf8_lossy(data));
    buf.set_highlight_syntax(true);

    let lang_guess = app.account.lang_mngr.guess_language(Some(&info.name), None);
    buf.set_language(lang_guess.as_ref());

    if info.ext == "md" {
        let account_op_tx = &app.account.op_chan;
        connect_sview_clipboard_paste(account_op_tx, &txt_ed, info.id);
        connect_sview_drop_controller(account_op_tx, &txt_ed, info.id);
        connect_sview_click_controller(account_op_tx, &txt_ed);
    }

    let id = info.id;
    let edit_alert_tx = app.bg_state.track(id);
    buf.connect_changed(move |_| edit_alert_tx.send(id).unwrap());

    let scheme_name = app.account.scheme_name.get();
    if let Some(ref scheme) = sv5::StyleSchemeManager::default().scheme(scheme_name) {
        buf.set_style_scheme(Some(scheme));
    }

    txt_ed
}

fn read_doc_err_to_string(err: lb::Error<lb::ReadDocumentError>) -> String {
    use lb::ReadDocumentError::*;
    match err {
        lb::Error::UiError(err) => match err {
            TreatedFolderAsDocument => "treated folder as document",
            FileDoesNotExist => "file does not exist",
        }
        .to_string(),
        lb::Error::Unexpected(msg) => msg,
    }
}

fn export_drawing_err_to_string(err: lb::Error<lb::ExportDrawingError>) -> String {
    use lb::ExportDrawingError::*;
    match err {
        lb::Error::UiError(err) => match err {
            FolderTreatedAsDrawing => "This is a folder, not a drawing.",
            FileDoesNotExist => "File doesn't exist.",
            InvalidDrawing => "Invalid drawing.",
        }
        .to_string(),
        lb::Error::Unexpected(msg) => msg,
    }
}

fn err_content(msg: &str) -> gtk::Label {
    gtk::Label::builder()
        .halign(gtk::Align::Center)
        .label(msg)
        .build()
}

fn image_content(data: Vec<u8>) -> gtk::Widget {
    let pbuf = match Pixbuf::from_read(io::Cursor::new(data)) {
        Ok(pbuf) => pbuf,
        Err(err) => return err_content(&err.to_string()).upcast::<gtk::Widget>(),
    };

    let pic = gtk::Picture::for_pixbuf(&pbuf);
    pic.set_halign(gtk::Align::Center);
    pic.set_valign(gtk::Align::Center);

    let img_content = ui::ImageTab::new();
    img_content.set_picture(&pic);

    img_content.upcast::<gtk::Widget>()
}

struct DocInfo {
    id: lb::Uuid,
    name: String,
    ext: String,
}

fn load_doc_info(core: &lb::Core, id: lb::Uuid) -> Result<DocInfo, String> {
    use lb::GetFileByIdError::*;
    let name = core
        .get_file_by_id(id)
        .map(|f| f.name)
        .map_err(|err| match err {
            lb::Error::UiError(NoFileWithThatId) => format!("no file with id '{}'", id),
            lb::Error::Unexpected(msg) => msg,
        })?;

    let ext = std::path::Path::new(&name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase();

    Ok(DocInfo { id, name, ext })
}

fn connect_sview_clipboard_paste(
    op_chan: &glib::Sender<ui::AccountOp>, txt_ed: &ui::TextEditor, id: lb::Uuid,
) {
    let op_chan = op_chan.clone();
    txt_ed.editor().connect_paste_clipboard(move |text_view| {
        let op_chan = op_chan.clone();
        let buf = text_view.buffer().downcast::<sv5::Buffer>().unwrap();

        let clip = gdk::Display::default().unwrap().clipboard();
        clip.read_texture_async(None::<gio::Cancellable>.as_ref(), move |res| {
            if let Ok(Some(texture)) = res {
                op_chan
                    .send(ui::AccountOp::SviewInsertTexture { id, buf, texture })
                    .unwrap();
                return;
            }

            let buf = buf.clone();
            let clip = gdk::Display::default().unwrap().clipboard();
            clip.read_value_async(
                gdk::FileList::static_type(),
                glib::PRIORITY_DEFAULT,
                None::<gio::Cancellable>.as_ref(),
                move |res| {
                    if let Ok(value) = res {
                        if let Ok(flist) = value.get::<gdk::FileList>() {
                            buf.undo();
                            op_chan
                                .send(ui::AccountOp::SviewInsertFileList { id, buf, flist })
                                .unwrap();
                        }
                    }
                },
            );
        });
    });
}

fn connect_sview_drop_controller(
    op_chan: &glib::Sender<ui::AccountOp>, txt_ed: &ui::TextEditor, id: lb::Uuid,
) {
    let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);

    let op_chan = op_chan.clone();
    let buf = txt_ed.editor().buffer().downcast::<sv5::Buffer>().unwrap();
    drop.connect_drop(move |_, value, _x, _y| {
        if let Ok(flist) = value.get::<gdk::FileList>() {
            let buf = buf.clone();
            op_chan
                .send(ui::AccountOp::SviewInsertFileList { id, buf, flist })
                .unwrap();
            true
        } else {
            false
        }
    });

    txt_ed.editor().add_controller(&drop);
}

fn connect_sview_click_controller(op_chan: &glib::Sender<ui::AccountOp>, txt_ed: &ui::TextEditor) {
    let click = gtk::GestureClick::new();
    click.set_button(gdk::ffi::GDK_BUTTON_PRIMARY as u32);

    let op_chan = op_chan.clone();
    let sview = txt_ed.editor().clone();
    click.connect_pressed(move |click, _, x, y| {
        if click.current_event_state() == gdk::ModifierType::CONTROL_MASK {
            let op_msg = ui::AccountOp::SviewCtrlClick {
                click: click.clone(),
                x: x as i32,
                y: y as i32,
                sview: sview.clone(),
            };
            op_chan.send(op_msg).unwrap();
        }
    });

    txt_ed.editor().add_controller(&click);
}
