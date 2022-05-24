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
        let compr_ratio = format!("{:.2}x", uncompressed.exact as f64 / server_usage);
        let is_free_tier = metrics.data_cap.exact == 1000000;

        let compr_stats = gtk::Grid::builder()
            .column_spacing(8)
            .row_spacing(8)
            .build();
        compr_stats.attach(&grid_key("Uncompressed usage: "), 0, 0, 1, 1);
        compr_stats.attach(&grid_val(&uncompressed.readable), 1, 0, 1, 1);
        compr_stats.attach(&grid_key("Compression ratio: "), 0, 1, 1, 1);
        compr_stats.attach(&grid_val(&compr_ratio), 1, 1, 1, 1);

        let compr_popover = gtk::Popover::new();
        compr_popover.set_child(Some(&compr_stats));

        let usage_home = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let info_icon = gtk::Image::from_icon_name("dialog-information-symbolic");
        let current_title = gtk::Box::new(gtk::Orientation::Horizontal, 8);

        let motion = gtk::EventControllerMotion::new();
        motion.connect_enter({
            //let current_title = current_title.clone();
            //let info_icon = info_icon.clone();
            let p = compr_popover.clone();

            move |_, x, y| {
                //let bounds = info_icon.compute_bounds(&current_title).unwrap();
                let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                p.set_pointing_to(Some(&rect));
                p.popup();
            }
        });
        let p = compr_popover.clone();
        motion.connect_leave(move |_| p.popdown());

        current_title.add_controller(&motion);

        current_title.append(&heading("Current"));
        current_title.append(&info_icon);
        current_title.append(&compr_popover);

        let current_usage = ui::UsageTier::new(server_usage, metrics.data_cap.exact as f64);
        current_usage.set_title(&current_title);
        current_usage.set_price(if is_free_tier { "Free" } else { "$2.50 / month" });

        let upgraded_usage = ui::UsageTier::new(server_usage, 50000000000.0);
        upgraded_usage.set_title(&heading("Premium"));
        upgraded_usage.set_price("$2.50 / month");

        let btn_upgrade = gtk::Button::new();
        btn_upgrade.set_child(Some(&upgraded_usage.cntr));

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
