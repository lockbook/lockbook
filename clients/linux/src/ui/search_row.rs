use gtk::glib;
use gtk::subclass::prelude::*;

glib::wrapper! {
    pub struct SearchRow(ObjectSubclass<imp::SearchRow>)
        @extends gtk::Widget, gtk::HeaderBar,
        @implements gtk::Accessible;
}

impl SearchRow {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("failed to create custom SearchRow")
    }

    pub fn set_data(&self, id: lb::Uuid, path: &str) {
        self.imp().id.set(id);
        self.imp().path.set_text(path);
    }

    pub fn id(&self) -> lb::Uuid {
        self.imp().id.get()
    }

    pub fn path(&self) -> String {
        self.imp().path.text().to_string()
    }
}

impl Default for SearchRow {
    fn default() -> Self {
        Self::new()
    }
}

mod imp {
    use std::cell::Cell;

    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    #[derive(Debug, Default)]
    pub struct SearchRow {
        pub id: Cell<lb::Uuid>,
        pub path: gtk::Label,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SearchRow {
        const NAME: &'static str = "SearchRow";
        type Type = super::SearchRow;
        type ParentType = gtk::Widget;

        fn class_init(c: &mut Self::Class) {
            // The layout manager determines how child widgets are laid out.
            c.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for SearchRow {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            self.path.set_halign(gtk::Align::Start);
            self.path.set_parent(obj);
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.path.unparent();
        }
    }

    impl WidgetImpl for SearchRow {}
}
