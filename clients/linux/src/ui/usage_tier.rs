use gtk::prelude::*;

#[derive(Clone)]
pub struct UsageTier {
    title_slot: gtk::Box,
    price: gtk::Label,
    pub cntr: gtk::Box,
}

impl UsageTier {
    pub fn new(used: f64, available: f64) -> Self {
        let title_slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let price = gtk::Label::builder()
            .halign(gtk::Align::End)
            .hexpand(true)
            .build();

        let lbl_amount = gtk::Label::new(None);
        lbl_amount.set_markup(&format!("<b>{}</b>", &lb::bytes_to_human(used as u64)));

        let percent = used / available;

        let lbl_percent = gtk::Label::new(Some(&format!("({:.2} %)", percent)));

        let lbl_total = gtk::Label::builder()
            .label(&lb::bytes_to_human(available as u64))
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

        let pbar = gtk::ProgressBar::builder().fraction(percent).build();

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 8);
        cntr.set_margin_top(8);
        cntr.set_margin_bottom(8);
        cntr.append(&header);
        cntr.append(&labels);
        cntr.append(&pbar);

        Self { title_slot, price, cntr }
    }

    pub fn set_title(&self, title: &impl IsA<gtk::Widget>) {
        self.title_slot.append(title);
    }

    pub fn set_price(&self, price: &str) {
        self.price.set_markup(price);
    }
}
