use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

glib::wrapper! {
    pub struct UsageTier(ObjectSubclass<UsageTierImp>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible;
}

impl UsageTier {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("failed to create UsageTier")
    }

    pub fn set_title<W: IsA<gtk::Widget>>(&self, title: &W) {
        self.imp().title_slot.append(title);
    }

    pub fn set_price(&self, price: &str) {
        self.imp().price.set_markup(price);
    }

    pub fn set_metrics(&self, val: f64, total: f64) {
        let percent = val / total;

        let imp = self.imp();
        imp.lbl_amount
            .set_markup(&format!("<b>{}</b>", &lb::bytes_to_human(val as u64)));
        imp.lbl_percent.set_text(&format!("({:.2} %)", percent));
        imp.lbl_total.set_text(&lb::bytes_to_human(total as u64));
        imp.pbar.set_fraction(percent);
    }
}

impl Default for UsageTier {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
pub struct UsageTierImp {
    title_slot: gtk::Box,
    price: gtk::Label,
    lbl_amount: gtk::Label,
    lbl_percent: gtk::Label,
    lbl_total: gtk::Label,
    pbar: gtk::ProgressBar,
    cntr: gtk::Box,
}

#[glib::object_subclass]
impl ObjectSubclass for UsageTierImp {
    const NAME: &'static str = "UsageTier";
    type Type = UsageTier;
    type ParentType = gtk::Widget;

    fn class_init(c: &mut Self::Class) {
        c.set_layout_manager_type::<gtk::BinLayout>();
    }
}

impl ObjectImpl for UsageTierImp {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);

        self.title_slot
            .set_orientation(gtk::Orientation::Horizontal);

        self.price.set_halign(gtk::Align::End);
        self.price.set_hexpand(true);

        self.lbl_total.set_halign(gtk::Align::End);
        self.lbl_total.set_hexpand(true);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        header.append(&self.title_slot);
        header.append(&self.price);

        let labels = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        labels.set_margin_top(8);
        labels.append(&self.lbl_amount);
        labels.append(&self.lbl_percent);
        labels.append(&self.lbl_total);

        self.cntr.set_orientation(gtk::Orientation::Vertical);
        self.cntr.set_spacing(8);
        self.cntr.set_margin_top(8);
        self.cntr.set_margin_bottom(8);
        self.cntr.append(&header);
        self.cntr.append(&labels);
        self.cntr.append(&self.pbar);
        self.cntr.set_parent(obj);
    }

    fn dispose(&self, _obj: &Self::Type) {
        self.cntr.unparent();
    }
}

impl WidgetImpl for UsageTierImp {}
