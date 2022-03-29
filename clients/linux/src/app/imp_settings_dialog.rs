use gdk_pixbuf::Pixbuf;
use gtk::glib;
use gtk::prelude::*;
use qrcode_generator::QrCodeEcc;

use crate::ui;
use crate::ui::icons;

impl super::App {
    pub fn open_settings_dialog(&self) {
        let d = gtk::Dialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .default_width(500)
            .default_height(425)
            .resizable(false)
            .title("Settings")
            .build();

        let tabs = gtk::Notebook::builder()
            .tab_pos(gtk::PositionType::Left)
            .show_border(false)
            .build();
        tab(&tabs, "Account", icons::ACCOUNT, &self.acct_settings(&d));
        tab(&tabs, "Usage", icons::USAGE, &self.usage_settings());
        tab(&tabs, "Application", icons::APP, &self.app_settings());

        d.set_child(Some(&tabs));
        d.show();
    }

    fn acct_settings(&self, settings_win: &gtk::Dialog) -> gtk::Box {
        let cntr = settings_box();

        match self.api.account() {
            Ok(maybe_acct) => {
                cntr.append(&heading("Info"));
                cntr.append(&acct_info(maybe_acct.as_ref()));
                cntr.append(&separator());
                cntr.append(&heading("Export"));
                cntr.append(&self.acct_export(settings_win));
            }
            Err(err) => {
                let err_lbl = gtk::Label::builder()
                    .label(&err)
                    .halign(gtk::Align::Center)
                    .valign(gtk::Align::Center)
                    .build();
                cntr.append(&err_lbl);
            }
        }
        cntr
    }

    fn acct_export(&self, settings_win: &gtk::Dialog) -> gtk::Box {
        let cntr = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_start(4)
            .margin_bottom(20)
            .build();

        let acct_secret = match self.api.export_account() {
            Ok(v) => v,
            Err(err) => {
                cntr.append(&gtk::Label::new(Some(&format!("{:?}", err)))); //todo
                return cntr;
            }
        };

        let warning = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .label(EXPORT_DESC)
            .use_markup(true)
            .wrap(true)
            .margin_bottom(20)
            .build();

        let btn_copy = ui::clipboard_btn("Copy Key to Clipboard", &acct_secret);
        let btn_show_qr = gtk::Button::builder().label("Show Key as QR Code").build();

        let win = settings_win.clone();
        btn_show_qr.connect_clicked(move |btn_show_qr| {
            let spinner = gtk::Spinner::new();
            spinner.start();
            let loading = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            loading.set_halign(gtk::Align::Center);
            loading.append(&spinner);
            loading.append(&gtk::Label::new(Some("Generating QR...")));
            btn_show_qr.set_child(Some(&loading));

            let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

            let acct_secret = acct_secret.clone();
            std::thread::spawn(move || {
                let bytes: Vec<u8> =
                    qrcode_generator::to_png_to_vec(&acct_secret, QrCodeEcc::Low, 1024).unwrap();
                tx.send(bytes).unwrap();
            });

            let btn_show_qr = btn_show_qr.clone();
            let win = win.clone();
            rx.attach(None, move |bytes| {
                let pixbuf = Pixbuf::from_read(std::io::Cursor::new(bytes)).unwrap();
                let qr_code = gtk::Image::builder()
                    .width_request(200)
                    .height_request(200)
                    .margin_top(20)
                    .margin_bottom(20)
                    .build();
                qr_code.set_from_pixbuf(Some(&pixbuf));
                btn_show_qr.set_label("Show Key as QR Code");
                gtk::Dialog::builder()
                    .transient_for(&win)
                    .modal(true)
                    .child(&qr_code)
                    .build()
                    .show();
                glib::Continue(false)
            });
        });

        let buttons = gtk::Grid::builder()
            .column_spacing(4)
            .column_homogeneous(true)
            .build();
        buttons.attach(&btn_copy, 0, 0, 1, 1);
        buttons.attach(&btn_show_qr, 1, 0, 1, 1);

        cntr.append(&warning);
        cntr.append(&buttons);
        cntr
    }

    fn usage_settings(&self) -> gtk::Box {
        let cntr = settings_box();
        cntr.set_margin_top(20);
        cntr.set_spacing(0);

        match self.api.usage() {
            Ok(metrics) => {
                usage_metrics(&cntr, &metrics);
                match self.api.uncompressed_usage() {
                    Ok(uncompressed) => usage_compression(&cntr, &metrics, &uncompressed),
                    Err(err) => cntr.append(&gtk::Label::new(Some(&format!("{:?}", err)))), //todo
                }
            }
            Err(err) => cntr.append(&gtk::Label::new(Some(&format!("{:?}", err)))), //todo
        }
        cntr
    }

    fn app_settings(&self) -> gtk::Box {
        let cntr = settings_box();
        cntr.append(&heading("General"));
        cntr.append(&self.general_settings());
        cntr.append(&separator());
        cntr.append(&heading("File Tree"));
        cntr.append(&self.filetree_settings());
        cntr
    }

    fn general_settings(&self) -> gtk::Box {
        let section = section();
        // Maximize on startup.
        {
            let s = self.settings.clone();
            let ch = gtk::CheckButton::with_label("Maximize window on startup");
            ch.set_active(s.read().unwrap().window_maximize);
            ch.connect_toggled(move |ch| {
                s.write().unwrap().window_maximize = ch.is_active();
            });
            section.append(&ch);
        }
        // Auto save.
        {
            let s = self.settings.clone();
            let ch = gtk::CheckButton::with_label("Auto-save");
            ch.set_active(s.read().unwrap().auto_save);
            ch.connect_toggled(move |ch| {
                let auto_save = ch.is_active();
                s.write().unwrap().auto_save = auto_save;
                //self.toggle_auto_save(auto_save);
            });
            section.append(&ch);
        }
        // Auto sync.
        {
            let s = self.settings.clone();
            let ch = gtk::CheckButton::with_label("Auto-sync");
            ch.set_active(s.read().unwrap().auto_sync);
            ch.connect_toggled(move |ch| {
                let auto_sync = ch.is_active();
                s.write().unwrap().auto_sync = auto_sync;
                //self.toggle_auto_sync(auto_sync);
            });
            section.append(&ch);
        }
        section
    }

