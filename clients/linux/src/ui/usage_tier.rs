use gtk::prelude::*;

#[derive(Clone)]
pub struct UsageTier {
    title_slot: gtk::Box,
    lbl_amount: gtk::Label,
    lbl_percent: gtk::Label,
    lbl_total: gtk::Label,
    pbar: gtk::ProgressBar,
    price: gtk::Label,
    pub cntr: gtk::Box,
}

impl UsageTier {
    pub fn new() -> Self {
        let title_slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let price = gtk::Label::builder()
            .halign(gtk::Align::End)
            .hexpand(true)
            .build();

        let lbl_amount = gtk::Label::new(None);

        let lbl_percent = gtk::Label::new(None);

        let lbl_total = gtk::Label::builder()
            .halign(gtk::Align::End)
            .hexpand(true)
            .build();

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        header.append(&title_slot);
        header.append(&price);

        let labels = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        labels.set_margin_top(8);
        labels.append(&lbl_amount);
        labels.append(&lbl_percent);
        labels.append(&lbl_total);

        let pbar = gtk::ProgressBar::new();

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 8);
        cntr.set_margin_top(8);
        cntr.set_margin_bottom(8);
        cntr.append(&header);
        cntr.append(&labels);
        cntr.append(&pbar);

        Self { title_slot, price, lbl_amount, lbl_percent, lbl_total, pbar, cntr }
    }

    pub fn set_title(&self, title: &impl IsA<gtk::Widget>) {
        self.title_slot.append(title);
    }

    pub fn set_price(&self, price: &str) {
        self.price.set_markup(price);
    }

    pub fn set_metrics(&self, used: f64, available: f64) {
        let percent = used / available;

        self.lbl_amount
            .set_markup(&format!("<b>{}</b>", &lb::bytes_to_human(used as u64)));
        self.lbl_percent.set_text(&format!("({:.2} %)", percent));
        self.lbl_total
            .set_text(&lb::bytes_to_human(available as u64));
        self.pbar.set_fraction(percent);
    }
}
