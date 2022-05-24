use gtk::prelude::*;

use crate::ui;

#[derive(Clone)]
pub struct UsageSettings {
    btn_upgrade: gtk::Button,
    pub pages: gtk::Stack,
}

impl UsageSettings {
    pub fn new(metrics: lb::UsageMetrics, uncompressed: lb::UsageItemMetric) -> Self {
        let server_usage = metrics.server_usage.exact as f64;
        let is_free_tier = metrics.data_cap.exact == 1000000;

        let current_title = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        current_title.append(&heading("Current"));

        if metrics.server_usage.exact > 0 {
            let compr_ratio = format!("{:.2}x", uncompressed.exact as f64 / server_usage);

            let compr_stats = gtk::Grid::builder()
                .column_spacing(8)
                .row_spacing(8)
                .build();
            compr_stats.attach(&grid_key("Uncompressed usage: "), 0, 0, 1, 1);
            compr_stats.attach(&grid_val(&uncompressed.readable), 1, 0, 1, 1);
            compr_stats.attach(&grid_key("Compression ratio: "), 0, 1, 1, 1);
            compr_stats.attach(&grid_val(&compr_ratio), 1, 1, 1, 1);

            let info_popover = gtk::Popover::new();
            info_popover.set_child(Some(&compr_stats));

            let info_btn = gtk::MenuButton::builder()
                .direction(gtk::ArrowType::Right)
                .popover(&info_popover)
                .child(&gtk::Image::from_icon_name("dialog-information-symbolic"))
                .build();
            current_title.append(&info_btn);
        }

        let current_usage = ui::UsageTier::new(server_usage, metrics.data_cap.exact as f64);
        current_usage.set_title(&current_title);
        current_usage.set_price(if is_free_tier { "Free" } else { "$2.50 / month" });

        let upgraded_usage = ui::UsageTier::new(server_usage, 50000000000.0);
        upgraded_usage.set_title(&heading("Premium"));
        upgraded_usage.set_price("$2.50 / month");

        let btn_upgrade = gtk::Button::new();
        btn_upgrade.set_child(Some(&upgraded_usage.cntr));

        let usage_home = gtk::Box::new(gtk::Orientation::Vertical, 12);
        usage_home.set_margin_start(12);
        usage_home.set_margin_end(12);
        usage_home.append(&current_usage.cntr);

        if is_free_tier {
            usage_home.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            usage_home.append(&btn_upgrade);
        }

        let pages = gtk::Stack::new();
        pages.add_named(&usage_home, Some("home"));

        Self { btn_upgrade, pages }
    }

    pub fn connect_begin_upgrade<F: Fn(&Self) + 'static>(&self, f: F) {
        let this = self.clone();
        self.btn_upgrade.connect_clicked(move |_| f(&this));
    }
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