    fn filetree_settings(&self) -> gtk::Box {
        let section = section();
        section.append(
            &gtk::Label::builder()
                .label("Show columns:")
                .margin_bottom(4)
                .build(),
        );
        for col in ui::FileTreeCol::removable() {
            let ch = gtk::CheckButton::with_label(&col.name());
            ch.set_active(
                !self
                    .settings
                    .read()
                    .unwrap()
                    .hidden_tree_cols
                    .contains(&col.name()),
            );
            let app = self.clone();
            ch.connect_toggled(move |_| app.tree_toggle_col(col));
            section.append(&ch);
        }
        section
    }
}

fn tab(tabs: &gtk::Notebook, name: &str, icon_name: &str, stuff: &gtk::Box) {
    let icon = gtk::Image::builder()
        .icon_name(icon_name)
        .pixel_size(22)
        .build();

    let icon_and_name = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin_bottom(12)
        .margin_top(12)
        .spacing(9)
        .build();

    icon_and_name.append(&icon);
    icon_and_name.append(&gtk::Label::new(Some(name)));

    let content = gtk::ScrolledWindow::new();
    content.set_child(Some(stuff));

    tabs.append_page(&content, Some(&icon_and_name));
}

fn acct_info(maybe_acct: Option<&lb::Account>) -> gtk::Grid {
    let info = gtk::Grid::builder()
        .column_spacing(16)
        .row_spacing(8)
        .margin_start(8)
        .margin_end(8)
        .build();
    match maybe_acct {
        Some(a) => {
            info.attach(&grid_key("Username: "), 0, 0, 1, 1);
            info.attach(&grid_val(&a.username), 1, 0, 1, 1);
            info.attach(&grid_key("Server: "), 0, 1, 1, 1);
            info.attach(&grid_val(&a.api_url), 1, 1, 1, 1);
        }
        None => info.attach(&grid_key("NO ACCOUNT"), 0, 0, 1, 1),
    }
    info
}

fn usage_metrics(cntr: &gtk::Box, m: &lb::UsageMetrics) {
    let su_pct = m.server_usage.exact as f64 / m.data_cap.exact as f64;
    let su_str = format!("<b>{}</b> / <b>{}</b>", m.server_usage.readable, m.data_cap.readable);
    let su_lbl = gtk::Label::builder()
        .label(&su_str)
        .use_markup(true)
        .halign(gtk::Align::Start)
        .tooltip_text(&format!("{} %", su_pct))
        .build();
    let su_pct_lbl = gtk::Label::builder()
        .label(&format!("({:.2} %)", su_pct))
        .halign(gtk::Align::End)
        .hexpand(true)
        .build();

    let texts = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    texts.append(&su_lbl);
    texts.append(&su_pct_lbl);

    let su_bar = gtk::ProgressBar::builder()
        .fraction(if su_pct > 1.0 { 1.0 } else { su_pct }) // clamp percentage at 1
        .margin_top(12)
        .build();

    cntr.append(
        &gtk::Label::builder()
            .label("Server utilization:")
            .halign(gtk::Align::Start)
            .margin_bottom(12)
            .build(),
    );
    cntr.append(&texts);
    cntr.append(&su_bar);
}

fn usage_compression(cntr: &gtk::Box, m: &lb::UsageMetrics, cmpr: &lb::UsageItemMetric) {
    let info = gtk::Grid::builder()
        .margin_top(30)
        .column_spacing(8)
        .row_spacing(8)
        .build();
    let compression_ratio = format!("{:.2}x", cmpr.exact as f64 / m.server_usage.exact as f64);
    info.attach(&grid_key("Uncompressed usage: "), 0, 0, 1, 1);
    info.attach(&grid_val(&cmpr.readable), 1, 0, 1, 1);
    info.attach(&grid_key("Compression ratio: "), 0, 1, 1, 1);
    info.attach(&grid_val(&compression_ratio), 1, 1, 1, 1);
    cntr.append(&info);
}

fn settings_box() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_start(12)
        .margin_end(12)
        .build()
}

fn heading(txt: &str) -> gtk::Label {
    gtk::Label::builder()
        .css_classes(vec!["settings-heading".to_string()])
        .label(txt)
        .halign(gtk::Align::Start)
        .margin_top(12)
        .margin_bottom(6)
        .build()
}

fn section() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Start)
        .margin_start(8)
        .build()
}

fn separator() -> gtk::Separator {
    gtk::Separator::builder()
        .orientation(gtk::Orientation::Horizontal)
        .margin_top(20)
        .margin_bottom(4)
        .build()
}

fn grid_key(txt: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(txt)
        .halign(gtk::Align::Start)
        .build()
}

fn grid_val(txt: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(&format!("<b>{}</b>", txt))
        .use_markup(true)
        .halign(gtk::Align::Start)
        .build()
}

const EXPORT_DESC: &str = "\
Lockbook encrypts your data with a secret key that remains on your devices. \
<b>Whoever has access to this key has complete knowledge and control of your data.</b>

Do not give this key to anyone. Do not display the QR code if there are cameras around.";
