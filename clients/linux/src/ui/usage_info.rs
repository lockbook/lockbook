use gtk::prelude::*;

pub struct UsageInfoPanel {
    pub name_box: gtk::Box,
    price: gtk::Label,
    lbl_amount: gtk::Label,
    lbl_percent: gtk::Label,
    lbl_total: gtk::Label,
    pbar: gtk::ProgressBar,
    pub cntr: gtk::Box,
}

impl UsageInfoPanel {
    pub fn new(name: &str) -> Self {
        let price = gtk::Label::builder()
            .halign(gtk::Align::End)
            .hexpand(true)
            .build();

        let lbl_name = gtk::Label::new(Some(name));
        lbl_name.add_css_class("settings-heading");

        let name_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        name_box.append(&lbl_name);

        let title = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        title.append(&name_box);
        title.append(&price);

        let lbl_amount = gtk::Label::new(None);

        let lbl_percent = gtk::Label::new(None);
        lbl_percent.set_margin_start(8);

        let lbl_total = gtk::Label::builder()
            .halign(gtk::Align::End)
            .hexpand(true)
            .build();

        let labels = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        labels.set_margin_top(8);
        labels.append(&lbl_amount);
        labels.append(&lbl_percent);
        labels.append(&lbl_total);

        let pbar = gtk::ProgressBar::new();

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 8);
        cntr.set_margin_top(8);
        cntr.set_margin_bottom(8);
        cntr.append(&title);
        cntr.append(&labels);
        cntr.append(&pbar);

        Self { name_box, price, lbl_amount, lbl_percent, lbl_total, pbar, cntr }
    }

    pub fn set_price(&self, price: &str) {
        self.price.set_markup(price);
    }

    pub fn set_metrics(&self, val: f64, total: f64) {
        let percent = val / total;

        self.lbl_amount
            .set_markup(&format!("<b>{}</b>", &lb::bytes_to_human(val as u64)));
        self.lbl_percent.set_text(&format!("({:.2} %)", percent));
        self.lbl_total.set_text(&lb::bytes_to_human(total as u64));
        self.pbar.set_fraction(percent);
    }
}
