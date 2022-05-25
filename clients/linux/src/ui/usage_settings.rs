use gtk::prelude::*;

use crate::ui;

#[derive(Clone)]
pub struct UsageSettings {
    lbl_uncompr_usage: gtk::Label,
    lbl_compr_ratio: gtk::Label,
    current_usage: ui::UsageTier,
    upgraded_usage: ui::UsageTier,
    btn_upgrade: gtk::Button,
    usage_home: gtk::Box,
    pub pages: gtk::Stack,
}

impl UsageSettings {
    pub fn new() -> Self {
        let lbl_uncompr_usage = gtk::Label::builder().halign(gtk::Align::Start).build();
        let lbl_compr_ratio = gtk::Label::builder().halign(gtk::Align::Start).build();

        let compr_stats = gtk::Grid::builder()
            .column_spacing(8)
            .row_spacing(8)
            .build();
        compr_stats.attach(&grid_key("Uncompressed usage: "), 0, 0, 1, 1);
        compr_stats.attach(&lbl_uncompr_usage, 1, 0, 1, 1);
        compr_stats.attach(&grid_key("Compression ratio: "), 0, 1, 1, 1);
        compr_stats.attach(&lbl_compr_ratio, 1, 1, 1, 1);

        let info_popover = gtk::Popover::new();
        info_popover.set_child(Some(&compr_stats));

        let info_btn = gtk::MenuButton::builder()
            .direction(gtk::ArrowType::Right)
            .popover(&info_popover)
            .child(&gtk::Image::from_icon_name("dialog-information-symbolic"))
            .build();

        let current_title = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        current_title.append(&heading("Current"));
        current_title.append(&info_btn);

        let current_usage = ui::UsageTier::new();
        current_usage.set_title(&current_title);

        let upgraded_usage = ui::UsageTier::new();
        upgraded_usage.set_title(&heading("Premium"));
        upgraded_usage.set_price("$2.50 / month");

        let btn_upgrade = gtk::Button::new();
        btn_upgrade.set_child(Some(&upgraded_usage.cntr));

        let usage_home = gtk::Box::new(gtk::Orientation::Vertical, 12);
        usage_home.set_margin_start(12);
        usage_home.set_margin_end(12);

        let pages = gtk::Stack::new();
        pages.add_named(&usage_home, Some("home"));

        Self {
            lbl_uncompr_usage,
            lbl_compr_ratio,
            current_usage,
            upgraded_usage,
            btn_upgrade,
            usage_home,
            pages,
        }
    }

    pub fn set_metrics(
        &self, metrics_result: Result<lb::UsageMetrics, lb::Error<lb::GetUsageError>>,
        uncompressed_result: Result<lb::UsageItemMetric, lb::Error<lb::GetUsageError>>,
    ) {
        ui::clear(&self.usage_home);

        let err_lbl = |msg: &str| {
            gtk::Label::builder()
                .css_classes(vec!["err".to_string()])
                .margin_top(20)
                .margin_bottom(20)
                .label(msg)
                .build()
        };

        let metrics = match metrics_result {
            Ok(metrics) => metrics,
            Err(err) => {
                self.usage_home.append(&err_lbl(&format!("{:?}", err))); //todo
                return;
            }
        };

        let server_usage = metrics.server_usage.exact as f64;
        let is_free_tier = metrics.data_cap.exact == 1000000;

        self.current_usage
            .set_metrics(server_usage, metrics.data_cap.exact as f64);
        self.current_usage
            .set_price(if is_free_tier { "Free" } else { "$2.50 / month" });

        self.upgraded_usage.set_metrics(server_usage, 50000000000.0);

        let uncompressed = match uncompressed_result {
            Ok(data) => data,
            Err(err) => {
                self.usage_home.append(&err_lbl(&format!("{:?}", err))); //todo
                return;
            }
        };

        let compr_ratio = match metrics.server_usage.exact {
            0 => "0".to_string(),
            _ => format!("{:.2}x", uncompressed.exact as f64 / server_usage),
        };
        self.lbl_uncompr_usage
            .set_markup(&format!("<b>{}</b>", uncompressed.readable));
        self.lbl_compr_ratio
            .set_markup(&format!("<b>{}</b>", compr_ratio));

        self.usage_home.append(&self.current_usage.cntr);
        if is_free_tier {
            self.usage_home
                .append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            self.usage_home.append(&self.btn_upgrade);
        }
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
